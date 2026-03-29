//! MCP (Model Context Protocol) server — stdio transport.
//!
//! Exposes VORTEX capabilities as MCP tools that Claude Code can call.
//! Reads JSON-RPC 2.0 messages from stdin, writes responses to stdout.
//!
//! Usage: `vortex serve` (Claude Code spawns the process and uses stdio).
//!
//! Tools:
//! | Tool              | What it does                                        |
//! |-------------------|-----------------------------------------------------|
//! | create_project    | Create a new project (in-memory, returns project_id)|
//! | add_clip          | Append a clip to the timeline                       |
//! | add_effect        | Attach an effect to a specific clip                 |
//! | add_music         | Add a music/audio track to the project              |
//! | analyse_video     | Run kill/beat/scene analysis on a video file        |
//! | render_project    | Render to video via FFmpeg (blocking)               |
//! | apply_style       | Apply a named style template to the project         |
//! | list_styles       | Return available style templates                    |
//! | get_project       | Inspect the current project state                   |

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use vortex_core::{AudioTrack, Clip, Effect, Project, TimeRange};
use vortex_core::{
    ChromaticEffect, ColorEffect, FlashEffect, GlitchEffect, LetterboxEffect,
    ShakeEffect, VelocityEffect, VignetteEffect, ZoomEffect,
};

// ─── JSON-RPC types ───────────────────────────────────────────────────────────

/// MCP JSON-RPC 2.0 request.
#[derive(Debug, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    /// May be null for notifications.
    #[serde(default)]
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// MCP JSON-RPC 2.0 response.
#[derive(Debug, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// JSON-RPC error object.
#[derive(Debug, Serialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
}

impl McpResponse {
    pub fn ok(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
    }

    pub fn err(id: serde_json::Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(McpError { code, message: message.into() }),
        }
    }

    /// Wrap a tool result string as an MCP content response.
    fn tool_ok(id: serde_json::Value, text: String) -> Self {
        Self::ok(id, serde_json::json!({
            "content": [{ "type": "text", "text": text }]
        }))
    }

    /// Wrap a tool error as an MCP content response with isError=true.
    fn tool_err(id: serde_json::Value, message: String) -> Self {
        Self::ok(id, serde_json::json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true
        }))
    }
}

/// Tool descriptor for `tools/list`.
#[derive(Debug, Serialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// ─── MCP Server ───────────────────────────────────────────────────────────────

/// The MCP server. Holds an in-memory project store keyed by project_id.
pub struct McpServer {
    host: String,
    port: u16,
    /// Session state: project_id → Project.
    projects: Arc<Mutex<HashMap<String, Project>>>,
}

impl McpServer {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
            projects: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// All VORTEX tools (for `tools/list`).
    pub fn tools(&self) -> Vec<ToolDescriptor> {
        vec![
            ToolDescriptor {
                name: "create_project".into(),
                description: "Create a new VORTEX project. Returns a project_id to use in subsequent calls.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Project name" },
                        "style": { "type": "string", "description": "Optional: 'aggressive', 'chill', or 'cinematic'" }
                    },
                    "required": ["name"]
                }),
            },
            ToolDescriptor {
                name: "add_clip".into(),
                description: "Append a video clip to the project timeline. Clips are placed end-to-end automatically.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "source_path": { "type": "string", "description": "Absolute path to source video file" },
                        "source_start": { "type": "number", "description": "Trim start in seconds" },
                        "source_end": { "type": "number", "description": "Trim end in seconds" },
                        "label": { "type": "string", "description": "Optional label for this clip" },
                        "speed": { "type": "number", "description": "Playback speed (1.0 = normal, 0.5 = half-speed)" }
                    },
                    "required": ["project_id", "source_path", "source_start", "source_end"]
                }),
            },
            ToolDescriptor {
                name: "add_effect".into(),
                description: "Attach a visual effect to a specific clip. Effects are applied in order.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "clip_id": { "type": "string" },
                        "effect_type": {
                            "type": "string",
                            "enum": ["velocity", "zoom", "shake", "flash", "color", "chromatic", "letterbox", "vignette", "glitch"],
                            "description": "Effect type. velocity=slow-mo ramp, zoom=scale punch, shake=camera jitter, flash=white burst, color=grade+LUT, chromatic=RGB split, letterbox=cinema bars"
                        },
                        "params": {
                            "type": "object",
                            "description": "Effect params. velocity: {min_speed, ramp_in_secs, ramp_out_secs}. zoom: {to_scale, duration_secs}. shake: {intensity_x, intensity_y, frequency}. flash: {color, peak_opacity, duration_secs}. color: {saturation, contrast, brightness, lut_path}. chromatic: {strength}. letterbox: {aspect_ratio}. vignette: {strength}. glitch: {displacement, duration_secs}"
                        }
                    },
                    "required": ["project_id", "clip_id", "effect_type"]
                }),
            },
            ToolDescriptor {
                name: "add_music".into(),
                description: "Add a music or audio track to the project.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "source_path": { "type": "string", "description": "Path to audio file (mp3, wav, aac, etc.)" },
                        "volume": { "type": "number", "description": "Volume 0.0–1.0 (default 0.85)" },
                        "fade_in_secs": { "type": "number" },
                        "fade_out_secs": { "type": "number" }
                    },
                    "required": ["project_id", "source_path"]
                }),
            },
            ToolDescriptor {
                name: "analyse_video".into(),
                description: "Analyse a video for kill/highlight moments, beat markers, and scene cuts. Run this first before building the timeline.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "source_path": { "type": "string", "description": "Absolute path to video file" }
                    },
                    "required": ["source_path"]
                }),
            },
            ToolDescriptor {
                name: "render_project".into(),
                description: "Render the project to a video file via FFmpeg. Blocks until complete. FFmpeg must be in PATH.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "output_path": { "type": "string", "description": "Output video file path (e.g. output/montage.mp4)" },
                        "hw_accel": { "type": "string", "enum": ["none", "nvidia", "amf", "videotoolbox"], "description": "Hardware encoding backend (default: none)" }
                    },
                    "required": ["project_id", "output_path"]
                }),
            },
            ToolDescriptor {
                name: "apply_style".into(),
                description: "Apply a named style template to the project.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "style_name": { "type": "string", "enum": ["aggressive", "chill", "cinematic"] }
                    },
                    "required": ["project_id", "style_name"]
                }),
            },
            ToolDescriptor {
                name: "list_styles".into(),
                description: "Return all available VORTEX style templates with their settings.".into(),
                input_schema: serde_json::json!({ "type": "object", "properties": {} }),
            },
            ToolDescriptor {
                name: "get_project".into(),
                description: "Get the current state of a project (clips, effects, timeline duration).".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" }
                    },
                    "required": ["project_id"]
                }),
            },
        ]
    }

    /// Dispatch a JSON-RPC request to the appropriate handler.
    pub async fn dispatch(&self, req: McpRequest) -> Option<McpResponse> {
        tracing::debug!(method = %req.method, "MCP request");

        match req.method.as_str() {
            // Notifications — no response
            m if m.starts_with("notifications/") => None,

            "initialize" => Some(McpResponse::ok(req.id, serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "vortex",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }))),

            "tools/list" => Some(McpResponse::ok(req.id, serde_json::json!({
                "tools": self.tools()
            }))),

            "tools/call" => {
                let tool_name = req.params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let args = req.params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));

                let result = self.dispatch_tool(&tool_name, &args).await;
                Some(match result {
                    Ok(text) => McpResponse::tool_ok(req.id, text),
                    Err(msg) => McpResponse::tool_err(req.id, msg),
                })
            }

            other => Some(McpResponse::err(
                req.id,
                -32601,
                format!("Method not found: {other}"),
            )),
        }
    }

    async fn dispatch_tool(
        &self,
        tool: &str,
        args: &serde_json::Value,
    ) -> Result<String, String> {
        match tool {
            "create_project" => self.tool_create_project(args).await,
            "add_clip" => self.tool_add_clip(args).await,
            "add_effect" => self.tool_add_effect(args).await,
            "add_music" => self.tool_add_music(args).await,
            "analyse_video" => self.tool_analyse_video(args).await,
            "render_project" => self.tool_render_project(args).await,
            "apply_style" => self.tool_apply_style(args).await,
            "list_styles" => self.tool_list_styles().await,
            "get_project" => self.tool_get_project(args).await,
            other => Err(format!(
                "Unknown tool: '{other}'. Available: create_project, add_clip, add_effect, add_music, analyse_video, render_project, apply_style, list_styles, get_project"
            )),
        }
    }

    // ─── Tool handlers ─────────────────────────────────────────────────────

    async fn tool_create_project(&self, args: &serde_json::Value) -> Result<String, String> {
        let name = str_arg(args, "name")?;
        let style = args.get("style").and_then(|v| v.as_str());

        let mut project = Project::new(name);
        if let Some(s) = style {
            project.style = Some(s.to_string());
        }
        let project_id = project.id.to_string();

        self.projects.lock().await.insert(project_id.clone(), project);

        Ok(serde_json::json!({
            "project_id": project_id,
            "name": name,
            "message": format!("Created project '{name}'. Use project_id '{project_id}' in all subsequent calls.")
        })
        .to_string())
    }

    async fn tool_add_clip(&self, args: &serde_json::Value) -> Result<String, String> {
        let project_id = str_arg(args, "project_id")?;
        let source_path = str_arg(args, "source_path")?;
        let source_start = f64_arg(args, "source_start")?;
        let source_end = f64_arg(args, "source_end")?;
        let label = args.get("label").and_then(|v| v.as_str());
        let speed = args.get("speed").and_then(|v| v.as_f64()).unwrap_or(1.0);

        let source_range =
            TimeRange::new(source_start, source_end).map_err(|e| e.to_string())?;

        let mut projects = self.projects.lock().await;
        let project = projects
            .get_mut(project_id)
            .ok_or_else(|| format!("Project not found: {project_id}"))?;

        let timeline_start = project.timeline.duration;
        let clip_duration = (source_end - source_start) / speed;
        let timeline_range =
            TimeRange::new(timeline_start, timeline_start + clip_duration)
                .map_err(|e| e.to_string())?;

        let mut clip = Clip::new(source_path, source_range, timeline_range);
        clip.speed = speed;
        if let Some(lbl) = label {
            clip = clip.with_label(lbl);
        }
        let clip_id = clip.id.to_string();
        project.timeline.push_clip(clip);

        Ok(serde_json::json!({
            "clip_id": clip_id,
            "timeline_start": timeline_start,
            "timeline_end": timeline_start + clip_duration,
            "duration": clip_duration,
        })
        .to_string())
    }

    async fn tool_add_effect(&self, args: &serde_json::Value) -> Result<String, String> {
        let project_id = str_arg(args, "project_id")?;
        let clip_id_str = str_arg(args, "clip_id")?;
        let effect_type = str_arg(args, "effect_type")?;
        let params = args
            .get("params")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        let clip_id = Uuid::parse_str(clip_id_str)
            .map_err(|_| format!("Invalid clip_id UUID: {clip_id_str}"))?;

        let effect = build_effect(effect_type, &params)?;

        let mut projects = self.projects.lock().await;
        let project = projects
            .get_mut(project_id)
            .ok_or_else(|| format!("Project not found: {project_id}"))?;

        let clip = project
            .timeline
            .clips
            .iter_mut()
            .find(|c| c.id == clip_id)
            .ok_or_else(|| format!("Clip not found: {clip_id_str}"))?;

        clip.add_effect(effect);
        let effect_count = clip.effects.len();

        Ok(serde_json::json!({
            "clip_id": clip_id_str,
            "effect_type": effect_type,
            "total_effects_on_clip": effect_count,
        })
        .to_string())
    }

    async fn tool_add_music(&self, args: &serde_json::Value) -> Result<String, String> {
        let project_id = str_arg(args, "project_id")?;
        let source_path = str_arg(args, "source_path")?;
        let volume = args.get("volume").and_then(|v| v.as_f64()).unwrap_or(0.85);
        let fade_in = args.get("fade_in_secs").and_then(|v| v.as_f64()).unwrap_or(0.5);
        let fade_out = args.get("fade_out_secs").and_then(|v| v.as_f64()).unwrap_or(1.5);

        let mut track = AudioTrack::new("Music", source_path).with_volume(volume);
        track.fade_in_secs = fade_in;
        track.fade_out_secs = fade_out;
        let track_id = track.id.to_string();

        let mut projects = self.projects.lock().await;
        let project = projects
            .get_mut(project_id)
            .ok_or_else(|| format!("Project not found: {project_id}"))?;

        project.timeline.audio_tracks.push(track);

        Ok(serde_json::json!({
            "track_id": track_id,
            "source_path": source_path,
            "volume": volume,
        })
        .to_string())
    }

    async fn tool_analyse_video(&self, args: &serde_json::Value) -> Result<String, String> {
        let source_path = str_arg(args, "source_path")?;

        use vortex_analysis::{
            BeatDetector, BeatDetectorConfig, KillDetector, KillDetectorConfig,
            SceneDetector, SceneDetectorConfig,
        };

        let (kills, beats, scenes, duration) = tokio::join!(
            KillDetector::new(KillDetectorConfig::default()).detect(source_path),
            BeatDetector::new(BeatDetectorConfig::default()).analyse(source_path),
            SceneDetector::new(SceneDetectorConfig::default()).detect(source_path),
            vortex_analysis::probe_duration(source_path),
        );

        let analysis = vortex_analysis::ClipAnalysis {
            source_path: source_path.to_string(),
            duration_secs: duration.unwrap_or(0.0),
            kill_moments: kills.map_err(|e| e.to_string())?,
            scene_cuts: scenes.map_err(|e| e.to_string())?,
            beats: Some(beats.map_err(|e| e.to_string())?),
        };

        serde_json::to_string_pretty(&analysis).map_err(|e| e.to_string())
    }

    async fn tool_render_project(&self, args: &serde_json::Value) -> Result<String, String> {
        let project_id = str_arg(args, "project_id")?;
        let output_path = str_arg(args, "output_path")?;
        let hw_accel = args.get("hw_accel").and_then(|v| v.as_str()).unwrap_or("none");

        let project = {
            let projects = self.projects.lock().await;
            projects
                .get(project_id)
                .cloned()
                .ok_or_else(|| format!("Project not found: {project_id}"))?
        };

        if project.timeline.clips.is_empty() {
            return Err("Cannot render: timeline has no clips. Add clips first with add_clip.".into());
        }

        let config = vortex_render::RenderConfig {
            hw_accel: hw_accel.to_string(),
            ..Default::default()
        };
        let pipeline = vortex_render::RenderPipeline::new(config);

        tracing::info!(clips = project.timeline.clips.len(), "Starting render");

        let mut rx = pipeline
            .render(&project, output_path)
            .await
            .map_err(|e| e.to_string())?;

        let mut completed = false;
        let mut last_error: Option<String> = None;

        while let Some(event) = rx.recv().await {
            use vortex_render::RenderProgress;
            match event {
                RenderProgress::Started { total_frames } => {
                    tracing::info!("Render started ({total_frames} frames)");
                }
                RenderProgress::Frame { current, total, fps, eta_secs } => {
                    tracing::info!("Frame {current}/{total} @ {fps:.1}fps  ETA {eta_secs:.0}s");
                }
                RenderProgress::Complete { .. } => {
                    completed = true;
                }
                RenderProgress::Failed { error } => {
                    last_error = Some(error);
                }
            }
        }

        if completed {
            Ok(serde_json::json!({
                "status": "complete",
                "output_path": output_path,
                "clips": project.timeline.clips.len(),
                "duration_secs": project.timeline.duration,
                "message": format!("Render complete → {output_path}"),
            })
            .to_string())
        } else {
            Err(last_error.unwrap_or_else(|| "Render failed (unknown error)".into()))
        }
    }

    async fn tool_apply_style(&self, args: &serde_json::Value) -> Result<String, String> {
        let project_id = str_arg(args, "project_id")?;
        let style_name = str_arg(args, "style_name")?;

        let registry =
            vortex_styles::StyleRegistry::load_default().map_err(|e| e.to_string())?;
        let style = registry.get(style_name).map_err(|e| e.to_string())?;

        let mut projects = self.projects.lock().await;
        let project = projects
            .get_mut(project_id)
            .ok_or_else(|| format!("Project not found: {project_id}"))?;

        project.style = Some(style.name.clone());

        Ok(serde_json::json!({
            "project_id": project_id,
            "style": style.name,
            "description": style.description,
            "cuts_per_minute": style.cuts.cuts_per_minute,
        })
        .to_string())
    }

    async fn tool_list_styles(&self) -> Result<String, String> {
        let registry =
            vortex_styles::StyleRegistry::load_default().map_err(|e| e.to_string())?;
        let styles: Vec<_> = registry
            .styles()
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "description": s.description,
                    "tags": s.tags,
                    "cuts_per_minute": s.cuts.cuts_per_minute,
                    "cut_trigger": s.cuts.cut_trigger,
                    "velocity_enabled": s.velocity.enabled,
                    "letterbox": s.effects.letterbox,
                })
            })
            .collect();

        Ok(serde_json::json!({ "styles": styles }).to_string())
    }

    async fn tool_get_project(&self, args: &serde_json::Value) -> Result<String, String> {
        let project_id = str_arg(args, "project_id")?;
        let projects = self.projects.lock().await;
        let project = projects
            .get(project_id)
            .ok_or_else(|| format!("Project not found: {project_id}"))?;

        // Return a summary rather than the full serialized project
        let clips: Vec<_> = project
            .timeline
            .clips
            .iter()
            .map(|c| {
                serde_json::json!({
                    "clip_id": c.id,
                    "label": c.label,
                    "source_path": c.source_path,
                    "source_range": { "start": c.source_range.start, "end": c.source_range.end },
                    "timeline_range": { "start": c.timeline_range.start, "end": c.timeline_range.end },
                    "effects": c.effects.iter().map(|e| e.name()).collect::<Vec<_>>(),
                    "speed": c.speed,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "project_id": project_id,
            "name": project.name,
            "style": project.style,
            "duration_secs": project.timeline.duration,
            "clip_count": clips.len(),
            "audio_track_count": project.timeline.audio_tracks.len(),
            "clips": clips,
        })
        .to_string())
    }

    // ─── Transport ─────────────────────────────────────────────────────────

    /// Run the MCP server with stdio transport (default for Claude Code MCP).
    ///
    /// Reads newline-delimited JSON-RPC 2.0 from stdin, writes responses to stdout.
    pub async fn run(&self) -> anyhow::Result<()> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        tracing::info!(
            host = %self.host,
            port = self.port,
            "VORTEX MCP server ready — listening on stdio"
        );
        eprintln!("VORTEX MCP server ready (stdio transport)");

        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    tracing::info!("stdin closed — shutting down");
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    let response =
                        match serde_json::from_str::<McpRequest>(trimmed) {
                            Ok(req) => self.dispatch(req).await,
                            Err(e) => Some(McpResponse::err(
                                serde_json::Value::Null,
                                -32700,
                                format!("Parse error: {e}"),
                            )),
                        };

                    if let Some(resp) = response {
                        let mut resp_str = serde_json::to_string(&resp)?;
                        resp_str.push('\n');
                        stdout.write_all(resp_str.as_bytes()).await?;
                        stdout.flush().await?;
                    }
                }
                Err(e) => {
                    tracing::error!("stdin read error: {e}");
                    break;
                }
            }
        }

        Ok(())
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn str_arg<'a>(args: &'a serde_json::Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Missing required param: '{key}'"))
}

fn f64_arg(args: &serde_json::Value, key: &str) -> Result<f64, String> {
    args.get(key)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| format!("Missing required param: '{key}' (must be a number)"))
}

/// Build a `vortex_core::Effect` from an effect type string and JSON params.
fn build_effect(effect_type: &str, params: &serde_json::Value) -> Result<Effect, String> {
    Ok(match effect_type {
        "velocity" => {
            let mut e = VelocityEffect::default();
            if let Some(v) = params.get("min_speed").and_then(|v| v.as_f64()) {
                e.min_speed = v;
            }
            if let Some(v) = params.get("ramp_in_secs").and_then(|v| v.as_f64()) {
                e.ramp_in_secs = v;
            }
            if let Some(v) = params.get("ramp_out_secs").and_then(|v| v.as_f64()) {
                e.ramp_out_secs = v;
            }
            if let Some(v) = params.get("easing").and_then(|v| v.as_str()) {
                e.easing = v.to_string();
            }
            Effect::Velocity(e)
        }
        "zoom" => {
            let mut e = ZoomEffect::default();
            if let Some(v) = params.get("to_scale").and_then(|v| v.as_f64()) {
                e.to_scale = v;
            }
            if let Some(v) = params.get("from_scale").and_then(|v| v.as_f64()) {
                e.from_scale = v;
            }
            if let Some(v) = params.get("duration_secs").and_then(|v| v.as_f64()) {
                e.duration_secs = v;
            }
            if let Some(v) = params.get("focal_x").and_then(|v| v.as_f64()) {
                e.focal_x = v;
            }
            if let Some(v) = params.get("focal_y").and_then(|v| v.as_f64()) {
                e.focal_y = v;
            }
            Effect::Zoom(e)
        }
        "shake" => {
            let mut e = ShakeEffect::default();
            if let Some(v) = params.get("intensity_x").and_then(|v| v.as_f64()) {
                e.intensity_x = v;
            }
            if let Some(v) = params.get("intensity_y").and_then(|v| v.as_f64()) {
                e.intensity_y = v;
            }
            if let Some(v) = params.get("frequency").and_then(|v| v.as_f64()) {
                e.frequency = v;
            }
            if let Some(v) = params.get("decay").and_then(|v| v.as_f64()) {
                e.decay = v;
            }
            Effect::Shake(e)
        }
        "flash" => {
            let mut e = FlashEffect::default();
            if let Some(v) = params.get("color").and_then(|v| v.as_str()) {
                e.color = v.to_string();
            }
            if let Some(v) = params.get("peak_opacity").and_then(|v| v.as_f64()) {
                e.peak_opacity = v;
            }
            if let Some(v) = params.get("duration_secs").and_then(|v| v.as_f64()) {
                e.duration_secs = v;
            }
            Effect::Flash(e)
        }
        "color" => {
            let mut e = ColorEffect::default();
            if let Some(v) = params.get("saturation").and_then(|v| v.as_f64()) {
                e.saturation = v;
            }
            if let Some(v) = params.get("contrast").and_then(|v| v.as_f64()) {
                e.contrast = v;
            }
            if let Some(v) = params.get("brightness").and_then(|v| v.as_f64()) {
                e.brightness = v;
            }
            if let Some(v) = params.get("hue_shift").and_then(|v| v.as_f64()) {
                e.hue_shift = v;
            }
            if let Some(v) = params.get("lut_path").and_then(|v| v.as_str()) {
                e.lut_path = Some(v.to_string());
            }
            if let Some(v) = params.get("lut_strength").and_then(|v| v.as_f64()) {
                e.lut_strength = v;
            }
            Effect::Color(e)
        }
        "chromatic" => {
            let mut e = ChromaticEffect::default();
            if let Some(v) = params.get("strength").and_then(|v| v.as_f64()) {
                e.strength = v;
                // scale offsets proportionally
                e.offset_r_x = 4.0 * v;
                e.offset_b_x = -4.0 * v;
            }
            if let Some(v) = params.get("offset_r_x").and_then(|v| v.as_f64()) {
                e.offset_r_x = v;
            }
            if let Some(v) = params.get("offset_b_x").and_then(|v| v.as_f64()) {
                e.offset_b_x = v;
            }
            Effect::Chromatic(e)
        }
        "letterbox" => {
            let mut e = LetterboxEffect::default();
            if let Some(v) = params.get("aspect_ratio").and_then(|v| v.as_f64()) {
                e.aspect_ratio = v;
            }
            Effect::Letterbox(e)
        }
        "vignette" => {
            let mut e = VignetteEffect::default();
            if let Some(v) = params.get("strength").and_then(|v| v.as_f64()) {
                e.strength = v;
            }
            Effect::Vignette(e)
        }
        "glitch" => {
            let mut e = GlitchEffect::default();
            if let Some(v) = params.get("displacement").and_then(|v| v.as_f64()) {
                e.displacement = v;
            }
            if let Some(v) = params.get("duration_secs").and_then(|v| v.as_f64()) {
                e.duration_secs = v;
            }
            if let Some(v) = params.get("scan_lines").and_then(|v| v.as_u64()) {
                e.scan_lines = v as u32;
            }
            Effect::Glitch(e)
        }
        other => {
            return Err(format!(
                "Unknown effect type: '{other}'. Valid types: velocity, zoom, shake, flash, color, chromatic, letterbox, vignette, glitch"
            ))
        }
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> McpServer {
        McpServer::new("127.0.0.1", 7700)
    }

    async fn call(server: &McpServer, method: &str, params: serde_json::Value) -> serde_json::Value {
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(1),
            method: method.into(),
            params,
        };
        let resp = server.dispatch(req).await.expect("expected response");
        assert!(resp.error.is_none(), "unexpected error: {:?}", resp.error);
        resp.result.unwrap()
    }

    #[tokio::test]
    async fn mcp_initialize() {
        let s = server();
        let result = call(&s, "initialize", serde_json::Value::Null).await;
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "vortex");
    }

    #[tokio::test]
    async fn mcp_tools_list() {
        let s = server();
        let result = call(&s, "tools/list", serde_json::Value::Null).await;
        let tools = result["tools"].as_array().unwrap();
        assert!(tools.len() >= 7);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"create_project"));
        assert!(names.contains(&"render_project"));
        assert!(names.contains(&"analyse_video"));
    }

    #[tokio::test]
    async fn tool_create_and_inspect_project() {
        let s = server();
        let result = call(
            &s,
            "tools/call",
            serde_json::json!({ "name": "create_project", "arguments": { "name": "test-montage" } }),
        )
        .await;
        let text = result["content"][0]["text"].as_str().unwrap();
        let data: serde_json::Value = serde_json::from_str(text).unwrap();
        let pid = data["project_id"].as_str().unwrap().to_string();
        assert!(!pid.is_empty());

        // get_project should work
        let result2 = call(
            &s,
            "tools/call",
            serde_json::json!({ "name": "get_project", "arguments": { "project_id": pid } }),
        )
        .await;
        let text2 = result2["content"][0]["text"].as_str().unwrap();
        let data2: serde_json::Value = serde_json::from_str(text2).unwrap();
        assert_eq!(data2["name"], "test-montage");
        assert_eq!(data2["clip_count"], 0);
    }

    #[tokio::test]
    async fn tool_add_clip_appends_to_timeline() {
        let s = server();

        // Create project
        let r1 = call(
            &s,
            "tools/call",
            serde_json::json!({ "name": "create_project", "arguments": { "name": "p" } }),
        )
        .await;
        let pid: String = serde_json::from_str(r1["content"][0]["text"].as_str().unwrap())
            .map(|v: serde_json::Value| v["project_id"].as_str().unwrap().to_string())
            .unwrap();

        // Add first clip
        let r2 = call(
            &s,
            "tools/call",
            serde_json::json!({
                "name": "add_clip",
                "arguments": {
                    "project_id": pid,
                    "source_path": "/video/gameplay.mp4",
                    "source_start": 10.0,
                    "source_end": 13.0
                }
            }),
        )
        .await;
        let d2: serde_json::Value =
            serde_json::from_str(r2["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(d2["timeline_start"], 0.0);
        assert_eq!(d2["timeline_end"], 3.0);

        // Add second clip — should start where first ended
        let r3 = call(
            &s,
            "tools/call",
            serde_json::json!({
                "name": "add_clip",
                "arguments": {
                    "project_id": pid,
                    "source_path": "/video/gameplay.mp4",
                    "source_start": 20.0,
                    "source_end": 22.0
                }
            }),
        )
        .await;
        let d3: serde_json::Value =
            serde_json::from_str(r3["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(d3["timeline_start"], 3.0);
        assert_eq!(d3["timeline_end"], 5.0);
    }

    #[tokio::test]
    async fn tool_add_effect_to_clip() {
        let s = server();

        let r1 = call(
            &s,
            "tools/call",
            serde_json::json!({ "name": "create_project", "arguments": { "name": "p" } }),
        )
        .await;
        let pid: String =
            serde_json::from_str::<serde_json::Value>(r1["content"][0]["text"].as_str().unwrap())
                .map(|v| v["project_id"].as_str().unwrap().to_string())
                .unwrap();

        let r2 = call(
            &s,
            "tools/call",
            serde_json::json!({
                "name": "add_clip",
                "arguments": { "project_id": pid, "source_path": "/v.mp4", "source_start": 0.0, "source_end": 5.0 }
            }),
        )
        .await;
        let clip_id: String =
            serde_json::from_str::<serde_json::Value>(r2["content"][0]["text"].as_str().unwrap())
                .map(|v| v["clip_id"].as_str().unwrap().to_string())
                .unwrap();

        let r3 = call(
            &s,
            "tools/call",
            serde_json::json!({
                "name": "add_effect",
                "arguments": {
                    "project_id": pid,
                    "clip_id": clip_id,
                    "effect_type": "flash",
                    "params": { "peak_opacity": 0.9 }
                }
            }),
        )
        .await;
        let d3: serde_json::Value =
            serde_json::from_str(r3["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(d3["total_effects_on_clip"], 1);
    }

    #[tokio::test]
    async fn tool_list_styles_returns_three() {
        let s = server();
        let r = call(
            &s,
            "tools/call",
            serde_json::json!({ "name": "list_styles", "arguments": {} }),
        )
        .await;
        let d: serde_json::Value =
            serde_json::from_str(r["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(d["styles"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn notification_returns_no_response() {
        let s = server();
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::Value::Null,
            method: "notifications/initialized".into(),
            params: serde_json::Value::Null,
        };
        assert!(s.dispatch(req).await.is_none());
    }
}
