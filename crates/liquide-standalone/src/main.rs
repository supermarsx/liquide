//! Standalone compositor binary for the LiquiDE desktop environment.
//!
//! Launches the full desktop environment directly from a TTY with
//! DRM/KMS output, raw evdev input, and Wayland protocol support for
//! client applications.
//!
//! Usage: liquid-standalone [OPTIONS]
//!
//! This binary is the TTY entry point. The existing `liquid-session`
//! binary continues to work for the remote desktop path.

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, warn};

/// Standalone LiquiDE compositor - launches from TTY.
///
/// Provides DRM/KMS display output, raw evdev input, and Wayland
/// protocol support. This is the local compositor path — the remote
/// desktop path uses `liquid-session` + `liquidclient` instead.
#[derive(Parser, Debug)]
#[command(name = "liquid-standalone", version, about)]
struct Cli {
    /// Enable developer mode with additional diagnostics.
    #[arg(long)]
    dev_mode: bool,

    /// VT number to use (0 = auto-allocate).
    #[arg(long, default_value = "0")]
    vt: u32,

    /// DRM device path (empty = auto-detect).
    #[arg(long, default_value = "")]
    drm_device: String,

    /// Maximum frames per second (0 = VSYNC-limited).
    #[arg(long, default_value = "0")]
    fps_cap: u32,

    /// Wayland socket name (default: wayland-0).
    #[arg(long, default_value = "wayland-0")]
    wayland_socket: String,

    /// Enable XWayland for X11 application support.
    #[arg(long, default_value = "true")]
    xwayland: bool,

    /// Disable the Wayland server (shell-only mode for testing).
    #[arg(long)]
    no_wayland: bool,

    /// Window width in pixels (windowed/dev mode). Unset = derive from output.
    #[arg(long)]
    width: Option<u32>,

    /// Window height in pixels (windowed/dev mode). Unset = derive from output.
    #[arg(long)]
    height: Option<u32>,

    /// Log to file instead of stderr.
    #[arg(long)]
    log_file: Option<String>,
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

    info!("LiquiDE Standalone Compositor starting");

    if cli.dev_mode {
        warn!("Developer mode enabled");
    }

    run(cli).await
}

async fn run(cli: Cli) -> Result<()> {
    use liquide_standalone::config::StandaloneConfig;
    use liquide_standalone::launcher::StandaloneLauncher;

    let config = StandaloneConfig {
        dev_mode: cli.dev_mode,
        vt_number: if cli.vt == 0 { None } else { Some(cli.vt) },
        drm_device: if cli.drm_device.is_empty() {
            None
        } else {
            Some(cli.drm_device)
        },
        fps_cap: cli.fps_cap,
        wayland_socket: cli.wayland_socket,
        enable_xwayland: cli.xwayland,
        enable_wayland: !cli.no_wayland,
        width: cli.width,
        height: cli.height,
    };

    let mut launcher = StandaloneLauncher::new(config.clone());

    // Phase 1: Session & VT setup
    info!("Phase 1: Session setup");
    launcher
        .setup_session()
        .context("Failed to set up session/VT")?;

    // Phases 2 & 3 (DRM/KMS display + evdev input) belong to the production
    // DRM path. In dev/windowed mode they are skipped entirely: the host-window
    // backend (Win32 / X11 / Wayland / Cocoa) provides window, input, and
    // present, and requesting a DRM device on a host OS would fail (e.g.
    // "no suitable DRM device found" on Windows).
    if config.dev_mode {
        info!("Dev mode: skipping DRM/KMS and evdev setup (host-window backend)");
    } else {
        // Phase 2: DRM/KMS initialization
        info!("Phase 2: DRM/KMS setup");
        launcher
            .setup_display()
            .context("Failed to set up DRM/KMS display")?;

        // Phase 3: Input device enumeration
        info!("Phase 3: Input setup");
        launcher
            .setup_input()
            .context("Failed to set up input devices")?;
    }

    // Phases 4 & 5 (Wayland server + XWayland) also belong to the production
    // DRM path: they serve external Wayland/X11 clients against the compositor
    // and are unsupported on a host OS (e.g. "Failed to set up Wayland server:
    // not supported on this platform" on Windows). In dev/windowed mode they
    // are skipped — the dev window hosts the desktop shell for inspection, not
    // a client-serving session.
    if config.dev_mode {
        info!("Dev mode: skipping Wayland/XWayland server setup (host-window backend)");
    } else {
        // Phase 4: Wayland server (if enabled)
        if config.enable_wayland {
            info!("Phase 4: Wayland server setup");
            launcher
                .setup_wayland()
                .context("Failed to set up Wayland server")?;
        }

        // Phase 5: XWayland (if enabled)
        if config.enable_xwayland && config.enable_wayland {
            info!("Phase 5: XWayland setup");
            launcher
                .setup_xwayland()
                .context("Failed to set up XWayland")?;
        }
    }

    // Phase 6: Run the compositor event loop
    info!("Phase 6: Entering compositor event loop");
    launcher.run().context("Compositor event loop failed")?;

    info!("LiquiDE Standalone Compositor shut down cleanly");
    Ok(())
}
