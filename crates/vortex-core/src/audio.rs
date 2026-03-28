//! Audio track data models.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An audio track on the timeline (music, SFX, or voice).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTrack {
    /// Unique identifier.
    pub id: Uuid,
    /// Human-readable label (e.g. "Music", "SFX Layer 1").
    pub label: String,
    /// Absolute path to the audio source file.
    pub source_path: String,
    /// Offset into the *source* file where playback starts (seconds).
    pub source_start: f64,
    /// Where this track begins on the output timeline (seconds).
    pub timeline_start: f64,
    /// Volume level 0.0–1.0.
    pub volume: f64,
    /// Whether to loop the track if shorter than the timeline.
    pub looped: bool,
    /// Fade-in duration in seconds.
    pub fade_in_secs: f64,
    /// Fade-out duration in seconds.
    pub fade_out_secs: f64,
    /// Beat markers detected in this audio track.
    pub beat_markers: Vec<BeatMarker>,
}

impl AudioTrack {
    /// Create a new audio track at timeline position 0.
    pub fn new(label: impl Into<String>, source_path: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: label.into(),
            source_path: source_path.into(),
            source_start: 0.0,
            timeline_start: 0.0,
            volume: 1.0,
            looped: false,
            fade_in_secs: 0.5,
            fade_out_secs: 1.0,
            beat_markers: Vec::new(),
        }
    }

    /// Set volume (0.0–1.0).
    pub fn with_volume(mut self, volume: f64) -> Self {
        self.volume = volume.clamp(0.0, 1.0);
        self
    }

    /// Enable looping.
    pub fn looped(mut self) -> Self {
        self.looped = true;
        self
    }
}

/// A single detected beat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatMarker {
    /// Time in seconds relative to audio track start.
    pub time: f64,
    /// Beat strength (0.0–1.0).
    pub strength: f64,
    /// Beat category: "kick", "snare", "hihat", "beat".
    pub beat_type: String,
}

impl BeatMarker {
    pub fn new(time: f64, strength: f64, beat_type: impl Into<String>) -> Self {
        Self {
            time,
            strength,
            beat_type: beat_type.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_track_volume_clamp() {
        let t = AudioTrack::new("Music", "/audio/track.mp3").with_volume(1.5);
        assert!((t.volume - 1.0).abs() < 1e-9);

        let t2 = AudioTrack::new("SFX", "/audio/sfx.wav").with_volume(-0.1);
        assert!((t2.volume).abs() < 1e-9);
    }
}
