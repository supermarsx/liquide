use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, warn};

/// Per-user session process for the Liquide desktop environment.
///
/// Each authenticated user gets a dedicated `liquid-session` process
/// that manages their compositor, shell, input routing, and application
/// lifecycle.
#[derive(Parser, Debug)]
#[command(name = "liquid-session", version, about)]
struct Cli {
    /// Enable developer mode with additional diagnostics and relaxed security.
    #[arg(long)]
    dev_mode: bool,

    /// Unique identifier for this session, assigned by the supervisor.
    #[arg(long)]
    session_id: Option<String>,
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
    let session_id = cli
        .session_id
        .unwrap_or_else(|| uuid_stub());

    info!(
        session_id = %session_id,
        dev_mode = cli.dev_mode,
        "Starting liquid-session"
    );

    if cli.dev_mode {
        warn!("Developer mode is enabled — security checks are relaxed");
    }

    // TODO: Initialize the compositor subsystem.
    info!("Initializing compositor...");

    // TODO: Initialize the shell (panels, launcher, workspace management).
    info!("Initializing shell...");

    // TODO: Start the input-routing pipeline.
    info!("Initializing input routing...");

    // TODO: Start clipboard, audio, and accessibility bridges.
    info!("Initializing auxiliary services (clipboard, audio, a11y)...");

    // TODO: Load plugins via the plugin host.
    info!("Loading plugins...");

    // TODO: Enter the main event loop.
    info!("Session ready — entering event loop");

    // Placeholder: keep the process alive until shutdown signal.
    tokio::signal::ctrl_c()
        .await
        .context("Failed to listen for shutdown signal")?;

    info!("Received shutdown signal — tearing down session");
    Ok(())
}

/// Generate a placeholder session ID when none is provided.
fn uuid_stub() -> String {
    format!("session-{:08x}", std::process::id())
}
