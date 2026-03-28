//! Chromatic aberration effect — RGB channel separation.
//!
//! Offsets the red and blue channels independently, leaving green centered.
//! Implemented via FFmpeg `geq` accessing `r(X+dx, Y+dy)` for each channel.

use crate::{EffectContext, FilterFragment, Result};
use vortex_core::ChromaticEffect;

/// Generate the chromatic aberration filter fragment.
///
/// # TODO (Phase 1)
/// - Support radial aberration (stronger near edges) by computing offset
///   as a function of distance from center.
/// - Add barrel distortion via `lens_correction` for a more realistic look.
pub fn chromatic_filter(effect: &ChromaticEffect, _ctx: &EffectContext) -> Result<FilterFragment> {
    let rx = effect.offset_r_x;
    let ry = effect.offset_r_y;
    let bx = effect.offset_b_x;
    let by = effect.offset_b_y;
    let s = effect.strength;

    // Blend shifted channels with original at `strength`
    let filter = format!(
        "geq=\
         r='r(X,Y)*(1-{s})+r(X+{rx},Y+{ry})*{s}':\
         g='g(X,Y)':\
         b='b(X,Y)*(1-{s})+b(X+{bx},Y+{by})*{s}'",
        rx = rx, ry = ry, bx = bx, by = by, s = s,
    );

    Ok(FilterFragment::new(
        filter,
        format!(
            "chromatic aberration R({:+.1},{:+.1}) B({:+.1},{:+.1}) strength={:.2}",
            rx, ry, bx, by, s
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chromatic_filter_generates() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 5.0);
        let f = chromatic_filter(&ChromaticEffect::default(), &ctx).unwrap();
        assert!(f.filter.contains("geq="));
        assert!(f.filter.contains("r(X"));
        assert!(f.filter.contains("b(X"));
    }
}
