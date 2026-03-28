//! Shared types used across the focus crate.

use serde::{Deserialize, Serialize};

/// Unique window identifier (matches the shell's `WindowId(u64)` newtype).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u64);

impl std::fmt::Display for WindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WindowId({})", self.0)
    }
}

/// The reason an activation change occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivateReason {
    /// User clicked on the window.
    Click,
    /// Keyboard navigation (Alt+Tab, etc.).
    Keyboard,
    /// Programmatic activation via API.
    Api,
    /// Minimise-then-restore or restore-from-minimised.
    MinRestore,
    /// Catch-all for other triggers.
    Other,
}
