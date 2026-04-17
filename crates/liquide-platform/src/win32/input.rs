//! Win32 virtual key and scancode to `liquide_input::KeyCode` mapping.
//!
//! Provides helper functions to translate Win32 virtual-key codes and raw
//! scancodes into the platform-independent `KeyCode` enum, and to query
//! the current modifier key state via `GetKeyState`.

use liquide_input::keyboard::{KeyCode, Modifiers};

use super::ffi;

/// Map a Win32 virtual-key code to a `KeyCode`.
///
/// Returns `None` for virtual-key codes that have no corresponding `KeyCode`
/// variant (e.g. volume keys, browser keys, IME keys).
#[must_use]
pub fn vk_to_keycode(vk: u32) -> Option<KeyCode> {
    match vk {
        // Letters A-Z (0x41..=0x5A)
        ffi::VK_A => Some(KeyCode::A),
        ffi::VK_B => Some(KeyCode::B),
        ffi::VK_C => Some(KeyCode::C),
        ffi::VK_D => Some(KeyCode::D),
        ffi::VK_E => Some(KeyCode::E),
        ffi::VK_F => Some(KeyCode::F),
        ffi::VK_G => Some(KeyCode::G),
        ffi::VK_H => Some(KeyCode::H),
        ffi::VK_I => Some(KeyCode::I),
        ffi::VK_J => Some(KeyCode::J),
        ffi::VK_K => Some(KeyCode::K),
        ffi::VK_L => Some(KeyCode::L),
        ffi::VK_M => Some(KeyCode::M),
        ffi::VK_N => Some(KeyCode::N),
        ffi::VK_O => Some(KeyCode::O),
        ffi::VK_P => Some(KeyCode::P),
        ffi::VK_Q => Some(KeyCode::Q),
        ffi::VK_R => Some(KeyCode::R),
        ffi::VK_S => Some(KeyCode::S),
        ffi::VK_T => Some(KeyCode::T),
        ffi::VK_U => Some(KeyCode::U),
        ffi::VK_V => Some(KeyCode::V),
        ffi::VK_W => Some(KeyCode::W),
        ffi::VK_X => Some(KeyCode::X),
        ffi::VK_Y => Some(KeyCode::Y),
        ffi::VK_Z => Some(KeyCode::Z),

        // Digits 0-9 (0x30..=0x39)
        ffi::VK_0 => Some(KeyCode::Digit0),
        ffi::VK_1 => Some(KeyCode::Digit1),
        ffi::VK_2 => Some(KeyCode::Digit2),
        ffi::VK_3 => Some(KeyCode::Digit3),
        ffi::VK_4 => Some(KeyCode::Digit4),
        ffi::VK_5 => Some(KeyCode::Digit5),
        ffi::VK_6 => Some(KeyCode::Digit6),
        ffi::VK_7 => Some(KeyCode::Digit7),
        ffi::VK_8 => Some(KeyCode::Digit8),
        ffi::VK_9 => Some(KeyCode::Digit9),

        // Function keys F1-F12
        ffi::VK_F1 => Some(KeyCode::F1),
        ffi::VK_F2 => Some(KeyCode::F2),
        ffi::VK_F3 => Some(KeyCode::F3),
        ffi::VK_F4 => Some(KeyCode::F4),
        ffi::VK_F5 => Some(KeyCode::F5),
        ffi::VK_F6 => Some(KeyCode::F6),
        ffi::VK_F7 => Some(KeyCode::F7),
        ffi::VK_F8 => Some(KeyCode::F8),
        ffi::VK_F9 => Some(KeyCode::F9),
        ffi::VK_F10 => Some(KeyCode::F10),
        ffi::VK_F11 => Some(KeyCode::F11),
        ffi::VK_F12 => Some(KeyCode::F12),

        // Navigation / editing
        ffi::VK_ESCAPE => Some(KeyCode::Escape),
        ffi::VK_RETURN => Some(KeyCode::Enter),
        ffi::VK_TAB => Some(KeyCode::Tab),
        ffi::VK_BACK => Some(KeyCode::Backspace),
        ffi::VK_SPACE => Some(KeyCode::Space),
        ffi::VK_INSERT => Some(KeyCode::Insert),
        ffi::VK_DELETE => Some(KeyCode::Delete),
        ffi::VK_HOME => Some(KeyCode::Home),
        ffi::VK_END => Some(KeyCode::End),
        ffi::VK_PRIOR => Some(KeyCode::PageUp),
        ffi::VK_NEXT => Some(KeyCode::PageDown),

        // Arrow keys
        ffi::VK_UP => Some(KeyCode::ArrowUp),
        ffi::VK_DOWN => Some(KeyCode::ArrowDown),
        ffi::VK_LEFT => Some(KeyCode::ArrowLeft),
        ffi::VK_RIGHT => Some(KeyCode::ArrowRight),

        // Lock keys
        ffi::VK_CAPITAL => Some(KeyCode::CapsLock),
        ffi::VK_NUMLOCK => Some(KeyCode::NumLock),
        ffi::VK_SCROLL => Some(KeyCode::ScrollLock),
        ffi::VK_SNAPSHOT => Some(KeyCode::PrintScreen),
        ffi::VK_PAUSE => Some(KeyCode::Pause),

        // Modifier keys (side-specific)
        ffi::VK_LSHIFT => Some(KeyCode::LeftShift),
        ffi::VK_RSHIFT => Some(KeyCode::RightShift),
        ffi::VK_LCONTROL => Some(KeyCode::LeftCtrl),
        ffi::VK_RCONTROL => Some(KeyCode::RightCtrl),
        ffi::VK_LMENU => Some(KeyCode::LeftAlt),
        ffi::VK_RMENU => Some(KeyCode::RightAlt),
        ffi::VK_LWIN => Some(KeyCode::LeftSuper),
        ffi::VK_RWIN => Some(KeyCode::RightSuper),

        // Generic shift/ctrl/alt -- map to left variants as fallback
        ffi::VK_SHIFT => Some(KeyCode::LeftShift),
        ffi::VK_CONTROL => Some(KeyCode::LeftCtrl),
        ffi::VK_MENU => Some(KeyCode::LeftAlt),

        // Punctuation / OEM keys (US keyboard layout)
        ffi::VK_OEM_COMMA => Some(KeyCode::Comma),
        ffi::VK_OEM_PERIOD => Some(KeyCode::Period),
        ffi::VK_OEM_2 => Some(KeyCode::Slash),
        ffi::VK_OEM_1 => Some(KeyCode::Semicolon),
        ffi::VK_OEM_7 => Some(KeyCode::Quote),
        ffi::VK_OEM_4 => Some(KeyCode::BracketLeft),
        ffi::VK_OEM_6 => Some(KeyCode::BracketRight),
        ffi::VK_OEM_5 => Some(KeyCode::Backslash),
        ffi::VK_OEM_MINUS => Some(KeyCode::Minus),
        ffi::VK_OEM_PLUS => Some(KeyCode::Equal),
        ffi::VK_OEM_3 => Some(KeyCode::Grave),

        // Context menu key
        ffi::VK_APPS => Some(KeyCode::ContextMenu),

        _ => None,
    }
}

/// Query the current keyboard modifier state using `GetKeyState`.
///
/// Checks Shift, Ctrl, Alt, and Super (Win) keys.
#[must_use]
pub fn modifiers_from_state() -> Modifiers {
    let mut bits: u8 = 0;

    // Safety: GetKeyState is safe to call at any time and only reads
    // keyboard state from the calling thread's message queue.
    unsafe {
        if ffi::GetKeyState(ffi::VK_SHIFT as i32) < 0 {
            bits |= Modifiers::SHIFT;
        }
        if ffi::GetKeyState(ffi::VK_CONTROL as i32) < 0 {
            bits |= Modifiers::CTRL;
        }
        if ffi::GetKeyState(ffi::VK_MENU as i32) < 0 {
            bits |= Modifiers::ALT;
        }
        if ffi::GetKeyState(ffi::VK_LWIN as i32) < 0
            || ffi::GetKeyState(ffi::VK_RWIN as i32) < 0
        {
            bits |= Modifiers::SUPER;
        }
    }

    Modifiers::from_bits(bits)
}

/// Map a raw Win32 scancode to a `KeyCode`.
///
/// Win32 scancodes follow the PS/2 Set 1 layout. This function handles
/// the most common scancodes (main block + extended via 0xE0 prefix).
/// The scan code passed here should already have the extended bit (bit 24
/// from lParam) folded in: for extended keys, pass `scancode | 0x100`.
#[must_use]
pub fn scancode_to_keycode(scancode: u32) -> Option<KeyCode> {
    match scancode {
        // Main keyboard scancodes (PS/2 Set 1)
        0x01 => Some(KeyCode::Escape),
        0x02 => Some(KeyCode::Digit1),
        0x03 => Some(KeyCode::Digit2),
        0x04 => Some(KeyCode::Digit3),
        0x05 => Some(KeyCode::Digit4),
        0x06 => Some(KeyCode::Digit5),
        0x07 => Some(KeyCode::Digit6),
        0x08 => Some(KeyCode::Digit7),
        0x09 => Some(KeyCode::Digit8),
        0x0A => Some(KeyCode::Digit9),
        0x0B => Some(KeyCode::Digit0),
        0x0C => Some(KeyCode::Minus),
        0x0D => Some(KeyCode::Equal),
        0x0E => Some(KeyCode::Backspace),
        0x0F => Some(KeyCode::Tab),
        0x10 => Some(KeyCode::Q),
        0x11 => Some(KeyCode::W),
        0x12 => Some(KeyCode::E),
        0x13 => Some(KeyCode::R),
        0x14 => Some(KeyCode::T),
        0x15 => Some(KeyCode::Y),
        0x16 => Some(KeyCode::U),
        0x17 => Some(KeyCode::I),
        0x18 => Some(KeyCode::O),
        0x19 => Some(KeyCode::P),
        0x1A => Some(KeyCode::BracketLeft),
        0x1B => Some(KeyCode::BracketRight),
        0x1C => Some(KeyCode::Enter),
        0x1D => Some(KeyCode::LeftCtrl),
        0x1E => Some(KeyCode::A),
        0x1F => Some(KeyCode::S),
        0x20 => Some(KeyCode::D),
        0x21 => Some(KeyCode::F),
        0x22 => Some(KeyCode::G),
        0x23 => Some(KeyCode::H),
        0x24 => Some(KeyCode::J),
        0x25 => Some(KeyCode::K),
        0x26 => Some(KeyCode::L),
        0x27 => Some(KeyCode::Semicolon),
        0x28 => Some(KeyCode::Quote),
        0x29 => Some(KeyCode::Grave),
        0x2A => Some(KeyCode::LeftShift),
        0x2B => Some(KeyCode::Backslash),
        0x2C => Some(KeyCode::Z),
        0x2D => Some(KeyCode::X),
        0x2E => Some(KeyCode::C),
        0x2F => Some(KeyCode::V),
        0x30 => Some(KeyCode::B),
        0x31 => Some(KeyCode::N),
        0x32 => Some(KeyCode::M),
        0x33 => Some(KeyCode::Comma),
        0x34 => Some(KeyCode::Period),
        0x35 => Some(KeyCode::Slash),
        0x36 => Some(KeyCode::RightShift),
        0x38 => Some(KeyCode::LeftAlt),
        0x39 => Some(KeyCode::Space),
        0x3A => Some(KeyCode::CapsLock),
        0x3B => Some(KeyCode::F1),
        0x3C => Some(KeyCode::F2),
        0x3D => Some(KeyCode::F3),
        0x3E => Some(KeyCode::F4),
        0x3F => Some(KeyCode::F5),
        0x40 => Some(KeyCode::F6),
        0x41 => Some(KeyCode::F7),
        0x42 => Some(KeyCode::F8),
        0x43 => Some(KeyCode::F9),
        0x44 => Some(KeyCode::F10),
        0x45 => Some(KeyCode::NumLock),
        0x46 => Some(KeyCode::ScrollLock),
        0x57 => Some(KeyCode::F11),
        0x58 => Some(KeyCode::F12),

        // Extended keys (0xE0 prefix → bit 8 set)
        0x11D => Some(KeyCode::RightCtrl),
        0x135 => Some(KeyCode::Slash),   // Numpad divide on extended
        0x137 => Some(KeyCode::PrintScreen),
        0x138 => Some(KeyCode::RightAlt),
        0x145 => Some(KeyCode::NumLock),
        0x147 => Some(KeyCode::Home),
        0x148 => Some(KeyCode::ArrowUp),
        0x149 => Some(KeyCode::PageUp),
        0x14B => Some(KeyCode::ArrowLeft),
        0x14D => Some(KeyCode::ArrowRight),
        0x14F => Some(KeyCode::End),
        0x150 => Some(KeyCode::ArrowDown),
        0x151 => Some(KeyCode::PageDown),
        0x152 => Some(KeyCode::Insert),
        0x153 => Some(KeyCode::Delete),
        0x15B => Some(KeyCode::LeftSuper),
        0x15C => Some(KeyCode::RightSuper),
        0x15D => Some(KeyCode::ContextMenu),
        0x11C => Some(KeyCode::Enter), // Numpad enter

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::ffi;

    // ── vk_to_keycode tests ─────────────────────────────────────────

    #[test]
    fn vk_letters_a_through_z() {
        let expected = [
            (ffi::VK_A, KeyCode::A),
            (ffi::VK_B, KeyCode::B),
            (ffi::VK_M, KeyCode::M),
            (ffi::VK_Z, KeyCode::Z),
        ];
        for (vk, key) in expected {
            assert_eq!(vk_to_keycode(vk), Some(key), "failed for VK 0x{vk:02X}");
        }
    }

    #[test]
    fn vk_digits_0_through_9() {
        let expected = [
            (ffi::VK_0, KeyCode::Digit0),
            (ffi::VK_1, KeyCode::Digit1),
            (ffi::VK_5, KeyCode::Digit5),
            (ffi::VK_9, KeyCode::Digit9),
        ];
        for (vk, key) in expected {
            assert_eq!(vk_to_keycode(vk), Some(key), "failed for VK 0x{vk:02X}");
        }
    }

    #[test]
    fn vk_function_keys_f1_through_f12() {
        assert_eq!(vk_to_keycode(ffi::VK_F1), Some(KeyCode::F1));
        assert_eq!(vk_to_keycode(ffi::VK_F6), Some(KeyCode::F6));
        assert_eq!(vk_to_keycode(ffi::VK_F12), Some(KeyCode::F12));
    }

    #[test]
    fn vk_arrow_keys() {
        assert_eq!(vk_to_keycode(ffi::VK_UP), Some(KeyCode::ArrowUp));
        assert_eq!(vk_to_keycode(ffi::VK_DOWN), Some(KeyCode::ArrowDown));
        assert_eq!(vk_to_keycode(ffi::VK_LEFT), Some(KeyCode::ArrowLeft));
        assert_eq!(vk_to_keycode(ffi::VK_RIGHT), Some(KeyCode::ArrowRight));
    }

    #[test]
    fn vk_navigation_keys() {
        assert_eq!(vk_to_keycode(ffi::VK_HOME), Some(KeyCode::Home));
        assert_eq!(vk_to_keycode(ffi::VK_END), Some(KeyCode::End));
        assert_eq!(vk_to_keycode(ffi::VK_PRIOR), Some(KeyCode::PageUp));
        assert_eq!(vk_to_keycode(ffi::VK_NEXT), Some(KeyCode::PageDown));
        assert_eq!(vk_to_keycode(ffi::VK_INSERT), Some(KeyCode::Insert));
        assert_eq!(vk_to_keycode(ffi::VK_DELETE), Some(KeyCode::Delete));
    }

    #[test]
    fn vk_modifier_keys_side_specific() {
        assert_eq!(vk_to_keycode(ffi::VK_LSHIFT), Some(KeyCode::LeftShift));
        assert_eq!(vk_to_keycode(ffi::VK_RSHIFT), Some(KeyCode::RightShift));
        assert_eq!(vk_to_keycode(ffi::VK_LCONTROL), Some(KeyCode::LeftCtrl));
        assert_eq!(vk_to_keycode(ffi::VK_RCONTROL), Some(KeyCode::RightCtrl));
        assert_eq!(vk_to_keycode(ffi::VK_LMENU), Some(KeyCode::LeftAlt));
        assert_eq!(vk_to_keycode(ffi::VK_RMENU), Some(KeyCode::RightAlt));
        assert_eq!(vk_to_keycode(ffi::VK_LWIN), Some(KeyCode::LeftSuper));
        assert_eq!(vk_to_keycode(ffi::VK_RWIN), Some(KeyCode::RightSuper));
    }

    #[test]
    fn vk_generic_modifiers_fallback_to_left() {
        assert_eq!(vk_to_keycode(ffi::VK_SHIFT), Some(KeyCode::LeftShift));
        assert_eq!(vk_to_keycode(ffi::VK_CONTROL), Some(KeyCode::LeftCtrl));
        assert_eq!(vk_to_keycode(ffi::VK_MENU), Some(KeyCode::LeftAlt));
    }

    #[test]
    fn vk_oem_punctuation() {
        assert_eq!(vk_to_keycode(ffi::VK_OEM_COMMA), Some(KeyCode::Comma));
        assert_eq!(vk_to_keycode(ffi::VK_OEM_PERIOD), Some(KeyCode::Period));
        assert_eq!(vk_to_keycode(ffi::VK_OEM_2), Some(KeyCode::Slash));
        assert_eq!(vk_to_keycode(ffi::VK_OEM_1), Some(KeyCode::Semicolon));
        assert_eq!(vk_to_keycode(ffi::VK_OEM_7), Some(KeyCode::Quote));
        assert_eq!(vk_to_keycode(ffi::VK_OEM_4), Some(KeyCode::BracketLeft));
        assert_eq!(vk_to_keycode(ffi::VK_OEM_6), Some(KeyCode::BracketRight));
        assert_eq!(vk_to_keycode(ffi::VK_OEM_5), Some(KeyCode::Backslash));
        assert_eq!(vk_to_keycode(ffi::VK_OEM_MINUS), Some(KeyCode::Minus));
        assert_eq!(vk_to_keycode(ffi::VK_OEM_PLUS), Some(KeyCode::Equal));
        assert_eq!(vk_to_keycode(ffi::VK_OEM_3), Some(KeyCode::Grave));
    }

    #[test]
    fn vk_common_keys() {
        assert_eq!(vk_to_keycode(ffi::VK_ESCAPE), Some(KeyCode::Escape));
        assert_eq!(vk_to_keycode(ffi::VK_RETURN), Some(KeyCode::Enter));
        assert_eq!(vk_to_keycode(ffi::VK_TAB), Some(KeyCode::Tab));
        assert_eq!(vk_to_keycode(ffi::VK_BACK), Some(KeyCode::Backspace));
        assert_eq!(vk_to_keycode(ffi::VK_SPACE), Some(KeyCode::Space));
    }

    #[test]
    fn vk_lock_keys() {
        assert_eq!(vk_to_keycode(ffi::VK_CAPITAL), Some(KeyCode::CapsLock));
        assert_eq!(vk_to_keycode(ffi::VK_NUMLOCK), Some(KeyCode::NumLock));
        assert_eq!(vk_to_keycode(ffi::VK_SCROLL), Some(KeyCode::ScrollLock));
    }

    #[test]
    fn vk_unknown_returns_none() {
        assert!(vk_to_keycode(0x00).is_none());
        assert!(vk_to_keycode(0xFF).is_none());
        assert!(vk_to_keycode(0x07).is_none()); // undefined VK
    }

    #[test]
    fn vk_context_menu() {
        assert_eq!(vk_to_keycode(ffi::VK_APPS), Some(KeyCode::ContextMenu));
    }

    // ── scancode_to_keycode tests ───────────────────────────────────

    #[test]
    fn scancode_escape() {
        assert_eq!(scancode_to_keycode(0x01), Some(KeyCode::Escape));
    }

    #[test]
    fn scancode_digit_row() {
        assert_eq!(scancode_to_keycode(0x02), Some(KeyCode::Digit1));
        assert_eq!(scancode_to_keycode(0x0B), Some(KeyCode::Digit0));
    }

    #[test]
    fn scancode_qwerty_row() {
        assert_eq!(scancode_to_keycode(0x10), Some(KeyCode::Q));
        assert_eq!(scancode_to_keycode(0x11), Some(KeyCode::W));
        assert_eq!(scancode_to_keycode(0x12), Some(KeyCode::E));
        assert_eq!(scancode_to_keycode(0x13), Some(KeyCode::R));
        assert_eq!(scancode_to_keycode(0x14), Some(KeyCode::T));
        assert_eq!(scancode_to_keycode(0x15), Some(KeyCode::Y));
    }

    #[test]
    fn scancode_home_row() {
        assert_eq!(scancode_to_keycode(0x1E), Some(KeyCode::A));
        assert_eq!(scancode_to_keycode(0x1F), Some(KeyCode::S));
        assert_eq!(scancode_to_keycode(0x20), Some(KeyCode::D));
        assert_eq!(scancode_to_keycode(0x21), Some(KeyCode::F));
    }

    #[test]
    fn scancode_function_keys() {
        assert_eq!(scancode_to_keycode(0x3B), Some(KeyCode::F1));
        assert_eq!(scancode_to_keycode(0x44), Some(KeyCode::F10));
        assert_eq!(scancode_to_keycode(0x57), Some(KeyCode::F11));
        assert_eq!(scancode_to_keycode(0x58), Some(KeyCode::F12));
    }

    #[test]
    fn scancode_extended_arrow_keys() {
        assert_eq!(scancode_to_keycode(0x148), Some(KeyCode::ArrowUp));
        assert_eq!(scancode_to_keycode(0x150), Some(KeyCode::ArrowDown));
        assert_eq!(scancode_to_keycode(0x14B), Some(KeyCode::ArrowLeft));
        assert_eq!(scancode_to_keycode(0x14D), Some(KeyCode::ArrowRight));
    }

    #[test]
    fn scancode_extended_navigation() {
        assert_eq!(scancode_to_keycode(0x147), Some(KeyCode::Home));
        assert_eq!(scancode_to_keycode(0x14F), Some(KeyCode::End));
        assert_eq!(scancode_to_keycode(0x149), Some(KeyCode::PageUp));
        assert_eq!(scancode_to_keycode(0x151), Some(KeyCode::PageDown));
        assert_eq!(scancode_to_keycode(0x152), Some(KeyCode::Insert));
        assert_eq!(scancode_to_keycode(0x153), Some(KeyCode::Delete));
    }

    #[test]
    fn scancode_extended_modifiers() {
        assert_eq!(scancode_to_keycode(0x11D), Some(KeyCode::RightCtrl));
        assert_eq!(scancode_to_keycode(0x138), Some(KeyCode::RightAlt));
        assert_eq!(scancode_to_keycode(0x15B), Some(KeyCode::LeftSuper));
        assert_eq!(scancode_to_keycode(0x15C), Some(KeyCode::RightSuper));
    }

    #[test]
    fn scancode_unknown_returns_none() {
        assert!(scancode_to_keycode(0x00).is_none());
        assert!(scancode_to_keycode(0xFFFF).is_none());
        assert!(scancode_to_keycode(0x37).is_none()); // gap in table
    }

    #[test]
    fn scancode_numpad_enter() {
        assert_eq!(scancode_to_keycode(0x11C), Some(KeyCode::Enter));
    }
}
