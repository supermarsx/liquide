//! Standalone compositor launcher — coordinates all subsystems.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::config::StandaloneConfig;
use crate::display::{DisplayOutput, OutputInfo};
use crate::event_loop::{EventLoop, EventLoopConfig};
use crate::wayland::WaylandServerState;
use liquide_drm::DrmDevice;
use liquide_logind::{VirtualTerminal, VtMode, Privileges};
use liquide_libinput::EvdevEnumerator;
use liquide_wayland_server::WaylandDisplay;
use liquide_xwayland::{XWaylandProcess, XWaylandConfig};
use tracing::{debug, info, warn};

/// The standalone compositor launcher.
pub struct StandaloneLauncher {
    config: StandaloneConfig,
    vt: Option<VirtualTerminal>,
    drm: Option<DrmDevice>,
    wayland: Option<WaylandDisplay>,
    xwayland: Option<XWaylandProcess>,
    display_output: DisplayOutput,
    wayland_state: WaylandServerState,
    running: Arc<AtomicBool>,
}

impl StandaloneLauncher {
    /// Create a new launcher with the given configuration.
    pub fn new(config: StandaloneConfig) -> Self {
        Self {
            config,
            vt: None,
            drm: None,
            wayland: None,
            xwayland: None,
            display_output: DisplayOutput::new(),
            wayland_state: WaylandServerState::new(),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Phase 1: Set up session and VT.
    pub fn setup_session(&mut self) -> anyhow::Result<()> {
        info!("setting up session management");

        // Set up environment variables.
        let uid = Privileges::effective_uid();
        let env = Privileges::setup_environment(uid);
        for (key, value) in &env {
            info!(key = %key, value = %value, "environment");
        }

        // Allocate / open VT.
        let vt = if let Some(vt_num) = self.config.vt_number {
            VirtualTerminal::open(vt_num)?
        } else {
            VirtualTerminal::allocate_next()?
        };
        info!(vt = vt.vt_number(), "VT allocated");
        self.vt = Some(vt);

        Ok(())
    }

    /// Phase 2: Set up DRM/KMS display output.
    pub fn setup_display(&mut self) -> anyhow::Result<()> {
        info!("setting up DRM/KMS display output");

        let drm = if let Some(ref path) = self.config.drm_device {
            DrmDevice::open(path)?
        } else {
            DrmDevice::find_primary()?
        };
        info!("DRM device opened");

        // Switch VT to graphics mode.
        if let Some(ref mut vt) = self.vt {
            vt.set_mode(VtMode::Graphics)?;
            info!("VT switched to graphics mode");
        }

        self.drm = Some(drm);
        Ok(())
    }

    /// Phase 3: Set up input devices.
    pub fn setup_input(&mut self) -> anyhow::Result<()> {
        info!("enumerating input devices");

        let enumerator = EvdevEnumerator::new();
        let devices = enumerator.scan()?;

        let keyboards = devices.iter().filter(|d| d.device_class == liquide_libinput::DeviceClass::Keyboard).count();
        let pointers = devices.iter().filter(|d| matches!(d.device_class, liquide_libinput::DeviceClass::Mouse | liquide_libinput::DeviceClass::Touchpad)).count();

        info!(
            total = devices.len(),
            keyboards = keyboards,
            pointers = pointers,
            "input devices enumerated"
        );

        Ok(())
    }

    /// Phase 4: Set up Wayland server.
    pub fn setup_wayland(&mut self) -> anyhow::Result<()> {
        info!(socket = %self.config.wayland_socket, "starting Wayland server");

        let mut display = WaylandDisplay::with_socket(&self.config.wayland_socket);
        display.bind()?;

        info!("Wayland server listening");
        self.wayland = Some(display);
        Ok(())
    }

    /// Phase 5: Set up XWayland.
    pub fn setup_xwayland(&mut self) -> anyhow::Result<()> {
        info!("starting XWayland");

        let mut xwl = XWaylandProcess::new(XWaylandConfig::default());
        xwl.start()?;

        info!(display = xwl.display_number(), "XWayland running");
        self.xwayland = Some(xwl);
        Ok(())
    }

    /// Phase 6: Run the compositor event loop.
    ///
    /// This is the main loop that:
    /// - Polls DRM for page flip events
    /// - Polls input devices for keyboard/mouse/touch
    /// - Polls Wayland server for client requests
    /// - Renders frames and presents to display
    pub fn run(&mut self) -> anyhow::Result<()> {
        self.running.store(true, Ordering::Release);
        info!("standalone compositor running");

        info!(
            drm = self.drm.is_some(),
            wayland = self.wayland.is_some(),
            xwayland = self.xwayland.is_some(),
            "subsystem status"
        );

        // ── Determine target refresh rate from primary output ───────
        let refresh_hz = self
            .display_output
            .primary()
            .map(|o| o.mode.refresh_hz)
            .unwrap_or(60);

        let mut loop_config = EventLoopConfig::with_refresh_hz(refresh_hz);
        loop_config.drm_fd = self.drm.as_ref().map(|d| d.fd());
        loop_config.wayland_active = self.wayland.as_ref().map_or(false, |w| w.is_running());

        info!(
            refresh_hz,
            frame_interval_ms = loop_config.frame_interval.as_millis(),
            "frame pacing configured"
        );

        // ── Create event loop ───────────────────────────────────────
        let (mut event_loop, running_flag) = EventLoop::new(loop_config)
            .map_err(|e| anyhow::anyhow!("failed to create event loop: {e}"))?;

        // Store a reference so signal handlers / external code can stop us.
        self.running = running_flag;

        // ── Register fds ────────────────────────────────────────────
        if let Some(ref drm) = self.drm {
            if let Err(e) = event_loop.register_drm(drm.fd()) {
                warn!(%e, "failed to register DRM fd — pageflip events unavailable");
            }
        }

        // ── Run ─────────────────────────────────────────────────────
        let result = event_loop.run(
            // on_input: process input events
            || {
                debug!("processing input events");
                // In a full implementation this would read from libinput and
                // dispatch to the compositor input router. Return true if any
                // input produced visual damage (e.g. cursor move, window focus).
                false
            },
            // on_wayland: process Wayland client events
            || {
                debug!("processing Wayland client events");
                // In a full implementation this would accept new clients and
                // process protocol requests. Return true if surface content changed.
                false
            },
            // on_render: render and present a frame
            || {
                // The actual rendering delegates to the compositor + renderer.
                // Return true if a frame was successfully submitted for scanout.
                debug!("rendering frame");
                true
            },
        );

        if let Err(e) = result {
            anyhow::bail!("event loop error: {e}");
        }

        self.running.store(false, Ordering::Release);
        Ok(())
    }

    /// Whether the compositor is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

impl Drop for StandaloneLauncher {
    fn drop(&mut self) {
        // Signal the event loop to stop if it's still running.
        self.running.store(false, Ordering::Release);

        // Stop XWayland first.
        if let Some(ref mut xwl) = self.xwayland {
            let _ = xwl.stop();
        }
        // Shut down Wayland server.
        if let Some(ref mut display) = self.wayland {
            display.shutdown();
        }
        // Release DRM master before closing the device.
        if let Some(ref mut drm) = self.drm {
            if drm.is_master() {
                if let Err(e) = drm.drop_master() {
                    warn!(%e, "failed to release DRM master");
                }
            }
        }
        // Restore VT mode.
        if let Some(ref mut vt) = self.vt {
            let _ = vt.set_mode(VtMode::Text);
        }
        info!("standalone compositor cleaned up");
    }
}
