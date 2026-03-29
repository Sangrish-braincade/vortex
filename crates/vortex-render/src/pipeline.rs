//! FFmpeg render pipeline.
//!
//! Translates a `Project` into an FFmpeg command and spawns the process,
//! streaming progress back to the caller via a tokio channel.
//!
//! ## Architecture
//!
//! ```text
//! Project
//!   └─ Timeline.clips[]
//!        └─ per-clip: ffmpeg -ss {start} -t {dur} -i {path} -vf {effects}
//!   └─ concat filter_complex
//!   └─ audio mixdown
//!   └─ encode → output file
//! ```
//!
//! ## Implementation roadmap (Phase 1)
//!
//! 1. Implement `build_filter_complex()`:
//!    - For each clip: `-ss {src.start} -t {src.dur} -i {path}`
//!    - Per-clip effect chain from `vortex-effects::compose_effects()`
//!    - `concat=n={N}:v=1:a=1` to join clips
//! 2. Implement audio mixdown:
//!    - Mix `Timeline.audio_tracks` with `amix`
//!    - Apply `afade` for fade-in/out per track
//! 3. Wire progress: parse FFmpeg stderr `frame=N fps=X time=HH:MM:SS`
//!    and emit `RenderProgress` events.
//! 4. Write integration test: render a 5-second clip from `test-clips/`.

use std::path::Path;
use thiserror::Error;
use tokio::sync::mpsc;
use vortex_core::{OutputSettings, Project};
use vortex_effects::{compose_effects, EffectContext};

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("FFmpeg not found in PATH")]
    FfmpegNotFound,

    #[error("FFmpeg process failed with exit code {code}: {stderr}")]
    FfmpegFailed { code: i32, stderr: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Effect error: {0}")]
    EffectError(#[from] vortex_effects::EffectError),

    #[error("Output directory does not exist: {0}")]
    OutputDirMissing(String),

    #[error("No clips in timeline")]
    EmptyTimeline,
}

pub type Result<T> = std::result::Result<T, RenderError>;

/// A render progress event streamed during encoding.
#[derive(Debug, Clone)]
pub enum RenderProgress {
    /// FFmpeg started.
    Started { total_frames: u64 },
    /// Frame update during encode.
    Frame { current: u64, total: u64, fps: f32, eta_secs: f32 },
    /// Render completed successfully.
    Complete { output_path: String, duration_secs: f64 },
    /// Render failed.
    Failed { error: String },
}

/// Render configuration (overrides project defaults).
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Hardware acceleration: "none", "nvidia", "amf", "videotoolbox".
    pub hw_accel: String,
    /// Number of encoding threads (0 = auto).
    pub threads: u32,
    /// Extra FFmpeg flags appended to the command.
    pub extra_flags: Vec<String>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            hw_accel: "none".into(),
            threads: 0,
            extra_flags: vec![],
        }
    }
}

/// The main render pipeline.
pub struct RenderPipeline {
    config: RenderConfig,
}

impl RenderPipeline {
    /// Create a new pipeline with the given render config.
    pub fn new(config: RenderConfig) -> Self {
        Self { config }
    }

    /// Build the full FFmpeg command for rendering a project.
    ///
    /// Returns the command as a `Vec<String>` for inspection or subprocess spawn.
    pub fn build_command(
        &self,
        project: &Project,
        output_path: &str,
    ) -> Result<Vec<String>> {
        if project.timeline.clips.is_empty() {
            return Err(RenderError::EmptyTimeline);
        }

        let output = &project.output;
        let mut cmd = vec!["ffmpeg".to_string(), "-y".to_string()];

        // Input files — one per clip
        for clip in &project.timeline.clips {
            cmd.push("-ss".into());
            cmd.push(format!("{:.6}", clip.source_range.start));
            cmd.push("-t".into());
            cmd.push(format!("{:.6}", clip.source_range.duration()));
            cmd.push("-i".into());
            cmd.push(clip.source_path.clone());
        }

        // Audio inputs
        for track in &project.timeline.audio_tracks {
            cmd.push("-ss".into());
            cmd.push(format!("{:.6}", track.source_start));
            cmd.push("-i".into());
            cmd.push(track.source_path.clone());
        }

        // Build filter_complex
        let filter_complex = self.build_filter_complex(project, output)?;
        if !filter_complex.is_empty() {
            cmd.push("-filter_complex".into());
            cmd.push(filter_complex);
            cmd.push("-map".into());
            cmd.push("[vout]".into());
            cmd.push("-map".into());
            cmd.push("[aout]".into());
        }

        // Encoding settings
        cmd.extend(self.encoding_flags(output));

        // Hardware acceleration flags
        if self.config.hw_accel != "none" {
            cmd.extend(self.hw_accel_flags());
        }

        // Extra user flags
        cmd.extend(self.config.extra_flags.clone());

        cmd.push(output_path.to_string());
        Ok(cmd)
    }

    /// Build the `-filter_complex` string.
    ///
    /// # TODO (Phase 1)
    /// - Per-clip effect chains wired together.
    /// - Concat video streams.
    /// - Mix audio tracks.
    fn build_filter_complex(
        &self,
        project: &Project,
        output: &OutputSettings,
    ) -> Result<String> {
        let clips = &project.timeline.clips;
        let mut parts: Vec<String> = Vec::new();
        let mut video_labels: Vec<String> = Vec::new();
        let mut audio_labels: Vec<String> = Vec::new();

        for (i, clip) in clips.iter().enumerate() {
            let ctx = EffectContext::new(
                output.width,
                output.height,
                output.fps,
                clip.source_range.duration(),
            );

            let effect_chain = compose_effects(&clip.effects, &ctx)?;

            // Scale to output resolution
            let scale = format!(
                "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2",
                output.width, output.height, output.width, output.height,
            );

            let vfilter = if effect_chain.is_empty() {
                format!("[{i}:v]{scale}[v{i}]")
            } else {
                format!("[{i}:v]{effect_chain},{scale}[v{i}]")
            };

            parts.push(vfilter);
            video_labels.push(format!("[v{i}]"));

            // Audio: volume + speed adjustment
            let afilter = if (clip.speed - 1.0).abs() > 0.001 {
                format!(
                    "[{i}:a]atempo={speed:.4},volume={vol:.4}[a{i}]",
                    speed = clip.speed,
                    vol = db_to_linear(clip.audio_gain_db),
                )
            } else {
                format!(
                    "[{i}:a]volume={vol:.4}[a{i}]",
                    vol = db_to_linear(clip.audio_gain_db),
                )
            };
            parts.push(afilter);
            audio_labels.push(format!("[a{i}]"));
        }

        // Concat all clips
        let n = clips.len();
        let concat_inputs = format!(
            "{video}{audio}concat=n={n}:v=1:a=1[vconcat][aconcat]",
            video = video_labels.join(""),
            audio = audio_labels.join(""),
        );
        parts.push(concat_inputs);

        // Mix in music tracks
        let audio_tracks = &project.timeline.audio_tracks;
        if audio_tracks.is_empty() {
            parts.push("[aconcat]anull[aout]".into());
        } else {
            let music_inputs: String = audio_tracks
                .iter()
                .enumerate()
                .map(|(j, t)| {
                    format!(
                        "[{}:a]volume={:.4}[m{j}]",
                        n + j,
                        t.volume
                    )
                })
                .collect::<Vec<_>>()
                .join(";");
            if !music_inputs.is_empty() {
                parts.push(music_inputs);
            }
            let all_audio = std::iter::once("[aconcat]".to_string())
                .chain(audio_tracks.iter().enumerate().map(|(j, _)| format!("[m{j}]")))
                .collect::<Vec<_>>()
                .join("");
            parts.push(format!(
                "{all_audio}amix=inputs={n_mix}:duration=first:normalize=0[aout]",
                n_mix = 1 + audio_tracks.len(),
            ));
        }

        parts.push("[vconcat]null[vout]".into());

        Ok(parts.join(";"))
    }

    /// Output encoding flags for the target codec.
    fn encoding_flags(&self, output: &OutputSettings) -> Vec<String> {
        match output.codec.as_str() {
            "h264" => vec![
                "-c:v".into(), "libx264".into(),
                "-preset".into(), "fast".into(),
                "-crf".into(), "18".into(),
                "-c:a".into(), "aac".into(),
                "-b:a".into(), "192k".into(),
                "-pix_fmt".into(), "yuv420p".into(),
                "-movflags".into(), "+faststart".into(),
                "-r".into(), format!("{}", output.fps),
            ],
            "h265" => vec![
                "-c:v".into(), "libx265".into(),
                "-preset".into(), "fast".into(),
                "-crf".into(), "20".into(),
                "-c:a".into(), "aac".into(),
                "-b:a".into(), "192k".into(),
                "-pix_fmt".into(), "yuv420p".into(),
                "-r".into(), format!("{}", output.fps),
            ],
            "vp9" => vec![
                "-c:v".into(), "libvpx-vp9".into(),
                "-b:v".into(), "0".into(),
                "-crf".into(), "31".into(),
                "-c:a".into(), "libopus".into(),
                "-b:a".into(), "192k".into(),
                "-r".into(), format!("{}", output.fps),
            ],
            _ => vec![
                "-c:v".into(), "copy".into(),
                "-c:a".into(), "copy".into(),
            ],
        }
    }

    fn hw_accel_flags(&self) -> Vec<String> {
        match self.config.hw_accel.as_str() {
            "nvidia" => vec![
                "-hwaccel".into(), "cuda".into(),
                "-hwaccel_output_format".into(), "cuda".into(),
            ],
            "amf" => vec!["-hwaccel".into(), "d3d11va".into()],
            "videotoolbox" => vec!["-hwaccel".into(), "videotoolbox".into()],
            _ => vec![],
        }
    }

    /// Render a project, streaming progress events to the returned channel.
    ///
    /// Spawns `ffmpeg` as a subprocess and parses stderr for frame progress.
    /// Sends `Started → Frame* → Complete | Failed` events.
    pub async fn render(
        &self,
        project: &Project,
        output_path: &str,
    ) -> Result<mpsc::Receiver<RenderProgress>> {
        let (tx, rx) = mpsc::channel::<RenderProgress>(128);

        // Validate output directory up-front so the error is synchronous
        if let Some(parent) = Path::new(output_path).parent() {
            if !parent.exists() && parent != Path::new("") {
                return Err(RenderError::OutputDirMissing(
                    parent.display().to_string(),
                ));
            }
        }

        let cmd_args = self.build_command(project, output_path)?;
        tracing::info!(cmd = ?cmd_args, "FFmpeg command built");

        // Extract what we need before spawning (can't move `project` into async block)
        let output_path_str = output_path.to_string();
        let total_frames =
            (project.timeline.duration * project.output.fps as f64).ceil() as u64;
        let project_duration = project.timeline.duration;

        tokio::spawn(async move {
            let _ = tx.send(RenderProgress::Started { total_frames }).await;

            let spawn_result = tokio::process::Command::new(&cmd_args[0])
                .args(&cmd_args[1..])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn();

            let mut child = match spawn_result {
                Ok(c) => c,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    let _ = tx
                        .send(RenderProgress::Failed {
                            error:
                                "FFmpeg not found in PATH. Install from https://ffmpeg.org/download.html"
                                    .into(),
                        })
                        .await;
                    return;
                }
                Err(e) => {
                    let _ = tx
                        .send(RenderProgress::Failed {
                            error: format!("Failed to spawn FFmpeg: {e}"),
                        })
                        .await;
                    return;
                }
            };

            // Read stderr line by line and parse progress
            if let Some(stderr) = child.stderr.take() {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(ffmpeg = %line);
                    if let Some(event) = parse_ffmpeg_progress(&line, total_frames) {
                        let _ = tx.send(event).await;
                    }
                }
            }

            match child.wait().await {
                Ok(status) if status.success() => {
                    let _ = tx
                        .send(RenderProgress::Complete {
                            output_path: output_path_str,
                            duration_secs: project_duration,
                        })
                        .await;
                }
                Ok(status) => {
                    let _ = tx
                        .send(RenderProgress::Failed {
                            error: format!(
                                "FFmpeg exited with code {}",
                                status.code().unwrap_or(-1)
                            ),
                        })
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(RenderProgress::Failed {
                            error: format!("Process wait error: {e}"),
                        })
                        .await;
                }
            }
        });

        Ok(rx)
    }
}

/// Parse a single FFmpeg stderr progress line into a `RenderProgress::Frame` event.
///
/// FFmpeg emits lines like:
/// `frame=  240 fps= 58 q=-1.0 size=    2048kB time=00:00:04.00 bitrate=4194.3kbits/s`
fn parse_ffmpeg_progress(line: &str, total: u64) -> Option<RenderProgress> {
    if !line.contains("frame=") || !line.contains("fps=") {
        return None;
    }
    let current: u64 = extract_kv(line, "frame=")?.trim().parse().ok()?;
    let fps: f32 = extract_kv(line, "fps=")?.trim().parse().ok()?;
    let eta = if fps > 0.0 && total > current {
        (total - current) as f32 / fps
    } else {
        0.0
    };
    Some(RenderProgress::Frame { current, total, fps, eta_secs: eta })
}

/// Extract the value immediately following a key in a space-delimited FFmpeg line.
fn extract_kv<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let pos = line.find(key)?;
    let rest = line[pos + key.len()..].trim_start();
    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    Some(&rest[..end])
}

/// Convert dB gain to linear amplitude multiplier.
fn db_to_linear(db: f64) -> f64 {
    if db <= -144.0 {
        0.0 // effectively silent
    } else {
        10.0_f64.powf(db / 20.0)
    }
}

impl Default for RenderPipeline {
    fn default() -> Self {
        Self::new(RenderConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vortex_core::{Clip, Project, TimeRange};

    fn make_project() -> Project {
        let mut p = Project::new("test");
        let clip = Clip::new(
            "test-clips/sample-gameplay.mp4",
            TimeRange::new(0.0, 5.0).unwrap(),
            TimeRange::new(0.0, 5.0).unwrap(),
        );
        p.timeline.push_clip(clip);
        p
    }

    #[test]
    fn build_command_produces_ffmpeg() {
        let pipeline = RenderPipeline::default();
        let project = make_project();
        let cmd = pipeline.build_command(&project, "output/test.mp4").unwrap();
        assert_eq!(cmd[0], "ffmpeg");
        assert!(cmd.contains(&"output/test.mp4".to_string()));
    }

    #[test]
    fn empty_timeline_errors() {
        let pipeline = RenderPipeline::default();
        let project = Project::new("empty");
        assert!(pipeline.build_command(&project, "out.mp4").is_err());
    }

    #[test]
    fn db_to_linear_values() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
        assert!((db_to_linear(20.0) - 10.0).abs() < 1e-3);
        assert_eq!(db_to_linear(-200.0), 0.0);
    }

    #[tokio::test]
    async fn render_sends_started_then_terminal_event() {
        let pipeline = RenderPipeline::default();
        let project = make_project();
        // Use the OS temp dir so the output directory always exists
        let output = std::env::temp_dir().join("vortex-test.mp4");
        let mut rx = pipeline.render(&project, output.to_str().unwrap()).await.unwrap();
        // First event is always Started
        let first = rx.recv().await.unwrap();
        assert!(matches!(first, RenderProgress::Started { .. }));
        // Second event is Complete (if FFmpeg is in PATH) or Failed (if not).
        // Both are acceptable in a unit test environment.
        let second = rx.recv().await.unwrap();
        assert!(
            matches!(second, RenderProgress::Complete { .. } | RenderProgress::Failed { .. }),
            "unexpected event: {second:?}"
        );
    }

    #[test]
    fn parse_progress_line() {
        let line = "frame=  240 fps= 58 q=-1.0 size=    2048kB time=00:00:04.00 bitrate=4194.3kbits/s";
        let event = parse_ffmpeg_progress(line, 600);
        assert!(matches!(event, Some(RenderProgress::Frame { current: 240, .. })));
    }
}
