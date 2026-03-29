//! Kill / highlight moment detection using YOLOv8 object detection.
//!
//! ## Implementation roadmap (Phase 2)
//!
//! 1. Load YOLOv8 ONNX model via `vortex-ml` `OnnxRuntime`.
//! 2. Decode video frames with FFmpeg (every N frames for efficiency).
//! 3. Run inference: detect "enemy" bounding boxes.
//! 4. Apply kill heuristics:
//!    - Enemy count drops sharply between frames → kill registered.
//!    - Killcam / death screen colour signature detected.
//!    - Crosshair placement confidence model (aim assist / headshot).
//! 5. Cluster nearby detections into [`KillMoment`] events.
//! 6. Return sorted list of moments by confidence.

use vortex_core::{BoundingBox, Detection, KillMoment};

/// Configuration for the kill detector.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KillDetectorConfig {
    /// Path to the YOLOv8 ONNX model file.
    pub model_path: String,
    /// Minimum confidence threshold to register a detection.
    pub confidence_threshold: f64,
    /// How many frames to skip between inference calls (1 = every frame).
    pub frame_stride: u32,
    /// Minimum gap between kill moments in seconds (de-duplication window).
    pub min_gap_secs: f64,
}

impl Default for KillDetectorConfig {
    fn default() -> Self {
        Self {
            model_path: "models/yolov8-kills.onnx".into(),
            confidence_threshold: 0.65,
            frame_stride: 3,
            min_gap_secs: 1.5,
        }
    }
}

/// Kill detection engine.
pub struct KillDetector {
    config: KillDetectorConfig,
}

impl KillDetector {
    /// Create a new detector with the given configuration.
    pub fn new(config: KillDetectorConfig) -> Self {
        Self { config }
    }

    /// Analyse a video file and return all detected kill moments.
    ///
    /// # TODO (Phase 2)
    /// - Load ONNX model from `self.config.model_path` via `vortex_ml::OnnxRuntime`.
    /// - Iterate frames using FFmpeg frame decoder.
    /// - Run YOLOv8 inference per frame.
    /// - Apply clustering and de-duplication logic.
    pub async fn detect(&self, video_path: &str) -> crate::Result<Vec<KillMoment>> {
        tracing::info!(path = video_path, "Starting kill detection (STUB)");

        // TODO: real YOLOv8 inference
        // For now return a stub result so tests can compile and run.
        let _ = &self.config; // suppress unused warning until implemented

        Ok(vec![
            KillMoment {
                source_time: 5.2,
                confidence: 0.91,
                event_type: "kill".into(),
                detections: vec![Detection {
                    label: "enemy".into(),
                    confidence: 0.91,
                    bbox: BoundingBox {
                        x: 0.45,
                        y: 0.35,
                        width: 0.08,
                        height: 0.15,
                    },
                }],
            },
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn kill_detector_stub_returns_moments() {
        let detector = KillDetector::new(KillDetectorConfig::default());
        let moments = detector.detect("fake_path.mp4").await.unwrap();
        assert!(!moments.is_empty());
        assert!(moments[0].confidence > 0.0);
    }
}
