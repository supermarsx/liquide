use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;

use liquide_client::config::{self, ClientConfig};
use liquide_client::connection::ConnectionState;
use liquide_client::runtime::ClientRuntime;

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

    /// Connection profile to load.
    #[arg(long)]
    profile: Option<String>,

    /// Path to the configuration file.
    #[arg(long)]
    config: Option<String>,
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

    // Load configuration.
    let config_path = cli
        .config
        .map(std::path::PathBuf::from)
        .or_else(config::default_config_path);
    if let Some(ref path) = config_path {
        info!(path = %path.display(), "Configuration path resolved");
    }

    let mut client_config = ClientConfig::default();
    if cli.fullscreen {
        client_config.window.start_fullscreen = true;
    }

    // Build the runtime.
    let mut runtime = ClientRuntime::new(client_config);

    // If a profile was requested, note it in the audit trail.
    if let Some(ref profile_name) = cli.profile {
        info!(profile = %profile_name, "Loading connection profile");
    }

    // Connect to the server.
    info!(server = %cli.server, "Connecting to server...");
    runtime
        .connect(&cli.server)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("Failed to connect to server")?;
    info!("Connected successfully");

    // Apply fullscreen if requested.
    if cli.fullscreen {
        runtime.toggle_fullscreen();
    }

    // Drain initial audit events.
    for event in runtime.drain_audit_events() {
        info!(event = event.event_name(), "audit");
    }

    // Enter the event loop.
    info!("Client connected -- entering event loop");
    let mut reconnect_interval = tokio::time::interval(std::time::Duration::from_secs(5));

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal");
                break;
            }
            _ = reconnect_interval.tick() => {
                if runtime.state() != ConnectionState::Connected
                    && runtime.state() != ConnectionState::Disconnected
                {
                    info!("Attempting reconnect...");
                    if let Err(e) = runtime.connection_manager_mut().reconnect() {
                        info!(error = %e, "Reconnect attempt failed");
                    }
                }
            }
        }
    }

    // Graceful disconnect.
    runtime.disconnect();
    for event in runtime.drain_audit_events() {
        info!(event = event.event_name(), "audit");
    }

    info!("Disconnected from server");
    Ok(())
}
