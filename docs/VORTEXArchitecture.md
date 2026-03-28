# VORTEX — AI-Powered Video Montage Engine

## Vision

An open-source, agent-driven video editing engine. Instead of a timeline UI, an AI agent (Claude) writes **edit scripts** that the engine executes. Think "Remotion but the programmer is an LLM."

The engine is built in **Rust** for performance, with a **scripting layer** (JS/TS via V8, or Lua) so the agent can write expressive compositions without recompiling.

---

## Goal

VORTEX is an **automated video editor that Claude Code can use as a tool** to edit videos with:

- **Rotoscoping** — AI-powered subject isolation using SAM3, allowing separate effects on subject vs background
- **Color grading** — LUT-based and parametric color correction (exposure, contrast, saturation, tint)
- **VFX** — Screen shake, chromatic aberration, zoom punches, velocity ramps, impact frames, film grain, letterboxing
- **Beat-synced editing** — Auto-detect beats in music and sync cuts/effects to them
- **Kill detection** — YOLOv8 model trained on Valorant kill feed to auto-find highlight moments
- **Style templates** — Pre-built editing grammars (aggressive, chill, cinematic) that map game events to effects

The key insight: **Claude Code writes JS/TS edit scripts → VORTEX engine executes them → renders final video.** No manual timeline editing needed.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Claude Code (Agent)                       │
│    Writes TypeScript edit scripts  •  Calls MCP tools directly  │
└──────────────────────────┬──────────────────────────────────────┘
                           │
              ┌────────────▼─────────────┐
              │       vortex-server       │
              │   CLI  •  MCP server      │
              │  stdio / HTTP+SSE         │
              └─────┬──────────────┬─────┘
                    │              │
         ┌──────────▼───┐   ┌──────▼──────────┐
         │ vortex-script │   │  vortex-styles   │
         │  V8 / Deno    │   │  TOML templates  │
         │  TS runtime   │   │  style registry  │
         └──────────┬───┘   └─────────────────-┘
                    │
    ┌───────────────▼────────────────────────────────────┐
    │                    vortex-core                      │
    │   Project • Timeline • Clip • Effect • AudioTrack   │
    │              TimeRange • OutputSettings             │
    └──────────┬──────────────────────────┬──────────────┘
               │                          │
  ┌────────────▼──────┐      ┌────────────▼──────────────┐
  │  vortex-effects   │      │      vortex-analysis       │
  │  FFmpeg filter    │      │  Kill detection (YOLOv8)   │
  │  graph generation │      │  Beat detection (aubio)    │
  │  per Effect type  │      │  Scene detection (FFmpeg)  │
  └────────────┬──────┘      └────────────┬──────────────┘
               │                          │
  ┌────────────▼──────────┐   ┌───────────▼────────────┐
  │     vortex-render     │   │       vortex-ml         │
  │   Full FFmpeg encode  │   │   ONNX runtime (ORT)    │
  │   pipeline + progress │   │   YOLOv8 • SAM3 • ...  │
  └───────────────────────┘   └────────────────────────┘
```

---

## Module Breakdown

### vortex-core — Data Models

The foundation. All other crates depend on this.

```rust
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub timeline: Timeline,
    pub output: OutputSettings,
    pub style: Option<String>,
}

pub struct Timeline {
    pub clips: Vec<Clip>,
    pub audio_tracks: Vec<AudioTrack>,
    pub duration: f64,
}

pub struct Clip {
    pub id: Uuid,
    pub source_path: String,
    pub source_range: TimeRange,   // trim in/out in the source file
    pub timeline_range: TimeRange, // placement on output timeline
    pub effects: Vec<Effect>,
    pub speed: f64,
    pub audio_gain_db: f64,
    pub is_kill_moment: bool,
    pub kill_confidence: f64,
}

pub enum Effect {
    Velocity(VelocityEffect),    // temporal ramp (slow-mo / speed-up)
    Zoom(ZoomEffect),            // scale punch / pull
    Shake(ShakeEffect),          // camera shake / jitter
    Color(ColorEffect),          // LUT + parametric grade
    Flash(FlashEffect),          // white/color frame burst
    Chromatic(ChromaticEffect),  // RGB channel separation
    Letterbox(LetterboxEffect),  // cinematic bars
    Vignette(VignetteEffect),    // edge darkening
    Glitch(GlitchEffect),        // datamosh / scan-line displacement
}
```

### vortex-effects — FFmpeg Filter Graph Generation

Each `Effect` variant maps to an FFmpeg `-vf` filter fragment. Effects are composable — a `Vec<Effect>` on a clip is rendered as a comma-joined filter chain.

```
Velocity  → setpts=N*PTS  (time-varying for ramps)
Zoom      → zoompan=z='expr':x=cx:y=cy:d=frames
Shake     → crop=W:H:'x+intensity*sin(N*freq)':'y+...'
Color     → eq=brightness=B:contrast=C:saturation=S + lut3d
Flash     → geq=r='r(X,Y)+(255-r(X,Y))*alpha':g='...':b='...'
Chromatic → geq=r='r(X+dx,Y)':g='g(X,Y)':b='b(X-dx,Y)'
Letterbox → crop + pad with black bars
Vignette  → vignette filter
Glitch    → geq with per-scanline displacement
```

### vortex-analysis — ML-Powered Video Understanding

Three analysers run in parallel on source clips:

**Kill Detection** (`kills.rs`)
- Input: video file path
- Model: YOLOv8n ONNX fine-tuned on Valorant kill feed
- Process: decode every 3rd frame → inference → NMS → kill heuristic
- Kill heuristic: enemy bounding box count drops 1→0 within 500ms
- Output: `Vec<KillMoment>` with `source_time`, `confidence`, `event_type`

**Beat Detection** (`beats.rs`)
- Input: audio stream (extracted via FFmpeg → PCM f32)
- Library: `aubio` via C FFI
- Process: `aubio_tempo` for BPM + `aubio_onset` for transients
- Output: `BeatAnalysis` with `bpm`, `Vec<BeatMarker>` (kick/snare/hihat classified)

**Scene Detection** (`scenes.rs`)
- Input: video file
- Process: `ffmpeg -vf select=gt(scene\,0.3)` + stdout parsing
- Output: `Vec<SceneCut>` with `time_secs` and `score`

### vortex-ml — ONNX Runtime Wrapper

Unified inference engine over `ort` (ONNX Runtime for Rust):

```rust
pub struct OnnxRuntime {
    backend: InferenceBackend, // Cpu | Cuda | TensorRT | CoreMl
}

impl OnnxRuntime {
    pub fn load_model(&mut self, path: &str) -> Result<OnnxSession>;
    pub fn run(&self, session: &OnnxSession, inputs: Vec<Tensor>) -> Result<InferenceOutput>;
}
```

CUDA path enabled via `ort = { version = "2", features = ["cuda"] }`.

### vortex-render — FFmpeg Encode Pipeline

Assembles a full `ffmpeg` command from a `Project`:

1. One `-i` input per clip (with `-ss` / `-t` for trim)
2. Per-clip effect filter chain from `vortex-effects`
3. `scale=W:H:force_original_aspect_ratio=decrease,pad=W:H` to normalize resolution
4. `concat=n=N:v=1:a=1` to join all clips
5. `amix` for music track blending + `afade` for fade in/out
6. Output encoding flags per codec (h264/h265/vp9)

Progress is streamed via `tokio::sync::mpsc` by parsing FFmpeg stderr:
```
frame=  240 fps= 58 q=-1.0 size=    2048kB time=00:00:04.00 bitrate=4194.3kbits/s
```

### vortex-script — TypeScript Scripting Runtime

Embeds V8 (via `deno_core`) to execute agent-written TypeScript scripts.

Registered Rust ops:
```typescript
vortex.createProject(name, options)   // → Project
vortex.addClip(project, path, s, e)   // → Clip
vortex.addEffect(clip, effectSpec)    // → void
vortex.addMusic(project, path, opts)  // → void
vortex.applyStyle(project, styleName) // → void
vortex.analyse(videoPath)             // → Promise<Analysis>
vortex.render(project, outputPath)    // → Promise<RenderResult>
```

### vortex-server — CLI + MCP Server

**CLI subcommands:**
```bash
vortex render   --project project.json --output out.mp4
vortex analyse  --input gameplay.mp4 --output analysis.json
vortex script   scripts/montage.ts --output out.mp4
vortex serve    --host 127.0.0.1 --port 7700
vortex styles
```

**MCP Tools** (JSON-RPC 2.0 over stdio):

| Tool | Description |
|------|-------------|
| `create_project` | Create a new project, optionally with a style |
| `add_clip` | Add a clip to the timeline |
| `add_effect` | Attach an effect to a clip |
| `analyse_video` | Run kill/beat/scene analysis |
| `render_project` | Trigger FFmpeg encode pipeline |
| `apply_style` | Apply a style template to the project |
| `list_styles` | Return available styles |

### vortex-styles — Style Template System

TOML files that define a complete montage personality:

```toml
[cuts]
cuts_per_minute = 45.0
beat_snap_tolerance_secs = 0.05
cut_trigger = "hybrid"  # "beat" | "kill" | "scene" | "hybrid"

[velocity]
enabled = true
min_speed = 0.15
ramp_in_secs = 0.25

[effects]
zoom_on_kill = true
shake_on_impact = true
flash_on_beat = true
chromatic_aberration = true
letterbox = true

[color]
lut = "luts/cineon.cube"
saturation = 1.35

[audio]
music_volume = 0.85
gameplay_volume = 0.25
sidechain_db = -12.0
```

---

## Style Templates

### aggressive.toml
The default competitive FPS montage style:
- 45 cuts/min, beat-snapped
- 15% slow-mo velocity ramps on kills (85% probability)
- Zoom punch 1.2× on kills
- Heavy shake (14px intensity)
- White flash on beat drops (85% opacity)
- Chromatic aberration (75% strength)
- 2.39:1 letterbox
- Cineon LUT, +35% saturation, +20% contrast

### chill.toml
Lo-fi / casual highlights:
- 18 cuts/min, beat-driven
- No velocity ramp, no shake
- Soft matte LUT, -10% saturation
- No letterbox, no chromatic
- Longer clips (2–8s)

### cinematic.toml
Film-quality look:
- 28 cuts/min, scene-driven
- 25% slow-mo on kills (55% probability)
- Subtle zoom (1.08×)
- Film print LUT (80% strength)
- 2.39:1 letterbox always on
- Light chromatic (25%)
- -18dB music sidechain (very clean audio)

---

## Agent Workflow

This is how Claude Code uses VORTEX end-to-end:

### Via MCP Tools (preferred for interactive sessions)
```
User: "Make a montage from gameplay.mp4 with the aggressive style"

Claude Code:
1. tools/call: analyse_video { source_path: "gameplay.mp4" }
   → { killMoments: [...], beats: { bpm: 128 }, sceneCuts: [...] }

2. tools/call: create_project { name: "montage", style: "aggressive" }
   → { project_id: "abc-123" }

3. For each top kill moment:
   tools/call: add_clip { project_id, source_path, source_start, source_end }
   tools/call: add_effect { project_id, clip_id, effect_type: "velocity", params: {...} }
   tools/call: add_effect { project_id, clip_id, effect_type: "flash", params: {...} }

4. tools/call: render_project { project_id, output_path: "output/montage.mp4" }
   → streams RenderProgress events → "Done! output/montage.mp4 (47.3s)"
```

### Via TypeScript Script (preferred for repeatable compositions)
```typescript
// Claude Code writes this script, then runs: vortex script montage.ts
const project = vortex.createProject("montage");
const analysis = await vortex.analyse("gameplay.mp4");

const topKills = analysis.killMoments
  .filter(k => k.confidence > 0.75)
  .sort((a, b) => b.confidence - a.confidence)
  .slice(0, 8);

for (const kill of topKills) {
  const clip = vortex.addClip(project, "gameplay.mp4",
    kill.sourceTime - 1.5, kill.sourceTime + 1.0);
  vortex.addEffect(clip, { type: "velocity", minSpeed: 0.15 });
  vortex.addEffect(clip, { type: "zoom", toScale: 1.15 });
  vortex.addEffect(clip, { type: "flash", color: "#FFFFFF" });
}

vortex.addMusic(project, "music.mp3", { volume: 0.85 });
vortex.applyStyle(project, "aggressive");
await vortex.render(project, "output/montage.mp4");
```

---

## Crate Structure

```
vortex/
├── Cargo.toml                  (workspace)
├── crates/
│   ├── vortex-core/            ← Data models (Project, Clip, Effect, Timeline)
│   ├── vortex-analysis/        ← Kill detection, beat detection, scene detection
│   ├── vortex-ml/              ← ONNX runtime wrapper
│   ├── vortex-script/          ← V8/Deno TypeScript runtime
│   ├── vortex-effects/         ← FFmpeg filter graph generation
│   ├── vortex-render/          ← FFmpeg encode pipeline
│   ├── vortex-server/          ← CLI + MCP server (binary)
│   └── vortex-styles/          ← TOML style template parser
├── styles/
│   ├── aggressive.toml
│   ├── chill.toml
│   └── cinematic.toml
├── scripts/
│   ├── basic-montage.ts        ← Example agent script
│   └── beat-sync-demo.ts       ← Beat-synchronised example
├── tests/
│   ├── test_timeline.rs
│   ├── test_effects.rs
│   └── test_render.rs
├── docs/
│   └── VORTEXArchitecture.md   ← This file
└── .github/
    └── CLAUDE.md               ← Implementation guide for Claude Code
```

---

## Hardware Requirements

| Tier | Config | Render Speed |
|------|--------|-------------|
| Minimum | CPU only, any modern processor | ~0.2× realtime |
| Recommended | NVIDIA GPU (RTX 3060+), CUDA 12 | ~4× realtime |
| Optimal | RTX 4080+ with NVEnc + TensorRT | ~12× realtime |

ML inference (kill detection) requires a GPU for practical frame rates.
CPU inference is possible but takes ~30s per minute of footage.

---

## Build & Run

```bash
# Prerequisites: Rust 1.75+, FFmpeg 6+ in PATH
git clone https://github.com/Sangrish-braincade/vortex
cd vortex
cargo build --release

# Trim a test clip
ffmpeg -i source.mp4 -ss 45 -t 30 -c copy test-clips/sample-gameplay.mp4

# Run tests
cargo test

# Analyse a clip
./target/release/vortex analyse --input test-clips/sample-gameplay.mp4

# Run an example script
./target/release/vortex script scripts/basic-montage.ts --output output/montage.mp4

# Start MCP server (for Claude Code integration)
./target/release/vortex serve
```

### Claude Code / MCP integration

Add to `claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "vortex": {
      "command": "/path/to/vortex",
      "args": ["serve"]
    }
  }
}
```

---

## Why Rust + JS/TS?

**Rust** gives us:
- Zero-cost abstractions for video processing hot paths
- Memory safety without GC pauses (critical for real-time frame processing)
- Direct FFI to C libraries (libavcodec, aubio, ONNX Runtime)
- Fearless concurrency for parallel clip processing

**TypeScript scripting layer** gives us:
- An expressive, dynamic language for composition logic
- Easy for Claude Code to write and reason about
- No recompile cycle when changing edit logic
- Familiar syntax for anyone who's written web code

The split is deliberate: **Rust owns the hot path** (frame decode, inference, encode). **TypeScript owns the edit logic** (which clips go where, which effects to apply, how to respond to beats).

---

## Roadmap

### Phase 1 — Core Engine (current)
- [x] Data models (Project, Timeline, Clip, Effect types)
- [x] FFmpeg filter graph generation for all 9 effects
- [x] Render pipeline command builder
- [x] CLI skeleton (render, analyse, script, serve)
- [x] Style template parser (aggressive, chill, cinematic)
- [ ] Real FFmpeg subprocess with progress streaming
- [ ] Integration test: render actual video from test clip
- [ ] Velocity ramp: piecewise PTS expression (not constant)
- [ ] Zoom easing curves (spring, ease_in_out)

### Phase 2 — ML Integration
- [ ] ONNX runtime (ort crate, CPU + CUDA)
- [ ] YOLOv8 kill detection with real inference
- [ ] Aubio beat detection via C FFI
- [ ] Scene detection via FFmpeg scene filter
- [ ] SAM3 rotoscoping / subject isolation
- [ ] Frame extraction pipeline (FFmpeg → raw frames → inference)

### Phase 3 — Agent Layer
- [ ] V8/Deno embedding (deno_core)
- [ ] TypeScript op registration (addClip, addEffect, render, analyse)
- [ ] MCP stdio transport
- [ ] MCP HTTP+SSE transport
- [ ] Full tool handler implementations
- [ ] Session state management (project map in server)
- [ ] End-to-end: Claude Code → MCP → render → output
