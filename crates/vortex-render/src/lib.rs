//! # vortex-render
//!
//! FFmpeg-based video encoding pipeline. Takes a [`Project`] and produces
//! the final rendered video file.
//!
//! All FFmpeg invocations live here. Other crates call `vortex-render`;
//! they must NOT invoke `ffmpeg` directly.

pub mod pipeline;
pub use pipeline::*;
