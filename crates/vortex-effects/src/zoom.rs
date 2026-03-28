//! Zoom punch / pull effect.
//!
//! Uses FFmpeg `zoompan` for animated zoom, or `scale` + `crop` for
//! a static zoom-in.

use crate::{EffectContext, EffectError, FilterFragment, Result};
use vortex_core::ZoomEffect;

/// Generate the zoom filter fragment.
///
/// # TODO (Phase 1)
/// - Implement easing curves (ease_in, ease_out, spring) by computing
///   the per-frame scale value and embedding it as an expression.
/// - Use `zoompan=z='expr':x='cx':y='cy':d=N:s=WxH` with proper frame count.
pub fn zoom_filter(effect: &ZoomEffect, ctx: &EffectContext) -> Result<FilterFragment> {
    if effect.from_scale <= 0.0 || effect.to_scale <= 0.0 {
        return Err(EffectError::InvalidParameter {
            param: "scale".into(),
            reason: "scale must be > 0".into(),
        });
    }

    let total_frames = (effect.duration_secs * ctx.fps as f64) as u32;
    let zoom_frames = total_frames.max(1);

    // Focal point in pixel coords
    let cx = (effect.focal_x * ctx.width as f64) as u32;
    let cy = (effect.focal_y * ctx.height as f64) as u32;

    // Animated zoom: zoom from from_scale to to_scale over duration_secs
    // z expression: linear interpolation from_scale → to_scale
    let delta = effect.to_scale - effect.from_scale;
    let z_expr = if delta.abs() < 1e-6 {
        format!("{:.4}", effect.from_scale)
    } else {
        format!(
            "if(lte(in,{frames}),{from}+({delta}*(in/{frames})),{to})",
            frames = zoom_frames,
            from = effect.from_scale,
            delta = delta,
            to = effect.to_scale,
        )
    };

    let filter = format!(
        "zoompan=z='{z}':x='{cx}':y='{cy}':d={frames}:s={w}x{h}:fps={fps}",
        z = z_expr,
        cx = cx,
        cy = cy,
        frames = zoom_frames,
        w = ctx.width,
        h = ctx.height,
        fps = ctx.fps,
    );

    Ok(FilterFragment::new(
        filter,
        format!(
            "zoom {:.2}→{:.2} over {:.2}s (focal {:.2},{:.2})",
            effect.from_scale, effect.to_scale, effect.duration_secs,
            effect.focal_x, effect.focal_y,
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_filter_generates() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 3.0);
        let effect = ZoomEffect::default();
        let f = zoom_filter(&effect, &ctx).unwrap();
        assert!(f.filter.contains("zoompan"));
    }

    #[test]
    fn zoom_filter_rejects_negative_scale() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 3.0);
        let effect = ZoomEffect {
            from_scale: -1.0,
            ..Default::default()
        };
        assert!(zoom_filter(&effect, &ctx).is_err());
    }
}
