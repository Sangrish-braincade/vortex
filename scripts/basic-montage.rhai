// basic-montage.rhai — sample VORTEX montage script
//
// Run with: vortex script scripts/basic-montage.rhai --output output/montage.mp4
//
// This script demonstrates the VORTEX scripting API:
// - create_project(name)
// - add_clip(project_id, path, start_secs, end_secs) -> clip_id
// - add_effect(clip_id, effect_type, params_map)
// - set_bpm(bpm)
// - render(project_id, output_path)

let pid = create_project("basic-montage");

// Set global BPM for beat-synced cuts
set_bpm(128.0);

// Clip 1: opening — normal speed, color grade
let c1 = add_clip(pid, "test-clips/sample-gameplay.mp4", 10.0, 20.0);
add_effect(c1, "color", #{
    saturation: 1.3,
    contrast: 1.1
});

// Clip 2: highlight — slow-mo + zoom punch
let c2 = add_clip(pid, "test-clips/sample-gameplay.mp4", 45.0, 55.0);
add_effect(c2, "velocity", #{
    min_speed: 0.15,
    max_speed: 1.0,
    ramp_in_secs: 0.5,
    ramp_out_secs: 0.5
});
add_effect(c2, "zoom", #{
    from_scale: 1.0,
    to_scale: 1.2,
    duration_secs: 0.3,
    easing: "spring"
});
add_effect(c2, "flash", #{
    peak_opacity: 0.9,
    duration_secs: 0.1
});

// Clip 3: outro
let c3 = add_clip(pid, "test-clips/sample-gameplay.mp4", 120.0, 128.0);
add_effect(c3, "shake", #{
    intensity_x: 10.0,
    intensity_y: 6.0,
    frequency: 20.0
});

// Trigger render
render(pid, "output/basic-montage.mp4");
