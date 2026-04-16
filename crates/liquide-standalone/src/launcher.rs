//! Standalone compositor launcher — coordinates all subsystems.

use crate::config::StandaloneConfig;
use liquide_drm::{DrmDevice, DrmError};
use liquide_logind::{VirtualTerminal, VtMode, Privileges, StubSeat};
use liquide_logind::session::StubSession;
use liquide_libinput::EvdevEnumerator;
use liquide_wayland_server::WaylandDisplay;
use liquide_xwayland::{XWaylandProcess, XWaylandConfig};
use tracing::{info, warn};

/// The standalone compositor launcher.
pub struct StandaloneLauncher {
    config: StandaloneConfig,
    vt: Option<VirtualTerminal>,
    drm: Option<DrmDevice>,
    wayland: Option<WaylandDisplay>,
    xwayland: Option<XWaylandProcess>,
    running: bool,
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
            running: false,
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
        self.running = true;
        info!("standalone compositor running");

        // The event loop will be implemented to integrate with the
        // existing DesktopCompositor from liquide-session. For now,
        // log the successful initialization.
        info!("standalone compositor initialized — event loop ready");
        info!(
            drm = self.drm.is_some(),
            wayland = self.wayland.is_some(),
            xwayland = self.xwayland.is_some(),
            "subsystem status"
        );

        // TODO: Integrate with DesktopCompositor event loop.
        // The plan is to create a StandalonePlatform that implements
        // PlatformBackend using DRM for output and evdev for input,
        // then pass it to DesktopCompositor::run() just like the
        // existing X11/Wayland/Win32 backends.

        self.running = false;
        Ok(())
    }

    /// Whether the compositor is currently running.
    pub fn is_running(&self) -> bool {
        self.running
    }
}

impl Drop for StandaloneLauncher {
    fn drop(&mut self) {
        // Stop XWayland first.
        if let Some(ref mut xwl) = self.xwayland {
            let _ = xwl.stop();
        }
        // Shut down Wayland server.
        if let Some(ref mut display) = self.wayland {
            display.shutdown();
        }
        // Restore VT mode.
        if let Some(ref mut vt) = self.vt {
            let _ = vt.set_mode(VtMode::Text);
        }
        info!("standalone compositor cleaned up");
    }
}
