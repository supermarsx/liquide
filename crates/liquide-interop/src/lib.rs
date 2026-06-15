//! OS integration layer — desktop entry parsing, XDG directory resolution,
//! MIME type handling, icon themes, notifications, and system tray abstractions.

pub mod app_view;
pub mod desktop_entry;
pub mod icon;
pub mod mime;
pub mod notification;
pub mod tray;
pub mod xdg;

#[cfg(test)]
mod tests;

pub use app_view::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, ContentKind, ContentRow,
    ContentSpan,
};
pub use desktop_entry::{DesktopAction, DesktopEntry, DesktopEntryType};
pub use icon::{IconContext, IconDirectory, IconLookup, IconMatch, IconTheme, IconType};
pub use mime::{MimeAssociation, MimeDatabase, MimeSource, MimeType};
pub use notification::{
    MemoryNotificationService, Notification, NotificationAction, NotificationService,
    NullNotificationService, Urgency,
};
pub use tray::{SystemTray, TrayItem, TrayItemStatus, TrayMenuItem};
pub use xdg::XdgDirs;

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
