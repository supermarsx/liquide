use anyhow::{Context, Result};
use clap::Parser;
use tracing::{error, info, warn};

use liquide_session::config::{
    JailConfig, ResourceLimits, ResumeConfig, SessionConfig, SupervisorConfig,
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

    // Install the panic hook AFTER the subscriber so panic diagnostics
    // (message + file:line:col + thread) are emitted through tracing (H1).
    // This fires under both panic=abort and panic=unwind; the per-thread
    // catch_unwind boundaries (e.g. the render worker) are the survival half
    // and only become load-bearing under panic=unwind (see install_panic_hook).
    liquide_session::install_panic_hook();

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

    // Resolve the authenticated session principal. In a fully wired login flow
    // this comes from the supervisor/login handshake; until that lands, fall
    // back to the OS user so the audit trail and authorization subject are
    // attributed to a real principal rather than an empty string.
    let principal = session_principal();

    // Create the session runtime bound to the principal, then construct the
    // authorization + audit plane (t67-authz-wire): one `AuthorizationRuntime`
    // writing to the platform-default audit file, with the session principal's
    // Subject. This is the production consumer that turns the authz/audit planes
    // from tested-only into driven-in-production.
    let mut runtime = SessionRuntime::with_principal(
        session_id.clone(),
        principal,
        session_config,
        supervisor_config,
        resource_limits,
        resume_config,
        jail_config,
        cli.safe_mode,
    )
    .with_authz(current_uid(), std::process::id());

    info!(
        audit_path = ?runtime.authz().map(|a| a.audit_path().to_path_buf()),
        "Authorization + audit plane wired to shared audit file"
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

    // Persist initialization session-lifecycle audit events to the shared
    // append-only audit file (spec §3.6) — the live consumer of
    // `drain_audit_events_to`. Falls back to log-only drain if no plane is
    // attached or the sink write fails (the unrecorded tail is preserved).
    match runtime.drain_session_audit_to_sink() {
        Ok(Some(n)) => info!(recorded = n, "drained init audit events to shared audit file"),
        Ok(None) => {
            for event in runtime.drain_audit_events() {
                info!(event = event.event_name(), "audit: {:?}", event);
            }
        }
        Err(err) => {
            warn!(error = %err, "failed to write init audit events to shared sink");
            for event in runtime.drain_audit_events() {
                info!(event = event.event_name(), "audit: {:?}", event);
            }
        }
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
        run_desktop(
            cli.width,
            cli.height,
            cli.fps_cap,
            cli.debug_perf,
            cli.dev_mode,
        )
    }
}

/// Run the desktop compositor with a real platform backend.
///
/// This creates the platform backend (Win32, X11, Wayland, or macOS),
/// instantiates the desktop compositor with the shell, and enters the
/// blocking event loop.
fn run_desktop(
    width: u32,
    height: u32,
    fps_cap: u32,
    debug_perf: bool,
    dev_mode: bool,
) -> Result<()> {
    let mut platform =
        liquide_platform::create_platform().context("Failed to create platform backend")?;

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

                    // Persist tick audit events to the shared audit file
                    // (spec §3.6). If no plane is attached / the sink fails,
                    // fall back to log-only drain.
                    match runtime.drain_session_audit_to_sink() {
                        Ok(Some(_)) => {}
                        Ok(None) => drain_audit_to_log(runtime),
                        Err(err) => {
                            warn!(error = %err, "failed to write tick audit events to shared sink");
                            drain_audit_to_log(runtime);
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

/// Resolve the authenticated session principal for the audit subject.
///
/// A complete login flow would thread this from the supervisor handshake; until
/// then, fall back to the OS user (`USERNAME` on Windows, `USER`/`LOGNAME` on
/// unix) so the audit trail and authorization subject are attributed to a real
/// principal rather than an empty string.
fn session_principal() -> String {
    let var = if cfg!(windows) { "USERNAME" } else { "USER" };
    std::env::var(var)
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Resolve the current numeric uid for the authorization subject.
///
/// On unix this is the real uid; on Windows (no numeric uid) we use the process
/// id as a stable, non-zero session-scoped identifier (the principal string
/// carries the real account identity for credential verification).
fn current_uid() -> u32 {
    #[cfg(unix)]
    {
        // SAFETY: `getuid` is always safe; it has no preconditions and cannot fail.
        unsafe { libc_getuid() }
    }
    #[cfg(not(unix))]
    {
        std::process::id()
    }
}

/// Minimal `getuid` shim so the session binary does not pull in the full `libc`
/// crate just for one call. `geteuid`/`getuid` take no arguments and return the
/// uid; this matches the C ABI on every supported unix.
#[cfg(unix)]
unsafe fn libc_getuid() -> u32 {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() }
}

/// Fallback: drain session audit events to the tracing log (used when no authz
/// plane is attached or the shared sink write failed).
fn drain_audit_to_log(runtime: &mut SessionRuntime) {
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
