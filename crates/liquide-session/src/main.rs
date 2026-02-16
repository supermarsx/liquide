use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, warn, error};

use liquide_session::config::{
    JailConfig, ResumeConfig, ResourceLimits, SessionConfig, SupervisorConfig,
};
use liquide_session::desktop::DesktopCompositor;
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

    /// Initial window width in pixels.
    #[arg(long, default_value = "1280")]
    width: u32,

    /// Initial window height in pixels.
    #[arg(long, default_value = "720")]
    height: u32,

    /// Run in headless mode without creating a window.
    #[arg(long)]
    headless: bool,

    /// Maximum frames per second (0 = unlimited).
    #[arg(long, default_value = "60")]
    fps_cap: u32,

    /// Enable per-frame performance timing in the logs (use RUST_LOG=debug).
    #[arg(long)]
    debug_perf: bool,
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

    // Load configuration.
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
        "Session initialized"
    );

    // Drain and log any initialization audit events.
    for event in runtime.drain_audit_events() {
        info!(event = event.event_name(), "audit: {:?}", event);
    }

    if cli.headless {
        // Headless mode: run the session runtime event loop without
        // creating a window (useful for testing / CI).
        info!("Running in headless mode — no window will be created");
        run_headless(&mut runtime).await
    } else {
        // Desktop mode: create a platform backend, a desktop compositor
        // with full shell integration, and run the blocking event loop.
        info!(
            width = cli.width,
            height = cli.height,
            fps_cap = cli.fps_cap,
            debug_perf = cli.debug_perf,
            "Launching desktop compositor"
        );
        run_desktop(cli.width, cli.height, cli.fps_cap, cli.debug_perf, cli.dev_mode)
    }
}

/// Run the desktop compositor with a real platform backend.
///
/// This creates the platform backend (Win32, X11, Wayland, or macOS),
/// instantiates the desktop compositor with the shell, and enters the
/// blocking event loop.
fn run_desktop(width: u32, height: u32, fps_cap: u32, debug_perf: bool, dev_mode: bool) -> Result<()> {
    let mut platform = liquide_platform::create_platform()
        .context("Failed to create platform backend")?;

    info!(
        platform = platform.platform_name(),
        "Platform backend created"
    );

    let mut desktop = DesktopCompositor::new(width, height);
    desktop.set_fps_cap(fps_cap);
    desktop.set_debug_perf(debug_perf);
    desktop.set_dev_mode(dev_mode);

    info!("Entering desktop event loop");
    desktop.run(platform.as_mut());

    info!(
        frames = desktop.frame_count(),
        "Desktop compositor shut down"
    );
    Ok(())
}

/// Run in headless mode with just the session runtime heartbeat loop.
async fn run_headless(runtime: &mut SessionRuntime) -> Result<()> {
    let heartbeat_interval = tokio::time::Duration::from_secs(5);
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
