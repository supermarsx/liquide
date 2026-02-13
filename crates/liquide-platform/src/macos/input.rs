//! macOS virtual keycode and modifier flag mapping.
//!
//! Provides helper functions to translate macOS virtual keycodes (from
//! `[NSEvent keyCode]`) and modifier flags (`[NSEvent modifierFlags]`) into
//! the platform-independent `KeyCode` and `Modifiers` types used by
//! `liquide_input`.
//!
//! macOS virtual keycodes are defined in `<Carbon/Events.h>` (kVK_* constants)
//! and are hardware-layout-independent ordinals in the range 0x00..0x7E.

use liquide_input::keyboard::{KeyCode, Modifiers};

use super::ffi;

// ---------------------------------------------------------------------------
// macOS virtual keycode constants (from Carbon Events.h)
// ---------------------------------------------------------------------------

pub const kVK_ANSI_A: u16 = 0x00;
pub const kVK_ANSI_S: u16 = 0x01;
pub const kVK_ANSI_D: u16 = 0x02;
pub const kVK_ANSI_F: u16 = 0x03;
pub const kVK_ANSI_H: u16 = 0x04;
pub const kVK_ANSI_G: u16 = 0x05;
pub const kVK_ANSI_Z: u16 = 0x06;
pub const kVK_ANSI_X: u16 = 0x07;
pub const kVK_ANSI_C: u16 = 0x08;
pub const kVK_ANSI_V: u16 = 0x09;
pub const kVK_ANSI_B: u16 = 0x0B;
pub const kVK_ANSI_Q: u16 = 0x0C;
pub const kVK_ANSI_W: u16 = 0x0D;
pub const kVK_ANSI_E: u16 = 0x0E;
pub const kVK_ANSI_R: u16 = 0x0F;
pub const kVK_ANSI_Y: u16 = 0x10;
pub const kVK_ANSI_T: u16 = 0x11;
pub const kVK_ANSI_1: u16 = 0x12;
pub const kVK_ANSI_2: u16 = 0x13;
pub const kVK_ANSI_3: u16 = 0x14;
pub const kVK_ANSI_4: u16 = 0x15;
pub const kVK_ANSI_6: u16 = 0x16;
pub const kVK_ANSI_5: u16 = 0x17;
pub const kVK_ANSI_Equal: u16 = 0x18;
pub const kVK_ANSI_9: u16 = 0x19;
pub const kVK_ANSI_7: u16 = 0x1A;
pub const kVK_ANSI_Minus: u16 = 0x1B;
pub const kVK_ANSI_8: u16 = 0x1C;
pub const kVK_ANSI_0: u16 = 0x1D;
pub const kVK_ANSI_RightBracket: u16 = 0x1E;
pub const kVK_ANSI_O: u16 = 0x1F;
pub const kVK_ANSI_U: u16 = 0x20;
pub const kVK_ANSI_LeftBracket: u16 = 0x21;
pub const kVK_ANSI_I: u16 = 0x22;
pub const kVK_ANSI_P: u16 = 0x23;
pub const kVK_ANSI_L: u16 = 0x25;
pub const kVK_ANSI_J: u16 = 0x26;
pub const kVK_ANSI_Quote: u16 = 0x27;
pub const kVK_ANSI_K: u16 = 0x28;
pub const kVK_ANSI_Semicolon: u16 = 0x29;
pub const kVK_ANSI_Backslash: u16 = 0x2A;
pub const kVK_ANSI_Comma: u16 = 0x2B;
pub const kVK_ANSI_Slash: u16 = 0x2C;
pub const kVK_ANSI_N: u16 = 0x2D;
pub const kVK_ANSI_M: u16 = 0x2E;
pub const kVK_ANSI_Period: u16 = 0x2F;
pub const kVK_ANSI_Grave: u16 = 0x32;

pub const kVK_Return: u16 = 0x24;
pub const kVK_Tab: u16 = 0x30;
pub const kVK_Space: u16 = 0x31;
pub const kVK_Delete: u16 = 0x33; // Backspace
pub const kVK_Escape: u16 = 0x35;
pub const kVK_Command: u16 = 0x37; // Left Command
pub const kVK_Shift: u16 = 0x38; // Left Shift
pub const kVK_CapsLock: u16 = 0x39;
pub const kVK_Option: u16 = 0x3A; // Left Option/Alt
pub const kVK_Control: u16 = 0x3B; // Left Control
pub const kVK_RightCommand: u16 = 0x36;
pub const kVK_RightShift: u16 = 0x3C;
pub const kVK_RightOption: u16 = 0x3D;
pub const kVK_RightControl: u16 = 0x3E;
pub const kVK_Function: u16 = 0x3F;

pub const kVK_F17: u16 = 0x40;
pub const kVK_F18: u16 = 0x4F;
pub const kVK_F19: u16 = 0x50;
pub const kVK_F20: u16 = 0x5A;

pub const kVK_F5: u16 = 0x60;
pub const kVK_F6: u16 = 0x61;
pub const kVK_F7: u16 = 0x62;
pub const kVK_F8: u16 = 0x63;
pub const kVK_F3: u16 = 0x64;
pub const kVK_F9: u16 = 0x65;
pub const kVK_F11: u16 = 0x67;
pub const kVK_F13: u16 = 0x69;
pub const kVK_F16: u16 = 0x6A;
pub const kVK_F14: u16 = 0x6B;
pub const kVK_F10: u16 = 0x6D;
pub const kVK_F12: u16 = 0x6F;
pub const kVK_F15: u16 = 0x71;
pub const kVK_Help: u16 = 0x72;
pub const kVK_Home: u16 = 0x73;
pub const kVK_PageUp: u16 = 0x74;
pub const kVK_ForwardDelete: u16 = 0x75; // Delete (forward)
pub const kVK_F4: u16 = 0x76;
pub const kVK_End: u16 = 0x77;
pub const kVK_F2: u16 = 0x78;
pub const kVK_PageDown: u16 = 0x79;
pub const kVK_F1: u16 = 0x7A;
pub const kVK_LeftArrow: u16 = 0x7B;
pub const kVK_RightArrow: u16 = 0x7C;
pub const kVK_DownArrow: u16 = 0x7D;
pub const kVK_UpArrow: u16 = 0x7E;

// ---------------------------------------------------------------------------
// Virtual keycode to KeyCode mapping
// ---------------------------------------------------------------------------

/// Map a macOS virtual keycode to a `KeyCode`.
///
/// The virtual keycode comes from `[NSEvent keyCode]` and corresponds to the
/// Carbon `kVK_*` constants.  Returns `None` for keycodes that have no
/// corresponding `KeyCode` variant.
#[must_use]
pub fn vk_to_keycode(vk: u16) -> Option<KeyCode> {
    match vk {
        // Letters
        kVK_ANSI_A => Some(KeyCode::A),
        kVK_ANSI_B => Some(KeyCode::B),
        kVK_ANSI_C => Some(KeyCode::C),
        kVK_ANSI_D => Some(KeyCode::D),
        kVK_ANSI_E => Some(KeyCode::E),
        kVK_ANSI_F => Some(KeyCode::F),
        kVK_ANSI_G => Some(KeyCode::G),
        kVK_ANSI_H => Some(KeyCode::H),
        kVK_ANSI_I => Some(KeyCode::I),
        kVK_ANSI_J => Some(KeyCode::J),
        kVK_ANSI_K => Some(KeyCode::K),
        kVK_ANSI_L => Some(KeyCode::L),
        kVK_ANSI_M => Some(KeyCode::M),
        kVK_ANSI_N => Some(KeyCode::N),
        kVK_ANSI_O => Some(KeyCode::O),
        kVK_ANSI_P => Some(KeyCode::P),
        kVK_ANSI_Q => Some(KeyCode::Q),
        kVK_ANSI_R => Some(KeyCode::R),
        kVK_ANSI_S => Some(KeyCode::S),
        kVK_ANSI_T => Some(KeyCode::T),
        kVK_ANSI_U => Some(KeyCode::U),
        kVK_ANSI_V => Some(KeyCode::V),
        kVK_ANSI_W => Some(KeyCode::W),
        kVK_ANSI_X => Some(KeyCode::X),
        kVK_ANSI_Y => Some(KeyCode::Y),
        kVK_ANSI_Z => Some(KeyCode::Z),

        // Digits
        kVK_ANSI_0 => Some(KeyCode::Digit0),
        kVK_ANSI_1 => Some(KeyCode::Digit1),
        kVK_ANSI_2 => Some(KeyCode::Digit2),
        kVK_ANSI_3 => Some(KeyCode::Digit3),
        kVK_ANSI_4 => Some(KeyCode::Digit4),
        kVK_ANSI_5 => Some(KeyCode::Digit5),
        kVK_ANSI_6 => Some(KeyCode::Digit6),
        kVK_ANSI_7 => Some(KeyCode::Digit7),
        kVK_ANSI_8 => Some(KeyCode::Digit8),
        kVK_ANSI_9 => Some(KeyCode::Digit9),

        // Function keys
        kVK_F1 => Some(KeyCode::F1),
        kVK_F2 => Some(KeyCode::F2),
        kVK_F3 => Some(KeyCode::F3),
        kVK_F4 => Some(KeyCode::F4),
        kVK_F5 => Some(KeyCode::F5),
        kVK_F6 => Some(KeyCode::F6),
        kVK_F7 => Some(KeyCode::F7),
        kVK_F8 => Some(KeyCode::F8),
        kVK_F9 => Some(KeyCode::F9),
        kVK_F10 => Some(KeyCode::F10),
        kVK_F11 => Some(KeyCode::F11),
        kVK_F12 => Some(KeyCode::F12),

        // Navigation / editing
        kVK_Escape => Some(KeyCode::Escape),
        kVK_Return => Some(KeyCode::Enter),
        kVK_Tab => Some(KeyCode::Tab),
        kVK_Delete => Some(KeyCode::Backspace),
        kVK_Space => Some(KeyCode::Space),
        kVK_ForwardDelete => Some(KeyCode::Delete),
        kVK_Home => Some(KeyCode::Home),
        kVK_End => Some(KeyCode::End),
        kVK_PageUp => Some(KeyCode::PageUp),
        kVK_PageDown => Some(KeyCode::PageDown),
        kVK_Help => Some(KeyCode::Insert), // Mac Help key maps to Insert

        // Arrow keys
        kVK_UpArrow => Some(KeyCode::ArrowUp),
        kVK_DownArrow => Some(KeyCode::ArrowDown),
        kVK_LeftArrow => Some(KeyCode::ArrowLeft),
        kVK_RightArrow => Some(KeyCode::ArrowRight),

        // Lock keys
        kVK_CapsLock => Some(KeyCode::CapsLock),

        // Modifier keys
        kVK_Shift => Some(KeyCode::LeftShift),
        kVK_RightShift => Some(KeyCode::RightShift),
        kVK_Control => Some(KeyCode::LeftCtrl),
        kVK_RightControl => Some(KeyCode::RightCtrl),
        kVK_Option => Some(KeyCode::LeftAlt),
        kVK_RightOption => Some(KeyCode::RightAlt),
        kVK_Command => Some(KeyCode::LeftSuper),
        kVK_RightCommand => Some(KeyCode::RightSuper),

        // Punctuation / symbol keys (US keyboard layout)
        kVK_ANSI_Comma => Some(KeyCode::Comma),
        kVK_ANSI_Period => Some(KeyCode::Period),
        kVK_ANSI_Slash => Some(KeyCode::Slash),
        kVK_ANSI_Semicolon => Some(KeyCode::Semicolon),
        kVK_ANSI_Quote => Some(KeyCode::Quote),
        kVK_ANSI_LeftBracket => Some(KeyCode::BracketLeft),
        kVK_ANSI_RightBracket => Some(KeyCode::BracketRight),
        kVK_ANSI_Backslash => Some(KeyCode::Backslash),
        kVK_ANSI_Minus => Some(KeyCode::Minus),
        kVK_ANSI_Equal => Some(KeyCode::Equal),
        kVK_ANSI_Grave => Some(KeyCode::Grave),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// NSEventModifierFlags to Modifiers mapping
// ---------------------------------------------------------------------------

/// Convert `NSEventModifierFlags` (from `[NSEvent modifierFlags]`) to
/// `Modifiers`.
///
/// On macOS, `Command` (Cmd) maps to `SUPER` and `Option` (Alt) maps to
/// `ALT`, matching the standard cross-platform convention.
#[must_use]
pub fn modifiers_from_flags(flags: u64) -> Modifiers {
    let mut bits: u8 = 0;

    if flags & ffi::NSEventModifierFlagShift != 0 {
        bits |= Modifiers::SHIFT;
    }
    if flags & ffi::NSEventModifierFlagControl != 0 {
        bits |= Modifiers::CTRL;
    }
    if flags & ffi::NSEventModifierFlagOption != 0 {
        bits |= Modifiers::ALT;
    }
    if flags & ffi::NSEventModifierFlagCommand != 0 {
        bits |= Modifiers::SUPER;
    }
    if flags & ffi::NSEventModifierFlagCapsLock != 0 {
        bits |= Modifiers::CAPS_LOCK;
    }

    Modifiers::from_bits(bits)
}

// ---------------------------------------------------------------------------
// Scancode to KeyCode mapping
// ---------------------------------------------------------------------------

/// Map a macOS "scancode" to a `KeyCode`.
///
/// On macOS the virtual keycode *is* effectively the scancode (both come from
/// `[NSEvent keyCode]`).  This function simply delegates to `vk_to_keycode`
/// for API symmetry with the Win32 and X11 backends.
#[must_use]
pub fn scancode_to_keycode(scancode: u32) -> Option<KeyCode> {
    vk_to_keycode(scancode as u16)
}
