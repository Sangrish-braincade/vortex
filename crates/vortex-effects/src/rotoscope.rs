//! Rotoscope effect — subject isolation via chroma/luma keying or ML segmentation.
//!
//! ## Modes
//!
//! ### `chromakey` / `lumakey`
//! Pure FFmpeg filters — handled entirely in [`crate::rotoscope_filter`].
//! No subprocess needed; composited inline in the main encode.
//!
//! ### `sam2` — Segment Anything Model 2
//! SAM 2 provides **temporally consistent** video segmentation: it tracks
//! the segmented subject across frames, producing smooth masks even through
//! motion blur and occlusion. This is what makes it the right tool for
//! gaming montage rotoscoping (character isolation, background swap/blur).
//!
//! Pipeline (implemented in `vortex-render`):
//! 1. Extract frames: `ffmpeg -i clip.mp4 frames/%04d.png`
//! 2. Run SAM 2 with a single prompt point (centre of frame by default):
//!    ```sh
//!    python -m sam2.tools.video_predictor \
//!      --checkpoint models/sam2_hiera_tiny.pt \
//!      --frames-dir frames/ --masks-dir masks/ \
//!      --point 0.5,0.45
//!    ```
//! 3. Apply masks as alpha channel + composite:
//!    ```sh
//!    ffmpeg -i frames/%04d.png -i masks/%04d.png \
//!      -filter_complex "[0:v][1:v]alphamerge,format=yuva420p" \
//!      -c:v libvpx-vp9 output_alpha.webm
//!    ```
//!
//! ### `rembg` — per-frame ML background removal (fallback)
//! Uses U²-Net (or IS-Net) ONNX models for single-frame background removal.
//! Less temporally stable than SAM 2 but requires no prompt.
//! ```sh
//! rembg p frames/ masks/
//! ```
//!
//! ## SAM 2 setup
//! ```sh
//! pip install sam-2
//! # Download checkpoint (tiny = fastest, good enough for montage work):
//! wget https://dl.fbaipublicfiles.com/segment_anything_2/072824/sam2_hiera_tiny.pt \
//!   -O models/sam2_hiera_tiny.pt
//! ```

// This module intentionally has no Rust code — all logic lives in
// `rotoscope_filter` in `lib.rs` and the SAM 2 subprocess driver in
// `vortex-render/src/rotoscope.rs`. This file serves as documentation hub.
