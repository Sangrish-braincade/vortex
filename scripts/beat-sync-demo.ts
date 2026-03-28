/**
 * beat-sync-demo.ts
 *
 * Demonstrates VORTEX's beat-synchronised editing: clips are cut precisely
 * on kick drum beats, with flash effects on downbeats and chromatic
 * aberration on off-beats.
 *
 * Run with:
 *   vortex script scripts/beat-sync-demo.ts --output output/beat-sync.mp4
 */

declare const vortex: VortexAPI;

// (Types repeated here for standalone script clarity — in practice these
//  would come from an imported types/vortex.d.ts)
interface VortexAPI {
  createProject(name: string, options?: ProjectOptions): Project;
  addClip(project: Project, path: string, start: number, end: number, label?: string): Clip;
  addEffect(clip: Clip, effect: EffectSpec): void;
  addMusic(project: Project, path: string, options?: AudioOptions): void;
  render(project: Project, outputPath: string): Promise<RenderResult>;
  analyse(videoPath: string): Promise<Analysis>;
}

interface ProjectOptions { width?: number; height?: number; fps?: number; }
interface Project { id: string; }
interface Clip { id: string; }
interface EffectSpec { type: string; [k: string]: unknown; }
interface AudioOptions { volume?: number; looped?: boolean; fadeIn?: number; fadeOut?: number; }
interface RenderResult { outputPath: string; durationSecs: number; }
interface BeatMarker { time: number; strength: number; beatType: string; }
interface Analysis {
  killMoments: Array<{ sourceTime: number; confidence: number }>;
  beats: { bpm: number; markers: BeatMarker[] };
  sceneCuts: Array<{ timeSecs: number }>;
}

// ─── Config ────────────────────────────────────────────────────────────────

const SOURCE = "test-clips/sample-gameplay.mp4";
const MUSIC  = "audio/track.mp3";

/** Seconds of gameplay to use as clip candidates. */
const CLIP_POOL_DURATION = 30;

// ─── Script ───────────────────────────────────────────────────────────────────

async function main() {
  console.log("🥁 VORTEX Beat-Sync Demo");

  const project = vortex.createProject("Beat Sync Demo", {
    width: 1920,
    height: 1080,
    fps: 60,
  });

  // Analyse source for beats and gameplay moments
  const analysis = await vortex.analyse(SOURCE);
  const { bpm, markers } = analysis.beats;
  console.log(`BPM: ${bpm.toFixed(1)} — ${markers.length} beat markers detected`);

  // Filter to kicks only (downbeats = hardest cut points)
  const kicks = markers.filter(b => b.beatType === "kick");
  const snares = markers.filter(b => b.beatType === "snare");

  console.log(`Kicks: ${kicks.length}  Snares: ${snares.length}`);

  // Build clip duration from BPM — each clip is 2 beats long
  const beatDuration = 60.0 / bpm;
  const clipDuration = beatDuration * 2;

  // Assign gameplay segments to each kick beat
  // We advance through the source pool as we add clips
  let sourcePos = 0;
  const maxClips = Math.min(kicks.length, Math.floor(CLIP_POOL_DURATION / clipDuration));

  for (let i = 0; i < maxClips; i++) {
    const beat = kicks[i];
    const clipEnd = Math.min(sourcePos + clipDuration, CLIP_POOL_DURATION);

    const clip = vortex.addClip(project, SOURCE, sourcePos, clipEnd, `beat_${i}`);
    sourcePos = clipEnd;

    // On every 4th kick (bar start): zoom punch + flash
    if (i % 4 === 0) {
      vortex.addEffect(clip, {
        type: "zoom",
        fromScale: 1.0,
        toScale: 1.12,
        durationSecs: beatDuration * 0.5,
        easing: "ease_out",
      });

      vortex.addEffect(clip, {
        type: "flash",
        color: "#FFFFFF",
        peakOpacity: 0.6,
        durationSecs: beatDuration * 0.3,
      });
    }

    // On every 2nd kick (half-bar): chromatic aberration burst
    if (i % 2 === 0 && i % 4 !== 0) {
      vortex.addEffect(clip, {
        type: "chromatic",
        offsetRX: 6.0,
        offsetBX: -6.0,
        strength: 0.8,
      });
    }

    // On snares (off-beats): subtle shake
    const nearestSnare = snares.find(
      s => Math.abs(s.time - beat.time - beatDuration) < 0.05
    );
    if (nearestSnare) {
      vortex.addEffect(clip, {
        type: "shake",
        intensityX: 6.0,
        intensityY: 4.0,
        frequency: 20.0,
        decay: 0.9,
      });
    }

    // Cinematic letterbox on all clips
    vortex.addEffect(clip, {
      type: "letterbox",
      aspectRatio: 2.39,
      animateSecs: 0,
    });
  }

  // Add music track
  vortex.addMusic(project, MUSIC, {
    volume: 0.9,
    looped: false,
    fadeIn: 0.5,
    fadeOut: 2.0,
  });

  // Render
  console.log(`Rendering ${maxClips} clips (${(maxClips * clipDuration).toFixed(1)}s)...`);
  const result = await vortex.render(project, "output/beat-sync.mp4");
  console.log(`✅ Done! → ${result.outputPath} (${result.durationSecs.toFixed(1)}s)`);
}

main().catch(console.error);
