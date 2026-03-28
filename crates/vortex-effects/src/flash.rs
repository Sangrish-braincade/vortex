//! White/color flash effect.
//!
//! Implemented as a color overlay that fades in then out over the
//! clip duration using FFmpeg `geq` (general equation) or `overlay`.

use crate::{EffectContext, EffectError, FilterFragment, Result};
use vortex_core::FlashEffect;

/// Parse a hex color string (#RRGGBB) into (r, g, b) 0–255.
fn parse_hex_color(hex: &str) -> std::result::Result<(u8, u8, u8), String> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return Err(format!("invalid hex color: {hex}"));
    }
    let r = u8::from_str_radix(&h[0..2], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&h[2..4], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&h[4..6], 16).map_err(|e| e.to_string())?;
    Ok((r, g, b))
}

/// Generate the flash filter fragment.
///
/// Uses `geq` to blend a solid color over the video with a time-varying
/// alpha envelope: linear ramp up over `attack_ratio * duration`,
/// then linear ramp down over the rest.
///
/// # TODO (Phase 1)
/// For more complex flashes (e.g. burst of frames), use `overlay` with
/// a `color` source and `format=rgba` pipeline.
pub fn flash_filter(effect: &FlashEffect, ctx: &EffectContext) -> Result<FilterFragment> {
    let (r, g, b) = parse_hex_color(&effect.color).map_err(|e| EffectError::FilterGraphError {
        effect: "flash".into(),
        reason: e,
    })?;

    let total_frames = ctx.total_frames().max(1);
    let attack_frames = (effect.attack_ratio * total_frames as f64) as u64;

    // Alpha envelope as a piecewise linear expression over frame index N:
    //   if N < attack_frames: N/attack_frames * peak_opacity
    //   else: (1 - (N - attack_frames)/(total_frames - attack_frames)) * peak_opacity
    let peak = effect.peak_opacity;
    let alpha_expr = format!(
        "if(lt(N,{af}),{peak}*N/{af},{peak}*(1-(N-{af})/max(1,{tf}-{af})))",
        af = attack_frames,
        peak = peak,
        tf = total_frames,
    );

    let filter = format!(
        "geq=r='r(X,Y)+({r}-r(X,Y))*({alpha})':g='g(X,Y)+({g}-g(X,Y))*({alpha})':b='b(X,Y)+({b}-b(X,Y))*({alpha})'",
        r = r,
        g = g,
        b = b,
        alpha = alpha_expr,
    );

    Ok(FilterFragment::new(
        filter,
        format!(
            "flash {} peak={:.0}% dur={:.3}s",
            effect.color,
            effect.peak_opacity * 100.0,
            effect.duration_secs,
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_filter_generates() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 0.12);
        let f = flash_filter(&FlashEffect::default(), &ctx).unwrap();
        assert!(f.filter.contains("geq="));
    }

    #[test]
    fn parse_color_white() {
        let (r, g, b) = parse_hex_color("#FFFFFF").unwrap();
        assert_eq!((r, g, b), (255, 255, 255));
    }

    #[test]
    fn parse_color_invalid() {
        assert!(parse_hex_color("#ZZZZZZ").is_err());
        assert!(parse_hex_color("red").is_err());
    }
}
