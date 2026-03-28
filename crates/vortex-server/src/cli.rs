//! CLI command definitions and dispatch.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "vortex",
    about = "VORTEX — AI-powered video montage engine",
    version,
    author,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Render a project JSON file to a video.
    Render {
        /// Path to the project JSON file.
        #[arg(short, long)]
        project: String,

        /// Output video file path.
        #[arg(short, long, default_value = "output.mp4")]
        output: String,

        /// Hardware acceleration backend: none, nvidia, amf, videotoolbox.
        #[arg(long, default_value = "none")]
        hw_accel: String,
    },

    /// Analyse a video file for kill moments, beats, and scene cuts.
    Analyse {
        /// Path to the source video file.
        #[arg(short, long)]
        input: String,

        /// Output JSON file for analysis results.
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Execute a TypeScript montage script.
    Script {
        /// Path to the `.ts` script file.
        path: String,

        /// Output video file path.
        #[arg(short, long, default_value = "output.mp4")]
        output: String,
    },

    /// Start the MCP (Model Context Protocol) server.
    Serve {
        /// Host to bind to.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to listen on.
        #[arg(long, default_value = "7700")]
        port: u16,
    },

    /// List available style templates.
    Styles,
}

/// Parse args and run the selected subcommand.
pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Render { project, output, hw_accel } => {
            cmd_render(&project, &output, &hw_accel).await
        }
        Commands::Analyse { input, output } => {
            cmd_analyse(&input, output.as_deref()).await
        }
        Commands::Script { path, output } => {
            cmd_script(&path, &output).await
        }
        Commands::Serve { host, port } => {
            cmd_serve(&host, port).await
        }
        Commands::Styles => {
            cmd_styles().await
        }
    }
}

async fn cmd_render(project_path: &str, output_path: &str, hw_accel: &str) -> anyhow::Result<()> {
    use vortex_render::{RenderConfig, RenderPipeline, RenderProgress};

    tracing::info!("Loading project from {project_path}");
    let json = tokio::fs::read_to_string(project_path).await?;
    let project = vortex_core::Project::from_json(&json)?;

    let config = RenderConfig {
        hw_accel: hw_accel.to_string(),
        ..Default::default()
    };
    let pipeline = RenderPipeline::new(config);

    tracing::info!("Rendering → {output_path}");
    let mut rx = pipeline.render(&project, output_path).await?;

    while let Some(event) = rx.recv().await {
        match event {
            RenderProgress::Started { total_frames } => {
                println!("Render started ({total_frames} frames)");
            }
            RenderProgress::Frame { current, total, fps, eta_secs } => {
                let pct = if total > 0 { current * 100 / total } else { 0 };
                print!("\r[{pct:3}%] frame {current}/{total} @ {fps:.1}fps  ETA {eta_secs:.0}s  ");
            }
            RenderProgress::Complete { output_path, duration_secs } => {
                println!("\nDone! → {output_path}  ({duration_secs:.2}s)");
            }
            RenderProgress::Failed { error } => {
                anyhow::bail!("Render failed: {error}");
            }
        }
    }

    Ok(())
}

async fn cmd_analyse(input_path: &str, output_path: Option<&str>) -> anyhow::Result<()> {
    use vortex_analysis::{BeatDetector, BeatDetectorConfig, KillDetector, KillDetectorConfig, SceneDetector, SceneDetectorConfig};

    tracing::info!("Analysing {input_path}");

    let kill_detector = KillDetector::new(KillDetectorConfig::default());
    let beat_detector = BeatDetector::new(BeatDetectorConfig::default());
    let scene_detector = SceneDetector::new(SceneDetectorConfig::default());

    let (kills, beats, scenes) = tokio::join!(
        kill_detector.detect(input_path),
        beat_detector.analyse(input_path),
        scene_detector.detect(input_path),
    );

    let analysis = vortex_analysis::ClipAnalysis {
        source_path: input_path.to_string(),
        duration_secs: 0.0, // TODO: probe with ffprobe
        kill_moments: kills?,
        scene_cuts: scenes?,
        beats: Some(beats?),
    };

    let json = serde_json::to_string_pretty(&analysis)?;

    if let Some(out) = output_path {
        tokio::fs::write(out, &json).await?;
        println!("Analysis written to {out}");
    } else {
        println!("{json}");
    }

    Ok(())
}

async fn cmd_script(script_path: &str, output_path: &str) -> anyhow::Result<()> {
    use vortex_render::{RenderConfig, RenderPipeline};
    use vortex_script::ScriptRuntime;

    tracing::info!("Executing script {script_path}");
    let mut runtime = ScriptRuntime::new();
    let project = runtime.execute_file(script_path).await
        .map_err(|e| anyhow::anyhow!("Script error: {e}"))?;

    let pipeline = RenderPipeline::new(RenderConfig::default());
    let mut rx = pipeline.render(&project, output_path).await?;
    while let Some(event) = rx.recv().await {
        tracing::info!(?event, "Render event");
    }

    Ok(())
}

async fn cmd_serve(host: &str, port: u16) -> anyhow::Result<()> {
    use crate::mcp::McpServer;
    tracing::info!("Starting MCP server on {host}:{port}");
    let server = McpServer::new(host, port);
    server.run().await
}

async fn cmd_styles() -> anyhow::Result<()> {
    use vortex_styles::StyleRegistry;
    let registry = StyleRegistry::load_default()?;
    for style in registry.styles() {
        println!("{}: {} — {}", style.name, style.description, style.tags.join(", "));
    }
    Ok(())
}
