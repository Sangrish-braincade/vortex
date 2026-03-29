//! Velocity ramp effect — temporal slow-motion / speed-up.
//!
//! ## FFmpeg implementation
//!
//! We use `setpts` (set presentation timestamps) to control playback speed.
//! The expression maps each input frame's time `T` to an output time `O(T)`
//! by integrating the inverse of the speed function over three phases:
//!
//! - Phase 1 `[0, ramp_in_secs)`: plays at `max_speed`
//! - Phase 2 `[ramp_in_secs, duration - ramp_out_secs)`: holds at `min_speed`
//! - Phase 3 `[duration - ramp_out_secs, end)`: plays at `max_speed` again
//!
//! Output time without discontinuities:
//! ```text
//! O(T) = T / max_s                                              if T ≤ t1
//!       = t1/max_s + (T−t1)/min_s                              if t1 < T ≤ t2
//!       = t1/max_s + (t2−t1)/min_s + (T−t2)/max_s             if T > t2
//! ```
//! The `setpts` filter expression is `O(T) / TB`.

use crate::{EffectContext, EffectError, FilterFragment, Result};
use vortex_core::VelocityEffect;

/// Generate an FFmpeg filter fragment for a velocity ramp effect.
///
/// Produces a piecewise `setpts` expression with three speed zones:
/// fast → slow-mo → fast, with seamless accumulated timestamp offsets
/// so there are no jumps in output time.
pub fn velocity_filter(effect: &VelocityEffect, ctx: &EffectContext) -> Result<FilterFragment> {
    if effect.min_speed <= 0.0 {
        return Err(EffectError::InvalidParameter {
            param: "min_speed".into(),
            reason: "must be > 0".into(),
        });
    }
    if effect.max_speed <= 0.0 {
        return Err(EffectError::InvalidParameter {
            param: "max_speed".into(),
            reason: "must be > 0".into(),
        });
    }

    let max_s = effect.max_speed;
    let min_s = effect.min_speed;
    let duration = ctx.duration_secs;

    // Phase boundaries in input time
    let t1 = effect.ramp_in_secs.min(duration * 0.5);
    let t2 = (duration - effect.ramp_out_secs).max(t1);

    // Pre-computed accumulated output time offsets
    let off1 = t1 / max_s;                // output time at end of phase 1
    let off2 = off1 + (t2 - t1) / min_s; // output time at end of phase 2

    // If all three phases collapse (very short clip), fall back to constant
    if (t2 - t1).abs() < 1e-6 {
        let pts_factor = 1.0 / min_s;
        let filter = format!("setpts={:.6}*PTS", pts_factor);
        return Ok(FilterFragment::new(
            filter,
            format!("velocity constant {:.0}%", min_s * 100.0),
        ));
    }

    // Piecewise expression: O(T)/TB evaluated per frame.
    // FFmpeg `setpts` receives `T` (seconds) and `TB` (time base).
    // Returning O(T)/TB gives the new PTS in timebase units.
    let filter = format!(
        "setpts=if(lte(T,{t1:.6}),\
T/{max_s:.6},\
if(lte(T,{t2:.6}),\
{off1:.6}+(T-{t1:.6})/{min_s:.6},\
{off2:.6}+(T-{t2:.6})/{max_s:.6}\
))/TB",
        t1 = t1,
        t2 = t2,
        max_s = max_s,
        min_s = min_s,
        off1 = off1,
        off2 = off2,
    );

    Ok(FilterFragment::new(
        filter,
        format!(
            "velocity ramp {:.0}%→{:.0}%→{:.0}% (in {:.2}s, out {:.2}s)",
            max_s * 100.0,
            min_s * 100.0,
            max_s * 100.0,
            effect.ramp_in_secs,
            effect.ramp_out_secs,
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_filter_piecewise() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 5.0);
        let effect = VelocityEffect {
            min_speed: 0.15,
            max_speed: 1.0,
            ramp_in_secs: 0.5,
            ramp_out_secs: 0.5,
            easing: "linear".into(),
        };
        let f = velocity_filter(&effect, &ctx).unwrap();
        assert!(f.filter.starts_with("setpts=if("));
        assert!(f.filter.contains("lte(T,"));
        assert!(f.filter.contains("/TB"));
    }

    #[test]
    fn velocity_filter_constant_fallback() {
        // Very short clip — t1 and t2 collapse, should get constant setpts
        let ctx = EffectContext::new(1920, 1080, 60.0, 0.1);
        let effect = VelocityEffect {
            min_speed: 0.5,
            max_speed: 1.0,
            ramp_in_secs: 0.3,
            ramp_out_secs: 0.3,
            ..Default::default()
        };
        let f = velocity_filter(&effect, &ctx).unwrap();
        assert!(f.filter.starts_with("setpts="));
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

    #[test]
    fn velocity_filter_15pct_has_min_speed() {
        // min_speed=0.15 should appear in the piecewise expression as a divisor
        let ctx = EffectContext::new(1920, 1080, 60.0, 5.0);
        let effect = VelocityEffect {
            min_speed: 0.15,
            ..Default::default()
        };
        let f = velocity_filter(&effect, &ctx).unwrap();
        assert!(f.filter.contains("0.150000"), "filter: {}", f.filter);
    }
}
