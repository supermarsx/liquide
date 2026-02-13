//! Input event handling for the LiquiDE remote desktop protocol.
//!
//! Provides keyboard, mouse, and touch event types, input state tracking,
//! and event routing to surfaces.

pub mod keyboard;
pub mod mouse;
pub mod touch;
pub mod event;
pub mod state;
pub mod router;
pub mod device;

use thiserror::Error;

/// Errors produced by the input subsystem.
#[derive(Debug, Error)]
pub enum InputError {
    /// Invalid key code value.
    #[error("invalid key code: {0}")]
    InvalidKeyCode(u32),

    /// Invalid mouse button value.
    #[error("invalid button: {0}")]
    InvalidButton(u8),

    /// No focus target available.
    #[error("no focus target")]
    NoFocusTarget,

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for the input subsystem.
pub type Result<T> = std::result::Result<T, InputError>;

// Re-exports
pub use keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
pub use mouse::{ButtonState, MouseButton, MouseEvent, ScrollAxis};
pub use touch::{TouchEvent, TouchPhase, TouchPoint};
pub use event::{EventSource, InputEvent, InputPacket};
pub use state::InputState;
pub use router::{GrabMode, HitTestResult, InputRouter, InputTarget};
pub use device::InputDevice;

#[cfg(test)]
mod tests;
