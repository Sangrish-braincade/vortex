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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_names() {
        assert_eq!(Effect::Flash(FlashEffect::default()).name(), "flash");
        assert_eq!(Effect::Shake(ShakeEffect::default()).name(), "shake");
        assert_eq!(Effect::Velocity(VelocityEffect::default()).name(), "velocity");
    }

    #[test]
    fn effect_round_trips_json() {
        let e = Effect::Zoom(ZoomEffect::default());
        let json = serde_json::to_string(&e).unwrap();
        let back: Effect = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name(), "zoom");
    }
}
