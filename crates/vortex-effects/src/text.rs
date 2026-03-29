//! Text / title overlay effect using FFmpeg `drawtext`.
//!
//! Supports animated lower-thirds, kill-feed labels, and static titles.
//! Text is positioned in normalised [0,1] coordinates, converted to pixel
//! expressions via `(W*x)` and `(H*y)` at render time.
//!
//! ## Centering
//! Use `x=0.5` with the generated `x=(W-text_w)/2` expression to centre horizontally.
//!
//! ## Animation
//! Fade in/out is achieved via the `alpha` expression: ramps from 0→1 over
//! `fade_in_secs`, holds, then ramps 1→0 over `fade_out_secs`.

use crate::{EffectContext, FilterFragment, Result};
use vortex_core::TextEffect;

/// Generate the `drawtext` filter fragment for a text overlay.
pub fn text_filter(effect: &TextEffect, ctx: &EffectContext) -> Result<FilterFragment> {
    let fps = ctx.fps as f64;
    let duration = ctx.duration_secs;

    // Pixel position
    let x_expr = if (effect.x - 0.5).abs() < 0.01 {
        "(W-text_w)/2".to_string()
    } else {
        format!("W*{:.4}", effect.x)
    };
    let y_expr = if (effect.y - 0.5).abs() < 0.01 {
        "(H-text_h)/2".to_string()
    } else {
        format!("H*{:.4}", effect.y)
    };

    // Alpha expression for fade in/out (no commas — uses boolean masking)
    let fi = (effect.fade_in_secs * fps).max(1.0) as u64;
    let fo_frames = (effect.fade_out_secs * fps).max(1.0) as u64;
    let total_frames = (duration * fps) as u64;
    let fo_start = if total_frames > fo_frames { total_frames - fo_frames } else { 0 };

    // alpha = ramp-in + hold + ramp-out using boolean mask arithmetic (no commas)
    let alpha_expr = format!(
        "(N<{fi})*(N/{fi})+(N>={fi})*(N<{fo_start})*1+(N>={fo_start})*(N<{tf})*(({tf}-N)/{fo_frames})",
        fi = fi,
        fo_start = fo_start,
        tf = total_frames,
        fo_frames = fo_frames,
    );

    // Build drawtext option string — use ':' separators, escape special chars
    let text_escaped = effect.text.replace('\'', "\\'").replace(':', "\\:");
    let color_str = effect.color.trim_start_matches('#');

    let mut opts = format!(
        "text='{text}':fontsize={size}:fontcolor=0x{color}:x={x}:y={y}:alpha='{alpha}'",
        text = text_escaped,
        size = effect.font_size as u32,
        color = color_str,
        x = x_expr,
        y = y_expr,
        alpha = alpha_expr,
    );

    if !effect.font_path.is_empty() {
        opts.push_str(&format!(":fontfile='{}'", effect.font_path.replace('\\', "/")));
    }

    if !effect.box_color.is_empty() {
        let box_color = effect.box_color.replace('#', "0x");
        opts.push_str(&format!(
            ":box=1:boxcolor={color}:boxborderw={pad}",
            color = box_color,
            pad = effect.box_padding,
        ));
    }

    // Show/hide timing
    if effect.start_secs >= 0.0 {
        opts.push_str(&format!(":enable='between(t\\,{:.3}\\,{:.3})'",
            effect.start_secs,
            if effect.duration_secs > 0.0 { effect.start_secs + effect.duration_secs } else { duration }
        ));
    }

    let filter = format!("drawtext={}", opts);

    Ok(FilterFragment::new(
        filter,
        format!("text '{}' at ({:.2},{:.2})", effect.text, effect.x, effect.y),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_filter_generates_drawtext() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 5.0);
        let effect = TextEffect {
            text: "ACE ROUND".into(),
            ..Default::default()
        };
        let f = text_filter(&effect, &ctx).unwrap();
        assert!(f.filter.starts_with("drawtext="), "filter: {}", f.filter);
        assert!(f.filter.contains("ACE ROUND"));
        assert!(f.filter.contains("fontsize="));
    }

    #[test]
    fn text_filter_centers_at_half() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 5.0);
        let effect = TextEffect { x: 0.5, y: 0.5, ..Default::default() };
        let f = text_filter(&effect, &ctx).unwrap();
        assert!(f.filter.contains("(W-text_w)/2"), "filter: {}", f.filter);
        assert!(f.filter.contains("(H-text_h)/2"), "filter: {}", f.filter);
    }

    #[test]
    fn text_filter_has_alpha_expression() {
        let ctx = EffectContext::new(1920, 1080, 60.0, 5.0);
        let effect = TextEffect { fade_in_secs: 0.5, fade_out_secs: 0.5, ..Default::default() };
        let f = text_filter(&effect, &ctx).unwrap();
        assert!(f.filter.contains("alpha="), "filter: {}", f.filter);
    }
}
