use anyhow::{Context, Result};
use clap::Parser;
use liquide_manager::{ManagerConfig, ManagerRuntime};
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

    // Load configuration (stub: use defaults until config file parsing is wired).
    info!(path = %cli.config, "Loading configuration...");
    let config = ManagerConfig::default();

    // Initialize the management runtime.
    info!("Initializing management runtime...");
    let runtime = ManagerRuntime::new(config);

    info!(
        servers = runtime.servers().count(),
        gateways = runtime.gateways().count(),
        admins = runtime.admins().count(),
        "Runtime initialized"
    );

    // Register API endpoints.
    let endpoints = liquide_manager::api::default_endpoints();
    info!(count = endpoints.len(), "Registered API endpoints");

    // Build and log initial dashboard state.
    let dash = runtime.dashboard(0);
    info!(
        healthy = dash.servers_healthy,
        unhealthy = dash.servers_unhealthy,
        offline = dash.servers_offline,
        "Initial dashboard state"
    );

    info!(addr = %cli.listen_addr, "HTTP server ready — listening for requests");

    // Keep the process alive until shutdown signal.
    tokio::signal::ctrl_c()
        .await
        .context("Failed to listen for shutdown signal")?;

    info!("Received shutdown signal — stopping manager");
    Ok(())
}
