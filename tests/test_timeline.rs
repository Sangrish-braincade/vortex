//! Integration tests for timeline construction and manipulation.

use vortex_core::{Clip, Effect, FlashEffect, Project, ShakeEffect, TimeRange, Timeline, VelocityEffect, ZoomEffect};

fn sample_clip(start: f64, end: f64) -> Clip {
    Clip::new(
        "test-clips/sample-gameplay.mp4",
        TimeRange::new(start, end).unwrap(),
        TimeRange::new(start, end).unwrap(),
    )
}

#[test]
fn timeline_push_and_retrieve() {
    let mut tl = Timeline::new();
    let c1 = sample_clip(0.0, 5.0);
    let c2 = sample_clip(5.0, 10.0);
    let id1 = c1.id;

    tl.push_clip(c1);
    tl.push_clip(c2);

    assert_eq!(tl.clips.len(), 2);
    assert!((tl.duration - 10.0).abs() < 1e-9);
    assert!(tl.find_clip(&id1).is_some());
}

#[test]
fn timeline_remove_clip() {
    let mut tl = Timeline::new();
    let c = sample_clip(0.0, 5.0);
    let id = c.id;
    tl.push_clip(c);
    let removed = tl.remove_clip(&id).unwrap();
    assert_eq!(removed.id, id);
    assert_eq!(tl.clips.len(), 0);
    assert!((tl.duration).abs() < 1e-9);
}

#[test]
fn timeline_clips_at_range() {
    let mut tl = Timeline::new();
    tl.push_clip(sample_clip(0.0, 5.0));
    tl.push_clip(sample_clip(5.0, 10.0));
    tl.push_clip(sample_clip(10.0, 15.0));

    let query = TimeRange::new(4.0, 7.0).unwrap();
    let results = tl.clips_at(&query);
    assert_eq!(results.len(), 2);
}

#[test]
fn project_round_trip_json() {
    let mut project = Project::new("Test Project");
    let c = sample_clip(0.0, 3.0)
        .with_effect(Effect::Flash(FlashEffect::default()))
        .with_effect(Effect::Velocity(VelocityEffect::default()))
        .with_label("kill_01");
    project.timeline.push_clip(c);

    let json = project.to_json().unwrap();
    let restored = Project::from_json(&json).unwrap();

    assert_eq!(restored.name, "Test Project");
    assert_eq!(restored.timeline.clips.len(), 1);
    assert_eq!(restored.timeline.clips[0].effects.len(), 2);
    assert_eq!(restored.timeline.clips[0].label.as_deref(), Some("kill_01"));
}

#[test]
fn clip_effect_chain_order_preserved() {
    let clip = sample_clip(0.0, 5.0)
        .with_effect(Effect::Shake(ShakeEffect::default()))
        .with_effect(Effect::Zoom(ZoomEffect::default()))
        .with_effect(Effect::Flash(FlashEffect::default()));

    assert_eq!(clip.effects[0].name(), "shake");
    assert_eq!(clip.effects[1].name(), "zoom");
    assert_eq!(clip.effects[2].name(), "flash");
}

#[test]
fn time_range_display() {
    let r = TimeRange::new(1.5, 3.75).unwrap();
    let s = format!("{r}");
    assert!(s.contains("1.500"));
    assert!(s.contains("3.750"));
}
