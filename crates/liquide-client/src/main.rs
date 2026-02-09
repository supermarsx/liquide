use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;

/// Liquide desktop client.
///
/// `liquidclient` connects to a remote Liquide session, decodes the
/// streamed desktop frames, renders them locally, and forwards input
/// events back to the server.
#[derive(Parser, Debug)]
#[command(name = "liquidclient", version, about)]
struct Cli {
    /// Server address in the form `host:port`.
    #[arg(long)]
    server: String,

    /// Username for authentication.
    #[arg(long)]
    username: Option<String>,

    /// Launch in fullscreen mode.
    #[arg(long)]
    fullscreen: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    run(cli).await
}

async fn run(cli: Cli) -> Result<()> {
    info!(
        server = %cli.server,
        username = cli.username.as_deref().unwrap_or("<prompt>"),
        fullscreen = cli.fullscreen,
        "Starting liquidclient"
    );

    // TODO: Resolve server address and establish a TLS connection.
    info!(server = %cli.server, "Connecting to server...");

    // TODO: Perform authentication handshake.
    info!("Authenticating...");

    // TODO: Negotiate protocol capabilities and display parameters.
    info!("Negotiating session parameters...");

    // TODO: Initialize the client-side renderer (window / surface).
    info!(fullscreen = cli.fullscreen, "Initializing renderer...");

    // TODO: Start input capture and clipboard/audio bridges.
    info!("Starting input and media bridges...");

    // TODO: Enter the frame-decode / render loop.
    info!("Client connected — entering render loop");

    // Placeholder: keep the process alive until shutdown signal.
    tokio::signal::ctrl_c()
        .await
        .context("Failed to listen for shutdown signal")?;

    info!("Disconnecting from server");
    Ok(())
}
