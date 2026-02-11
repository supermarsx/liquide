use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, warn, error};

use liquide_session::config::{
    JailConfig, ResumeConfig, ResourceLimits, SessionConfig, SupervisorConfig,
};
use liquide_session::runtime::SessionRuntime;
use liquide_session::state::SessionState;

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

    /// Start the session in safe mode with non-essential features disabled.
    #[arg(long)]
    safe_mode: bool,

    /// Path to a TOML configuration file.
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
    let session_id = cli.session_id.unwrap_or_else(uuid_stub);

    info!(
        session_id = %session_id,
        dev_mode = cli.dev_mode,
        safe_mode = cli.safe_mode,
        "Starting liquid-session"
    );

    if cli.dev_mode {
        warn!("Developer mode is enabled — security checks are relaxed");
    }

    // Load configuration (a real implementation would read from the TOML file).
    let session_config = SessionConfig::default();
    let supervisor_config = SupervisorConfig::default();
    let resource_limits = ResourceLimits::default();
    let resume_config = ResumeConfig::default();
    let jail_config = JailConfig::default();

    // Create the session runtime.
    let mut runtime = SessionRuntime::new(
        session_id.clone(),
        session_config,
        supervisor_config,
        resource_limits,
        resume_config,
        jail_config,
        cli.safe_mode,
    );

    // Initialize: authenticate, set up sandbox, start workers.
    info!("Initializing session runtime...");
    runtime
        .initialize()
        .context("Failed to initialize session runtime")?;

    info!(
        state = %runtime.state(),
        safe_mode = runtime.is_safe_mode(),
        "Session initialized — entering event loop"
    );

    // Drain and log any initialization audit events.
    for event in runtime.drain_audit_events() {
        info!(event = event.event_name(), "audit: {:?}", event);
    }

    // Enter the main event loop.
    let heartbeat_interval =
        tokio::time::Duration::from_secs(5);
    let mut heartbeat_tick = tokio::time::interval(heartbeat_interval);

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal — tearing down session");
                break;
            }
            _ = heartbeat_tick.tick() => {
                if runtime.state() == SessionState::Running {
                    runtime.tick();

                    // Drain tick audit events.
                    for event in runtime.drain_audit_events() {
                        match event.level() {
                            liquide_session::audit::AuditLevel::Error => {
                                error!(event = event.event_name(), "{:?}", event);
                            }
                            liquide_session::audit::AuditLevel::Warn => {
                                warn!(event = event.event_name(), "{:?}", event);
                            }
                            _ => {
                                info!(event = event.event_name(), "{:?}", event);
                            }
                        }
                    }
                }
            }
        }
    }

    info!("Session terminated");
    Ok(())
}

/// Generate a placeholder session ID when none is provided.
fn uuid_stub() -> String {
    format!("session-{:08x}", std::process::id())
}
