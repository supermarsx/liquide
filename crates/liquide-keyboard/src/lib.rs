//! Keyboard layout management, key mapping, and on-screen keyboard support.
//!
//! This crate provides:
//! - **[`KeyboardLayout`]** / **[`KeyMapping`]** / **[`DeadKey`]** — layout definitions
//! - **Built-in layouts** — US QWERTY, UK QWERTY, German QWERTZ, French AZERTY, US Dvorak
//! - **[`KeyboardLayoutManager`]** — scancode translation with dead key composition
//! - **[`OskLayout`]** / **[`compute_osk_layout`]** — on-screen keyboard geometry
//! - **[`KeyRepeat`]** / **[`KeyRepeatTracker`]** — key repeat timing
//! - **[`xkb`]** — XKB keymap abstraction with modifier state tracking
//! - **[`repeat_fsm`]** — Key repeat state machine with modifier filtering
//! - **[`numpad`]** — Numpad/NumLock translation
//! - **[`compose`]** — Multi-key compose sequences (X11 Compose)
//! - **[`accessibility`]** — StickyKeys, SlowKeys, BounceKeys, MouseKeys

pub mod accessibility;
pub mod builtin;
pub mod compose;
pub mod layout;
pub mod manager;
pub mod numpad;
pub mod osk;
pub mod repeat;
pub mod repeat_fsm;
pub mod xkb;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_new;

// Re-export primary types at the crate root.
pub use accessibility::{
    AccessibilityConfig, BounceKeys, KeyDecision, MouseButton, MouseKeyAction, MouseKeys, SlowKeys,
    StickyKeys, process_key,
};
pub use compose::{ComposeState, ComposeStatus, ComposeTable};
pub use layout::{DeadKey, DeadKeyId, KeyMapping, KeyboardLayout};
pub use manager::{KeyOutput, KeyboardLayoutManager, Modifiers};
pub use numpad::{NavKey, NumpadOutput, NumpadState, numpad_translate};
pub use osk::{ModifierKind, OskKey, OskKeyType, OskLayout, OskRow, compute_osk_layout};
pub use repeat::{KeyRepeat, KeyRepeatTracker};
pub use repeat_fsm::{RepeatAction, RepeatConfig, RepeatState};
pub use xkb::{
    KeySymEntry, KeymapConfig, ModifierChange, ModifierChangeKind, ModifierMask, XkbKeymap,
    XkbState, compile_keymap, lookup_keysym,
};
