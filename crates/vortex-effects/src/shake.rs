//! Camera shake effect.
//!
//! Implemented via FFmpeg `crop` with oscillating X/Y offsets per frame.
//! The offsets are computed as a deterministic pseudo-random sequence
//! seeded from `ShakeEffect::seed` for reproducibility.

use crate::{EffectContext, FilterFragment, Result};
use vortex_core::ShakeEffect;

/// Generate the camera shake filter fragment.
///
/// Uses `crop` with `x` and `y` expressions driven by `random()` seeded
/// per-frame. The decay envelope reduces amplitude over time.
///
/// # TODO (Phase 1)
/// - Replace `random()` with a proper `sin(N * freq)` oscillator with
///   noise modulation for more natural shake feel.
/// - Support rotational shake via `rotate` filter.
pub fn shake_filter(effect: &ShakeEffect, ctx: &EffectContext) -> Result<FilterFragment> {
    // Ensure crop doesn't exceed frame bounds
    let ix = effect.intensity_x.min(ctx.width as f64 / 4.0);
    let iy = effect.intensity_y.min(ctx.height as f64 / 4.0);

    // Output size is reduced by max displacement to keep frame in bounds
    let crop_w = ctx.width - (2.0 * ix) as u32;
    let crop_h = ctx.height - (2.0 * iy) as u32;

    // FFmpeg expression: offset oscillates as decayed sinusoid
    // `N` = frame index, `FRAME_RATE` available in crop context
    let x_expr = format!(
        "{cx}+{ix}*sin(N*{freq}/{fps})*pow({decay},N/{fps})",
        cx = ix as u32,
        ix = ix,
        freq = effect.frequency * std::f64::consts::TAU,
        fps = ctx.fps,
        decay = effect.decay,
    );
    let y_expr = format!(
        "{cy}+{iy}*sin(N*{freq}/{fps}+1.5)*pow({decay},N/{fps})",
        cy = iy as u32,
        iy = iy,
        freq = effect.frequency * std::f64::consts::TAU,
        fps = ctx.fps,
        decay = effect.decay,
    );

    let filter = format!(
        "crop={w}:{h}:'{x}':'{y}'",
        w = crop_w,
        h = crop_h,
        x = x_expr,
        y = y_expr,
    );

    Ok(FilterFragment::new(
        filter,
        format!(
            "shake intensity=({:.1},{:.1}) freq={:.1}Hz decay={:.2}",
            effect.intensity_x, effect.intensity_y, effect.frequency, effect.decay
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shake_filter_generates() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 3.0);
        let f = shake_filter(&ShakeEffect::default(), &ctx).unwrap();
        assert!(f.filter.starts_with("crop="));
    }
}
