use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;

/// Network gateway for the Liquide desktop environment.
///
/// `liquid-gateway` accepts incoming client connections over the network,
/// performs TLS termination and authentication hand-off, then routes each
/// client to the appropriate session process.
#[derive(Parser, Debug)]
#[command(name = "liquid-gateway", version, about)]
struct Cli {
    /// Path to the gateway configuration file.
    #[arg(long, default_value = "/etc/liquide/gateway.toml")]
    config: String,

    /// Address and port to listen on for incoming client connections.
    #[arg(long, default_value = "0.0.0.0:3900")]
    listen_addr: String,
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
    info!(config = %cli.config, listen_addr = %cli.listen_addr, "Starting liquid-gateway");

    // TODO: Load and validate the configuration file.
    info!(path = %cli.config, "Loading configuration...");

    // TODO: Initialize TLS context with server certificates.
    info!("Initializing TLS subsystem...");

    // TODO: Bind the TCP listener.
    info!(addr = %cli.listen_addr, "Binding listener...");

    // TODO: Connect to the supervisor for session routing information.
    info!("Connecting to supervisor...");

    // TODO: Enter the accept loop, spawning a task per client connection.
    info!("Gateway ready — accepting connections");

    // Placeholder: keep the process alive until shutdown signal.
    tokio::signal::ctrl_c()
        .await
        .context("Failed to listen for shutdown signal")?;

    info!("Received shutdown signal — draining connections");
    Ok(())
}
