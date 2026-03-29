//! Effect data models — every visual/motion effect VORTEX can apply.
//!
//! Effects are composable: a [`Clip`] holds a `Vec<Effect>` and the render
//! pipeline applies them in order, building an FFmpeg filter graph chain.

use serde::{Deserialize, Serialize};

/// All effect variants supported by VORTEX.
///
/// Each variant carries a strongly-typed settings struct.
/// Use [`Effect::chain`] to combine multiple effects fluently.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Effect {
    /// Temporal velocity ramp (slow-mo / speed-up).
    Velocity(VelocityEffect),
    /// Zoom-in punch or zoom-out pull.
    Zoom(ZoomEffect),
    /// Camera shake / jitter.
    Shake(ShakeEffect),
    /// Color grade / LUT overlay.
    Color(ColorEffect),
    /// White or color flash frame burst.
    Flash(FlashEffect),
    /// Chromatic aberration lens distortion.
    Chromatic(ChromaticEffect),
    /// Letterbox / aspect ratio overlay.
    Letterbox(LetterboxEffect),
    /// Vignette darkening around the edges.
    Vignette(VignetteEffect),
    /// Glitch / datamosh artifact.
    Glitch(GlitchEffect),
    /// Rotoscoping — background removal or chroma key compositing.
    Rotoscope(RotoscopeEffect),
    /// Text / title overlay.
    Text(TextEffect),
    /// Video stabilization (remove camera wobble).
    Stabilize(StabilizeEffect),
}

impl Effect {
    /// Returns the effect name as a string slice (for logging / filter graph labelling).
    pub fn name(&self) -> &'static str {
        match self {
            Effect::Velocity(_) => "velocity",
            Effect::Zoom(_) => "zoom",
            Effect::Shake(_) => "shake",
            Effect::Color(_) => "color",
            Effect::Flash(_) => "flash",
            Effect::Chromatic(_) => "chromatic",
            Effect::Letterbox(_) => "letterbox",
            Effect::Vignette(_) => "vignette",
            Effect::Glitch(_) => "glitch",
            Effect::Rotoscope(_) => "rotoscope",
            Effect::Text(_) => "text",
            Effect::Stabilize(_) => "stabilize",
        }
    }
}

impl std::fmt::Display for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Effect::{}", self.name())
    }
}

// ─── Velocity ────────────────────────────────────────────────────────────────

/// Temporal velocity ramp: slow the clip down at the highlight moment,
/// then snap back to full speed. Implemented via FFmpeg `setpts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VelocityEffect {
    /// Speed at the slowest point (e.g. 0.15 = 15% speed → extreme slow-mo).
    pub min_speed: f64,
    /// Speed at normal playback (1.0).
    pub max_speed: f64,
    /// Duration of the slow-in ramp in seconds.
    pub ramp_in_secs: f64,
    /// Duration of the slow-out ramp back to full speed in seconds.
    pub ramp_out_secs: f64,
    /// Easing curve: "linear", "ease_in_out", "cubic".
    pub easing: String,
}

impl Default for VelocityEffect {
    fn default() -> Self {
        Self {
            min_speed: 0.15,
            max_speed: 1.0,
            ramp_in_secs: 0.3,
            ramp_out_secs: 0.5,
            easing: "ease_in_out".into(),
        }
    }
}

// ─── Zoom ─────────────────────────────────────────────────────────────────────

/// Scale punch / pull effect. Implemented via FFmpeg `zoompan` or `scale`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomEffect {
    /// Start scale factor (1.0 = no zoom).
    pub from_scale: f64,
    /// End scale factor.
    pub to_scale: f64,
    /// Duration of the zoom animation in seconds.
    pub duration_secs: f64,
    /// Focal point X in normalized coordinates [0, 1] (0.5 = center).
    pub focal_x: f64,
    /// Focal point Y in normalized coordinates [0, 1].
    pub focal_y: f64,
    /// Easing: "linear", "ease_in", "ease_out", "spring".
    pub easing: String,
}

impl Default for ZoomEffect {
    fn default() -> Self {
        Self {
            from_scale: 1.0,
            to_scale: 1.15,
            duration_secs: 0.2,
            focal_x: 0.5,
            focal_y: 0.5,
            easing: "ease_out".into(),
        }
    }
}

// ─── Shake ────────────────────────────────────────────────────────────────────

/// Camera shake / jitter. Implemented via FFmpeg `crop` with per-frame offsets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShakeEffect {
    /// Maximum horizontal displacement in pixels.
    pub intensity_x: f64,
    /// Maximum vertical displacement in pixels.
    pub intensity_y: f64,
    /// Number of shake oscillations per second.
    pub frequency: f64,
    /// Decay factor: how fast the shake fades out (0 = no decay, 1 = instant).
    pub decay: f64,
    /// Random seed for reproducible shake patterns.
    pub seed: u64,
}

impl Default for ShakeEffect {
    fn default() -> Self {
        Self {
            intensity_x: 12.0,
            intensity_y: 8.0,
            frequency: 24.0,
            decay: 0.85,
            seed: 42,
        }
    }
}

// ─── Color ────────────────────────────────────────────────────────────────────

/// Color grading effect. Supports LUT files and manual curve adjustments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorEffect {
    /// Optional path to a `.cube` or `.3dl` LUT file.
    pub lut_path: Option<String>,
    /// LUT blend strength (0.0 = no effect, 1.0 = full LUT).
    pub lut_strength: f64,
    /// Saturation multiplier (1.0 = unchanged, 0 = grayscale, 2 = vivid).
    pub saturation: f64,
    /// Contrast multiplier (1.0 = unchanged).
    pub contrast: f64,
    /// Brightness offset (-1.0 to +1.0, 0 = unchanged).
    pub brightness: f64,
    /// Hue rotation in degrees (-180 to +180).
    pub hue_shift: f64,
}

impl Default for ColorEffect {
    fn default() -> Self {
        Self {
            lut_path: None,
            lut_strength: 1.0,
            saturation: 1.2,
            contrast: 1.1,
            brightness: 0.0,
            hue_shift: 0.0,
        }
    }
}

// ─── Flash ────────────────────────────────────────────────────────────────────

/// White/color flash overlay at a beat or highlight moment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashEffect {
    /// Flash color as RGBA hex string (e.g. "#FFFFFF" or "#FF5500").
    pub color: String,
    /// Peak opacity at the flash center frame (0.0–1.0).
    pub peak_opacity: f64,
    /// Total flash duration in seconds (fade in + out).
    pub duration_secs: f64,
    /// Fraction of `duration_secs` spent fading in (0.0–1.0).
    pub attack_ratio: f64,
}

impl Default for FlashEffect {
    fn default() -> Self {
        Self {
            color: "#FFFFFF".into(),
            peak_opacity: 0.85,
            duration_secs: 0.12,
            attack_ratio: 0.2,
        }
    }
}

// ─── Chromatic ────────────────────────────────────────────────────────────────

/// Chromatic aberration — RGB channel separation for a lens distortion look.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChromaticEffect {
    /// Horizontal channel offset in pixels (red channel).
    pub offset_r_x: f64,
    /// Vertical channel offset in pixels (red channel).
    pub offset_r_y: f64,
    /// Horizontal channel offset in pixels (blue channel).
    pub offset_b_x: f64,
    /// Vertical channel offset in pixels (blue channel).
    pub offset_b_y: f64,
    /// Blend strength (0.0–1.0).
    pub strength: f64,
}

impl Default for ChromaticEffect {
    fn default() -> Self {
        Self {
            offset_r_x: 4.0,
            offset_r_y: 0.0,
            offset_b_x: -4.0,
            offset_b_y: 0.0,
            strength: 0.7,
        }
    }
}

// ─── Letterbox ────────────────────────────────────────────────────────────────

/// Cinematic letterbox bars.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LetterboxEffect {
    /// Target aspect ratio (e.g. 2.39 for anamorphic).
    pub aspect_ratio: f64,
    /// Bar color (default: black).
    pub bar_color: String,
    /// Animate bars sliding in over this many seconds (0 = instant).
    pub animate_secs: f64,
}

impl Default for LetterboxEffect {
    fn default() -> Self {
        Self {
            aspect_ratio: 2.39,
            bar_color: "#000000".into(),
            animate_secs: 0.3,
        }
    }
}

// ─── Vignette ─────────────────────────────────────────────────────────────────

/// Edge darkening vignette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VignetteEffect {
    /// Vignette strength (0.0–1.0).
    pub strength: f64,
    /// Inner radius of the vignette in normalized coordinates.
    pub inner_radius: f64,
    /// Outer radius (feather edge).
    pub outer_radius: f64,
}

impl Default for VignetteEffect {
    fn default() -> Self {
        Self {
            strength: 0.5,
            inner_radius: 0.4,
            outer_radius: 0.9,
        }
    }
}

// ─── Rotoscope ────────────────────────────────────────────────────────────────

/// Rotoscoping — isolate subjects from background using keying or ML segmentation.
///
/// Four modes:
/// - `"chromakey"`: remove a background colour (green/blue screen). Pure FFmpeg, instant.
/// - `"lumakey"`: key out bright or dark regions. Pure FFmpeg.
/// - `"sam2"`: ML-based video segmentation via SAM 2 (Segment Anything Model 2).
///   Uses `sam_onnx_rust` or the SAM 2 Python CLI. Temporally consistent across frames.
/// - `"rembg"`: per-frame ML background removal via `rembg` CLI (fallback).
///
/// SAM 2 is the preferred ML mode: it tracks subjects across frames and produces
/// smooth, temporally consistent masks — essential for video montage work.
/// See: <https://github.com/facebookresearch/sam2>
/// Rust ONNX wrapper: <https://github.com/AndreyGermanov/sam_onnx_rust>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotoscopeEffect {
    /// Keying mode: `"chromakey"`, `"lumakey"`, `"sam2"`, or `"rembg"`.
    pub mode: String,
    /// Key colour for chromakey mode (hex, e.g. `"#00FF00"` for green screen).
    pub key_color: String,
    /// Similarity threshold for chroma/luma keying (0.0–1.0).
    pub similarity: f64,
    /// Edge blend softness / mask feather (0.0–1.0).
    pub blend: f64,
    /// Background to composite onto: path to image/video, `"blur"`, or `"transparent"`.
    pub background: String,
    /// Whether to invert the mask (keep background, remove foreground).
    pub invert: bool,
    /// SAM 2 / rembg model variant: `"sam2_t"` (tiny), `"sam2_s"` (small), `"sam2_b+"` (base+), `"sam2_l"` (large).
    pub model_variant: String,
    /// Path to SAM 2 ONNX encoder/decoder model directory. Empty = auto-download.
    pub model_dir: String,
    /// Prompt point for SAM 2 in normalised coords `[x, y]` (0.5,0.5 = centre).
    pub prompt_point: [f32; 2],
}

impl Default for RotoscopeEffect {
    fn default() -> Self {
        Self {
            mode: "sam2".into(),
            key_color: "#00FF00".into(),
            similarity: 0.3,
            blend: 0.05,
            background: "transparent".into(),
            invert: false,
            model_variant: "sam2_t".into(),
            model_dir: String::new(),
            prompt_point: [0.5, 0.45],
        }
    }
}

// ─── Glitch ───────────────────────────────────────────────────────────────────

/// Datamosh / glitch artifact overlay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlitchEffect {
    /// Number of horizontal scan lines to displace per frame.
    pub scan_lines: u32,
    /// Maximum pixel displacement per scan line.
    pub displacement: f64,
    /// Duration in seconds.
    pub duration_secs: f64,
    /// Random seed.
    pub seed: u64,
}

impl Default for GlitchEffect {
    fn default() -> Self {
        Self {
            scan_lines: 8,
            displacement: 20.0,
            duration_secs: 0.1,
            seed: 0,
        }
    }
}

// ─── Text ─────────────────────────────────────────────────────────────────────

/// Text / title overlay rendered via FFmpeg `drawtext`.
///
/// Supports lower-thirds, kill-feed text, animated titles, and watermarks.
/// Font sizing is in points; position in normalised [0,1] coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEffect {
    /// The text string to render (supports newlines with `\n`).
    pub text: String,
    /// Font file path, or empty to use FFmpeg's built-in font.
    pub font_path: String,
    /// Font size in points.
    pub font_size: f64,
    /// Text color in hex (e.g. `"#FFFFFF"`) or named color.
    pub color: String,
    /// Background box color (e.g. `"#000000@0.5"` for semi-transparent). Empty = no box.
    pub box_color: String,
    /// Box padding in pixels.
    pub box_padding: u32,
    /// X position as fraction of width (0.0 = left, 0.5 = center, 1.0 = right).
    pub x: f64,
    /// Y position as fraction of height (0.0 = top, 1.0 = bottom).
    pub y: f64,
    /// Fade-in duration in seconds (0 = instant).
    pub fade_in_secs: f64,
    /// Fade-out duration in seconds (0 = no fade out).
    pub fade_out_secs: f64,
    /// When to show text (seconds from clip start). Negative = always visible.
    pub start_secs: f64,
    /// Duration to show text. 0 = entire clip.
    pub duration_secs: f64,
    /// Font weight: `"normal"` or `"bold"`.
    pub weight: String,
}

impl Default for TextEffect {
    fn default() -> Self {
        Self {
            text: "VORTEX".into(),
            font_path: String::new(),
            font_size: 48.0,
            color: "#FFFFFF".into(),
            box_color: "#000000@0.5".into(),
            box_padding: 8,
            x: 0.5,
            y: 0.85,
            fade_in_secs: 0.3,
            fade_out_secs: 0.3,
            start_secs: 0.0,
            duration_secs: 0.0,
            weight: "bold".into(),
        }
    }
}

// ─── Stabilize ────────────────────────────────────────────────────────────────

/// Video stabilization to remove unwanted camera shake/wobble.
///
/// Uses FFmpeg's `vidstab` two-pass stabilization:
/// 1. `vidstabdetect` — analyses motion vectors and writes to a `.trf` file.
/// 2. `vidstabtransform` — applies the stabilization.
///
/// This requires FFmpeg to be built with `--enable-libvidstab`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilizeEffect {
    /// Smoothing strength (1–30). Higher = smoother but more cropping. Default: 10.
    pub smoothing: u32,
    /// How much to allow the stabilized frame to shift (crop margin 0.0–1.0).
    pub crop_margin: f64,
    /// Zoom in slightly to hide borders created by stabilization.
    pub zoom: f64,
    /// Path to write the motion vectors `.trf` file (temp file, auto-cleaned).
    pub vectors_path: String,
}

impl Default for StabilizeEffect {
    fn default() -> Self {
        Self {
            smoothing: 10,
            crop_margin: 0.05,
            zoom: 0.02,
            vectors_path: String::new(), // auto-generated
        }
    }
}

// ─── Transition ──────────────────────────────────────────────────────────────

/// Transition between two clips in the timeline.
///
/// Transitions are attached to clips and applied at their boundary.
/// The render pipeline handles the xfade composite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    /// Transition type: `"fade"`, `"dissolve"`, `"wipe_left"`, `"wipe_right"`,
    /// `"zoom_in"`, `"zoom_out"`, `"slice"`, `"pixelize"`, `"radial"`.
    pub kind: String,
    /// Duration in seconds (typically 0.3–1.0s).
    pub duration_secs: f64,
    /// Easing: `"linear"`, `"ease_in"`, `"ease_out"`, `"ease_in_out"`.
    pub easing: String,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            kind: "dissolve".into(),
            duration_secs: 0.5,
            easing: "ease_in_out".into(),
        }
    }
}

impl Transition {
    pub fn new(kind: impl Into<String>, duration_secs: f64) -> Self {
        Self { kind: kind.into(), duration_secs, easing: "ease_in_out".into() }
    }

    /// Map transition kind to FFmpeg `xfade` transition name.
    pub fn xfade_name(&self) -> &str {
        match self.kind.as_str() {
            "fade" | "dissolve" => "dissolve",
            "wipe_left"  => "wipeleft",
            "wipe_right" => "wiperight",
            "wipe_up"    => "wipeup",
            "wipe_down"  => "wipedown",
            "zoom_in"    => "zoomin",
            "slice"      => "slideleft",
            "pixelize"   => "pixelize",
            "radial"     => "radial",
            "fade_black" => "fade",
            _ => "dissolve",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_names() {
        assert_eq!(Effect::Flash(FlashEffect::default()).name(), "flash");
        assert_eq!(Effect::Shake(ShakeEffect::default()).name(), "shake");
        assert_eq!(Effect::Velocity(VelocityEffect::default()).name(), "velocity");
        assert_eq!(Effect::Text(TextEffect::default()).name(), "text");
        assert_eq!(Effect::Stabilize(StabilizeEffect::default()).name(), "stabilize");
    }

    #[test]
    fn effect_round_trips_json() {
        let e = Effect::Zoom(ZoomEffect::default());
        let json = serde_json::to_string(&e).unwrap();
        let back: Effect = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name(), "zoom");
    }
}
