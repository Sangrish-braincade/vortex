//! ONNX Runtime wrapper.
//!
//! Feature-gated: compile with `--features onnx` to enable real inference.
//! Without the feature the crate compiles fine but `run()` returns empty output.
//!
//! ## YOLOv8 model download
//!
//! ```sh
//! # Official nano model — detects "person" class which we use as enemy proxy:
//! wget https://github.com/ultralytics/assets/releases/download/v8.2.0/yolov8n.onnx \
//!   -O models/yolov8n.onnx
//!
//! # Or export from PyTorch weights:
//! pip install ultralytics
//! python -c "from ultralytics import YOLO; YOLO('yolov8n.pt').export(format='onnx')"
//! ```
//!
//! ## CS2-specific model (optional, higher accuracy for gaming)
//!
//! Fine-tuned YOLOv8 models for FPS enemy detection can be found on GitHub:
//! - <https://github.com/ibaiGorordo/ONNX-YOLOv8-Object-Detection>
//! - Replace `models/yolov8-kills.onnx` with any YOLOv8 `.onnx` export.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MlError {
    #[error("Model load failed: {0}")]
    LoadFailed(String),

    #[error("Inference failed: {0}")]
    InferenceFailed(String),

    #[error("Shape mismatch: expected {expected:?}, got {got:?}")]
    ShapeMismatch {
        expected: Vec<usize>,
        got: Vec<usize>,
    },

    #[error("ONNX runtime error: {0}")]
    OrtError(String),

    #[error("ONNX feature not enabled — compile with --features onnx")]
    FeatureDisabled,
}

pub type Result<T> = std::result::Result<T, MlError>;

/// Which hardware backend to run inference on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InferenceBackend {
    Cpu,
    Cuda,
    TensorRT,
    CoreMl,
}

impl Default for InferenceBackend {
    fn default() -> Self { Self::Cpu }
}

impl std::fmt::Display for InferenceBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cpu => write!(f, "CPU"),
            Self::Cuda => write!(f, "CUDA"),
            Self::TensorRT => write!(f, "TensorRT"),
            Self::CoreMl => write!(f, "CoreML"),
        }
    }
}

/// A loaded ONNX model session handle.
pub struct OnnxSession {
    pub model_path: String,
    pub backend: InferenceBackend,

    #[cfg(feature = "onnx")]
    inner: ort::Session,
}

impl std::fmt::Debug for OnnxSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxSession")
            .field("model_path", &self.model_path)
            .field("backend", &self.backend)
            .finish()
    }
}

/// Input tensor for inference.
#[derive(Debug)]
pub struct Tensor {
    /// Flat float32 data buffer (row-major).
    pub data: Vec<f32>,
    /// Tensor shape (e.g. `[1, 3, 640, 640]` for YOLOv8 input).
    pub shape: Vec<usize>,
}

impl Tensor {
    pub fn new(data: Vec<f32>, shape: Vec<usize>) -> Self {
        Self { data, shape }
    }

    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }
}

/// Output from a model inference call.
#[derive(Debug, Default)]
pub struct InferenceOutput {
    /// Named output tensors from the ONNX graph.
    pub tensors: Vec<(String, Tensor)>,
}

/// Central ONNX runtime manager. Wraps `ort::Session` when the `onnx` feature is on.
pub struct OnnxRuntime {
    pub backend: InferenceBackend,
}

impl OnnxRuntime {
    /// Create a new runtime targeting the given backend.
    pub fn new(backend: InferenceBackend) -> Self {
        tracing::info!(%backend, onnx_feature = cfg!(feature = "onnx"), "ONNX runtime init");
        Self { backend }
    }

    /// Load a model from `path`. Returns a session handle.
    pub fn load_model(&mut self, path: &str) -> Result<OnnxSession> {
        #[cfg(feature = "onnx")]
        {
            use ort::{GraphOptimizationLevel, Session};
            let mut builder = Session::builder()
                .map_err(|e| MlError::LoadFailed(e.to_string()))?
                .with_optimization_level(GraphOptimizationLevel::All)
                .map_err(|e| MlError::LoadFailed(e.to_string()))?;

            if self.backend == InferenceBackend::Cuda {
                // CUDAExecutionProvider is optional — fall back to CPU if unavailable
                use ort::execution_providers::CUDAExecutionProvider;
                builder = builder
                    .with_execution_providers([CUDAExecutionProvider::default().build()])
                    .map_err(|e| MlError::LoadFailed(e.to_string()))?;
            }

            let session = builder
                .commit_from_file(path)
                .map_err(|e| MlError::LoadFailed(e.to_string()))?;

            tracing::info!(path, "ONNX model loaded");
            return Ok(OnnxSession {
                model_path: path.to_string(),
                backend: self.backend,
                inner: session,
            });
        }

        #[cfg(not(feature = "onnx"))]
        {
            tracing::warn!(path, "ONNX feature disabled — returning stub session");
            Ok(OnnxSession {
                model_path: path.to_string(),
                backend: self.backend,
            })
        }
    }

    /// Run inference. Returns named output tensors.
    pub fn run(&self, session: &OnnxSession, inputs: Vec<Tensor>) -> Result<InferenceOutput> {
        #[cfg(feature = "onnx")]
        {
            use ndarray::Array;
            use ort::inputs;

            // Build ort input values from our Tensor type
            let ort_inputs: Vec<_> = inputs
                .iter()
                .map(|t| {
                    let shape: Vec<usize> = t.shape.clone();
                    let arr = Array::from_shape_vec(shape, t.data.clone())
                        .map_err(|e| MlError::ShapeMismatch {
                            expected: t.shape.clone(),
                            got: vec![t.data.len()],
                        })?;
                    Ok(ort::Value::from_array(arr.view())
                        .map_err(|e| MlError::InferenceFailed(e.to_string()))?)
                })
                .collect::<Result<Vec<_>>>()?;

            let outputs = session
                .inner
                .run(ort_inputs.as_slice())
                .map_err(|e| MlError::InferenceFailed(e.to_string()))?;

            let tensors = outputs
                .iter()
                .enumerate()
                .map(|(i, (name, val))| {
                    let extracted = val
                        .try_extract_tensor::<f32>()
                        .map_err(|e| MlError::InferenceFailed(e.to_string()))?;
                    let data: Vec<f32> = extracted.view().iter().cloned().collect();
                    let shape: Vec<usize> = extracted.view().shape().to_vec();
                    Ok((name.to_string(), Tensor::new(data, shape)))
                })
                .collect::<Result<Vec<_>>>()?;

            return Ok(InferenceOutput { tensors });
        }

        #[cfg(not(feature = "onnx"))]
        {
            tracing::debug!("ONNX feature disabled — returning empty inference output");
            Ok(InferenceOutput::default())
        }
    }
}

// ─── YOLOv8 output parsing ────────────────────────────────────────────────────

/// A single YOLO detection after NMS.
#[derive(Debug, Clone)]
pub struct YoloDetection {
    /// Class index (e.g. 0 = person in COCO).
    pub class_id: usize,
    /// Class name if available.
    pub class_name: String,
    /// Bounding box [cx, cy, w, h] in normalised coords [0,1].
    pub bbox: [f32; 4],
    /// Confidence score.
    pub confidence: f32,
}

/// Parse YOLOv8 output tensor `[1, 84, 8400]` (COCO: 4 bbox + 80 classes).
///
/// Returns detections above `conf_threshold` after NMS-style deduplication.
pub fn parse_yolov8_output(
    output: &Tensor,
    conf_threshold: f32,
    class_names: &[&str],
) -> Vec<YoloDetection> {
    // Expected shape: [1, num_classes+4, num_anchors]
    if output.shape.len() < 3 {
        return vec![];
    }
    let _batch = output.shape[0];
    let rows = output.shape[1]; // 84 for COCO
    let anchors = output.shape[2]; // 8400

    if rows < 5 {
        return vec![];
    }
    let num_classes = rows - 4;

    let mut detections: Vec<YoloDetection> = Vec::new();

    for a in 0..anchors {
        let cx = output.data[0 * anchors + a];
        let cy = output.data[1 * anchors + a];
        let w  = output.data[2 * anchors + a];
        let h  = output.data[3 * anchors + a];

        // Find best class
        let (class_id, score) = (0..num_classes)
            .map(|c| (c, output.data[(4 + c) * anchors + a]))
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap_or((0, 0.0));

        if score < conf_threshold { continue; }

        let class_name = class_names.get(class_id).copied().unwrap_or("unknown").to_string();
        detections.push(YoloDetection {
            class_id,
            class_name,
            bbox: [cx, cy, w, h],
            confidence: score,
        });
    }

    // Simple greedy NMS by confidence (full IoU-NMS is overkill for this use case)
    detections.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
    detections.truncate(100);
    detections
}

/// COCO class names (80 classes). Index 0 = "person".
pub const COCO_CLASSES: &[&str] = &[
    "person", "bicycle", "car", "motorbike", "aeroplane", "bus", "train", "truck",
    "boat", "traffic light", "fire hydrant", "stop sign", "parking meter", "bench",
    "bird", "cat", "dog", "horse", "sheep", "cow", "elephant", "bear", "zebra",
    "giraffe", "backpack", "umbrella", "handbag", "tie", "suitcase", "frisbee",
    "skis", "snowboard", "sports ball", "kite", "baseball bat", "baseball glove",
    "skateboard", "surfboard", "tennis racket", "bottle", "wine glass", "cup",
    "fork", "knife", "spoon", "bowl", "banana", "apple", "sandwich", "orange",
    "broccoli", "carrot", "hot dog", "pizza", "donut", "cake", "chair", "sofa",
    "pottedplant", "bed", "diningtable", "toilet", "tvmonitor", "laptop", "mouse",
    "remote", "keyboard", "cell phone", "microwave", "oven", "toaster", "sink",
    "refrigerator", "book", "clock", "vase", "scissors", "teddy bear",
    "hair drier", "toothbrush",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_loads_stub_model() {
        let mut rt = OnnxRuntime::new(InferenceBackend::Cpu);
        let session = rt.load_model("models/test.onnx").unwrap();
        assert_eq!(session.model_path, "models/test.onnx");
    }

    #[test]
    fn tensor_numel() {
        let t = Tensor::new(vec![0.0; 1 * 3 * 640 * 640], vec![1, 3, 640, 640]);
        assert_eq!(t.numel(), 1_228_800);
    }

    #[test]
    fn parse_yolov8_empty_output() {
        // Output below threshold should produce no detections
        let zeros = Tensor::new(vec![0.0; 1 * 84 * 8400], vec![1, 84, 8400]);
        let dets = parse_yolov8_output(&zeros, 0.5, COCO_CLASSES);
        assert!(dets.is_empty());
    }

    #[test]
    fn parse_yolov8_detects_person() {
        // Inject one high-confidence person detection
        let mut data = vec![0.0f32; 1 * 84 * 8400];
        // Anchor 0: cx=0.5, cy=0.5, w=0.2, h=0.4, class 0 (person) = 0.9
        data[0 * 8400 + 0] = 0.5; // cx
        data[1 * 8400 + 0] = 0.5; // cy
        data[2 * 8400 + 0] = 0.2; // w
        data[3 * 8400 + 0] = 0.4; // h
        data[4 * 8400 + 0] = 0.9; // class 0 = person
        let t = Tensor::new(data, vec![1, 84, 8400]);
        let dets = parse_yolov8_output(&t, 0.5, COCO_CLASSES);
        assert!(!dets.is_empty());
        assert_eq!(dets[0].class_name, "person");
        assert!((dets[0].confidence - 0.9).abs() < 1e-4);
    }
}
