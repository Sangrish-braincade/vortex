//! Generates `examples/sample-project.json` by serialising a test project.
//!
//! Run: `cargo run --example create_sample_project > examples/sample-project.json`

use vortex_core::{
    Clip, ColorEffect, Effect, FlashEffect, Project, ShakeEffect, TimeRange, VelocityEffect,
    ZoomEffect,
};

fn main() {
    let mut project = Project::new("valorant-montage");
    // Clip 1: Ace round — slow-mo + zoom punch
    let src1 = TimeRange::new(45.0, 57.0).unwrap();
    let tl1 = TimeRange::new(0.0, 12.0).unwrap();
    let clip1 = Clip::new("test-clips/sample-gameplay.mp4", src1, tl1)
        .with_label("ace-round")
        .with_effect(Effect::Velocity(VelocityEffect {
            min_speed: 0.15,
            max_speed: 1.0,
            ramp_in_secs: 0.5,
            ramp_out_secs: 0.5,
            easing: "ease_in_out".into(),
        }))
        .with_effect(Effect::Zoom(ZoomEffect {
            from_scale: 1.0,
            to_scale: 1.2,
            duration_secs: 0.4,
            focal_x: 0.5,
            focal_y: 0.45,
            easing: "spring".into(),
        }))
        .with_effect(Effect::Flash(FlashEffect {
            color: "#FFFFFF".into(),
            peak_opacity: 0.9,
            duration_secs: 0.1,
            attack_ratio: 0.15,
        }));

    // Clip 2: Clutch 1v3 — shake + color grade
    let src2 = TimeRange::new(120.0, 130.0).unwrap();
    let tl2 = TimeRange::new(12.0, 22.0).unwrap();
    let clip2 = Clip::new("test-clips/sample-gameplay.mp4", src2, tl2)
        .with_label("clutch-1v3")
        .with_effect(Effect::Color(ColorEffect {
            lut_path: None,
            lut_strength: 1.0,
            saturation: 1.3,
            contrast: 1.15,
            brightness: 0.05,
            hue_shift: -5.0,
        }))
        .with_effect(Effect::Shake(ShakeEffect {
            intensity_x: 8.0,
            intensity_y: 6.0,
            frequency: 20.0,
            decay: 0.9,
            seed: 7,
        }));

    // Clip 3: Headshot highlight — velocity ramp only
    let src3 = TimeRange::new(200.0, 208.0).unwrap();
    let tl3 = TimeRange::new(22.0, 30.0).unwrap();
    let clip3 = Clip::new("test-clips/sample-gameplay.mp4", src3, tl3)
        .with_label("headshot")
        .with_speed(1.0)
        .with_effect(Effect::Velocity(VelocityEffect::default()));

    project.timeline.push_clip(clip1);
    project.timeline.push_clip(clip2);
    project.timeline.push_clip(clip3);

    let json = serde_json::to_string_pretty(&project).expect("serialise failed");
    println!("{}", json);
}
