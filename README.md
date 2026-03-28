# VORTEX

**AI-powered video montage engine** — feed it gameplay footage, and it produces a polished cinematic edit with kill detection, beat-sync, and composable visual effects.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Agent / User                          │
│              TypeScript scripts  •  MCP tool calls           │
└──────────────────────────┬──────────────────────────────────┘
                           │
              ┌────────────▼────────────┐
              │      vortex-server       │
              │   CLI  •  MCP server     │
              └─────┬─────────────┬─────┘
                    │             │
         ┌──────────▼──┐    ┌─────▼──────────┐
         │vortex-script│    │ vortex-styles   │
         │  V8 / Deno  │    │  TOML templates │
         └──────────┬──┘    └─────────────────┘
                    │
         ┌──────────▼──────────────────────────────────────┐
         │                  vortex-core                     │
         │  Project • Timeline • Clip • Effect • AudioTrack │
         └───────┬──────────────────────┬───────────────────┘
                 │                      │
    ┌────────────▼─────┐     ┌──────────▼──────────┐
    │  vortex-effects  │     │   vortex-analysis    │
    │  FFmpeg filters  │     │  Kill • Beat • Scene │
    │  per Effect type │     └──────────┬───────────┘
    └────────────┬─────┘                │
                 │              ┌───────▼────────┐
    ┌────────────▼─────────┐    │  vortex-ml     │
    │    vortex-render     │    │  ONNX runtime  │
    │   FFmpeg pipeline    │    │  YOLOv8 • SAM  │
    └──────────────────────┘    └────────────────┘
```

### Crates

| Crate | Role |
|-------|------|
| `vortex-core` | Data models: `Project`, `Timeline`, `Clip`, `Effect` enum, `AudioTrack` |
| `vortex-effects` | Translates each `Effect` variant to an FFmpeg filter graph fragment |
| `vortex-render` | Assembles FFmpeg command + spawns encode subprocess |
| `vortex-analysis` | Kill detection (YOLOv8), beat detection (aubio), scene cuts (FFmpeg) |
| `vortex-ml` | ONNX Runtime wrapper — shared inference engine for all ML models |
| `vortex-script` | Embeds a V8/Deno runtime to execute TypeScript montage scripts |
| `vortex-server` | CLI entrypoint + MCP server (Model Context Protocol) |
| `vortex-styles` | Parses `.toml` style templates; applies preset effect + pacing defaults |

---

## Effects

| Effect | FFmpeg impl | Description |
|--------|------------|-------------|
| `velocity` | `setpts` | Temporal slow-mo / speed ramp at kill moments |
| `zoom` | `zoompan` | Punch in / pull out with easing |
| `shake` | `crop` (oscillating x/y) | Camera shake on impacts |
| `color` | `eq` + `lut3d` | Saturation, contrast, LUT overlay |
| `flash` | `geq` | White/color flash on beat drops |
| `chromatic` | `geq` | RGB channel separation |
| `letterbox` | `crop` + `pad` | Cinematic 2.39:1 bars |
| `vignette` | `vignette` | Edge darkening |
| `glitch` | `geq` | Datamosh / scan-line displacement |

Effects are **composable** — chain as many as you like:

```rust
let clip = Clip::new(source, src_range, out_range)
    .with_effect(Effect::Velocity(VelocityEffect { min_speed: 0.15, ..Default::default() }))
    .with_effect(Effect::Zoom(ZoomEffect::default()))
    .with_effect(Effect::Flash(FlashEffect::default()));
```

---

## Style Templates

Pre-built montage personalities in `styles/`:

| Style | Cuts/min | Vibe |
|-------|----------|------|
| `aggressive` | 45 | Hard-hitting, extreme slow-mo, heavy shake + flash |
| `chill` | 18 | Relaxed pacing, soft color grade, no shake |
| `cinematic` | 28 | 2.39 anamorphic, film LUT, controlled velocity |

---

## Getting Started

### Prerequisites

- Rust 1.75+ (`rustup update stable`)
- FFmpeg 6+ in `$PATH`
- (Phase 2) CUDA 12+ for GPU inference

### Build

```bash
git clone https://github.com/sangrish/vortex
cd vortex
cargo build --release
```

### Run the CLI

```bash
# Analyse a video
./target/release/vortex analyse --input gameplay.mp4 --output analysis.json

# Render a project
./target/release/vortex render --project project.json --output montage.mp4

# Execute a TypeScript montage script
./target/release/vortex script scripts/basic-montage.ts --output montage.mp4

# Start MCP server (for Claude / AI agent use)
./target/release/vortex serve

# List style templates
./target/release/vortex styles
```

### Use as an MCP tool from Claude

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

Then ask Claude: *"Create a montage from gameplay.mp4 using the aggressive style"*

### Run tests

```bash
# Unit tests
cargo test

# Integration tests (requires FFmpeg + test clip)
ffmpeg -i /path/to/source.mp4 -ss 45 -t 30 -c copy test-clips/sample-gameplay.mp4
cargo test -- --ignored
```

---

## TypeScript Scripting API

```typescript
// scripts/basic-montage.ts
const project = vortex.createProject("My Montage", { fps: 60 });
const analysis = await vortex.analyse("gameplay.mp4");

for (const kill of analysis.killMoments.slice(0, 5)) {
  const clip = vortex.addClip(project, "gameplay.mp4", kill.sourceTime - 1.5, kill.sourceTime + 1.0);
  vortex.addEffect(clip, { type: "velocity", minSpeed: 0.15 });
  vortex.addEffect(clip, { type: "flash", color: "#FFFFFF" });
}

vortex.addMusic(project, "music.mp3", { volume: 0.85 });
vortex.applyStyle(project, "aggressive");
await vortex.render(project, "output/montage.mp4");
```

---

## Implementation Status

- [x] Phase 1 scaffolding — data models, effect stubs, CLI skeleton
- [ ] Phase 1 complete — real FFmpeg subprocess, effect filter QA, integration tests
- [ ] Phase 2 — YOLOv8 kill detection, aubio beat detection, ONNX runtime
- [ ] Phase 3 — V8/Deno scripting, MCP server transport, agent workflows

See `.github/CLAUDE.md` for the full implementation guide (designed to be read by Claude Code).

---

## License

MIT
