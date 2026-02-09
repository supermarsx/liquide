use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, warn};

/// Supervisor daemon for the Liquide desktop environment.
///
/// `liquid-desktopd` is a long-running system service that authenticates
/// users, enforces policy, spawns per-user session processes, and provides
/// an IPC control plane for management tools.
#[derive(Parser, Debug)]
#[command(name = "liquid-desktopd", version, about)]
struct Cli {
    /// Path to the supervisor configuration file.
    #[arg(long, default_value = "/etc/liquide/supervisor.toml")]
    config: String,

    /// Enable developer mode with relaxed security and verbose diagnostics.
    #[arg(long)]
    dev_mode: bool,
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
    info!(config = %cli.config, dev_mode = cli.dev_mode, "Starting liquid-desktopd");

    if cli.dev_mode {
        warn!("Developer mode is enabled — authentication and policy checks are relaxed");
    }

    // TODO: Load and validate the configuration file.
    info!(path = %cli.config, "Loading configuration...");

    // TODO: Initialize the crypto subsystem (TLS certificates, key storage).
    info!("Initializing crypto subsystem...");

    // TODO: Initialize the authentication backend.
    info!("Initializing authentication backend...");

    // TODO: Load policy engine rules.
    info!("Loading policy rules...");

    // TODO: Open the IPC control socket for management tools.
    let control_socket = "/run/liquide/supervisor.sock";
    info!(socket = %control_socket, "Opening control socket...");

    // TODO: Start the listener that accepts session-spawn requests.
    info!("Supervisor ready — listening for connections");

    // Placeholder: keep the daemon alive until shutdown signal.
    tokio::signal::ctrl_c()
        .await
        .context("Failed to listen for shutdown signal")?;

    info!("Received shutdown signal — stopping supervisor");
    Ok(())
}
