use anyhow::Result;
use clap::Parser;
use tracing::{info, warn, error};

use liquide_supervisor::{
    AdmissionConfig, ControlCommand, ControlResponse, DowngradeThresholds,
    ResourceDefaults, RestartPolicy, SupervisorConfig,
};
use liquide_supervisor::runtime::SupervisorRuntime;

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

    /// Override the number of CPU cores available (for testing).
    #[arg(long, default_value = "8")]
    host_cpu_cores: f64,

    /// Override the memory in megabytes available (for testing).
    #[arg(long, default_value = "32768")]
    host_memory_mb: u64,
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

    // Load configuration (a real implementation would parse the TOML file).
    info!(path = %cli.config, "Loading configuration...");
    let mut supervisor_config = SupervisorConfig::default();
    supervisor_config.dev_mode = cli.dev_mode;

    let resource_defaults = ResourceDefaults::default();
    let admission_config = AdmissionConfig::default();
    let downgrade_thresholds = DowngradeThresholds::default();
    let restart_policy = RestartPolicy::default();

    // Create the supervisor runtime.
    let mut runtime = SupervisorRuntime::new(
        supervisor_config,
        resource_defaults,
        admission_config,
        downgrade_thresholds,
        restart_policy,
        cli.host_cpu_cores,
        cli.host_memory_mb,
    );

    info!(
        socket = %runtime.control_channel().socket_path(),
        "Opening control socket..."
    );
    info!(
        host_cpu = cli.host_cpu_cores,
        host_memory_mb = cli.host_memory_mb,
        "Supervisor ready — listening for connections"
    );

    // Drain and log startup audit events.
    for event in runtime.drain_audit_events() {
        info!(event = event.event_name(), "audit: {:?}", event);
    }

    // Enter the main event loop.
    let heartbeat_interval = tokio::time::Duration::from_secs(5);
    let mut heartbeat_tick = tokio::time::interval(heartbeat_interval);

    // Consume the first immediate tick.
    heartbeat_tick.tick().await;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal — stopping supervisor");
                let resp = runtime.handle_control_command(ControlCommand::Shutdown);
                if let ControlResponse::Error(msg) = resp {
                    error!(error = %msg, "Error during shutdown");
                }
                break;
            }
            _ = heartbeat_tick.tick() => {
                runtime.tick();

                // Drain and log audit events from the tick.
                for event in runtime.drain_audit_events() {
                    match event.level() {
                        liquide_supervisor::AuditLevel::Error => {
                            error!(event = event.event_name(), "{:?}", event);
                        }
                        liquide_supervisor::AuditLevel::Warn => {
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

    let status = runtime.status();
    info!(
        active_sessions = status.active_sessions,
        uptime_sec = status.uptime_sec,
        "Supervisor stopped"
    );

    Ok(())
}
