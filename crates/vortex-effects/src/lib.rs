//! # vortex-effects
//!
//! Translates [`vortex_core::Effect`] variants into FFmpeg filter graph
//! fragments that can be composed into a full encode pipeline.
//!
//! Each effect module exports a `to_filter_graph(effect, ctx) -> FilterGraph`
//! function. The render pipeline collects these fragments and stitches them
//! together with FFmpeg `-filter_complex`.

pub mod chromatic;
pub mod color;
pub mod flash;
pub mod shake;
pub mod velocity;
pub mod zoom;
pub mod rotoscope;

pub use chromatic::*;
pub use color::*;
pub use flash::*;
pub use rotoscope::*;
pub use shake::*;
pub use velocity::*;
pub use zoom::*;

use thiserror::Error;
use vortex_core::Effect;

#[derive(Debug, Error)]
pub enum EffectError {
    #[error("Filter graph generation failed for {effect}: {reason}")]
    FilterGraphError { effect: String, reason: String },

    #[error("Invalid parameter {param}: {reason}")]
    InvalidParameter { param: String, reason: String },
}

pub type Result<T> = std::result::Result<T, EffectError>;

/// A fragment of an FFmpeg filtergraph string.
///
/// Multiple fragments are chained with `,` (sequential) or `;` (parallel).
#[derive(Debug, Clone)]
pub struct FilterFragment {
    /// The FFmpeg filter string (e.g. `"setpts=0.5*PTS"`).
    pub filter: String,
    /// Human-readable description for logging.
    pub description: String,
}

impl FilterFragment {
    pub fn new(filter: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            filter: filter.into(),
            description: description.into(),
        }
    }
}

/// Context passed to each effect renderer, providing clip geometry.
#[derive(Debug, Clone)]
pub struct EffectContext {
    /// Output frame width in pixels.
    pub width: u32,
    /// Output frame height in pixels.
    pub height: u32,
    /// Frames per second.
    pub fps: f32,
    /// Duration of the clip being processed in seconds.
    pub duration_secs: f64,
}

impl EffectContext {
    pub fn new(width: u32, height: u32, fps: f32, duration_secs: f64) -> Self {
        Self { width, height, fps, duration_secs }
    }

    pub fn total_frames(&self) -> u64 {
        (self.duration_secs * self.fps as f64) as u64
    }
}

/// Translate an [`Effect`] into an FFmpeg filter fragment.
pub fn effect_to_filter(effect: &Effect, ctx: &EffectContext) -> Result<FilterFragment> {
    match effect {
        Effect::Velocity(e) => velocity_filter(e, ctx),
        Effect::Zoom(e) => zoom_filter(e, ctx),
        Effect::Shake(e) => shake_filter(e, ctx),
        Effect::Color(e) => color_filter(e, ctx),
        Effect::Flash(e) => flash_filter(e, ctx),
        Effect::Chromatic(e) => chromatic_filter(e, ctx),
        Effect::Letterbox(e) => {
            // Letterbox via crop + pad
            let bar_height = (ctx.height as f64
                * (1.0 - (ctx.width as f64 / ctx.height as f64) / e.aspect_ratio)
                / 2.0) as u32;
            let filter = format!(
                "crop={}:{}:0:{},pad={}:{}:0:{}:{}",
                ctx.width, ctx.height - 2 * bar_height, bar_height,
                ctx.width, ctx.height, bar_height, e.bar_color.replace('#', "0x")
            );
            Ok(FilterFragment::new(filter, "letterbox"))
        }
        Effect::Vignette(e) => {
            let filter = format!(
                "vignette=PI/{}:mode=backward",
                2.0 * (1.0 - e.strength)
            );
            Ok(FilterFragment::new(filter, "vignette"))
        }
        Effect::Glitch(e) => {
            // Glitch via geq (pixel equation) — simplified
            let filter = format!(
                "geq=r='r(X+(random(1)-0.5)*{}*between(Y,mod(N,H/{})*({}/1),mod(N,H/{})*({}/1)+({}/1)),Y)':g='g(X,Y)':b='b(X,Y)'",
                e.displacement,
                e.scan_lines, ctx.height, e.scan_lines, ctx.height, ctx.height
            );
            Ok(FilterFragment::new(filter, "glitch"))
        }
        Effect::Rotoscope(e) => rotoscope_filter(e, ctx),
    }
}

/// Generate the rotoscope filter fragment.
///
/// For `"chromakey"` and `"lumakey"` modes this is a pure FFmpeg filter.
/// For `"sam2"` and `"rembg"` modes the filter is a passthrough — the actual
/// ML segmentation is handled out-of-band by the render pipeline which calls
/// the SAM 2 / rembg subprocess *before* the FFmpeg encode step.
pub fn rotoscope_filter(
    effect: &vortex_core::RotoscopeEffect,
    _ctx: &EffectContext,
) -> Result<FilterFragment> {
    match effect.mode.as_str() {
        "chromakey" => {
            // FFmpeg chromakey: remove key_color within similarity tolerance
            let color = effect.key_color.trim_start_matches('#');
            let filter = format!(
                "chromakey=color=0x{color}:similarity={sim:.3}:blend={blend:.3}",
                color = color,
                sim = effect.similarity.clamp(0.01, 1.0),
                blend = effect.blend.clamp(0.0, 1.0),
            );
            Ok(FilterFragment::new(filter, format!("chromakey #{}", color)))
        }
        "lumakey" => {
            // Luma key: remove dark (or bright if inverted) regions
            let threshold = if effect.invert { 1.0 - effect.similarity } else { effect.similarity };
            let filter = format!(
                "lumakey=threshold={thresh:.3}:tolerance={tol:.3}:softness={soft:.3}",
                thresh = threshold,
                tol = effect.similarity.clamp(0.01, 0.5),
                soft = effect.blend.clamp(0.0, 0.5),
            );
            Ok(FilterFragment::new(filter, "lumakey"))
        }
        // sam2 / rembg: handled by render pipeline — emit passthrough here
        _ => Ok(FilterFragment::new(
            String::from("null"),
            format!("rotoscope/{} (handled by render pipeline)", effect.mode),
        )),
    }
}

/// Compose a list of effects into a single FFmpeg `-vf` string.
pub fn compose_effects(effects: &[Effect], ctx: &EffectContext) -> Result<String> {
    let fragments: Result<Vec<FilterFragment>> = effects
        .iter()
        .map(|e| effect_to_filter(e, ctx))
        .collect();

    let fragments = fragments?;
    if fragments.is_empty() {
        return Ok(String::new());
    }

    let chain = fragments
        .iter()
        .map(|f| f.filter.as_str())
        .collect::<Vec<_>>()
        .join(",");

    Ok(chain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vortex_core::{FlashEffect, ShakeEffect, VelocityEffect};

    fn ctx() -> EffectContext {
        EffectContext::new(1920, 1080, 60.0, 5.0)
    }

    #[test]
    fn velocity_generates_filter() {
        let f = effect_to_filter(
            &Effect::Velocity(VelocityEffect::default()),
            &ctx(),
        )
        .unwrap();
        assert!(f.filter.contains("setpts") || f.filter.contains("PTS"));
    }

    #[test]
    fn compose_multiple_effects() {
        let effects = vec![
            Effect::Flash(FlashEffect::default()),
            Effect::Shake(ShakeEffect::default()),
        ];
        let chain = compose_effects(&effects, &ctx()).unwrap();
        assert!(chain.contains(','));
    }
}
