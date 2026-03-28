//! VORTEX — AI-powered video montage engine
//!
//! Entry point: parses CLI args and dispatches to `render`, `analyse`,
//! `serve` (MCP), or `script` subcommands.

mod cli;
mod mcp;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("vortex=info".parse()?)
                .add_directive("warn".parse()?),
        )
        .compact()
        .init();

    cli::run().await
}
