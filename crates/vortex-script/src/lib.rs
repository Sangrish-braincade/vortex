//! # vortex-script
//!
//! Embedded JavaScript / TypeScript runtime for VORTEX montage scripts.
//!
//! Agents and users write montage logic in TypeScript (see `scripts/`).
//! This crate embeds a V8 / Deno runtime, exposes the VORTEX scripting API,
//! and executes those scripts to produce [`Project`] timelines.
//!
//! ## Implementation roadmap (Phase 3)
//!
//! 1. Add `deno_core` dependency.
//! 2. Register Rust-side ops:
//!    - `vortex.addClip(path, start, end)` → adds a clip to the timeline
//!    - `vortex.addEffect(clipId, effectType, params)` → attaches an effect
//!    - `vortex.setBpm(bpm)` → sets global BPM for beat-sync
//!    - `vortex.render(outputPath)` → triggers FFmpeg pipeline
//! 3. Bundle TypeScript type definitions (`vortex.d.ts`) for IDE support.
//! 4. Implement sandboxing: disallow filesystem access outside allowed dirs.

use thiserror::Error;
use vortex_core::Project;

#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("Script execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Script compilation error: {0}")]
    CompileError(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Runtime not initialized")]
    NotInitialized,
}

pub type Result<T> = std::result::Result<T, ScriptError>;

/// The scripting runtime. Wraps a V8 isolate (Deno Core).
pub struct ScriptRuntime {
    // TODO (Phase 3): isolate: deno_core::JsRuntime,
}

impl ScriptRuntime {
    /// Create and initialise a new script runtime.
    ///
    /// # TODO (Phase 3)
    /// ```ignore
    /// let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
    ///     extensions: vec![vortex_ops::init()],
    ///     ..Default::default()
    /// });
    /// ```
    pub fn new() -> Self {
        tracing::info!("Initialising script runtime (STUB)");
        Self {}
    }

    /// Execute a TypeScript/JavaScript montage script.
    /// Returns the resulting [`Project`] built by the script.
    ///
    /// # TODO (Phase 3)
    /// - Transpile TypeScript with `swc` or Deno's built-in transpiler.
    /// - Set up a `Project` builder in JS context.
    /// - Execute the script.
    /// - Extract the resulting project state from JS → Rust.
    pub async fn execute(&mut self, source: &str) -> Result<Project> {
        tracing::info!(len = source.len(), "Executing script (STUB)");
        Ok(Project::new("script-output"))
    }

    /// Execute a script file by path.
    pub async fn execute_file(&mut self, path: &str) -> Result<Project> {
        let source = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ScriptError::ExecutionFailed(e.to_string()))?;
        self.execute(&source).await
    }
}

impl Default for ScriptRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runtime_executes_stub() {
        let mut rt = ScriptRuntime::new();
        let project = rt.execute("// empty script").await.unwrap();
        assert_eq!(project.name, "script-output");
    }
}
