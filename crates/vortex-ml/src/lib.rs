//! # vortex-ml
//!
//! ONNX runtime wrapper for ML model inference within VORTEX.
//! Provides a unified interface over `ort` (ORT — ONNX Runtime for Rust).

pub mod runtime;
pub use runtime::*;
