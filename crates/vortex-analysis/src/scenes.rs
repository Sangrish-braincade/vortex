//! Scene cut and boundary detection.
//!
//! ## Implementation roadmap (Phase 2)
//!
//! 1. Use FFmpeg `select=gt(scene\,0.3)` filter to detect scene changes.
//! 2. Optionally run SAM3 segmentation to identify subject presence.
//! 3. Return `SceneCut` list sorted by timestamp.

/// A detected scene boundary in a source video.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SceneCut {
    /// Timestamp in seconds where the cut occurs.
    pub time_secs: f64,
    /// Scene change score from FFmpeg (0.0–1.0). Higher = harder cut.
    pub score: f64,
    /// Cut type: "hard", "dissolve", "fade_in", "fade_out".
    pub cut_type: String,
}

/// Configuration for scene detection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SceneDetectorConfig {
    /// Minimum scene change score to register as a cut.
    pub threshold: f64,
    /// Minimum gap between cuts in seconds (de-duplicate near-duplicates).
    pub min_gap_secs: f64,
}

impl Default for SceneDetectorConfig {
    fn default() -> Self {
        Self {
            threshold: 0.3,
            min_gap_secs: 0.5,
        }
    }
}

/// Scene boundary detector.
pub struct SceneDetector {
    config: SceneDetectorConfig,
}

impl SceneDetector {
    pub fn new(config: SceneDetectorConfig) -> Self {
        Self { config }
    }

    /// Detect scene cuts in a video file.
    ///
    /// # TODO (Phase 2)
    /// - Run FFmpeg with `select=gt(scene\,threshold),showinfo` filter.
    /// - Parse stdout timestamps and scores.
    /// - Apply `min_gap_secs` de-duplication.
    pub async fn detect(&self, video_path: &str) -> crate::Result<Vec<SceneCut>> {
        tracing::info!(path = video_path, "Starting scene detection (STUB)");
        let _ = &self.config;

        // Stub: return a few synthetic cuts
        Ok(vec![
            SceneCut { time_secs: 3.2, score: 0.82, cut_type: "hard".into() },
            SceneCut { time_secs: 8.7, score: 0.74, cut_type: "hard".into() },
            SceneCut { time_secs: 15.1, score: 0.91, cut_type: "hard".into() },
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scene_detector_stub() {
        let d = SceneDetector::new(SceneDetectorConfig::default());
        let cuts = d.detect("fake.mp4").await.unwrap();
        assert!(!cuts.is_empty());
        // Cuts should be sorted by time
        let times: Vec<f64> = cuts.iter().map(|c| c.time_secs).collect();
        let mut sorted = times.clone();
        sorted.sort_by(f64::total_cmp);
        assert_eq!(times, sorted);
    }
}
