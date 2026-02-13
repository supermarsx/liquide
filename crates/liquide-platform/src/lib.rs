//! Cross-platform desktop environment abstraction layer.
//!
//! This crate defines trait interfaces for every major platform integration
//! point (displays, windows, taskbar, tray, notifications, drag-and-drop,
//! and keymap translation) together with null / default implementations
//! that can be used in tests or headless environments.

pub mod display;
pub mod dnd;
pub mod keymap;
pub mod notifications;
pub mod taskbar;
pub mod tray;
pub mod window_host;

// Re-exports
pub use display::{DisplayBackend, MonitorInfo, NullDisplayBackend};
pub use dnd::{NativeDragDrop, NullDragDrop};
pub use keymap::{DefaultKeymap, KeymapTranslator};
pub use notifications::{NativeNotificationParams, NativeNotifications, NullNativeNotifications};
pub use taskbar::{JumpListItem, NullTaskbar, TaskbarIntegration};
pub use tray::{NativeTray, NativeTrayHandle, NativeTrayParams, NullNativeTray, TrayUpdate};
pub use window_host::{NativeWindowHandle, NativeWindowHost, NativeWindowParams, NullWindowHost};

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
pub trait PlatformBackend: Send {
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

#[cfg(test)]
mod tests;
