//! Timeline data model — the ordered sequence of clips and global effects.

use crate::{Clip, VortexError};
use serde::{Deserialize, Serialize};

/// A half-open time interval `[start, end)` in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeRange {
    /// Start time in seconds (inclusive).
    pub start: f64,
    /// End time in seconds (exclusive).
    pub end: f64,
}

impl TimeRange {
    /// Create a new `TimeRange`, returning an error if `start >= end`.
    pub fn new(start: f64, end: f64) -> crate::Result<Self> {
        if start >= end {
            return Err(VortexError::InvalidTimeRange { start, end });
        }
        Ok(Self { start, end })
    }

    /// Duration of the range in seconds.
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }

    /// Returns `true` if the given time falls within `[start, end)`.
    pub fn contains(&self, t: f64) -> bool {
        t >= self.start && t < self.end
    }

    /// Returns `true` if this range overlaps with `other`.
    pub fn overlaps(&self, other: &TimeRange) -> bool {
        self.start < other.end && other.start < self.end
    }
}

impl std::fmt::Display for TimeRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:.3}s, {:.3}s)", self.start, self.end)
    }
}

/// The main timeline — an ordered list of clips laid out on a time axis,
/// plus global audio tracks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    /// Clips ordered by their placement on the timeline.
    pub clips: Vec<Clip>,
    /// Global audio tracks (music, SFX layers).
    pub audio_tracks: Vec<crate::AudioTrack>,
    /// Total duration in seconds (auto-computed from clips if 0).
    pub duration: f64,
}

impl Timeline {
    /// Create an empty timeline.
    pub fn new() -> Self {
        Self {
            clips: Vec::new(),
            audio_tracks: Vec::new(),
            duration: 0.0,
        }
    }

    /// Append a clip and update the timeline duration.
    pub fn push_clip(&mut self, clip: Clip) {
        let end = clip.timeline_range.end;
        if end > self.duration {
            self.duration = end;
        }
        self.clips.push(clip);
    }

    /// Remove a clip by ID. Returns `ClipNotFound` if the ID doesn't exist.
    pub fn remove_clip(&mut self, id: &uuid::Uuid) -> crate::Result<Clip> {
        let pos = self
            .clips
            .iter()
            .position(|c| &c.id == id)
            .ok_or_else(|| VortexError::ClipNotFound(id.to_string()))?;
        let clip = self.clips.remove(pos);
        self.recalculate_duration();
        Ok(clip)
    }

    /// Find a clip by ID (immutable).
    pub fn find_clip(&self, id: &uuid::Uuid) -> Option<&Clip> {
        self.clips.iter().find(|c| &c.id == id)
    }

    /// All clips that overlap with the given time range.
    pub fn clips_at(&self, range: &TimeRange) -> Vec<&Clip> {
        self.clips
            .iter()
            .filter(|c| c.timeline_range.overlaps(range))
            .collect()
    }

    /// Recalculate `duration` from the current clip set.
    fn recalculate_duration(&mut self) {
        self.duration = self
            .clips
            .iter()
            .map(|c| c.timeline_range.end)
            .fold(0.0_f64, f64::max);
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_range_duration() {
        let r = TimeRange::new(1.0, 4.0).unwrap();
        assert!((r.duration() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn time_range_invalid() {
        assert!(TimeRange::new(4.0, 1.0).is_err());
        assert!(TimeRange::new(2.0, 2.0).is_err());
    }

    #[test]
    fn time_range_overlaps() {
        let a = TimeRange::new(0.0, 5.0).unwrap();
        let b = TimeRange::new(3.0, 8.0).unwrap();
        let c = TimeRange::new(6.0, 9.0).unwrap();
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }
}
