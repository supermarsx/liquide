//! Standalone compositor launcher — coordinates all subsystems.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::StandaloneConfig;
use crate::display::{DisplayOutput, OutputInfo};
use liquide_drm::{DrmDevice, enumerate_connectors};
use liquide_libinput::EvdevEnumerator;
use liquide_logind::{Privileges, VirtualTerminal, VtMode};
use liquide_platform::PlatformBackend;
use liquide_platform::standalone::{
    StandaloneConfig as StandalonePlatformConfig, StandalonePlatform, StandalonePresentMode,
    StandaloneScriptHandle,
};
use liquide_session::desktop::DesktopCompositor;
use liquide_wayland_server::WaylandDisplay;
use liquide_xwayland::{XWaylandConfig, XWaylandProcess};
use tracing::{info, warn};

const DEFAULT_SURFACE_WIDTH: u32 = 1920;
const DEFAULT_SURFACE_HEIGHT: u32 = 1080;
const DEFAULT_REFRESH_HZ: u32 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StandaloneLaunchSummary {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) refresh_hz: u32,
    pub(crate) requested_fps_cap: u32,
    pub(crate) effective_fps_cap: u32,
    pub(crate) present_mode: StandalonePresentMode,
    pub(crate) live_present_feedback_capable: bool,
    pub(crate) refresh_sync_present_capable: bool,
    pub(crate) output_name: Option<String>,
    pub(crate) fallback_reason: StandaloneLaunchFallbackReason,
}

impl StandaloneLaunchSummary {
    fn log_surface_selection(&self) {
        match &self.fallback_reason.geometry {
            Some(StandaloneGeometryFallbackReason::NoOutputMetadata) => {
                warn!(
                    width = self.width,
                    height = self.height,
                    refresh_hz = self.refresh_hz,
                    "no standalone output metadata available; falling back to default desktop surface"
                );
            }
            _ => {
                if let Some(output_name) = self.output_name.as_deref() {
                    info!(
                        output = %output_name,
                        width = self.width,
                        height = self.height,
                        refresh_hz = self.refresh_hz,
                        "configured standalone desktop surface from output metadata"
                    );
                } else {
                    warn!(
                        width = self.width,
                        height = self.height,
                        refresh_hz = self.refresh_hz,
                        "no standalone output metadata available; falling back to default desktop surface"
                    );
                }
            }
        }
    }

    fn log_present_strategy(&self) {
        if self.fallback_reason.present_feedback.is_some() {
            warn!(
                requested_fps_cap = self.requested_fps_cap,
                fallback_pacing_hz = self.effective_fps_cap,
                "typed DRM present feedback unavailable; standalone desktop will use timer pacing"
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct StandaloneLaunchFallbackReason {
    pub(crate) geometry: Option<StandaloneGeometryFallbackReason>,
    pub(crate) present_feedback: Option<StandalonePresentFeedbackFallbackReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StandaloneGeometryFallbackReason {
    NoOutputMetadata,
    MissingModeFields {
        width_defaulted: bool,
        height_defaulted: bool,
        refresh_hz_defaulted: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StandalonePresentFeedbackFallbackReason {
    NoLiveFeedbackCapability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct StandalonePresentCapabilities {
    pub(crate) live_feedback: bool,
    pub(crate) refresh_sync: bool,
}

impl StandalonePresentCapabilities {
    const fn live_feedback(live_feedback: bool) -> Self {
        Self {
            live_feedback,
            refresh_sync: false,
        }
    }

    #[cfg(test)]
    const fn new(live_feedback: bool, refresh_sync: bool) -> Self {
        Self {
            live_feedback,
            refresh_sync,
        }
    }
}

/// Explicit width/height override for the initial compositor surface.
///
/// When set, these dimensions replace the DRM-derived (or 1920x1080 fallback)
/// surface size. Their primary purpose is to size the resizable dev-mode host
/// window, but they also override the fullscreen surface when supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct StandaloneSurfaceOverride {
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
}

impl StandaloneSurfaceOverride {
    /// Apply this override to a freshly-built launch summary in place.
    ///
    /// Each dimension is overridden independently; an unset (or zero) value
    /// preserves the existing DRM-derived / fallback dimension exactly.
    fn apply(self, summary: &mut StandaloneLaunchSummary) {
        if let Some(width) = self.width.filter(|width| *width > 0) {
            summary.width = width;
        }
        if let Some(height) = self.height.filter(|height| *height > 0) {
            summary.height = height;
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StandaloneLaunchRuntimeInputs {
    pub(crate) primary_output: Option<OutputInfo>,
    pub(crate) live_present_feedback_capable: bool,
    pub(crate) present_feedback_fd: Option<i32>,
    pub(crate) surface_override: StandaloneSurfaceOverride,
}

impl StandaloneLaunchRuntimeInputs {
    pub(crate) fn from_launcher(launcher: &StandaloneLauncher) -> Self {
        Self {
            primary_output: launcher.display_output.primary().cloned(),
            live_present_feedback_capable: launcher.live_present_feedback_capable(),
            present_feedback_fd: launcher.present_feedback_fd(),
            surface_override: StandaloneSurfaceOverride {
                width: launcher.config.width,
                height: launcher.config.height,
            },
        }
    }

    pub(crate) fn active_live_present_feedback_capability(&self) -> bool {
        self.live_present_feedback_capable && self.present_feedback_fd.is_some()
    }

    pub(crate) fn launch_summary(&self, requested_fps_cap: u32) -> StandaloneLaunchSummary {
        let mut summary = StandaloneLauncher::build_launch_plan_for_inputs(
            requested_fps_cap,
            self.primary_output.as_ref(),
            self.active_live_present_feedback_capability(),
        );
        // Apply the explicit width/height override (e.g. `--width/--height`)
        // last, so it wins over the DRM-derived / 1920x1080 fallback while
        // leaving every other field — refresh, present mode, fallback reason —
        // untouched. This overridden size feeds both the platform backend
        // config and `DesktopCompositor::new`, which becomes the dev-mode host
        // window size in `liquide-session`'s `event_loop.rs`.
        self.surface_override.apply(&mut summary);
        summary
    }

    fn drm_event_fd_for_summary(&self, summary: &StandaloneLaunchSummary) -> Option<i32> {
        match summary.present_mode {
            StandalonePresentMode::Queued => self.present_feedback_fd,
            StandalonePresentMode::Immediate => None,
        }
    }

    fn platform_config(
        &self,
        requested_fps_cap: u32,
    ) -> (StandaloneLaunchSummary, StandalonePlatformConfig) {
        let summary = self.launch_summary(requested_fps_cap);
        let config = StandalonePlatformConfig {
            width: summary.width,
            height: summary.height,
            hardware_cursor: true,
            present_mode: summary.present_mode,
            drm_event_fd: self.drm_event_fd_for_summary(&summary),
            // TODO: install a real DRM page-flip submitter once standalone
            // owns a scanned-out framebuffer id and selected CRTC at this
            // handoff. Today queued mode consumes real DRM feedback but the
            // platform still stores software pixels instead of issuing flips.
            #[cfg(target_os = "linux")]
            submitter: None,
        };
        (summary, config)
    }
}

/// Identifier for the host-window platform backend selected in dev/windowed
/// mode, by target OS. Used both to construct the backend and to assert the
/// selection in host-safe regression tests without opening a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Each variant is only constructed on its own target OS.
pub(crate) enum DevWindowBackend {
    /// Win32 / GDI host window (Windows).
    Win32,
    /// X11 host window (Linux). Chosen as the nested/dev backend because it is
    /// the self-contained windowed backend the codebase treats as the default
    /// host-window path; Wayland is used when no X server is reachable.
    X11,
    /// Wayland host window (Linux fallback when `$DISPLAY` is unavailable).
    Wayland,
    /// Cocoa host window (macOS).
    MacOS,
}

impl DevWindowBackend {
    /// The host-window backend this target OS uses for dev/windowed mode.
    ///
    /// This is the selection made when `dev_mode` is active: it bypasses
    /// DRM/KMS and evdev entirely because the host-window backend already
    /// provides window + input + present.
    pub(crate) const fn for_target() -> Self {
        #[cfg(windows)]
        {
            Self::Win32
        }
        #[cfg(target_os = "linux")]
        {
            Self::X11
        }
        #[cfg(target_os = "macos")]
        {
            Self::MacOS
        }
        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        {
            compile_error!("no host-window backend available for this target OS")
        }
    }
}

/// Construct the host-window platform backend for dev/windowed mode.
///
/// In dev mode the DRM display setup (`setup_display`) and evdev input setup
/// (`setup_input`) are skipped entirely — those belong to the production DRM
/// path. The host-window backend created here supplies window creation, input,
/// and frame presentation through the host OS windowing system, so the
/// `DesktopCompositor::run` event loop drives it the same way it drives the
/// standalone backend. The window itself is created lazily by the event loop
/// (`platform.window_host().create_window(...)`) at the requested geometry.
fn create_dev_window_backend() -> anyhow::Result<Box<dyn PlatformBackend>> {
    #[cfg(windows)]
    {
        let platform = liquide_platform::Win32Platform::new()
            .map_err(|error| anyhow::anyhow!("failed to create Win32 host backend: {error}"))?;
        Ok(Box::new(platform))
    }
    #[cfg(target_os = "linux")]
    {
        // Prefer X11 (the self-contained nested/dev host-window backend); fall
        // back to Wayland when no X server is reachable.
        match liquide_platform::X11Platform::new() {
            Ok(platform) => Ok(Box::new(platform)),
            Err(x11_error) => {
                warn!(%x11_error, "X11 host backend unavailable; falling back to Wayland");
                let platform = liquide_platform::WaylandPlatform::new().map_err(|wl_error| {
                    anyhow::anyhow!(
                        "failed to create host-window backend (X11: {x11_error}; Wayland: {wl_error})"
                    )
                })?;
                Ok(Box::new(platform))
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let platform = liquide_platform::MacOSPlatform::new()
            .map_err(|error| anyhow::anyhow!("failed to create macOS host backend: {error}"))?;
        Ok(Box::new(platform))
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        anyhow::bail!("no host-window backend available for this target OS")
    }
}

/// The standalone compositor launcher.
pub struct StandaloneLauncher {
    config: StandaloneConfig,
    vt: Option<VirtualTerminal>,
    drm: Option<DrmDevice>,
    wayland: Option<WaylandDisplay>,
    xwayland: Option<XWaylandProcess>,
    display_output: DisplayOutput,
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
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn build_launch_plan_for_inputs(
        requested_fps_cap: u32,
        primary_output: Option<&OutputInfo>,
        live_present_feedback_capable: bool,
    ) -> StandaloneLaunchSummary {
        Self::build_launch_plan_for_present_capabilities(
            requested_fps_cap,
            primary_output,
            StandalonePresentCapabilities::live_feedback(live_present_feedback_capable),
        )
    }

    pub(crate) fn build_launch_plan_for_present_capabilities(
        requested_fps_cap: u32,
        primary_output: Option<&OutputInfo>,
        present_capabilities: StandalonePresentCapabilities,
    ) -> StandaloneLaunchSummary {
        let width_defaulted = primary_output.map_or(true, |output| output.mode.width == 0);
        let height_defaulted = primary_output.map_or(true, |output| output.mode.height == 0);
        let refresh_hz_defaulted =
            primary_output.map_or(true, |output| output.mode.refresh_hz == 0);

        let width = primary_output
            .and_then(|output| (output.mode.width > 0).then_some(output.mode.width))
            .unwrap_or(DEFAULT_SURFACE_WIDTH);
        let height = primary_output
            .and_then(|output| (output.mode.height > 0).then_some(output.mode.height))
            .unwrap_or(DEFAULT_SURFACE_HEIGHT);
        let refresh_hz = primary_output
            .and_then(|output| (output.mode.refresh_hz > 0).then_some(output.mode.refresh_hz))
            .unwrap_or(DEFAULT_REFRESH_HZ);

        let live_present_feedback_capable = present_capabilities.live_feedback;
        let present_mode = if live_present_feedback_capable {
            StandalonePresentMode::Queued
        } else {
            StandalonePresentMode::Immediate
        };
        let effective_fps_cap = match present_mode {
            StandalonePresentMode::Queued => requested_fps_cap,
            StandalonePresentMode::Immediate if requested_fps_cap > 0 => requested_fps_cap,
            StandalonePresentMode::Immediate => refresh_hz,
        };

        let geometry = match primary_output {
            Some(_) if width_defaulted || height_defaulted || refresh_hz_defaulted => {
                Some(StandaloneGeometryFallbackReason::MissingModeFields {
                    width_defaulted,
                    height_defaulted,
                    refresh_hz_defaulted,
                })
            }
            Some(_) => None,
            None => Some(StandaloneGeometryFallbackReason::NoOutputMetadata),
        };

        StandaloneLaunchSummary {
            width,
            height,
            refresh_hz,
            requested_fps_cap,
            effective_fps_cap,
            present_mode,
            live_present_feedback_capable,
            refresh_sync_present_capable: present_capabilities.refresh_sync,
            output_name: primary_output.map(|output| output.name.clone()),
            fallback_reason: StandaloneLaunchFallbackReason {
                geometry,
                present_feedback: (!live_present_feedback_capable)
                    .then_some(StandalonePresentFeedbackFallbackReason::NoLiveFeedbackCapability),
            },
        }
    }

    fn current_runtime_inputs(&self) -> StandaloneLaunchRuntimeInputs {
        StandaloneLaunchRuntimeInputs::from_launcher(self)
    }

    fn live_present_feedback_capable(&self) -> bool {
        // A readable DRM fd is not enough to prove frame-correlated present
        // feedback. Keep queued pacing disabled until the launcher also owns
        // a real submitter that issues page flips or vblank waits for each
        // accepted frame.
        false
    }

    fn present_feedback_fd(&self) -> Option<i32> {
        self.drm.as_ref().map(|drm| drm.fd())
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

        self.display_output = match enumerate_connectors(&drm) {
            Ok(connectors) => {
                let display_output = DisplayOutput::from_connectors(&connectors);
                if let Some(primary_output) = display_output.primary() {
                    info!(
                        output = %primary_output.name,
                        width = primary_output.mode.width,
                        height = primary_output.mode.height,
                        refresh_hz = primary_output.mode.refresh_hz,
                        usable_outputs = display_output.outputs().len(),
                        "discovered standalone DRM output metadata"
                    );
                } else {
                    warn!(
                        enumerated_connectors = connectors.len(),
                        "no usable standalone DRM output metadata discovered; retaining default launch fallback"
                    );
                }
                display_output
            }
            Err(error) => {
                warn!(
                    %error,
                    "failed to enumerate standalone DRM output metadata; retaining default launch fallback"
                );
                DisplayOutput::new()
            }
        };

        self.drm = Some(drm);
        Ok(())
    }

    /// Phase 3: Set up input devices.
    pub fn setup_input(&mut self) -> anyhow::Result<()> {
        info!("enumerating input devices");

        let enumerator = EvdevEnumerator::new();
        let devices = enumerator.scan()?;

        let keyboards = devices
            .iter()
            .filter(|d| d.device_class == liquide_libinput::DeviceClass::Keyboard)
            .count();
        let pointers = devices
            .iter()
            .filter(|d| {
                matches!(
                    d.device_class,
                    liquide_libinput::DeviceClass::Mouse | liquide_libinput::DeviceClass::Touchpad
                )
            })
            .count();

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
        self.run_with_observer(|_, _| {})
    }

    pub(crate) fn run_with_observer<F>(&mut self, observer: F) -> anyhow::Result<()>
    where
        F: FnOnce(StandaloneLaunchSummary, StandaloneScriptHandle),
    {
        self.run_with_runtime_inputs(self.current_runtime_inputs(), observer)
    }

    pub(crate) fn run_with_runtime_inputs<F>(
        &mut self,
        runtime_inputs: StandaloneLaunchRuntimeInputs,
        observer: F,
    ) -> anyhow::Result<()>
    where
        F: FnOnce(StandaloneLaunchSummary, StandaloneScriptHandle),
    {
        self.running.store(true, Ordering::Release);
        info!("standalone compositor running");

        info!(
            drm = self.drm.is_some(),
            wayland = self.wayland.is_some(),
            xwayland = self.xwayland.is_some(),
            "subsystem status"
        );

        let (launch_plan, platform_config) = runtime_inputs.platform_config(self.config.fps_cap);
        launch_plan.log_surface_selection();
        launch_plan.log_present_strategy();

        // Dev/windowed mode: bypass DRM/KMS and evdev entirely and run the
        // compositor inside a host-OS window (Win32 / X11 / Wayland / Cocoa).
        // `setup_display`/`setup_input` are skipped for this path (see
        // `main.rs`), so no DRM device or evdev enumeration is required —
        // exactly what makes `--dev-mode` work on Windows.
        if self.config.dev_mode {
            return self.run_dev_windowed(launch_plan);
        }

        let mut platform = StandalonePlatform::new(platform_config).map_err(|error| {
            self.running.store(false, Ordering::Release);
            anyhow::anyhow!("failed to create standalone platform backend: {error}")
        })?;

        let script_handle = platform.script_handle();

        let mut desktop = DesktopCompositor::new(launch_plan.width, launch_plan.height);
        desktop.set_dev_mode(self.config.dev_mode);
        desktop.set_fps_cap(launch_plan.effective_fps_cap);
        // Real standalone host: host-consumed Shut Down / Restart requests
        // perform real OS power calls (t73-session item 2).
        desktop.set_real_runtime(true);

        info!(
            width = launch_plan.width,
            height = launch_plan.height,
            refresh_hz = launch_plan.refresh_hz,
            fps_cap = launch_plan.effective_fps_cap,
            present_mode = ?launch_plan.present_mode,
            refresh_sync_present_capable = launch_plan.refresh_sync_present_capable,
            "starting standalone desktop handoff"
        );

        observer(launch_plan, script_handle);

        desktop.run(&mut platform);

        self.running.store(false, Ordering::Release);
        info!(
            frames = desktop.frame_count(),
            "standalone desktop compositor exited"
        );
        Ok(())
    }

    /// Run the compositor in a host-OS window (dev/windowed mode).
    ///
    /// Selects the host-window `PlatformBackend` for the target OS, then drives
    /// the standard `DesktopCompositor::run` loop. The geometry in `launch_plan`
    /// (carrying any `--width/--height` override) reaches the dev-mode window
    /// because it becomes `DesktopCompositor::new(width, height)` → the session
    /// event loop's resizable-window params.
    fn run_dev_windowed(&mut self, launch_plan: StandaloneLaunchSummary) -> anyhow::Result<()> {
        info!(
            backend = ?DevWindowBackend::for_target(),
            width = launch_plan.width,
            height = launch_plan.height,
            "dev mode: running compositor in host-OS window (DRM/evdev bypassed)"
        );

        let mut platform = create_dev_window_backend().inspect_err(|_| {
            self.running.store(false, Ordering::Release);
        })?;

        let mut desktop = DesktopCompositor::new(launch_plan.width, launch_plan.height);
        desktop.set_dev_mode(true);
        desktop.set_fps_cap(launch_plan.effective_fps_cap);
        // Real standalone host (windowed dev): host-consumed power requests
        // perform real OS power calls (t73-session item 2).
        desktop.set_real_runtime(true);

        info!(
            width = launch_plan.width,
            height = launch_plan.height,
            fps_cap = launch_plan.effective_fps_cap,
            "starting standalone desktop handoff (windowed)"
        );

        desktop.run(platform.as_mut());

        self.running.store(false, Ordering::Release);
        info!(
            frames = desktop.frame_count(),
            "standalone windowed compositor exited"
        );
        Ok(())
    }

    /// Whether the compositor is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod t47_e8_tests {
    use super::*;
    use liquide_drm::{DrmMode, ModeFlags};

    fn output(name: &str, width: u32, height: u32, refresh_hz: u32) -> OutputInfo {
        OutputInfo {
            connector_id: 1,
            name: name.to_string(),
            mode: DrmMode {
                width,
                height,
                refresh_hz,
                clock_khz: 0,
                flags: ModeFlags::CURRENT,
                name: format!("{width}x{height}@{refresh_hz}"),
            },
            physical_width_mm: 600,
            physical_height_mm: 340,
            primary: true,
        }
    }

    #[test]
    fn refresh_sync_capability_is_reported_without_enabling_queued_feedback() {
        let output = output("WIN32-1", 3840, 2160, 120);
        let summary = StandaloneLauncher::build_launch_plan_for_present_capabilities(
            0,
            Some(&output),
            StandalonePresentCapabilities::new(false, true),
        );

        assert_eq!(summary.width, 3840);
        assert_eq!(summary.height, 2160);
        assert_eq!(summary.refresh_hz, 120);
        assert_eq!(summary.effective_fps_cap, 120);
        assert_eq!(summary.present_mode, StandalonePresentMode::Immediate);
        assert!(!summary.live_present_feedback_capable);
        assert!(summary.refresh_sync_present_capable);
        assert_eq!(summary.output_name.as_deref(), Some("WIN32-1"));
        assert_eq!(
            summary.fallback_reason.present_feedback,
            Some(StandalonePresentFeedbackFallbackReason::NoLiveFeedbackCapability)
        );
    }

    #[test]
    fn live_feedback_capability_still_selects_queued_mode() {
        let output = output("DRM-1", 2560, 1440, 144);
        let summary = StandaloneLauncher::build_launch_plan_for_present_capabilities(
            0,
            Some(&output),
            StandalonePresentCapabilities::new(true, true),
        );

        assert_eq!(summary.present_mode, StandalonePresentMode::Queued);
        assert!(summary.live_present_feedback_capable);
        assert!(summary.refresh_sync_present_capable);
        assert!(summary.fallback_reason.present_feedback.is_none());
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
