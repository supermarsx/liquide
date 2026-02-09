use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;

/// Management web UI backend for Liquide.
///
/// `liquid-manager` serves the administrative web interface and exposes
/// a REST API for managing users, sessions, policies, and system
/// configuration.
#[derive(Parser, Debug)]
#[command(name = "liquid-manager", version, about)]
struct Cli {
    /// Address and port to listen on for HTTP requests.
    #[arg(long, default_value = "127.0.0.1:8080")]
    listen_addr: String,

    /// Path to the manager configuration file.
    #[arg(long, default_value = "/etc/liquide/manager.toml")]
    config: String,
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
        listen_addr = %cli.listen_addr,
        config = %cli.config,
        "Starting liquid-manager"
    );

    // TODO: Load and validate the configuration file.
    info!(path = %cli.config, "Loading configuration...");

    // TODO: Initialize the authentication backend for admin users.
    info!("Initializing authentication backend...");

    // TODO: Load policy rules for authorization checks.
    info!("Loading policy rules...");

    // TODO: Connect to the supervisor control socket.
    info!("Connecting to supervisor...");

    // TODO: Build the HTTP router with REST API endpoints.
    info!("Building HTTP routes...");

    // TODO: Bind the HTTP listener and start serving.
    info!(addr = %cli.listen_addr, "HTTP server ready — listening for requests");

    // Placeholder: keep the process alive until shutdown signal.
    tokio::signal::ctrl_c()
        .await
        .context("Failed to listen for shutdown signal")?;

    info!("Received shutdown signal — stopping manager");
    Ok(())
}
