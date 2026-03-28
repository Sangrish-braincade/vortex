//! # vortex-core
//!
//! Core data models for the VORTEX AI-powered video montage engine.
//! Defines the fundamental types: [`Project`], [`Timeline`], [`Clip`],
//! [`Effect`], [`AudioTrack`], and all supporting structures.

pub mod audio;
pub mod clip;
pub mod effects;
pub mod timeline;

pub use audio::*;
pub use clip::*;
pub use effects::*;
pub use timeline::*;

use thiserror::Error;

/// Top-level VORTEX error type.
#[derive(Debug, Error)]
pub enum VortexError {
    #[error("Invalid time range: start {start} >= end {end}")]
    InvalidTimeRange { start: f64, end: f64 },

    #[error("Clip not found: {0}")]
    ClipNotFound(String),

    #[error("Effect error: {0}")]
    EffectError(String),

    #[error("Timeline error: {0}")]
    TimelineError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, VortexError>;

/// A complete VORTEX project — the root of everything.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Project {
    /// Unique project identifier.
    pub id: uuid::Uuid,
    /// Human-readable project name.
    pub name: String,
    /// The main timeline containing all clips and effects.
    pub timeline: Timeline,
    /// Global output settings.
    pub output: OutputSettings,
    /// Active style template name (e.g. "aggressive", "cinematic").
    pub style: Option<String>,
    /// Project creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last modified timestamp.
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Project {
    /// Create a new empty project with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: uuid::Uuid::new_v4(),
            name: name.into(),
            timeline: Timeline::new(),
            output: OutputSettings::default(),
            style: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Serialize the project to JSON.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Deserialize a project from JSON.
    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }
}

/// Output render settings for a project.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutputSettings {
    /// Output video width in pixels.
    pub width: u32,
    /// Output video height in pixels.
    pub height: u32,
    /// Frames per second.
    pub fps: f32,
    /// Target bitrate in kbps (0 = auto).
    pub bitrate_kbps: u32,
    /// Output codec (e.g. "h264", "h265", "vp9").
    pub codec: String,
    /// Output container format (e.g. "mp4", "mkv", "webm").
    pub format: String,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60.0,
            bitrate_kbps: 8000,
            codec: "h264".into(),
            format: "mp4".into(),
        }
    }
}
