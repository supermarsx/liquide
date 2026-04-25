//! Cross-platform desktop environment abstraction layer.
//!
//! This crate defines trait interfaces for every major platform integration
//! point (displays, windows, taskbar, tray, notifications, drag-and-drop,
//! keymap translation, event loops, and frame presentation) together with
//! null / default implementations for tests or headless environments.
//!
//! Real platform backends are compiled conditionally:
//!
//! - **Windows**: `win32` module — Win32 / GDI via raw FFI.
//! - **Linux (X11)**: `x11` module — Xlib via raw FFI.
//! - **Linux (Wayland)**: `wayland` module — libwayland-client via raw FFI.
//! - **macOS**: `macos` module — Cocoa / Core Graphics via Objective-C runtime FFI.

pub mod display;
pub mod dnd;
pub mod event_loop;
pub mod keymap;
pub mod notifications;
pub mod taskbar;
pub mod tray;
pub mod window_host;

// Platform-specific backends
#[cfg(target_os = "windows")]
pub mod win32;

#[cfg(target_os = "linux")]
pub mod x11;

#[cfg(target_os = "linux")]
pub mod wayland;

#[cfg(target_os = "macos")]
pub mod macos;

// Standalone compositor backend (DRM/KMS + evdev)
pub mod standalone;

// Re-exports — common types
pub use display::{DisplayBackend, MonitorInfo, NullDisplayBackend};
pub use dnd::{NativeDragDrop, NullDragDrop};
pub use event_loop::{ControlFlow, FramePresenter, NullFramePresenter, PlatformEvent};
pub use keymap::{DefaultKeymap, KeymapTranslator};
pub use notifications::{NativeNotificationParams, NativeNotifications, NullNativeNotifications};
pub use taskbar::{JumpListItem, NullTaskbar, TaskbarIntegration};
pub use tray::{NativeTray, NativeTrayHandle, NativeTrayParams, NullNativeTray, TrayUpdate};
pub use window_host::{NativeWindowHandle, NativeWindowHost, NativeWindowParams, NullWindowHost};

// Re-exports — platform backends
#[cfg(target_os = "windows")]
pub use win32::Win32Platform;

#[cfg(target_os = "linux")]
pub use x11::X11Platform;

#[cfg(target_os = "linux")]
pub use wayland::WaylandPlatform;

#[cfg(target_os = "macos")]
pub use macos::MacOSPlatform;

use liquide_compositor::pixel::PixelFormat;
use thiserror::Error;

/// System color scheme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorScheme {
    /// Light appearance.
    Light,
    /// Dark appearance.
    Dark,
}

impl ColorScheme {
    /// Returns `"light"` or `"dark"`.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

impl std::fmt::Display for ColorScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self::Light
    }
}

/// Query the operating system's preferred color scheme.
///
/// Platform detection strategy:
/// - **Windows**: reads `AppsUseLightTheme` from the registry.
/// - **macOS**: checks the `AppleInterfaceStyle` user default.
/// - **Linux**: reads the `GTK_THEME` environment variable for a `-dark` suffix,
///   or the `color-scheme` XDG portal setting.
/// - **Fallback**: returns [`ColorScheme::Light`].
#[must_use]
pub fn query_color_scheme() -> ColorScheme {
    platform_query_color_scheme()
}

#[cfg(target_os = "windows")]
fn platform_query_color_scheme() -> ColorScheme {
    // Read HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize
    // AppsUseLightTheme: DWORD (0 = dark, 1 = light)
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(
            key: isize,
            sub_key: *const u16,
            options: u32,
            sam: u32,
            result: *mut isize,
        ) -> i32;
        fn RegQueryValueExW(
            key: isize,
            value_name: *const u16,
            reserved: *mut u32,
            reg_type: *mut u32,
            data: *mut u8,
            data_len: *mut u32,
        ) -> i32;
        fn RegCloseKey(key: isize) -> i32;
    }

    const HKEY_CURRENT_USER: isize = -2147483647i32 as isize; // 0x80000001
    const KEY_READ: u32 = 0x20019;

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    let sub_key = to_wide("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let value_name = to_wide("AppsUseLightTheme");
    let mut hkey: isize = 0;

    // SAFETY: FFI call with valid pointers; hkey is written before read.
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, sub_key.as_ptr(), 0, KEY_READ, &mut hkey) != 0 {
            return ColorScheme::Light;
        }
        let mut data: u32 = 1;
        let mut data_len: u32 = 4;
        let mut reg_type: u32 = 0;
        let result = RegQueryValueExW(
            hkey,
            value_name.as_ptr(),
            std::ptr::null_mut(),
            &mut reg_type,
            &mut data as *mut u32 as *mut u8,
            &mut data_len,
        );
        RegCloseKey(hkey);
        if result != 0 {
            return ColorScheme::Light;
        }
        if data == 0 {
            ColorScheme::Dark
        } else {
            ColorScheme::Light
        }
    }
}

#[cfg(target_os = "macos")]
fn platform_query_color_scheme() -> ColorScheme {
    // `defaults read -g AppleInterfaceStyle` returns "Dark" when dark mode is on.
    match std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim().eq_ignore_ascii_case("dark") {
                ColorScheme::Dark
            } else {
                ColorScheme::Light
            }
        }
        Err(_) => ColorScheme::Light,
    }
}

#[cfg(target_os = "linux")]
fn platform_query_color_scheme() -> ColorScheme {
    // 1. Check GTK_THEME env for a "-dark" suffix (e.g. "Adwaita-dark").
    if let Ok(theme) = std::env::var("GTK_THEME") {
        if theme.to_ascii_lowercase().contains("-dark")
            || theme.to_ascii_lowercase().contains(":dark")
        {
            return ColorScheme::Dark;
        }
    }
    // 2. Try the XDG Desktop Portal color-scheme setting via dbus-send.
    //    org.freedesktop.appearance color-scheme: 0 = default, 1 = dark, 2 = light
    if let Ok(output) = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply=literal",
            "--dest=org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings.Read",
            "string:org.freedesktop.appearance",
            "string:color-scheme",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // The output contains a variant with a uint32 value.
        // Look for the value "1" which means prefer dark.
        if stdout.contains("uint32 1") {
            return ColorScheme::Dark;
        }
    }
    ColorScheme::Light
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn platform_query_color_scheme() -> ColorScheme {
    ColorScheme::Light
}

/// Errors that can occur within the platform abstraction layer.
#[derive(Debug, Error)]
pub enum PlatformError {
    /// A display / monitor operation failed.
    #[error("display error: {0}")]
    Display(String),

    /// A window operation failed.
    #[error("window error: {0}")]
    Window(String),

    /// A taskbar integration operation failed.
    #[error("taskbar error: {0}")]
    Taskbar(String),

    /// A system tray operation failed.
    #[error("tray error: {0}")]
    Tray(String),

    /// A notification operation failed.
    #[error("notification error: {0}")]
    Notification(String),

    /// A drag-and-drop operation failed.
    #[error("drag-drop error: {0}")]
    DragDrop(String),

    /// A keymap translation operation failed.
    #[error("keymap error: {0}")]
    Keymap(String),

    /// An event loop error.
    #[error("event loop error: {0}")]
    EventLoop(String),

    /// A frame presentation error.
    #[error("presentation error: {0}")]
    Presentation(String),

    /// An unclassified platform error.
    #[error("{0}")]
    Other(String),
}

/// Convenience result type for platform operations.
pub type PlatformResult<T> = std::result::Result<T, PlatformError>;

/// Backend-present acknowledgement metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentFeedback {
    /// Monotonic count of presents acknowledged by the backend.
    pub acknowledged_present_count: u64,
    /// Backend-provided sequence number, when available.
    pub sequence: Option<u32>,
    /// Backend-provided acknowledgement timestamp in nanoseconds, when available.
    pub timestamp_ns: Option<u64>,
    /// Backend-provided CRTC identifier, when available.
    pub crtc_id: Option<u32>,
}

/// Unified access to all platform backends.
///
/// Each method returns the corresponding sub-backend.  Implementations
/// are expected to be `Send` so they can be moved between threads.
///
/// The event-loop and frame-presentation methods have default no-op
/// implementations so that [`NullPlatform`] (and other headless backends)
/// compile without requiring a real windowing system.
pub trait PlatformBackend: Send {
    // ── existing sub-backend accessors ──────────────────────────────

    /// Access the display / monitor backend.
    fn display(&self) -> &dyn DisplayBackend;

    /// Access the native window host (mutable).
    fn window_host(&mut self) -> &mut dyn NativeWindowHost;

    /// Access the taskbar integration (mutable).
    fn taskbar(&mut self) -> &mut dyn TaskbarIntegration;

    /// Access the system tray backend (mutable).
    fn tray(&mut self) -> &mut dyn NativeTray;

    /// Access the notification backend (mutable).
    fn notifications(&mut self) -> &mut dyn NativeNotifications;

    /// Access the drag-and-drop backend (mutable).
    fn drag_drop(&mut self) -> &mut dyn NativeDragDrop;

    /// Access the keymap translator.
    fn keymap(&self) -> &dyn KeymapTranslator;

    /// Return the human-readable name of the current platform.
    #[must_use]
    fn platform_name(&self) -> &str;

    // ── event loop ──────────────────────────────────────────────────

    /// Poll for the next platform event without blocking.
    ///
    /// Returns `None` if no events are pending. Real backends translate
    /// window-system messages into [`PlatformEvent`] variants.
    fn poll_event(&mut self) -> Option<PlatformEvent> {
        None
    }

    /// Wait for the next platform event, blocking until one is available.
    ///
    /// Default implementation returns [`PlatformEvent::Quit`] immediately.
    fn wait_event(&mut self) -> PlatformEvent {
        PlatformEvent::Quit
    }

    /// Returns whether the backend can accept another present right now.
    ///
    /// Backends may opportunistically refresh lightweight presenter state
    /// before answering. The default keeps existing immediate-mode behavior.
    fn can_accept_present(&mut self) -> bool {
        true
    }

    /// Returns the oldest queued present acknowledgement, if any.
    ///
    /// The default surfaces no feedback so existing backends remain unchanged.
    fn take_present_feedback(&mut self) -> Option<PresentFeedback> {
        None
    }

    // ── frame presentation ──────────────────────────────────────────

    /// Present a rendered BGRA8 frame to the specified window.
    ///
    /// `pixels` contains `height * stride` bytes in `format` layout.
    /// The platform backend copies the data to the display surface using
    /// the fastest available mechanism (GDI `SetDIBitsToDevice` on Win32,
    /// `XPutImage`/MIT-SHM on X11, `wl_surface.attach` on Wayland,
    /// `CGBitmapContext` on macOS).
    fn present_frame(
        &mut self,
        _handle: NativeWindowHandle,
        _pixels: &[u8],
        _width: u32,
        _height: u32,
        _stride: u32,
        _format: PixelFormat,
    ) -> PlatformResult<()> {
        Ok(())
    }

    /// Request the window to be repainted.
    ///
    /// This causes a [`PlatformEvent::WindowRedraw`] to be emitted on the
    /// next event loop iteration.
    fn request_redraw(&mut self, _handle: NativeWindowHandle) {}

    /// Set the hardware cursor shape for a window.
    ///
    /// When supported, the OS renders the cursor directly — eliminating
    /// the need for software cursor rendering and allowing zero-cost
    /// mouse movement.  Returns `true` if the hardware cursor was set
    /// successfully, `false` if software cursor should be used instead.
    fn set_cursor_shape(&mut self, _handle: NativeWindowHandle, _shape: &str) -> bool {
        false
    }

    /// Hide the OS cursor for a window (for software cursor rendering).
    fn hide_cursor(&mut self, _handle: NativeWindowHandle) {}

    /// Show the OS cursor for a window.
    fn show_cursor(&mut self, _handle: NativeWindowHandle) {}

    /// Query the platform's preferred color scheme.
    ///
    /// Default implementation delegates to [`query_color_scheme()`].
    fn preferred_color_scheme(&self) -> ColorScheme {
        query_color_scheme()
    }
}

/// A [`PlatformBackend`] composed entirely of null / no-op sub-backends.
///
/// Useful for unit tests, headless servers, and as a starting point
/// for new platform ports.
#[derive(Debug, Default)]
pub struct NullPlatform {
    display_backend: NullDisplayBackend,
    window_host: NullWindowHost,
    taskbar: NullTaskbar,
    tray: NullNativeTray,
    notifications: NullNativeNotifications,
    drag_drop: NullDragDrop,
    keymap: DefaultKeymap,
}

impl NullPlatform {
    /// Create a new null platform with all default sub-backends.
    #[must_use]
    pub fn new() -> Self {
        Self {
            display_backend: NullDisplayBackend,
            window_host: NullWindowHost::new(),
            taskbar: NullTaskbar,
            tray: NullNativeTray::new(),
            notifications: NullNativeNotifications::new(),
            drag_drop: NullDragDrop,
            keymap: DefaultKeymap,
        }
    }
}

impl PlatformBackend for NullPlatform {
    fn display(&self) -> &dyn DisplayBackend {
        &self.display_backend
    }

    fn window_host(&mut self) -> &mut dyn NativeWindowHost {
        &mut self.window_host
    }

    fn taskbar(&mut self) -> &mut dyn TaskbarIntegration {
        &mut self.taskbar
    }

    fn tray(&mut self) -> &mut dyn NativeTray {
        &mut self.tray
    }

    fn notifications(&mut self) -> &mut dyn NativeNotifications {
        &mut self.notifications
    }

    fn drag_drop(&mut self) -> &mut dyn NativeDragDrop {
        &mut self.drag_drop
    }

    fn keymap(&self) -> &dyn KeymapTranslator {
        &self.keymap
    }

    fn platform_name(&self) -> &str {
        "null"
    }
}

/// Detect and create the best available platform backend for the current OS.
///
/// - On Windows: returns [`Win32Platform`].
/// - On Linux: tries Wayland first, falls back to X11.
/// - On macOS: returns [`MacOSPlatform`].
/// - Otherwise (or on failure): returns [`NullPlatform`].
pub fn create_platform() -> PlatformResult<Box<dyn PlatformBackend>> {
    #[cfg(target_os = "windows")]
    {
        return Win32Platform::new().map(|p| Box::new(p) as Box<dyn PlatformBackend>);
    }

    #[cfg(target_os = "linux")]
    {
        // Prefer Wayland if $WAYLAND_DISPLAY is set.
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            if let Ok(p) = WaylandPlatform::new() {
                return Ok(Box::new(p));
            }
        }
        // Fall back to X11.
        return X11Platform::new().map(|p| Box::new(p) as Box<dyn PlatformBackend>);
    }

    #[cfg(target_os = "macos")]
    {
        return MacOSPlatform::new().map(|p| Box::new(p) as Box<dyn PlatformBackend>);
    }

    #[allow(unreachable_code)]
    Ok(Box::new(NullPlatform::new()))
}

#[cfg(test)]
mod tests;
