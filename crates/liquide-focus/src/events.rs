//! Events generated during activation changes.

use serde::{Deserialize, Serialize};

use crate::types::{ActivateReason, WindowId};

/// An event produced by the activation protocol.
///
/// These mirror the NT message sequence: `WM_CANCELMODE`, `WM_NCACTIVATE`,
/// `WM_ACTIVATE`, `WM_ACTIVATEAPP`, `WM_KILLFOCUS`, `WM_SETFOCUS`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationEvent {
    /// Window is becoming the active window.
    Activate {
        window: WindowId,
        reason: ActivateReason,
    },
    /// Window is losing activation.
    Deactivate { window: WindowId },
    /// Window gained keyboard focus.
    FocusGained { window: WindowId },
    /// Window lost keyboard focus.
    FocusLost { window: WindowId },
    /// Application (process) gaining or losing foreground status.
    ActivateApp {
        window: WindowId,
        activating: bool,
        thread_id: u32,
    },
    /// Cancel any modal or tracking loops (menus, drag, size-move).
    CancelMode { window: WindowId },
    /// Non-client activation — tells the window to repaint its frame
    /// (title bar) as active or inactive.
    NcActivate { window: WindowId, active: bool },
}
