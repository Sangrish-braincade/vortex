//! ONNX Runtime wrapper.
//!
//! ## Implementation roadmap (Phase 2)
//!
//! 1. Add `ort` crate dependency (with CUDA feature flag for GPU inference).
//! 2. Implement `OnnxRuntime::load_model` to cache sessions.
//! 3. Implement `OnnxRuntime::run_inference` with proper tensor conversion.
//! 4. Add `ModelOutput` variants for detection (YOLO) and segmentation (SAM).
//! 5. Benchmark GPU vs CPU inference paths and document tradeoffs.

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
    fn default() -> Self {
        Self::Cpu
    }
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

/// A loaded ONNX model session.
///
/// # TODO (Phase 2)
/// Replace the stub with a real `ort::Session` wrapped behind this type.
pub struct OnnxSession {
    pub model_path: String,
    pub backend: InferenceBackend,
    // TODO: session: ort::Session,
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
    /// Flat float32 data buffer.
    pub data: Vec<f32>,
    /// Tensor shape (e.g. `[1, 3, 640, 640]` for YOLO input).
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
#[derive(Debug)]
pub struct InferenceOutput {
    /// Named output tensors from the ONNX graph.
    pub tensors: Vec<(String, Tensor)>,
}

/// The central ONNX runtime manager. Caches loaded sessions.
pub struct OnnxRuntime {
    backend: InferenceBackend,
    // TODO: sessions: HashMap<String, ort::Session>,
}

impl OnnxRuntime {
    /// Create a new runtime targeting the given backend.
    pub fn new(backend: InferenceBackend) -> Self {
        tracing::info!(%backend, "Initialising ONNX runtime (STUB)");
        Self { backend }
    }

    /// Load a model from `path`. Returns a session handle.
    ///
    /// # TODO (Phase 2)
    /// ```ignore
    /// let env = ort::Environment::builder().with_name("vortex").build()?;
    /// let session = ort::SessionBuilder::new(&env)?
    ///     .with_optimization_level(ort::GraphOptimizationLevel::All)?
    ///     .with_model_from_file(path)?;
    /// ```
    pub fn load_model(&mut self, path: &str) -> Result<OnnxSession> {
        tracing::info!(path, backend = %self.backend, "Loading ONNX model (STUB)");
        Ok(OnnxSession {
            model_path: path.to_string(),
            backend: self.backend,
        })
    }

    /// Run inference on a loaded session.
    ///
    /// # TODO (Phase 2)
    /// Convert `inputs` to `ort::Value` tensors, call `session.run()`,
    /// convert outputs back to `Tensor`.
    pub fn run(&self, _session: &OnnxSession, _inputs: Vec<Tensor>) -> Result<InferenceOutput> {
        tracing::debug!("Running inference (STUB — returning empty output)");
        Ok(InferenceOutput { tensors: vec![] })
    }
}

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
}
