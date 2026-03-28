//! Color grading effect.
//!
//! Applies saturation, contrast, brightness via FFmpeg `eq` filter,
//! and optionally overlays a `.cube` LUT via `lut3d`.

use crate::{EffectContext, FilterFragment, Result};
use vortex_core::ColorEffect;

/// Generate the color grade filter fragment.
pub fn color_filter(effect: &ColorEffect, _ctx: &EffectContext) -> Result<FilterFragment> {
    let mut parts: Vec<String> = Vec::new();

    // eq filter: brightness, contrast, saturation
    let eq = format!(
        "eq=brightness={b:.4}:contrast={c:.4}:saturation={s:.4}",
        b = effect.brightness,
        c = effect.contrast,
        s = effect.saturation,
    );
    parts.push(eq);

    // Hue rotation via hue filter
    if effect.hue_shift.abs() > 0.01 {
        parts.push(format!("hue=h={:.2}", effect.hue_shift));
    }

    // LUT overlay via lut3d
    if let Some(ref lut_path) = effect.lut_path {
        // lut3d doesn't support strength natively; blend with original via
        // `blend` filter. TODO (Phase 1): implement proper LUT blend.
        parts.push(format!(
            "lut3d=file={}:interp=trilinear",
            lut_path
        ));
    }

    Ok(FilterFragment::new(
        parts.join(","),
        format!(
            "color grade sat={:.2} con={:.2} bri={:.2} hue={:.1}°",
            effect.saturation, effect.contrast, effect.brightness, effect.hue_shift
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_filter_generates_eq() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 5.0);
        let f = color_filter(&ColorEffect::default(), &ctx).unwrap();
        assert!(f.filter.contains("eq="));
    }

    #[test]
    fn color_filter_with_hue() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 5.0);
        let effect = ColorEffect {
            hue_shift: 15.0,
            ..Default::default()
        };
        let f = color_filter(&effect, &ctx).unwrap();
        assert!(f.filter.contains("hue="));
    }
}
