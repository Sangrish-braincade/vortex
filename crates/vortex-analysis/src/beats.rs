//! Beat and rhythm detection from audio tracks.
//!
//! ## Implementation roadmap (Phase 2)
//!
//! 1. Extract audio stream with FFmpeg → PCM float32 samples.
//! 2. Bind to `aubio` via a C FFI wrapper:
//!    - `aubio_tempo` for BPM and beat onset detection.
//!    - `aubio_onset` for transient / hit detection.
//! 3. Return `BeatAnalysis` with per-beat timestamps and strength.
//! 4. Classify beats by frequency content (kick vs. snare vs. hihat).

use vortex_core::audio::BeatMarker;

/// Result of beat analysis on an audio source.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BeatAnalysis {
    /// Detected tempo in beats per minute.
    pub bpm: f64,
    /// Confidence of the BPM estimate (0.0–1.0).
    pub bpm_confidence: f64,
    /// All detected beat markers, sorted by time.
    pub markers: Vec<BeatMarker>,
    /// Duration of the analysed audio in seconds.
    pub duration_secs: f64,
}

impl BeatAnalysis {
    /// Return beat markers stronger than the given threshold.
    pub fn strong_beats(&self, min_strength: f64) -> Vec<&BeatMarker> {
        self.markers
            .iter()
            .filter(|b| b.strength >= min_strength)
            .collect()
    }

    /// Return only kick drum beats.
    pub fn kicks(&self) -> Vec<&BeatMarker> {
        self.markers
            .iter()
            .filter(|b| b.beat_type == "kick")
            .collect()
    }

    /// Compute average inter-beat interval in seconds.
    pub fn average_ibi_secs(&self) -> f64 {
        if self.bpm > 0.0 {
            60.0 / self.bpm
        } else {
            0.0
        }
    }
}

/// Configuration for the beat detector.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BeatDetectorConfig {
    /// Aubio onset detection method: "hfc", "complex", "phase", "energy".
    pub onset_method: String,
    /// Hop size for the FFT window.
    pub hop_size: u32,
    /// FFT window size.
    pub window_size: u32,
    /// Silence threshold in dB.
    pub silence_db: f64,
}

impl Default for BeatDetectorConfig {
    fn default() -> Self {
        Self {
            onset_method: "complex".into(),
            hop_size: 256,
            window_size: 1024,
            silence_db: -70.0,
        }
    }
}

/// Beat detection engine.
pub struct BeatDetector {
    config: BeatDetectorConfig,
}

impl BeatDetector {
    pub fn new(config: BeatDetectorConfig) -> Self {
        Self { config }
    }

    /// Analyse an audio or video file and return beat analysis.
    ///
    /// # TODO (Phase 2)
    /// - Extract audio to PCM via FFmpeg.
    /// - Feed to aubio tempo + onset analysers.
    /// - Classify beats by frequency band.
    pub async fn analyse(&self, media_path: &str) -> crate::Result<BeatAnalysis> {
        tracing::info!(path = media_path, "Starting beat analysis (STUB)");

        let _ = &self.config;

        // Stub: 128 BPM, beats every ~0.469 seconds
        let bpm = 128.0_f64;
        let ibi = 60.0 / bpm;
        let duration = 30.0_f64;
        let markers: Vec<BeatMarker> = (0..((duration / ibi) as usize))
            .map(|i| {
                let t = i as f64 * ibi;
                let beat_type = if i % 4 == 0 { "kick" } else if i % 2 == 0 { "snare" } else { "hihat" };
                BeatMarker::new(t, if i % 4 == 0 { 1.0 } else { 0.6 }, beat_type)
            })
            .collect();

        Ok(BeatAnalysis {
            bpm,
            bpm_confidence: 0.95,
            markers,
            duration_secs: duration,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn beat_detector_stub() {
        let detector = BeatDetector::new(BeatDetectorConfig::default());
        let analysis = detector.analyse("fake_audio.mp3").await.unwrap();
        assert!((analysis.bpm - 128.0).abs() < 0.01);
        assert!(!analysis.markers.is_empty());
        let kicks = analysis.kicks();
        assert!(!kicks.is_empty());
    }
}
