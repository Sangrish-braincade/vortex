//! Video stabilization effect using FFmpeg `vidstab`.
//!
//! Two-pass stabilization:
//! 1. **vidstabdetect** — analyses motion vectors, writes `.trf` file.
//! 2. **vidstabtransform** — applies correction using the vectors.
//!
//! The first pass must run before the main encode. This is handled by the
//! render pipeline which checks for `StabilizeEffect` and runs pass 1 first.
//!
//! The filter fragment returned here is for pass 2 (vidstabtransform).
//! Pass 1 is driven by `vortex_render::stabilize_pass1`.
//!
//! ## FFmpeg requirement
//! FFmpeg must be built with `--enable-libvidstab`. Gyan's full build includes it.

use std::path::PathBuf;
use crate::{EffectContext, FilterFragment, Result};
use vortex_core::StabilizeEffect;

/// Generate the `vidstabtransform` filter fragment (pass 2).
///
/// The caller must run `vidstabdetect` first to produce the `.trf` file.
/// The vectors path is embedded in the filter string.
pub fn stabilize_filter(effect: &StabilizeEffect, _ctx: &EffectContext) -> Result<FilterFragment> {
    let vectors_path = if effect.vectors_path.is_empty() {
        default_vectors_path()
    } else {
        PathBuf::from(&effect.vectors_path)
    };

    let filter = format!(
        "vidstabtransform=input='{}':smoothing={}:zoom={:.4}:crop=black",
        vectors_path.to_string_lossy().replace('\\', "/"),
        effect.smoothing,
        effect.zoom * 100.0, // vidstab zoom is in percent
    );

    Ok(FilterFragment::new(
        filter,
        format!("stabilize smoothing={} zoom={:.1}%", effect.smoothing, effect.zoom * 100.0),
    ))
}

/// FFmpeg command arguments for pass 1 (vidstabdetect).
/// Must be run *before* the main encode.
///
/// Returns `["ffmpeg", "-i", input, "-vf", "vidstabdetect=...", "-f", "null", "-"]`
pub fn stabilize_pass1_args(
    input: &str,
    effect: &StabilizeEffect,
) -> Vec<String> {
    let vectors_path = if effect.vectors_path.is_empty() {
        default_vectors_path()
    } else {
        PathBuf::from(&effect.vectors_path)
    };

    let detect_filter = format!(
        "vidstabdetect=result='{}':shakiness=5:accuracy=15",
        vectors_path.to_string_lossy().replace('\\', "/"),
    );

    vec![
        "ffmpeg".into(),
        "-hide_banner".into(),
        "-loglevel".into(), "error".into(),
        "-i".into(), input.to_string(),
        "-vf".into(), detect_filter,
        "-f".into(), "null".into(),
        "-".into(),
    ]
}

fn default_vectors_path() -> PathBuf {
    std::env::temp_dir().join("vortex-vidstab.trf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stabilize_filter_generates_transform() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 5.0);
        let effect = StabilizeEffect::default();
        let f = stabilize_filter(&effect, &ctx).unwrap();
        assert!(f.filter.starts_with("vidstabtransform="), "filter: {}", f.filter);
        assert!(f.filter.contains("smoothing=10"));
    }

    #[test]
    fn pass1_args_structure() {
        let effect = StabilizeEffect::default();
        let args = stabilize_pass1_args("clip.mp4", &effect);
        assert_eq!(args[0], "ffmpeg");
        assert!(args.contains(&"vidstabdetect=result='".to_string().split('=').next().unwrap_or("").to_string())
            || args.iter().any(|a| a.contains("vidstabdetect")));
    }
}
