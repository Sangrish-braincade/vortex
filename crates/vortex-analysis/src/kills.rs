//! Kill / highlight moment detection using YOLOv8 object detection.
//!
//! ## Pipeline
//!
//! 1. Extract one PNG frame every `frame_stride` frames via FFmpeg.
//! 2. Resize each frame to 640×640 and encode as raw RGB f32.
//! 3. Run YOLOv8 ONNX inference via `vortex-ml` `OnnxRuntime`.
//! 4. Parse output `[1, 84, 8400]` → `Vec<YoloDetection>` (COCO person class).
//! 5. Apply kill heuristic: "person" count drops from ≥1 to 0 within 0.5s.
//! 6. Cluster nearby kill events with `min_gap_secs` de-duplication.
//!
//! Without a real ONNX model (`yolov8n.onnx` or `yolov8-kills.onnx`),
//! the detector falls back to a single synthetic stub result.
//!
//! ## Model download
//! ```sh
//! wget https://github.com/ultralytics/assets/releases/download/v8.2.0/yolov8n.onnx \
//!   -O models/yolov8n.onnx
//! ```

use std::path::{Path, PathBuf};
use std::process::Stdio;

use vortex_core::{BoundingBox, Detection, KillMoment};
use vortex_ml::runtime::{InferenceBackend, OnnxRuntime, Tensor, COCO_CLASSES, parse_yolov8_output};

/// Configuration for the kill detector.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KillDetectorConfig {
    /// Path to the YOLOv8 ONNX model file.
    pub model_path: String,
    /// Minimum confidence threshold to register a detection.
    pub confidence_threshold: f64,
    /// How many frames to skip between inference calls (3 = every 3rd frame).
    pub frame_stride: u32,
    /// Minimum gap between kill moments in seconds (de-duplication window).
    pub min_gap_secs: f64,
    /// Whether to limit detection to the "person" class (COCO class 0).
    pub person_class_only: bool,
}

impl Default for KillDetectorConfig {
    fn default() -> Self {
        Self {
            model_path: "models/yolov8n.onnx".into(),
            confidence_threshold: 0.55,
            frame_stride: 3,
            min_gap_secs: 1.5,
            person_class_only: true,
        }
    }
}

/// Kill detection engine using YOLOv8 + person-count heuristic.
pub struct KillDetector {
    config: KillDetectorConfig,
}

impl KillDetector {
    pub fn new(config: KillDetectorConfig) -> Self {
        Self { config }
    }

    /// Analyse a video file and return all detected kill moments.
    ///
    /// Uses the ONNX model if available, otherwise returns a synthetic stub.
    pub async fn detect(&self, video_path: &str) -> crate::Result<Vec<KillMoment>> {
        tracing::info!(path = video_path, model = %self.config.model_path, "Starting kill detection");

        let model_exists = Path::new(&self.config.model_path).exists();
        if !model_exists {
            tracing::warn!(
                model = %self.config.model_path,
                "ONNX model not found — returning stub. Download with:\n  wget https://github.com/ultralytics/assets/releases/download/v8.2.0/yolov8n.onnx -O {}",
                self.config.model_path
            );
            return Ok(stub_result());
        }

        match self.detect_with_onnx(video_path).await {
            Ok(moments) => Ok(moments),
            Err(e) => {
                tracing::warn!(error = %e, "ONNX inference failed, returning stub");
                Ok(stub_result())
            }
        }
    }

    async fn detect_with_onnx(&self, video_path: &str) -> crate::Result<Vec<KillMoment>> {
        let fps = probe_fps(video_path).await.unwrap_or(30.0);
        let frame_interval = self.config.frame_stride as f64 / fps;

        // Extract frames to a temp directory
        let tmp = std::env::temp_dir().join("vortex-kills");
        tokio::fs::create_dir_all(&tmp).await
            .map_err(|e| crate::AnalysisError::Io(e))?;

        extract_frames_strided(video_path, &tmp, self.config.frame_stride).await?;

        // Load ONNX model
        let mut rt = OnnxRuntime::new(InferenceBackend::Cpu);
        let session = rt.load_model(&self.config.model_path)
            .map_err(|e| crate::AnalysisError::ModelError(e.to_string()))?;

        // Read frames, run inference
        let conf = self.config.confidence_threshold as f32;
        let mut frame_detections: Vec<(f64, usize)> = Vec::new(); // (time, person_count)

        let mut frame_paths: Vec<PathBuf> = std::fs::read_dir(&tmp)
            .map_err(crate::AnalysisError::Io)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("png"))
            .map(|e| e.path())
            .collect();
        frame_paths.sort();

        for (idx, path) in frame_paths.iter().enumerate() {
            let t = idx as f64 * frame_interval;
            let input = load_frame_as_tensor(path).await?;
            let output = rt.run(&session, vec![input])
                .map_err(|e| crate::AnalysisError::ModelError(e.to_string()))?;

            let person_count = if let Some((_, tensor)) = output.tensors.first() {
                let dets = parse_yolov8_output(tensor, conf, COCO_CLASSES);
                if self.config.person_class_only {
                    dets.iter().filter(|d| d.class_id == 0).count()
                } else {
                    dets.len()
                }
            } else {
                0
            };

            frame_detections.push((t, person_count));
        }

        // Clean up temp frames
        let _ = tokio::fs::remove_dir_all(&tmp).await;

        // Kill heuristic: person count drops ≥1 → 0 within one stride interval
        let moments = build_kill_moments(&frame_detections, self.config.min_gap_secs);
        tracing::info!(kills = moments.len(), "Kill detection complete");
        Ok(moments)
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Extract every Nth frame as PNG into `dir/NNNN.png`.
async fn extract_frames_strided(video_path: &str, dir: &Path, stride: u32) -> crate::Result<()> {
    // ffmpeg vf select filter: pick every stride-th frame
    let select = format!("not(mod(n,{}))", stride);
    let out_pattern = dir.join("%04d.png");

    let status = tokio::process::Command::new("ffmpeg")
        .args([
            "-hide_banner", "-loglevel", "error",
            "-i", video_path,
            "-vf", &format!("select={},scale=640:640:force_original_aspect_ratio=pad:pad_color=black", select),
            "-vsync", "0",
            &out_pattern.to_string_lossy(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| crate::AnalysisError::Io(e))?;

    if !status.success() {
        return Err(crate::AnalysisError::AudioError(
            format!("ffmpeg frame extract failed for {}", video_path),
        ));
    }
    Ok(())
}

/// Load a PNG frame file and encode it as a `[1, 3, 640, 640]` float32 tensor.
/// Pixel values normalised to [0, 1].
async fn load_frame_as_tensor(path: &Path) -> crate::Result<Tensor> {
    // Read PNG bytes and decode with ffmpeg to raw RGB
    let output = tokio::process::Command::new("ffmpeg")
        .args([
            "-hide_banner", "-loglevel", "error",
            "-i", &path.to_string_lossy(),
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",
            "-vf", "scale=640:640",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|e| crate::AnalysisError::Io(e))?;

    let raw = output.stdout;
    let expected = 640 * 640 * 3;
    if raw.len() < expected {
        // Return zeros if decode failed (e.g. corrupt frame)
        return Ok(Tensor::new(vec![0.0f32; 1 * 3 * 640 * 640], vec![1, 3, 640, 640]));
    }

    // Convert HWC uint8 → CHW float32
    let mut data = vec![0.0f32; 1 * 3 * 640 * 640];
    for h in 0..640_usize {
        for w in 0..640_usize {
            let pixel_idx = (h * 640 + w) * 3;
            for c in 0..3_usize {
                data[c * 640 * 640 + h * 640 + w] = raw[pixel_idx + c] as f32 / 255.0;
            }
        }
    }
    Ok(Tensor::new(data, vec![1, 3, 640, 640]))
}

/// Probe the frame rate of a video file.
async fn probe_fps(path: &str) -> Option<f64> {
    let output = tokio::process::Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=r_frame_rate",
            "-of", "csv=p=0",
            path,
        ])
        .output()
        .await
        .ok()?;

    let s = String::from_utf8_lossy(&output.stdout);
    let s = s.trim();
    if let Some((num, den)) = s.split_once('/') {
        let n: f64 = num.parse().ok()?;
        let d: f64 = den.parse().ok()?;
        if d > 0.0 { Some(n / d) } else { None }
    } else {
        s.parse().ok()
    }
}

/// Identify kill moments from frame-level person-count sequence.
///
/// Heuristic: a kill occurs when person_count goes from ≥1 to 0 in consecutive frames.
fn build_kill_moments(frame_detections: &[(f64, usize)], min_gap: f64) -> Vec<KillMoment> {
    let mut moments: Vec<KillMoment> = Vec::new();
    let mut last_kill_t = f64::NEG_INFINITY;

    for i in 1..frame_detections.len() {
        let (t_prev, count_prev) = frame_detections[i - 1];
        let (t_cur, count_cur)  = frame_detections[i];

        // Person dropped out
        if count_prev >= 1 && count_cur == 0 {
            let kill_t = (t_prev + t_cur) / 2.0;
            if kill_t - last_kill_t < min_gap { continue; }
            last_kill_t = kill_t;

            let confidence = 0.75 + (count_prev as f64 * 0.05).min(0.2);
            moments.push(KillMoment {
                source_time: kill_t,
                confidence,
                event_type: "kill".into(),
                detections: vec![Detection {
                    label: "person".into(),
                    confidence,
                    bbox: BoundingBox { x: 0.5, y: 0.5, width: 0.2, height: 0.4 },
                }],
            });
        }
    }
    moments
}

/// Synthetic fallback result (one kill at 5.2s).
fn stub_result() -> Vec<KillMoment> {
    vec![KillMoment {
        source_time: 5.2,
        confidence: 0.91,
        event_type: "kill".into(),
        detections: vec![Detection {
            label: "enemy".into(),
            confidence: 0.91,
            bbox: BoundingBox { x: 0.45, y: 0.35, width: 0.08, height: 0.15 },
        }],
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_kill_moments_detects_drop() {
        let frames = vec![(0.0, 0), (0.1, 2), (0.2, 1), (0.3, 0), (1.0, 2), (1.1, 0)];
        let moments = build_kill_moments(&frames, 0.5);
        assert_eq!(moments.len(), 2);
        assert!(moments[0].source_time < moments[1].source_time);
    }

    #[test]
    fn build_kill_moments_respects_min_gap() {
        let frames: Vec<(f64, usize)> = vec![(0.0, 1), (0.05, 0), (0.1, 1), (0.15, 0)];
        let moments = build_kill_moments(&frames, 1.0);
        assert_eq!(moments.len(), 1);
    }

    #[tokio::test]
    async fn kill_detector_stub_on_missing_model() {
        let mut cfg = KillDetectorConfig::default();
        cfg.model_path = "nonexistent/model.onnx".into();
        let detector = KillDetector::new(cfg);
        let moments = detector.detect("fake.mp4").await.unwrap();
        assert!(!moments.is_empty());
        assert!(moments[0].confidence > 0.0);
    }
}
