//! Input channel message types.
//!
//! These messages are used on the Input channel (0x50).  They carry
//! keyboard, mouse, touch, and IME events from the client to the server,
//! plus server-initiated composition requests and input state
//! synchronization.
//!
//! All structs are CBOR-serializable via `ciborium` and use the standard
//! Liquide derive set (`Serialize`, `Deserialize`, `Debug`, `Clone`,
//! `PartialEq`).

use serde::{Deserialize, Serialize};

// ── Keyboard ────────────────────────────────────────────────────────────

/// Key event (press or release).
///
/// Carries a single physical key state change together with the
/// corresponding logical keysym and active modifier bitmask.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyEventMsg {
    /// `"down"` for key press, `"up"` for key release.
    pub event_type: String,
    /// Platform-independent physical scancode.
    pub scancode: u32,
    /// Logical key symbol (XKB keysym space).
    pub keysym: u32,
    /// Bitmask of active modifiers:
    /// `shift=1, ctrl=2, alt=4, super=8, capslock=16`.
    pub modifiers: u32,
    /// UTF-8 text produced by this key event, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Event timestamp in microseconds since the session epoch.
    pub timestamp_us: u64,
}

// ── Mouse ───────────────────────────────────────────────────────────────

/// Mouse move event.
///
/// Reports pointer motion in either absolute (for remote desktop) or
/// relative (for captured/FPS mode) coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MouseMoveMsg {
    /// `"absolute"` or `"relative"`.
    pub mode: String,
    /// X coordinate or delta.
    pub x: f32,
    /// Y coordinate or delta.
    pub y: f32,
    /// Event timestamp in microseconds.
    pub timestamp_us: u64,
}

/// Mouse button event.
///
/// Reports a mouse button press or release together with the pointer
/// position at the time of the event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MouseButtonMsg {
    /// `"down"` for press, `"up"` for release.
    pub event_type: String,
    /// Button number: `1` = left, `2` = middle, `3` = right, `4+` = extra.
    pub button: u32,
    /// Pointer X at time of event.
    pub x: f32,
    /// Pointer Y at time of event.
    pub y: f32,
    /// Event timestamp in microseconds.
    pub timestamp_us: u64,
}

/// Scroll event.
///
/// Represents scroll wheel or trackpad scroll input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrollEventMsg {
    /// `"vertical"` or `"horizontal"`.
    pub axis: String,
    /// Scroll amount (positive = down/right, negative = up/left).
    pub delta: f32,
    /// `true` for discrete click-wheel steps, `false` for smooth scroll.
    pub discrete: bool,
    /// Event timestamp in microseconds.
    pub timestamp_us: u64,
}

// ── Touch ───────────────────────────────────────────────────────────────

/// Touch event.
///
/// Carries a single touch point state change for multi-touch input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TouchEventMsg {
    /// `"down"`, `"move"`, `"up"`, or `"cancel"`.
    pub event_type: String,
    /// Touch point identifier (stable for the lifetime of the contact).
    pub id: u32,
    /// X coordinate of the touch point.
    pub x: f32,
    /// Y coordinate of the touch point.
    pub y: f32,
    /// Event timestamp in microseconds.
    pub timestamp_us: u64,
}

// ── IME / Text Input ────────────────────────────────────────────────────

/// Committed UTF-8 text from client IME.
///
/// Sent when the client's input method commits a final string (e.g.,
/// after selecting a candidate in a CJK IME).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextInputMsg {
    /// The committed UTF-8 text.
    pub text: String,
    /// Event timestamp in microseconds.
    pub timestamp_us: u64,
}

/// IME composition state.
///
/// Reports the current preedit (composition) state to the server so that
/// it can display inline composition feedback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionUpdateMsg {
    /// Composition phase: `"begin"`, `"update"`, `"commit"`, or `"cancel"`.
    pub phase: String,
    /// Current preedit string (the uncommitted text being composed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preedit_string: Option<String>,
    /// Cursor position within the preedit string (zero-based code-unit
    /// offset).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_position: Option<u32>,
    /// Event timestamp in microseconds.
    pub timestamp_us: u64,
}

/// Server requests client to activate/deactivate IME.
///
/// Sent by the server when focus enters or leaves a text input field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionRequestMsg {
    /// `true` to activate the client IME, `false` to deactivate.
    pub activate: bool,
}

// ── Sync ────────────────────────────────────────────────────────────────

/// Request input state sync (after reconnect).
///
/// Sent by either side after a reconnect to synchronize modifier and
/// button state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputSyncRequestMsg {}

/// Current modifier/button state.
///
/// Response to [`InputSyncRequestMsg`] carrying the current state of
/// modifier keys and mouse buttons.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputSyncResponseMsg {
    /// Bitmask of currently active modifiers (same encoding as
    /// [`KeyEventMsg::modifiers`]).
    pub modifiers: u32,
    /// Bitmask of currently pressed mouse buttons.
    pub buttons: u32,
}
