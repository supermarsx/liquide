//! Configuration for standalone compositor mode.

/// Configuration for the standalone compositor.
#[derive(Debug, Clone)]
pub struct StandaloneConfig {
    /// Enable developer mode.
    pub dev_mode: bool,
    /// VT number to use (None = auto-allocate).
    pub vt_number: Option<u32>,
    /// DRM device path (None = auto-detect).
    pub drm_device: Option<String>,
    /// FPS cap (0 = VSYNC-limited).
    pub fps_cap: u32,
    /// Wayland socket name.
    pub wayland_socket: String,
    /// Enable XWayland for X11 apps.
    pub enable_xwayland: bool,
    /// Enable Wayland server.
    pub enable_wayland: bool,
}

impl Default for StandaloneConfig {
    fn default() -> Self {
        Self {
            dev_mode: false,
            vt_number: None,
            drm_device: None,
            fps_cap: 0,
            wayland_socket: "wayland-0".to_string(),
            enable_xwayland: true,
            enable_wayland: true,
        }
    }
}
