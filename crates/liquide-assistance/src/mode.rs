//! Assistance mode definitions — view-only, interactive, exclusive, and stealth.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The mode of a remote assistance session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssistanceMode {
    /// Observer can see the screen and hear audio but cannot interact.
    ViewOnly,
    /// Observer can see, hear, and provide input alongside the owner.
    Interactive,
    /// Observer has exclusive control; owner input is blocked.
    Exclusive,
    /// Observer can see the screen silently without the owner knowing.
    Stealth,
}

impl fmt::Display for AssistanceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ViewOnly => write!(f, "ViewOnly"),
            Self::Interactive => write!(f, "Interactive"),
            Self::Exclusive => write!(f, "Exclusive"),
            Self::Stealth => write!(f, "Stealth"),
        }
    }
}

impl AssistanceMode {
    /// Return the capabilities granted by this mode.
    #[must_use]
    pub fn capabilities(&self) -> ModeCapabilities {
        match self {
            Self::ViewOnly => ModeCapabilities {
                can_see_screen: true,
                can_hear_audio: true,
                can_move_mouse: false,
                can_keyboard: false,
                can_clipboard_read: false,
                can_clipboard_write: false,
                can_request_escalation: false,
                cursor_visible_to_owner: true,
                status_indicator: true,
                max_concurrent_observers: 5,
            },
            Self::Interactive => ModeCapabilities {
                can_see_screen: true,
                can_hear_audio: true,
                can_move_mouse: true,
                can_keyboard: true,
                can_clipboard_read: true,
                can_clipboard_write: true,
                can_request_escalation: true,
                cursor_visible_to_owner: true,
                status_indicator: true,
                max_concurrent_observers: 2,
            },
            Self::Exclusive => ModeCapabilities {
                can_see_screen: true,
                can_hear_audio: true,
                can_move_mouse: true,
                can_keyboard: true,
                can_clipboard_read: true,
                can_clipboard_write: true,
                can_request_escalation: false,
                cursor_visible_to_owner: true,
                status_indicator: true,
                max_concurrent_observers: 1,
            },
            Self::Stealth => ModeCapabilities {
                can_see_screen: true,
                can_hear_audio: false,
                can_move_mouse: false,
                can_keyboard: false,
                can_clipboard_read: false,
                can_clipboard_write: false,
                can_request_escalation: false,
                cursor_visible_to_owner: false,
                status_indicator: false,
                max_concurrent_observers: 3,
            },
        }
    }
}

/// Capabilities granted by a particular assistance mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeCapabilities {
    /// Observer can see the remote screen.
    pub can_see_screen: bool,
    /// Observer can hear remote audio.
    pub can_hear_audio: bool,
    /// Observer can move the mouse cursor.
    pub can_move_mouse: bool,
    /// Observer can use the keyboard.
    pub can_keyboard: bool,
    /// Observer can read the clipboard.
    pub can_clipboard_read: bool,
    /// Observer can write to the clipboard.
    pub can_clipboard_write: bool,
    /// Observer can request escalation to a higher mode.
    pub can_request_escalation: bool,
    /// Whether the observer's cursor is visible to the owner.
    pub cursor_visible_to_owner: bool,
    /// Whether a status indicator is shown to the owner.
    pub status_indicator: bool,
    /// Maximum number of concurrent observers in this mode.
    pub max_concurrent_observers: u32,
}

/// Restrictions that can be applied to an assistance session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Restriction {
    /// Disable audio streaming.
    NoAudio,
    /// Force view-only regardless of requested mode.
    ViewOnlyOverride,
    /// Limit session duration.
    TimeLimit { seconds: u64 },
}
