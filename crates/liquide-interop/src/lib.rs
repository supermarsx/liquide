//! OS integration layer — desktop entry parsing, XDG directory resolution,
//! MIME type handling, icon themes, notifications, and system tray abstractions.

pub mod desktop_entry;
pub mod xdg;
pub mod mime;
pub mod icon;
pub mod notification;
pub mod tray;

#[cfg(test)]
mod tests;

pub use desktop_entry::{DesktopEntry, DesktopEntryType, DesktopAction};
pub use xdg::XdgDirs;
pub use mime::{MimeType, MimeAssociation, MimeSource, MimeDatabase};
pub use icon::{IconTheme, IconDirectory, IconContext, IconType, IconLookup, IconMatch};
pub use notification::{
    Notification, Urgency, NotificationAction, NotificationService,
    NullNotificationService, MemoryNotificationService,
};
pub use tray::{TrayItem, TrayItemStatus, TrayMenuItem, SystemTray};

use thiserror::Error;

/// Errors produced by the interop layer.
#[derive(Debug, Error)]
pub enum InteropError {
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("{kind} not found: {name}")]
    NotFound { kind: String, name: String },
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(String),
    #[error("I/O error: {0}")]
    IoError(String),
    #[error("invalid desktop entry: {0}")]
    InvalidDesktopEntry(String),
    #[error("icon theme error: {0}")]
    IconThemeError(String),
    #[error("notification error: {0}")]
    NotificationError(String),
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for interop operations.
pub type Result<T> = std::result::Result<T, InteropError>;
