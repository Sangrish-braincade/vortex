/**
 * basic-montage.ts
 *
 * A simple VORTEX montage script. Demonstrates the core scripting API:
 * - Creating a project
 * - Adding clips
 * - Attaching effects
 * - Rendering to a file
 *
 * Run with:
 *   vortex script scripts/basic-montage.ts --output output/basic-montage.mp4
 *
 * Or executed by an AI agent via the MCP `tools/call` tool.
 */

// The `vortex` global is injected by the vortex-script runtime.
// Type definitions are in types/vortex.d.ts (TODO: generate from Rust)
declare const vortex: VortexAPI;

interface VortexAPI {
  createProject(name: string, options?: ProjectOptions): Project;
  addClip(project: Project, path: string, start: number, end: number, label?: string): Clip;
  addEffect(clip: Clip, effect: EffectSpec): void;
  addMusic(project: Project, path: string, options?: AudioOptions): void;
  applyStyle(project: Project, styleName: string): void;
  render(project: Project, outputPath: string): Promise<RenderResult>;
  analyse(videoPath: string): Promise<Analysis>;
}

interface ProjectOptions {
  width?: number;
  height?: number;
  fps?: number;
  style?: string;
}

interface Project { id: string; name: string; }
interface Clip { id: string; label?: string; }

interface EffectSpec {
  type: "velocity" | "zoom" | "shake" | "color" | "flash" | "chromatic" | "letterbox" | "vignette" | "glitch";
  [key: string]: unknown;
}

interface AudioOptions {
  volume?: number;
  looped?: boolean;
  fadeIn?: number;
  fadeOut?: number;
}

interface RenderResult {
  outputPath: string;
  durationSecs: number;
  fileSizeBytes: number;
}

interface Analysis {
  killMoments: Array<{ sourceTime: number; confidence: number; eventType: string }>;
  beats: { bpm: number; markers: Array<{ time: number; strength: number; beatType: string }> };
  sceneCuts: Array<{ timeSecs: number; score: number }>;
}

// ─── Script ───────────────────────────────────────────────────────────────────

const SOURCE = "test-clips/sample-gameplay.mp4";
const MUSIC  = "audio/track.mp3"; // Place your music file here

async function main() {
  console.log("🎬 VORTEX Basic Montage Script");

  // 1. Create a project
  const project = vortex.createProject("Basic Montage", {
    width: 1920,
    height: 1080,
    fps: 60,
  });

  // 2. Analyse the source video to find kill moments + beats
  console.log(`Analysing ${SOURCE}...`);
  const analysis = await vortex.analyse(SOURCE);
  console.log(`Found ${analysis.killMoments.length} kill moments @ BPM ${analysis.beats.bpm}`);

  // 3. Build clips around the top kill moments
  const topKills = analysis.killMoments
    .filter(k => k.confidence > 0.75)
    .sort((a, b) => b.confidence - a.confidence)
    .slice(0, 5);

  for (const kill of topKills) {
    const start = Math.max(0, kill.sourceTime - 1.5);
    const end   = kill.sourceTime + 1.0;

    const clip = vortex.addClip(project, SOURCE, start, end, `kill_${kill.eventType}`);

    // Velocity ramp at the kill moment
    vortex.addEffect(clip, {
      type: "velocity",
      minSpeed: 0.15,
      maxSpeed: 1.0,
      rampInSecs: 0.3,
      rampOutSecs: 0.5,
    });

    // Zoom punch
    vortex.addEffect(clip, {
      type: "zoom",
      fromScale: 1.0,
      toScale: 1.15,
      durationSecs: 0.2,
    });

    // White flash on the kill frame
    vortex.addEffect(clip, {
      type: "flash",
      color: "#FFFFFF",
      peakOpacity: 0.75,
      durationSecs: 0.1,
    });
  }

  // 4. Add music
  vortex.addMusic(project, MUSIC, {
    volume: 0.85,
    looped: false,
    fadeIn: 1.0,
    fadeOut: 2.0,
  });

  // 5. Apply aggressive style as a final polish pass
  vortex.applyStyle(project, "aggressive");

  // 6. Render!
  console.log("Rendering...");
  const result = await vortex.render(project, "output/basic-montage.mp4");
  console.log(`✅ Rendered to ${result.outputPath} (${result.durationSecs.toFixed(1)}s)`);
}

main().catch(console.error);
