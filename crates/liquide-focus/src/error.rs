//! Error types for focus operations.

use serde::{Deserialize, Serialize};

use crate::types::WindowId;

/// Errors that can occur during focus / activation changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusError {
    /// The target window does not exist in the window registry.
    WindowNotFound(WindowId),
    /// The target window is disabled and cannot receive focus.
    WindowDisabled(WindowId),
    /// The target window is minimised — must be restored first.
    WindowMinimized(WindowId),
    /// A foreground lock prevents this process from stealing focus.
    /// The shell should flash `flash_window` on the taskbar instead.
    ForegroundLocked { flash_window: WindowId },
    /// A modal dialog blocks activation of the target.
    ModalBlocked { modal_window: WindowId },
}

impl std::fmt::Display for FocusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WindowNotFound(id) => write!(f, "window {} not found", id),
            Self::WindowDisabled(id) => write!(f, "window {} is disabled", id),
            Self::WindowMinimized(id) => write!(f, "window {} is minimised", id),
            Self::ForegroundLocked { flash_window } => {
                write!(
                    f,
                    "foreground locked — flash {} instead",
                    flash_window
                )
            }
            Self::ModalBlocked { modal_window } => {
                write!(f, "blocked by modal window {}", modal_window)
            }
        }
    }
}

impl std::error::Error for FocusError {}
