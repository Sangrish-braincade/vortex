//! Camera shake effect.
//!
//! Implemented via FFmpeg `zoompan` with oscillating X/Y offsets per input frame.
//! The zoom is set slightly above 1.0 so there is enough padding to shift the
//! crop window without exposing black borders.
//!
//! ## Why zoompan instead of crop
//! FFmpeg's `crop` filter does not expose the frame-counter variable (`in`) in
//! its x/y expressions — only geometry constants like `iw`/`ih`. `zoompan`
//! exposes `in` (input frame index) which lets us build time-varying offsets.
//!
//! ## Decay
//! The amplitude decays as `exp(ln(decay) * in / fps)` — equivalent to
//! `decay ^ (in / fps)` but using `exp()` which is available in FFmpeg eval.

use crate::{EffectContext, FilterFragment, Result};
use vortex_core::ShakeEffect;

/// Generate the camera shake filter fragment.
///
/// Uses `zoompan` with a slight over-zoom and sinusoidal x/y offsets that
/// decay exponentially over time.  The `in` variable inside `zoompan`
/// expressions refers to the *input* frame index.
pub fn shake_filter(effect: &ShakeEffect, ctx: &EffectContext) -> Result<FilterFragment> {
    let ix = effect.intensity_x.min(ctx.width as f64 / 6.0);
    let iy = effect.intensity_y.min(ctx.height as f64 / 6.0);

    // Zoom slightly above 1.0 so the crop window has room to shift.
    // Required headroom: ix / (iw/2) + small margin.
    let zoom_x = 1.0 + 2.0 * ix / ctx.width as f64;
    let zoom_y = 1.0 + 2.0 * iy / ctx.height as f64;
    let zoom = zoom_x.max(zoom_y) + 0.002; // +0.2% safety margin

    let fps = ctx.fps as f64;
    let freq = effect.frequency * std::f64::consts::TAU; // radians per second
    let freq_per_frame = freq / fps;                      // radians per input frame

    // Decay: pow(decay, in/fps)  ==  exp(ln(decay) * in / fps)
    // ln(decay) is negative (decay < 1), so amplitude shrinks over time.
    let decay_rate = effect.decay.max(1e-6_f64).ln(); // ln(0.9) ≈ -0.1054

    // Zoompan center ± oscillating offset.
    // iw/2 - (iw/zoom/2)  is the top-left corner of a centred crop.
    let x_expr = format!(
        "iw/2-(iw/zoom/2)+{ix:.4}*sin(in*{freq:.6})*exp({dr:.6}*in/{fps:.4})",
        ix = ix,
        freq = freq_per_frame,
        dr = decay_rate,
        fps = fps,
    );
    let y_expr = format!(
        "ih/2-(ih/zoom/2)+{iy:.4}*sin(in*{freq:.6}+1.5)*exp({dr:.6}*in/{fps:.4})",
        iy = iy,
        freq = freq_per_frame,
        dr = decay_rate,
        fps = fps,
    );

    let filter = format!(
        "zoompan=z='{zoom:.6}':x='{x}':y='{y}':d=1:s={w}x{h}:fps={fps:.0}",
        zoom = zoom,
        x = x_expr,
        y = y_expr,
        w = ctx.width,
        h = ctx.height,
        fps = fps,
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
        assert!(f.filter.starts_with("zoompan="));
        assert!(f.filter.contains("sin(in*"));
        assert!(f.filter.contains("exp("));
    }

    #[test]
    fn shake_zoom_accounts_for_intensity() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 3.0);
        let big = shake_filter(
            &ShakeEffect { intensity_x: 40.0, intensity_y: 30.0, ..Default::default() },
            &ctx,
        ).unwrap();
        let small = shake_filter(
            &ShakeEffect { intensity_x: 2.0, intensity_y: 2.0, ..Default::default() },
            &ctx,
        ).unwrap();
        // Bigger intensity → higher zoom factor in the filter string
        // Extract zoom value to compare
        let zoom_big: f64 = big.filter
            .split("z='").nth(1).unwrap()
            .split('\'').next().unwrap()
            .parse().unwrap();
        let zoom_small: f64 = small.filter
            .split("z='").nth(1).unwrap()
            .split('\'').next().unwrap()
            .parse().unwrap();
        assert!(zoom_big > zoom_small);
    }
}
