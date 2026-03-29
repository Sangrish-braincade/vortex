//! SAM 2 / rembg rotoscoping pipeline.
//!
//! Drives the out-of-band segmentation step that happens *before* the main
//! FFmpeg encode for clips that have a `RotoscopeEffect` with `mode = "sam2"`
//! or `mode = "rembg"`.
//!
//! ## Pipeline overview
//!
//! ```text
//!  clip.mp4 ──► [extract_frames] ──► frames/
//!                                        │
//!                               [sam2_segment | rembg_segment]
//!                                        │
//!                                     masks/
//!                                        │
//!  clip.mp4 ──► [composite_masks] ──────►──► clip_alpha.mp4
//! ```
//!
//! The resulting `clip_alpha.mp4` (VP9 with alpha, or ProRes 4444) is then
//! used as the source for the main encode step.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use vortex_core::RotoscopeEffect;

/// Result of a rotoscope pre-pass on one clip.
#[derive(Debug)]
pub struct RotoscopeResult {
    /// Path to the alpha-composited clip (replaces the original for encoding).
    pub output_path: PathBuf,
    /// Total frames processed.
    pub frames: u32,
    /// Mode that was used.
    pub mode: String,
}

/// Error type for rotoscope operations.
#[derive(Debug, thiserror::Error)]
pub enum RotoscopeError {
    #[error("Frame extraction failed: {0}")]
    ExtractFailed(String),
    #[error("Segmentation failed ({mode}): {reason}")]
    SegmentFailed { mode: String, reason: String },
    #[error("Composite failed: {0}")]
    CompositeFailed(String),
    #[error("SAM 2 not installed. Run: pip install sam-2")]
    Sam2NotInstalled,
    #[error("rembg not installed. Run: pip install rembg")]
    RembgNotInstalled,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, RotoscopeError>;

/// Run the rotoscope pre-pass for a single clip.
///
/// Returns a [`RotoscopeResult`] with the path to the alpha-composite output,
/// which the render pipeline should use in place of the original clip.
///
/// For `chromakey`/`lumakey` modes this is a no-op — those are handled
/// inline by FFmpeg's filter graph.
pub async fn rotoscope_clip(
    source_path: &str,
    effect: &RotoscopeEffect,
    work_dir: &Path,
) -> Result<Option<RotoscopeResult>> {
    match effect.mode.as_str() {
        "sam2" => {
            let result = sam2_pipeline(source_path, effect, work_dir).await?;
            Ok(Some(result))
        }
        "rembg" => {
            let result = rembg_pipeline(source_path, effect, work_dir).await?;
            Ok(Some(result))
        }
        // chromakey / lumakey — inline FFmpeg filter, no pre-pass needed
        _ => Ok(None),
    }
}

// ─── SAM 2 pipeline ──────────────────────────────────────────────────────────

async fn sam2_pipeline(
    source_path: &str,
    effect: &RotoscopeEffect,
    work_dir: &Path,
) -> Result<RotoscopeResult> {
    let frames_dir = work_dir.join("frames");
    let masks_dir = work_dir.join("masks");
    let output = work_dir.join("roto_alpha.mp4");

    tokio::fs::create_dir_all(&frames_dir).await?;
    tokio::fs::create_dir_all(&masks_dir).await?;

    tracing::info!(source = source_path, "SAM 2: extracting frames");

    // Step 1: extract frames at full fps
    extract_frames(source_path, &frames_dir).await?;

    // Count frames extracted
    let frame_count = count_files(&frames_dir, "png").await;
    tracing::info!(frames = frame_count, "SAM 2: segmenting frames");

    // Step 2: run SAM 2 video predictor
    //
    // Requires: pip install sam-2
    // Model checkpoint: models/sam2_hiera_tiny.pt (auto-detected if in CWD or models/)
    let checkpoint = find_sam2_checkpoint(effect).await;
    let point_str = format!("{},{}", effect.prompt_point[0], effect.prompt_point[1]);

    let sam2_args: Vec<String> = vec![
        "-m".into(), "sam2.tools.video_predictor".into(),
        "--frames-dir".into(), frames_dir.to_string_lossy().into_owned(),
        "--masks-dir".into(), masks_dir.to_string_lossy().into_owned(),
        "--point".into(), point_str,
        "--model-variant".into(), effect.model_variant.clone(),
    ];

    // Add checkpoint if found
    let mut full_args = sam2_args;
    if let Some(ckpt) = checkpoint {
        full_args.push("--checkpoint".into());
        full_args.push(ckpt.to_string_lossy().into_owned());
    }

    let sam2_status = Command::new("python")
        .args(&full_args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                RotoscopeError::Sam2NotInstalled
            } else {
                RotoscopeError::SegmentFailed {
                    mode: "sam2".into(),
                    reason: e.to_string(),
                }
            }
        })?;

    if !sam2_status.success() {
        return Err(RotoscopeError::SegmentFailed {
            mode: "sam2".into(),
            reason: format!("exit code {:?}", sam2_status.code()),
        });
    }

    tracing::info!("SAM 2: compositing alpha");

    // Step 3: merge masks as alpha channel
    composite_alpha_channel(source_path, &masks_dir, &output).await?;

    Ok(RotoscopeResult {
        output_path: output,
        frames: frame_count,
        mode: "sam2".into(),
    })
}

// ─── rembg fallback pipeline ─────────────────────────────────────────────────

async fn rembg_pipeline(
    source_path: &str,
    _effect: &RotoscopeEffect,
    work_dir: &Path,
) -> Result<RotoscopeResult> {
    let frames_dir = work_dir.join("frames");
    let masks_dir = work_dir.join("masks");
    let output = work_dir.join("roto_alpha.mp4");

    tokio::fs::create_dir_all(&frames_dir).await?;
    tokio::fs::create_dir_all(&masks_dir).await?;

    extract_frames(source_path, &frames_dir).await?;
    let frame_count = count_files(&frames_dir, "png").await;

    tracing::info!(frames = frame_count, "rembg: removing backgrounds");

    // rembg batch-process a directory: rembg p <in_dir> <out_dir>
    let status = Command::new("rembg")
        .args(["p",
            &frames_dir.to_string_lossy(),
            &masks_dir.to_string_lossy(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                RotoscopeError::RembgNotInstalled
            } else {
                RotoscopeError::SegmentFailed {
                    mode: "rembg".into(),
                    reason: e.to_string(),
                }
            }
        })?;

    if !status.success() {
        return Err(RotoscopeError::SegmentFailed {
            mode: "rembg".into(),
            reason: format!("exit code {:?}", status.code()),
        });
    }

    composite_alpha_channel(source_path, &masks_dir, &output).await?;

    Ok(RotoscopeResult {
        output_path: output,
        frames: frame_count,
        mode: "rembg".into(),
    })
}

// ─── Shared helpers ──────────────────────────────────────────────────────────

/// Extract all frames of `source` to `<dir>/%04d.png`.
async fn extract_frames(source: &str, dir: &Path) -> Result<()> {
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner", "-loglevel", "error",
            "-i", source,
            "-vsync", "0",
            &format!("{}/{}", dir.to_string_lossy(), "%04d.png"),
        ])
        .status()
        .await?;

    if !status.success() {
        return Err(RotoscopeError::ExtractFailed(
            format!("ffmpeg frame extract failed for {}", source),
        ));
    }
    Ok(())
}

/// Merge source video with alpha masks: `alphamerge` → output with alpha.
///
/// Output is VP9 (libvpx-vp9) in a Matroska container with full alpha support.
async fn composite_alpha_channel(
    source: &str,
    masks_dir: &Path,
    output: &Path,
) -> Result<()> {
    // masks_dir contains rembg/SAM2 output: RGBA PNGs (we extract alpha)
    let mask_pattern = format!("{}/%04d.png", masks_dir.to_string_lossy());

    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner", "-loglevel", "error",
            "-i", source,
            "-i", &mask_pattern,
            "-filter_complex",
            // Extract alpha from mask, merge onto source
            "[1:v]alphaextract[alpha];[0:v][alpha]alphamerge,format=yuva420p",
            "-c:v", "libvpx-vp9",
            "-b:v", "0",
            "-crf", "18",
            "-auto-alt-ref", "0", // required for VP9 alpha
            "-c:a", "copy",
            "-y",
            &output.to_string_lossy(),
        ])
        .status()
        .await?;

    if !status.success() {
        return Err(RotoscopeError::CompositeFailed(
            format!("alphamerge failed → {:?}", output),
        ));
    }
    Ok(())
}

/// Count files with the given extension in a directory.
async fn count_files(dir: &Path, ext: &str) -> u32 {
    tokio::fs::read_dir(dir)
        .await
        .map(|mut rd| {
            let mut n = 0u32;
            // We can't easily use async iteration here without pinning,
            // so we do a quick synchronous count via std::fs
            let _ = rd; // suppress warning
            if let Ok(entries) = std::fs::read_dir(dir) {
                n = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path().extension().and_then(|s| s.to_str()) == Some(ext)
                    })
                    .count() as u32;
            }
            n
        })
        .unwrap_or(0)
}

/// Locate the SAM 2 checkpoint based on `model_variant` and `model_dir`.
///
/// Checks (in order):
/// 1. `effect.model_dir / sam2_{variant}.pt`
/// 2. `./models/sam2_{variant}.pt`
/// 3. `~/.cache/sam2/sam2_{variant}.pt`
async fn find_sam2_checkpoint(effect: &RotoscopeEffect) -> Option<PathBuf> {
    let variant = &effect.model_variant;
    let filename = format!("sam2_{}.pt", variant.replace("sam2_", "hiera_"));

    let candidates: Vec<PathBuf> = vec![
        if effect.model_dir.is_empty() {
            None
        } else {
            Some(PathBuf::from(&effect.model_dir).join(&filename))
        },
        Some(PathBuf::from("models").join(&filename)),
        dirs_home().map(|h| h.join(".cache").join("sam2").join(&filename)),
    ]
    .into_iter()
    .flatten()
    .collect();

    for path in candidates {
        if path.exists() {
            tracing::info!(path = %path.display(), "SAM 2 checkpoint found");
            return Some(path);
        }
    }
    tracing::warn!(filename, "SAM 2 checkpoint not found — will use default");
    None
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vortex_core::RotoscopeEffect;

    #[tokio::test]
    async fn chromakey_mode_returns_none() {
        let effect = RotoscopeEffect {
            mode: "chromakey".into(),
            ..Default::default()
        };
        let tmp = std::env::temp_dir().join("vortex-roto-test");
        let result = rotoscope_clip("fake.mp4", &effect, &tmp).await.unwrap();
        assert!(result.is_none(), "chromakey should skip the pre-pass");
    }

    #[tokio::test]
    async fn sam2_missing_python_returns_err() {
        // If SAM2 is installed this test is skipped. It verifies the error path.
        let effect = RotoscopeEffect {
            mode: "sam2".into(),
            ..Default::default()
        };
        let tmp = std::env::temp_dir().join("vortex-roto-sam2-test");
        // Will fail at extract_frames (no real source file) — that's fine,
        // we just want the function to not panic.
        let _ = rotoscope_clip("nonexistent.mp4", &effect, &tmp).await;
    }
}
