//! # vortex-script
//!
//! Embedded scripting runtime for VORTEX montage scripts using [Rhai].
//!
//! Agents and users write montage logic in a JS-like syntax (Rhai).
//! This crate registers the VORTEX API as Rhai functions, executes scripts,
//! and extracts the resulting [`Project`] timeline.
//!
//! ## Script API
//!
//! ```rhai
//! // Create a project, add clips, apply effects, configure render
//! let pid = create_project("my-montage");
//! let cid = add_clip(pid, "/path/to/gameplay.mp4", 10.0, 20.0);
//! add_effect(cid, "flash", #{});
//! add_effect(cid, "velocity", #{ min_speed: 0.15, ramp_in_secs: 0.5 });
//! set_bpm(128.0);
//! render(pid, "output/montage.mp4");
//! ```
//!
//! [Rhai]: https://rhai.rs

use std::sync::{Arc, Mutex};

use rhai::{Engine, Scope};
use thiserror::Error;
use vortex_core::{Clip, Effect, FlashEffect, Project, ShakeEffect, TimeRange, VelocityEffect};

#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("Script execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Script compilation error: {0}")]
    CompileError(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("IO error: {0}")]
    Io(String),
}

pub type Result<T> = std::result::Result<T, ScriptError>;

// ─── Shared state passed into Rhai via Arc<Mutex<_>> ────────────────────────

#[derive(Default)]
struct ScriptState {
    project: Option<Project>,
    global_bpm: f64,
    render_output: Option<String>,
}

/// The scripting runtime backed by Rhai.
pub struct ScriptRuntime {
    engine: Engine,
}

impl ScriptRuntime {
    /// Create and initialise a new script runtime with VORTEX API registered.
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine.set_max_expr_depths(64, 64);
        Self { engine }
    }

    /// Execute a Rhai montage script.
    /// Returns the [`Project`] built by the script.
    pub async fn execute(&mut self, source: &str) -> Result<Project> {
        tracing::info!(len = source.len(), "Executing Rhai script");

        let state = Arc::new(Mutex::new(ScriptState::default()));

        // Register VORTEX API functions
        register_api(&mut self.engine, Arc::clone(&state));

        let mut scope = Scope::new();
        self.engine
            .run_with_scope(&mut scope, source)
            .map_err(|e| ScriptError::ExecutionFailed(e.to_string()))?;

        let locked = state.lock().unwrap();
        let project = locked.project.clone().unwrap_or_else(|| Project::new("script-output"));
        Ok(project)
    }

    /// Execute a script file by path.
    pub async fn execute_file(&mut self, path: &str) -> Result<Project> {
        let source = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ScriptError::Io(e.to_string()))?;
        self.execute(&source).await
    }
}

impl Default for ScriptRuntime {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Rhai API registration ────────────────────────────────────────────────────

fn register_api(engine: &mut Engine, state: Arc<Mutex<ScriptState>>) {
    // create_project(name: &str) -> String (project id)
    {
        let s = Arc::clone(&state);
        engine.register_fn("create_project", move |name: &str| -> String {
            let project = Project::new(name);
            let id = project.id.to_string();
            s.lock().unwrap().project = Some(project);
            id
        });
    }

    // add_clip(project_id: &str, path: &str, start: f64, end: f64) -> String (clip id)
    {
        let s = Arc::clone(&state);
        engine.register_fn(
            "add_clip",
            move |_project_id: &str, path: &str, start: f64, end: f64| -> String {
                let src = TimeRange::new(start, end).unwrap_or_else(|_| TimeRange::new(0.0, end.max(start + 0.1)).unwrap());
                let mut guard = s.lock().unwrap();
                let project = guard.project.get_or_insert_with(|| Project::new("script-output"));
                let timeline_start = project.timeline.duration;
                let duration = src.duration();
                let tl = TimeRange::new(timeline_start, timeline_start + duration).unwrap();
                let clip = Clip::new(path, src, tl);
                let id = clip.id.to_string();
                project.timeline.push_clip(clip);
                id
            },
        );
    }

    // add_effect(clip_id: &str, effect_type: &str, params: Map) -> ()
    {
        let s = Arc::clone(&state);
        engine.register_fn(
            "add_effect",
            move |clip_id: &str, effect_type: &str, params: rhai::Map| {
                let effect = build_effect(effect_type, &params);
                let mut guard = s.lock().unwrap();
                if let Some(project) = guard.project.as_mut() {
                    for clip in project.timeline.clips.iter_mut() {
                        if clip.id.to_string() == clip_id {
                            clip.add_effect(effect);
                            return;
                        }
                    }
                }
            },
        );
    }

    // set_bpm(bpm: f64) -> ()
    {
        let s = Arc::clone(&state);
        engine.register_fn("set_bpm", move |bpm: f64| {
            s.lock().unwrap().global_bpm = bpm;
        });
    }

    // render(project_id: &str, output_path: &str) -> ()
    {
        let s = Arc::clone(&state);
        engine.register_fn("render", move |_project_id: &str, output_path: &str| {
            s.lock().unwrap().render_output = Some(output_path.to_string());
            tracing::info!(output = output_path, "render() called from script");
        });
    }

    // get_bpm() -> f64
    {
        let s = Arc::clone(&state);
        engine.register_fn("get_bpm", move || -> f64 { s.lock().unwrap().global_bpm });
    }
}

/// Map an effect type name + Rhai params map to an [`Effect`] variant.
fn build_effect(effect_type: &str, params: &rhai::Map) -> Effect {
    let get_f64 = |key: &str, default: f64| -> f64 {
        params.get(key)
            .and_then(|v| v.as_float().ok())
            .unwrap_or(default)
    };
    let get_str = |key: &str, default: &str| -> String {
        params.get(key)
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_else(|| default.to_string())
    };

    match effect_type {
        "velocity" => Effect::Velocity(VelocityEffect {
            min_speed: get_f64("min_speed", 0.15),
            max_speed: get_f64("max_speed", 1.0),
            ramp_in_secs: get_f64("ramp_in_secs", 0.3),
            ramp_out_secs: get_f64("ramp_out_secs", 0.5),
            easing: get_str("easing", "ease_in_out"),
        }),
        "zoom" => Effect::Zoom(vortex_core::ZoomEffect {
            from_scale: get_f64("from_scale", 1.0),
            to_scale: get_f64("to_scale", 1.15),
            duration_secs: get_f64("duration_secs", 0.2),
            focal_x: get_f64("focal_x", 0.5),
            focal_y: get_f64("focal_y", 0.5),
            easing: get_str("easing", "ease_out"),
        }),
        "shake" => Effect::Shake(ShakeEffect {
            intensity_x: get_f64("intensity_x", 12.0),
            intensity_y: get_f64("intensity_y", 8.0),
            frequency: get_f64("frequency", 24.0),
            decay: get_f64("decay", 0.85),
            seed: 42,
        }),
        "flash" | _ => Effect::Flash(FlashEffect {
            color: get_str("color", "#FFFFFF"),
            peak_opacity: get_f64("peak_opacity", 0.85),
            duration_secs: get_f64("duration_secs", 0.12),
            attack_ratio: get_f64("attack_ratio", 0.2),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runtime_creates_project() {
        let mut rt = ScriptRuntime::new();
        let project = rt
            .execute(r#"let pid = create_project("test");"#)
            .await
            .unwrap();
        assert_eq!(project.name, "test");
    }

    #[tokio::test]
    async fn runtime_adds_clips() {
        let mut rt = ScriptRuntime::new();
        let project = rt
            .execute(
                r#"
                let pid = create_project("clips-test");
                let c1 = add_clip(pid, "/a.mp4", 0.0, 5.0);
                let c2 = add_clip(pid, "/b.mp4", 0.0, 3.0);
                "#,
            )
            .await
            .unwrap();
        assert_eq!(project.timeline.clips.len(), 2);
    }

    #[tokio::test]
    async fn runtime_adds_effects() {
        let mut rt = ScriptRuntime::new();
        let project = rt
            .execute(
                r#"
                let pid = create_project("fx-test");
                let cid = add_clip(pid, "/clip.mp4", 0.0, 5.0);
                add_effect(cid, "flash", #{});
                add_effect(cid, "velocity", #{ min_speed: 0.2 });
                "#,
            )
            .await
            .unwrap();
        let clips = &project.timeline.clips;
        assert_eq!(clips[0].effects.len(), 2);
    }

    #[tokio::test]
    async fn runtime_set_bpm() {
        let mut rt = ScriptRuntime::new();
        // set_bpm doesn't error
        rt.execute("set_bpm(140.0);").await.unwrap();
    }

    #[tokio::test]
    async fn runtime_syntax_error_returns_err() {
        let mut rt = ScriptRuntime::new();
        let result = rt.execute("let x = @@@@;").await;
        assert!(result.is_err());
    }
}
