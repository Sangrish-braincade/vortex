//! Zoom punch / pull effect.
//!
//! Uses FFmpeg `zoompan` for animated zoom. The `z` expression controls
//! the per-frame scale factor using one of four easing curves:
//!
//! - `linear`:   uniform scale progression
//! - `ease_in`:  accelerating (slow start, fast end)
//! - `ease_out`: decelerating (fast start, slow end) — default
//! - `spring`:   overshoot and oscillate: `to + (from-to)*exp(-k*t)*cos(w*t)`

use crate::{EffectContext, EffectError, FilterFragment, Result};
use vortex_core::ZoomEffect;

/// Generate the zoom filter fragment with easing curve support.
///
/// The `zoompan` z-expression is parametric over `in` (input frame index).
/// We normalise frame index to `t = in / fps` (seconds) and apply the
/// chosen easing on `[0, duration_secs]`.
pub fn zoom_filter(effect: &ZoomEffect, ctx: &EffectContext) -> Result<FilterFragment> {
    if effect.from_scale <= 0.0 || effect.to_scale <= 0.0 {
        return Err(EffectError::InvalidParameter {
            param: "scale".into(),
            reason: "scale must be > 0".into(),
        });
    }

    let total_frames = ((effect.duration_secs * ctx.fps as f64) as u32).max(1);
    let cx = (effect.focal_x * ctx.width as f64) as u32;
    let cy = (effect.focal_y * ctx.height as f64) as u32;

    let from = effect.from_scale;
    let to = effect.to_scale;
    let delta = to - from;
    let fps = ctx.fps as f64;

    // Build z-expression for the requested easing.
    // `in` is the frame index (0-based) in zoompan context.
    // We clamp to total_frames so the zoom holds at `to` afterward.
    let z_expr = if delta.abs() < 1e-6 {
        // No animation needed — static zoom
        format!("{:.4}", from)
    } else {
        match effect.easing.as_str() {
            "linear" => {
                // t_norm = in / total_frames  (0 → 1)
                // z = from + delta * t_norm
                format!(
                    "{from:.4}+{delta:.4}*(min(in,{frames})*1.0/{frames})",
                    from = from,
                    delta = delta,
                    frames = total_frames,
                )
            }
            "ease_in" => {
                // Cubic ease-in: t_norm^3
                format!(
                    "{from:.4}+{delta:.4}*pow(min(in,{frames})*1.0/{frames},3)",
                    from = from,
                    delta = delta,
                    frames = total_frames,
                )
            }
            "spring" => {
                // Spring: to + (from-to) * exp(-k*t) * cos(w*t)
                // k = 4.0 (damping), w = 12.0 rad/s (oscillation freq)
                // t = in / fps  (seconds)
                let k = 4.0_f64;
                let w = 12.0_f64;
                format!(
                    "{to:.4}+({from:.4}-{to:.4})*exp(-{k:.4}*(in/{fps:.4}))*cos({w:.4}*(in/{fps:.4}))",
                    to = to,
                    from = from,
                    k = k,
                    w = w,
                    fps = fps,
                )
            }
            // "ease_out" and everything else
            _ => {
                // Cubic ease-out: 1 - (1-t)^3
                format!(
                    "{from:.4}+{delta:.4}*(1-pow(1-min(in,{frames})*1.0/{frames},3))",
                    from = from,
                    delta = delta,
                    frames = total_frames,
                )
            }
        }
    };

    // d=1: one output frame per input frame.
    // Larger d would buffer d input frames per output "zoom segment", causing
    // OOM for long video clips (e.g. d=24 on a 600-frame clip → 14 400 frames
    // buffered). The z-expression already handles the animation duration via
    // `min(in, total_frames)` clamping.
    let filter = format!(
        "zoompan=z='{z}':x='{cx}':y='{cy}':d=1:s={w}x{h}:fps={fps}",
        z = z_expr,
        cx = cx,
        cy = cy,
        w = ctx.width,
        h = ctx.height,
        fps = ctx.fps,
    );

    Ok(FilterFragment::new(
        filter,
        format!(
            "zoom {:.2}→{:.2} ({}) over {:.2}s @ ({:.2},{:.2})",
            from, to, effect.easing, effect.duration_secs, effect.focal_x, effect.focal_y,
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> EffectContext {
        EffectContext::new(1920, 1080, 60.0, 3.0)
    }

    #[test]
    fn zoom_default_ease_out() {
        let f = zoom_filter(&ZoomEffect::default(), &ctx()).unwrap();
        assert!(f.filter.contains("zoompan"));
        assert!(f.filter.contains("pow(1-"));
    }

    #[test]
    fn zoom_linear() {
        let effect = ZoomEffect { easing: "linear".into(), ..Default::default() };
        let f = zoom_filter(&effect, &ctx()).unwrap();
        assert!(f.filter.contains("zoompan"));
        // linear expression doesn't use pow
        assert!(!f.filter.contains("pow(1-"));
    }

    #[test]
    fn zoom_ease_in() {
        let effect = ZoomEffect { easing: "ease_in".into(), ..Default::default() };
        let f = zoom_filter(&effect, &ctx()).unwrap();
        assert!(f.filter.contains("pow(min(in"));
    }

    #[test]
    fn zoom_spring() {
        let effect = ZoomEffect { easing: "spring".into(), ..Default::default() };
        let f = zoom_filter(&effect, &ctx()).unwrap();
        assert!(f.filter.contains("exp("));
        assert!(f.filter.contains("cos("));
    }

    #[test]
    fn zoom_static_no_delta() {
        let effect = ZoomEffect {
            from_scale: 1.2,
            to_scale: 1.2,
            ..Default::default()
        };
        let f = zoom_filter(&effect, &ctx()).unwrap();
        // Static zoom — no dynamic expression needed
        assert!(f.filter.contains("z='1.2"));
    }

    #[test]
    fn zoom_rejects_negative_scale() {
        let effect = ZoomEffect { from_scale: -1.0, ..Default::default() };
        assert!(zoom_filter(&effect, &ctx()).is_err());
    }
}
