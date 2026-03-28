# CLAUDE.md — VORTEX Implementation Guide

This file is read by Claude Code when working in this repository.
Follow it carefully. It tells you what's built, what needs to be built,
how to do it, and what the standards are.

---

## Project Goal

VORTEX is an automated video editor that **YOU** (Claude Code) use as a tool to edit videos. When a user asks you to edit a video, you:

1. Load clips via the MCP tools or CLI
2. Analyze them (detect kills, beats, scene changes)
3. Write a JS/TS composition script that arranges clips, applies effects (rotoscoping, color grading, VFX, velocity ramps, etc.)
4. Render the final output

**You are both the developer building this tool AND the primary user of it.** Build it so you can use it effectively. Every API decision, every tool schema, every error message should be optimized for an AI agent calling it programmatically — not for a human using a GUI.

---

## What this project is

**VORTEX** is an AI-powered video montage engine written in Rust.
It takes FPS gameplay footage, analyses it for kill moments and beat markers,
and automatically produces a polished video montage with cinematic effects.

The engine exposes:
- A **CLI** (`vortex render / analyse / script / serve`)
- An **MCP server** so AI agents can drive it via JSON-RPC tool calls
- A **TypeScript scripting runtime** for custom montage logic

---

## Crate map

| Crate | Status | Purpose |
|-------|--------|---------|
| `vortex-core` | ✅ Data models complete | Project, Timeline, Clip, Effect types |
| `vortex-effects` | ✅ Filter stubs | FFmpeg filter graph generation per effect |
| `vortex-render` | ✅ Command builder | FFmpeg pipeline — needs real subprocess |
| `vortex-analysis` | 🔨 Stubs | Kill/beat/scene detection |
| `vortex-ml` | 🔨 Stub | ONNX runtime wrapper |
| `vortex-script` | 🔨 Stub | V8/Deno embedding |
| `vortex-server` | ✅ CLI + MCP skeleton | Needs MCP transport + tool handlers |
| `vortex-styles` | ✅ Parser complete | TOML style templates |

---

## Phase 1 — Core Engine (DO THIS FIRST)

### Step 1: Validate data models

```bash
cargo test -p vortex-core
cargo test -p vortex-effects
cargo clippy --all
```

Fix any compilation errors before proceeding.

### Step 2: Complete FFmpeg filter graph generation (vortex-effects)

Each effect in `crates/vortex-effects/src/` has a `// TODO (Phase 1)` comment.
Implement each one:

**velocity.rs**
- Replace the constant `setpts` with a piecewise-linear ramp expression
- Use FFmpeg's `if(lt(T,t0),expr1,if(lt(T,t1),expr2,expr3))` syntax
- Test that `setpts=1.0*PTS` produces normal speed and `setpts=6.67*PTS` produces 15% speed

**zoom.rs**
- Implement easing curves: linear, ease_in, ease_out, spring
- Spring easing: `scale = to + (from - to) * exp(-k * t) * cos(w * t)`
- The `zoompan` filter takes `z='expr'` where `in` = frame number

**shake.rs**
- Replace the `sin` oscillator with noise-modulated shake for more natural feel
- Consider using a `perlin`-style noise function via geq

**All effects**: Ensure they compose correctly via `compose_effects()` in lib.rs.

### Step 3: Implement real FFmpeg subprocess (vortex-render/pipeline.rs)

Find `// TODO (Phase 1): replace stub with actual subprocess` in `render()`.

Replace the stub with:
```rust
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};

let mut child = Command::new("ffmpeg")
    .args(&cmd[1..]) // skip "ffmpeg" string
    .stderr(std::process::Stdio::piped())
    .stdout(std::process::Stdio::null())
    .spawn()?;

let stderr = child.stderr.take().unwrap();
let mut lines = BufReader::new(stderr).lines();

while let Some(line) = lines.next_line().await? {
    // Parse: frame=  120 fps= 60 q=-1.0 size=    1234kB time=00:00:02.00 bitrate= 5043.2kbits/s speed=1.01x
    if let Some(progress) = parse_ffmpeg_progress(&line, total_frames) {
        let _ = tx.send(progress).await;
    }
}
```

Parse function should extract `frame=N`, `fps=X`, `time=HH:MM:SS.xx` from FFmpeg stderr.

### Step 4: Write integration test that renders a real clip

In `tests/test_render.rs`, the `render_real_clip` test is `#[ignore]`.
Once FFmpeg subprocess is implemented, run it:

```bash
# Place a 30s clip:
ffmpeg -i valorant_montage_source.mp4 -ss 45 -t 30 -c copy test-clips/sample-gameplay.mp4

# Run the integration test:
cargo test -- --ignored render_real_clip
```

The test should:
1. Load `test-clips/sample-gameplay.mp4`
2. Add 3 clips from it
3. Apply velocity + zoom + flash effects
4. Render to `output/integration-test.mp4`
5. Assert the output file exists and is non-empty

### Step 5: CLI end-to-end

```bash
cargo build
./target/debug/vortex render --project examples/sample-project.json --output output/test.mp4
```

Create `examples/sample-project.json` by serialising a test project:
```bash
cargo run --example create_sample_project > examples/sample-project.json
```

---

## Phase 2 — ML Integration

### Step 1: ONNX runtime (vortex-ml)

1. Uncomment `ort` dependency in `crates/vortex-ml/Cargo.toml`
2. Implement `OnnxRuntime::load_model` with real `ort::Session`
3. Implement `OnnxRuntime::run` converting `Tensor` ↔ `ort::Value`
4. Test with a dummy ONNX model (`cargo test -p vortex-ml`)

```rust
// crates/vortex-ml/src/runtime.rs
use ort::{Environment, Session, SessionBuilder, GraphOptimizationLevel};

pub fn load_model(path: &str) -> Result<OnnxSession> {
    let env = Arc::new(
        Environment::builder().with_name("vortex").build()?
    );
    let session = SessionBuilder::new(&env)?
        .with_optimization_level(GraphOptimizationLevel::All)?
        .with_model_from_file(path)?;
    Ok(OnnxSession { session, path: path.to_string(), backend: InferenceBackend::Cpu })
}
```

### Step 2: YOLOv8 kill detection (vortex-analysis/kills.rs)

1. Download YOLOv8n ONNX: `wget https://github.com/ultralytics/assets/releases/download/v8.2.0/yolov8n.pt && python -c "from ultralytics import YOLO; YOLO('yolov8n.pt').export(format='onnx')" `
2. Fine-tune or use zero-shot with "enemy player" class detection
3. Replace the stub in `KillDetector::detect` with:
   - Frame extraction via FFmpeg every 3rd frame
   - Resize frame to 640x640
   - Run `OnnxRuntime::run`
   - Parse YOLO output: `[batch, 84, 8400]` → NMS → `Vec<Detection>`
4. Kill heuristic: enemy count drops from ≥1 to 0 within 0.5s → kill moment

YOLO output parsing:
```rust
// Output shape: [1, 84, 8400] (cx, cy, w, h, 80 class scores)
// Transpose to [8400, 84], filter by confidence, apply NMS
```

### Step 3: Aubio beat detection (vortex-analysis/beats.rs)

1. Add aubio FFI bindings or use the `aubio` crate
2. Extract audio from source via: `ffmpeg -i input.mp4 -ac 1 -ar 44100 -f f32le pipe:1`
3. Feed PCM samples to `aubio_tempo_t` for BPM detection
4. Feed to `aubio_onset_t` with method="complex" for beat onset times
5. Classify beats by spectral centroid: low=kick, mid=snare, high=hihat

### Step 4: Scene detection (vortex-analysis/scenes.rs)

```rust
// Use FFmpeg scene detection filter:
let output = Command::new("ffmpeg")
    .args(["-i", video_path,
           "-vf", "select=gt(scene\\,0.3),metadata=print:file=-",
           "-an", "-f", "null", "-"])
    .output()
    .await?;
// Parse stdout for "pts_time:" values
```

---

## Phase 3 — Agent Layer

### Step 1: V8/Deno embedding (vortex-script)

1. Add `deno_core = "0.290"` to `Cargo.toml`
2. Register Rust ops:

```rust
#[op2]
fn op_add_clip(
    state: &mut OpState,
    #[string] path: String,
    start: f64,
    end: f64,
) -> Result<String, deno_core::error::AnyError> {
    let project = state.borrow_mut::<ProjectState>();
    let clip = project.add_clip(path, start, end);
    Ok(clip.id.to_string())
}
```

3. Bundle `scripts/types/vortex.d.ts` TypeScript type definitions
4. Test: `vortex script scripts/basic-montage.ts --output output/test.mp4`

### Step 2: MCP transport (vortex-server/mcp.rs)

1. Add `axum` dependency for HTTP transport
2. Implement stdio transport (default):

```rust
use tokio::io::{stdin, stdout, AsyncBufReadExt, AsyncWriteExt};

loop {
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let req: McpRequest = serde_json::from_str(&line)?;
    let resp = server.dispatch(req).await;
    let json = serde_json::to_string(&resp)? + "\n";
    writer.write_all(json.as_bytes()).await?;
}
```

3. Implement all 7 tool handlers in `dispatch()`:
   - `create_project` → create in session map, return project_id
   - `add_clip` → look up project by id, call `timeline.push_clip`
   - `add_effect` → parse effect JSON, call `clip.add_effect`
   - `analyse_video` → run `vortex-analysis` pipeline
   - `render_project` → spawn render, stream progress via SSE
   - `apply_style` → load style from registry, apply defaults to all clips
   - `list_styles` → return `StyleRegistry::load_default().styles()`

### Step 3: Claude Code integration

Add to `claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "vortex": {
      "command": "vortex",
      "args": ["serve"],
      "env": {}
    }
  }
}
```

Test with: "Create a montage from test-clips/sample-gameplay.mp4 using the aggressive style"

---

## Coding standards

- **All public API items** must have `///` doc comments
- **Every module** needs at least one `#[test]` block
- **Error handling**: use `Result<T, VortexError>` (or crate-local error type) — never `.unwrap()` in non-test code
- **FFmpeg calls**: always go through `vortex-render`. No other crate may call `ffmpeg` directly.
- **Effects are composable**: `clip.with_effect(a).with_effect(b)` must work and must chain in order
- **Logging**: `tracing::info!` for user-visible operations, `tracing::debug!` for internals

## Effect filter rules

- Each effect module exports exactly one public `*_filter(effect, ctx) -> Result<FilterFragment>` function
- `FilterFragment.filter` must be a valid FFmpeg `-vf` fragment (no input labels, no `[out]`)
- `compose_effects` joins fragments with `,` — do not include commas at end of individual fragments
- Test every filter with `ffmpeg -i test.mp4 -vf "FRAGMENT" -f null -` to verify it's valid FFmpeg syntax

## Test strategy

```bash
# Run all unit tests
cargo test

# Run with output (for debugging)
cargo test -- --nocapture

# Run integration tests (requires FFmpeg + test clip)
cargo test -- --ignored

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt --check
```

CI runs `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check`.
All three must pass before merging.

---

## Common pitfalls

1. **setpts timing**: `setpts=N*PTS` slows down, `setpts=(1/N)*PTS` speeds up.
   Double-check the math before writing velocity expressions.

2. **zoompan and concat**: `zoompan` changes frame count. After a zoom, you may
   need `fps=60` to normalise before `concat`. See FFmpeg wiki.

3. **Audio atempo limits**: FFmpeg's `atempo` filter only accepts 0.5–2.0 range.
   For extreme speed changes, chain multiple: `atempo=0.5,atempo=0.5` for 0.25×.

4. **ONNX shape**: YOLOv8 ONNX export uses dynamic batch size. Always set
   batch=1 explicitly to avoid shape inference errors.

5. **Style application order**: Style defaults are applied *before* script effects.
   Script effects override style defaults. Don't mutate clips that already have
   effects set by the script.
