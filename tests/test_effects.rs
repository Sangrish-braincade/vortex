//! Integration tests for effect filter graph generation.

use vortex_core::{
    ChromaticEffect, ColorEffect, Effect, FlashEffect, GlitchEffect, LetterboxEffect,
    ShakeEffect, VelocityEffect, VignetteEffect, ZoomEffect,
};
use vortex_effects::{compose_effects, effect_to_filter, EffectContext};

fn ctx() -> EffectContext {
    EffectContext::new(1920, 1080, 60.0, 5.0)
}

#[test]
fn all_effects_generate_filters() {
    let effects = vec![
        Effect::Velocity(VelocityEffect::default()),
        Effect::Zoom(ZoomEffect::default()),
        Effect::Shake(ShakeEffect::default()),
        Effect::Color(ColorEffect::default()),
        Effect::Flash(FlashEffect::default()),
        Effect::Chromatic(ChromaticEffect::default()),
        Effect::Letterbox(LetterboxEffect::default()),
        Effect::Vignette(VignetteEffect::default()),
        Effect::Glitch(GlitchEffect::default()),
    ];

    for effect in &effects {
        let result = effect_to_filter(effect, &ctx());
        assert!(
            result.is_ok(),
            "Effect '{}' failed to generate filter: {:?}",
            effect.name(),
            result.err()
        );
        let fragment = result.unwrap();
        assert!(!fragment.filter.is_empty(), "Effect '{}' generated empty filter", effect.name());
    }
}

#[test]
fn compose_empty_effects_returns_empty() {
    let chain = compose_effects(&[], &ctx()).unwrap();
    assert!(chain.is_empty());
}

#[test]
fn compose_single_effect() {
    let effects = vec![Effect::Flash(FlashEffect::default())];
    let chain = compose_effects(&effects, &ctx()).unwrap();
    assert!(!chain.is_empty());
    assert!(!chain.contains(',') || chain.contains("geq")); // no comma separator for single effect
}

#[test]
fn compose_multiple_effects_joined_with_comma() {
    let effects = vec![
        Effect::Velocity(VelocityEffect::default()),
        Effect::Zoom(ZoomEffect::default()),
        Effect::Shake(ShakeEffect::default()),
    ];
    let chain = compose_effects(&effects, &ctx()).unwrap();
    // Multiple effects should be comma-separated
    assert!(chain.contains(','));
}

#[test]
fn velocity_slowmo_factor() {
    // 0.15 speed → setpts factor should be ~6.67
    let effect = Effect::Velocity(VelocityEffect { min_speed: 0.15, ..Default::default() });
    let fragment = effect_to_filter(&effect, &ctx()).unwrap();
    assert!(fragment.filter.contains("setpts="));
    // 1.0 / 0.15 = 6.666...
    assert!(fragment.filter.contains("6.6") || fragment.filter.contains("6.7"));
}

#[test]
fn chromatic_separates_channels() {
    let effect = Effect::Chromatic(ChromaticEffect {
        offset_r_x: 5.0,
        offset_b_x: -5.0,
        ..Default::default()
    });
    let fragment = effect_to_filter(&effect, &ctx()).unwrap();
    // Should reference r() and b() with offsets
    assert!(fragment.filter.contains("r(X+5"));
    assert!(fragment.filter.contains("b(X-5") || fragment.filter.contains("b(X+-5"));
}

#[test]
fn effect_serialization_roundtrip() {
    let effects = vec![
        Effect::Velocity(VelocityEffect::default()),
        Effect::Zoom(ZoomEffect { to_scale: 1.3, ..Default::default() }),
        Effect::Flash(FlashEffect { color: "#FF5500".into(), ..Default::default() }),
    ];

    let json = serde_json::to_string_pretty(&effects).unwrap();
    let restored: Vec<Effect> = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.len(), 3);
    assert_eq!(restored[0].name(), "velocity");
    assert_eq!(restored[1].name(), "zoom");
    assert_eq!(restored[2].name(), "flash");
}
