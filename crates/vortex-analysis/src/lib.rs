//! # vortex-analysis
//!
//! Video and audio analysis: kill/highlight detection, beat detection,
//! and scene cut detection. Each analyser returns structured data that
//! the montage engine uses to drive automatic editing decisions.

pub mod beats;
pub mod kills;
pub mod scenes;

pub use beats::*;
pub use kills::*;
pub use scenes::*;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("FFmpeg probe failed: {0}")]
    FfmpegProbe(String),

    #[error("ML model error: {0}")]
    ModelError(String),

    #[error("Audio analysis error: {0}")]
    AudioError(String),

    #[error("Core error: {0}")]
    Core(#[from] vortex_core::VortexError),
}

pub type Result<T> = std::result::Result<T, AnalysisError>;

/// Aggregated analysis result for a single source clip.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ClipAnalysis {
    /// Path to the analysed source file.
    pub source_path: String,
    /// Duration in seconds.
    pub duration_secs: f64,
    /// Kill / highlight moments detected.
    pub kill_moments: Vec<vortex_core::KillMoment>,
    /// Scene cut boundaries (seconds into the file).
    pub scene_cuts: Vec<SceneCut>,
    /// Beat analysis from the embedded audio.
    pub beats: Option<BeatAnalysis>,
}
