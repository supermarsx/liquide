//! Status bar item types for the shell's scene-graph status bar.

use serde::{Deserialize, Serialize};

use crate::slot::StatusBarSlot;

/// The payload carried by a status bar widget.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StatusBarItemKind {
    /// A clock displaying the current time.
    Clock { format: String },
    /// Unread-notification badge and do-not-disturb state.
    NotificationIndicator { unread_count: u32, dnd_active: bool },
    /// Network round-trip quality indicator.
    ConnectionQuality { quality_percent: u8, latency_ms: u32 },
    /// System tray area for background services.
    TrayArea,
    /// Plugin-provided custom content.
    Custom { plugin_id: String, content: String },
    /// Session / power button (shutdown, lock, etc.).
    SessionButton,
}

/// A single widget placed inside the status bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusBarItem {
    /// Unique identifier (e.g. `"clock"`, `"notifications"`).
    pub id: String,
    /// Payload / variant data.
    pub kind: StatusBarItemKind,
    /// Which horizontal slot this item occupies.
    pub slot: StatusBarSlot,
    /// Whether the item should be rendered.
    pub visible: bool,
    /// Whether the last rendered frame is still valid.
    pub cached: bool,
    /// Monotonic timestamp of the last content update (microseconds).
    pub last_update_us: u64,
}
