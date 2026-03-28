//! Velocity ramp effect — temporal slow-motion / speed-up.
//!
//! ## FFmpeg implementation
//!
//! We use `setpts` (set presentation timestamps) to control playback speed.
//!
//! - `setpts=0.5*PTS` → 2× speed
//! - `setpts=2.0*PTS` → 0.5× speed (half speed / slow-mo)
//!
//! For animated ramps the expression becomes a time-varying function.
//! A true velocity ramp requires frame-by-frame PTS manipulation which
//! can be achieved via `setpts` with the `N` and `FRAME_RATE` variables.

use crate::{EffectContext, EffectError, FilterFragment, Result};
use vortex_core::VelocityEffect;

/// Generate an FFmpeg filter fragment for a velocity ramp effect.
///
/// # TODO (Phase 1)
/// Generate a proper piecewise linear PTS expression that:
/// 1. Plays at `max_speed` initially.
/// 2. Ramps to `min_speed` over `ramp_in_secs`.
/// 3. Holds `min_speed` at the highlight frame.
/// 4. Ramps back to `max_speed` over `ramp_out_secs`.
///
/// The current implementation applies a constant `min_speed` across
/// the whole clip as a placeholder.
pub fn velocity_filter(effect: &VelocityEffect, _ctx: &EffectContext) -> Result<FilterFragment> {
    if effect.min_speed <= 0.0 {
        return Err(EffectError::InvalidParameter {
            param: "min_speed".into(),
            reason: "must be > 0".into(),
        });
    }

    // Full ramp expression using FFmpeg's `setpts` and conditional expressions.
    // PTS = presentation timestamp in seconds.
    // N = frame number, FRAME_RATE = stream frame rate.
    //
    // Simplified: constant slow-mo for now. Phase 1 replaces this with
    // a piecewise expression using `if(lt(T,...), expr1, expr2)`.
    let speed_factor = effect.min_speed;
    let pts_factor = 1.0 / speed_factor;

    let filter = format!(
        "setpts={:.6}*PTS",
        pts_factor
    );

    Ok(FilterFragment::new(
        filter,
        format!(
            "velocity ramp {:.0}%→{:.0}% (in {:.2}s, out {:.2}s)",
            effect.min_speed * 100.0,
            effect.max_speed * 100.0,
            effect.ramp_in_secs,
            effect.ramp_out_secs,
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_filter_output() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 5.0);
        let effect = VelocityEffect {
            min_speed: 0.15,
            ..Default::default()
        };
        let f = velocity_filter(&effect, &ctx).unwrap();
        assert!(f.filter.starts_with("setpts="));
        // 1/0.15 ≈ 6.667
        assert!(f.filter.contains("6.6"));
    }

    #[test]
    fn velocity_filter_rejects_zero_speed() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 5.0);
        let effect = VelocityEffect {
            min_speed: 0.0,
            ..Default::default()
        };
        assert!(velocity_filter(&effect, &ctx).is_err());
    }
}
