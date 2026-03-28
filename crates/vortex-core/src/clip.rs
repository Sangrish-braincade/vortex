//! Clip data model — a segment of source video placed on the timeline.

use crate::{Effect, TimeRange};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A video clip placed at a position on the timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    /// Unique identifier for this clip instance.
    pub id: Uuid,
    /// Human-readable label (optional, used for debugging/display).
    pub label: Option<String>,
    /// Absolute path to the source media file.
    pub source_path: String,
    /// The range within the *source* file to use (trim in/out points).
    pub source_range: TimeRange,
    /// Where this clip sits on the *output* timeline.
    pub timeline_range: TimeRange,
    /// Effects applied to this clip, in application order.
    pub effects: Vec<Effect>,
    /// Playback speed multiplier (1.0 = normal, 0.5 = half-speed, 2.0 = double).
    pub speed: f64,
    /// Audio gain in dB relative to source (0.0 = unchanged, -inf = mute).
    pub audio_gain_db: f64,
    /// Whether to flip the clip horizontally.
    pub flip_horizontal: bool,
    /// Crop settings, if any.
    pub crop: Option<Crop>,
    /// Whether this clip was flagged by the kill-detection model.
    pub is_kill_moment: bool,
    /// Confidence score from ML analysis (0.0–1.0).
    pub kill_confidence: f64,
}

impl Clip {
    /// Create a new clip from a source file path and ranges.
    pub fn new(
        source_path: impl Into<String>,
        source_range: TimeRange,
        timeline_range: TimeRange,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: None,
            source_path: source_path.into(),
            source_range,
            timeline_range,
            effects: Vec::new(),
            speed: 1.0,
            audio_gain_db: 0.0,
            flip_horizontal: false,
            crop: None,
            is_kill_moment: false,
            kill_confidence: 0.0,
        }
    }

    /// Add an effect to this clip (appended to the end of the chain).
    pub fn add_effect(&mut self, effect: Effect) -> &mut Self {
        self.effects.push(effect);
        self
    }

    /// Chain effects fluently.
    pub fn with_effect(mut self, effect: Effect) -> Self {
        self.effects.push(effect);
        self
    }

    /// Set playback speed.
    pub fn with_speed(mut self, speed: f64) -> Self {
        self.speed = speed;
        self
    }

    /// Set the clip label for debugging.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Duration of this clip on the output timeline.
    pub fn output_duration(&self) -> f64 {
        self.timeline_range.duration()
    }

    /// Duration of the source segment used.
    pub fn source_duration(&self) -> f64 {
        self.source_range.duration()
    }
}

impl std::fmt::Display for Clip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Clip({}, src={}, out={})",
            self.label.as_deref().unwrap_or(&self.id.to_string()[..8]),
            self.source_range,
            self.timeline_range,
        )
    }
}

/// Rectangular crop applied to a clip before effects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crop {
    /// Left edge offset in pixels.
    pub x: u32,
    /// Top edge offset in pixels.
    pub y: u32,
    /// Width of the crop region in pixels.
    pub width: u32,
    /// Height of the crop region in pixels.
    pub height: u32,
}

/// A beat marker — a timestamp identified by beat/rhythm analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatMarker {
    /// Time in seconds from the audio track start.
    pub time: f64,
    /// Beat strength (0.0–1.0). Higher = harder hit.
    pub strength: f64,
    /// Beat category: "kick", "snare", "hihat", etc.
    pub beat_type: String,
}

/// A detected kill/highlight moment from gameplay analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillMoment {
    /// Time in the source clip where the kill occurs.
    pub source_time: f64,
    /// Model confidence score (0.0–1.0).
    pub confidence: f64,
    /// Type of event: "kill", "ace", "clutch", "headshot".
    pub event_type: String,
    /// Bounding boxes of detected subjects at this moment.
    pub detections: Vec<Detection>,
}

/// A single object detection result from YOLOv8.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection {
    /// Label (e.g. "enemy", "ally").
    pub label: String,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// Bounding box in normalized coordinates [0,1].
    pub bbox: BoundingBox,
}

/// Normalized bounding box.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TimeRange;

    fn make_clip() -> Clip {
        Clip::new(
            "/video/clip.mp4",
            TimeRange::new(0.0, 5.0).unwrap(),
            TimeRange::new(0.0, 5.0).unwrap(),
        )
    }

    #[test]
    fn clip_creation() {
        let clip = make_clip();
        assert_eq!(clip.effects.len(), 0);
        assert!((clip.speed - 1.0).abs() < 1e-9);
        assert!((clip.output_duration() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn clip_effect_chain() {
        let clip = make_clip()
            .with_effect(Effect::Flash(crate::FlashEffect::default()))
            .with_effect(Effect::Shake(crate::ShakeEffect::default()));
        assert_eq!(clip.effects.len(), 2);
    }
}
