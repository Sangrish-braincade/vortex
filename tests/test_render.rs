//! Integration tests for the render pipeline.
//!
//! Tests in this file build actual FFmpeg command strings and verify
//! they are well-formed. The `render_real_clip` test (ignored by default)
//! can be run with `cargo test -- --ignored` when FFmpeg is installed and
//! `test-clips/sample-gameplay.mp4` is present.

use vortex_core::{Clip, Effect, FlashEffect, Project, ShakeEffect, TimeRange, VelocityEffect, ZoomEffect};
use vortex_render::{RenderConfig, RenderPipeline};

fn project_with_clips(n: usize) -> Project {
    let mut p = Project::new("render-test");
    for i in 0..n {
        let start = i as f64 * 3.0;
        let clip = Clip::new(
            "test-clips/sample-gameplay.mp4",
            TimeRange::new(start, start + 3.0).unwrap(),
            TimeRange::new(start, start + 3.0).unwrap(),
        )
        .with_label(format!("clip_{i}"))
        .with_effect(Effect::Flash(FlashEffect::default()));
        p.timeline.push_clip(clip);
    }
    p
}

#[test]
fn build_command_single_clip() {
    let p = project_with_clips(1);
    let pipeline = RenderPipeline::default();
    let cmd = pipeline.build_command(&p, "output/test.mp4").unwrap();

    assert_eq!(cmd[0], "ffmpeg");
    assert!(cmd.contains(&"-i".to_string()));
    assert!(cmd.last().unwrap() == "output/test.mp4");
}

#[test]
fn build_command_multi_clip() {
    let p = project_with_clips(3);
    let pipeline = RenderPipeline::default();
    let cmd = pipeline.build_command(&p, "output/multi.mp4").unwrap();

    // Should have 3 -i flags
    let input_count = cmd.windows(2).filter(|w| w[0] == "-i").count();
    assert_eq!(input_count, 3);
}

#[test]
fn build_command_includes_filter_complex() {
    let p = project_with_clips(2);
    let pipeline = RenderPipeline::default();
    let cmd = pipeline.build_command(&p, "output/fc.mp4").unwrap();

    assert!(cmd.contains(&"-filter_complex".to_string()));
}

#[test]
fn build_command_h264_codec_flags() {
    let p = project_with_clips(1);
    let pipeline = RenderPipeline::default();
    let cmd = pipeline.build_command(&p, "output/h264.mp4").unwrap();

    assert!(cmd.contains(&"libx264".to_string()));
    assert!(cmd.contains(&"-crf".to_string()));
}

#[test]
fn build_command_h265_codec() {
    let mut p = project_with_clips(1);
    p.output.codec = "h265".into();
    let pipeline = RenderPipeline::default();
    let cmd = pipeline.build_command(&p, "output/h265.mp4").unwrap();

    assert!(cmd.contains(&"libx265".to_string()));
}

#[test]
fn build_command_with_effects_chain() {
    let mut p = Project::new("effects-test");
    let clip = Clip::new(
        "test-clips/sample-gameplay.mp4",
        TimeRange::new(0.0, 5.0).unwrap(),
        TimeRange::new(0.0, 5.0).unwrap(),
    )
    .with_effect(Effect::Velocity(VelocityEffect::default()))
    .with_effect(Effect::Shake(ShakeEffect::default()))
    .with_effect(Effect::Flash(FlashEffect::default()));
    p.timeline.push_clip(clip);

    let pipeline = RenderPipeline::default();
    let cmd = pipeline.build_command(&p, "output/effects.mp4").unwrap();

    // filter_complex should contain effect chains
    let fc_idx = cmd.iter().position(|s| s == "-filter_complex").unwrap();
    let fc = &cmd[fc_idx + 1];
    assert!(fc.contains("setpts")); // velocity
    assert!(fc.contains("crop"));   // shake
    assert!(fc.contains("geq"));    // flash
}

#[test]
fn build_command_nvidia_hw_accel() {
    let p = project_with_clips(1);
    let pipeline = RenderPipeline::new(RenderConfig {
        hw_accel: "nvidia".into(),
        ..Default::default()
    });
    let cmd = pipeline.build_command(&p, "output/nvidia.mp4").unwrap();
    assert!(cmd.contains(&"cuda".to_string()));
}

#[test]
fn empty_project_errors() {
    let p = Project::new("empty");
    let pipeline = RenderPipeline::default();
    assert!(pipeline.build_command(&p, "output/empty.mp4").is_err());
}

#[tokio::test]
#[ignore = "requires FFmpeg and test-clips/sample-gameplay.mp4"]
async fn render_real_clip() {
    use vortex_render::RenderProgress;

    let mut p = project_with_clips(1);
    p.output.fps = 60.0;
    p.output.codec = "h264".into();

    let pipeline = RenderPipeline::default();
    let mut rx = pipeline.render(&p, "output/integration-test.mp4").await.unwrap();

    let mut got_complete = false;
    while let Some(event) = rx.recv().await {
        match event {
            RenderProgress::Complete { .. } => got_complete = true,
            RenderProgress::Failed { error } => panic!("Render failed: {error}"),
            _ => {}
        }
    }
    assert!(got_complete);
    assert!(std::path::Path::new("output/integration-test.mp4").exists());
}
