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

    /// Password for authentication.
    ///
    /// Prefer the `LIQUIDE_PASSWORD` environment variable or an interactive
    /// prompt; passing a secret on the command line can leak it to the process
    /// list. When omitted, the password is read from `LIQUIDE_PASSWORD` or, if
    /// stdin is a terminal, prompted for.
    #[arg(long)]
    password: Option<String>,

    /// Authentication token (alternative to username/password).
    ///
    /// Prefer the `LIQUIDE_TOKEN` environment variable over the command line.
    #[arg(long)]
    token: Option<String>,

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
        .clone()
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

    // Resolve credentials from flags, then env, then an interactive prompt.
    // Secrets are never logged.
    let (username, password) = resolve_credentials(&cli)?;

    // Connect to the server. Credentials are passed straight to the connection
    // manager and never emitted to the audit log or tracing output.
    info!(
        server = %cli.server,
        username = %username,
        "Connecting to server..."
    );
    runtime
        .connect_with_credential(&cli.server, &username, &password)
        .await
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
                    if let Err(e) = runtime.connection_manager_mut().reconnect().await {
                        info!(error = %e, "Reconnect attempt failed");
                    }
                }
            }
        }
    }

    // Graceful disconnect.
    runtime.disconnect().await;
    for event in runtime.drain_audit_events() {
        info!(event = event.event_name(), "audit");
    }

    info!("Disconnected from server");
    Ok(())
}

/// Resolve `(username, password)` for authentication without ever logging the
/// secret.
///
/// Resolution order:
/// 1. `--token` / `LIQUIDE_TOKEN` — sent as the credential with an empty user.
/// 2. `--username` / `LIQUIDE_USERNAME` and `--password` / `LIQUIDE_PASSWORD`.
/// 3. If the password is still unset and stdin is a terminal, prompt for it.
fn resolve_credentials(cli: &Cli) -> Result<(String, String)> {
    // Token path takes precedence and carries the secret in the password slot.
    if let Some(token) = cli
        .token
        .clone()
        .or_else(|| std::env::var("LIQUIDE_TOKEN").ok())
    {
        if !token.is_empty() {
            return Ok((String::new(), token));
        }
    }

    let username = cli
        .username
        .clone()
        .or_else(|| std::env::var("LIQUIDE_USERNAME").ok())
        .unwrap_or_default();

    let mut password = cli
        .password
        .clone()
        .or_else(|| std::env::var("LIQUIDE_PASSWORD").ok())
        .unwrap_or_default();

    if password.is_empty() && std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        // Interactive fallback. Note: this echoes input; a production build
        // would use a hidden-input crate. We avoid adding that dependency here.
        use std::io::Write;
        print!(
            "Password for {}: ",
            if username.is_empty() { "session" } else { &username }
        );
        std::io::stdout().flush().ok();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_ok() {
            password = line.trim_end_matches(['\r', '\n']).to_string();
        }
    }

    Ok((username, password))
}
