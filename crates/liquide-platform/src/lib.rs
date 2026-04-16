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
    fn set_cursor_shape(
        &mut self,
        _handle: NativeWindowHandle,
        _shape: &str,
    ) -> bool {
        false
    }

    /// Hide the OS cursor for a window (for software cursor rendering).
    fn hide_cursor(&mut self, _handle: NativeWindowHandle) {}

    /// Show the OS cursor for a window.
    fn show_cursor(&mut self, _handle: NativeWindowHandle) {}
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
