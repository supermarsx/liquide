//! X11 keysym to `liquide_input` type mapping.
//!
//! Translates X11 keysyms, modifier masks, and button identifiers into
//! the `liquide_input` types used by the rest of the desktop engine.

use liquide_input::keyboard::{KeyCode, Modifiers};
use liquide_input::mouse::MouseButton;

use super::ffi;

/// Map an X11 keysym value to a `KeyCode`.
///
/// Returns `None` for keysyms that do not have a corresponding `KeyCode`
/// variant (e.g. exotic dead keys, multi-key sequences).
pub fn keysym_to_keycode(keysym: u64) -> Option<KeyCode> {
    // Uppercase Latin letters map to the same KeyCode as lowercase.
    let keysym = match keysym {
        ffi::XK_A => ffi::XK_a,
        ffi::XK_B => ffi::XK_b,
        ffi::XK_C => ffi::XK_c,
        ffi::XK_D => ffi::XK_d,
        ffi::XK_E => ffi::XK_e,
        ffi::XK_F => ffi::XK_f,
        ffi::XK_G => ffi::XK_g,
        ffi::XK_H => ffi::XK_h,
        ffi::XK_I => ffi::XK_i,
        ffi::XK_J => ffi::XK_j,
        ffi::XK_K => ffi::XK_k,
        ffi::XK_L => ffi::XK_l,
        ffi::XK_M => ffi::XK_m,
        ffi::XK_N => ffi::XK_n,
        ffi::XK_O => ffi::XK_o,
        ffi::XK_P => ffi::XK_p,
        ffi::XK_Q => ffi::XK_q,
        ffi::XK_R => ffi::XK_r,
        ffi::XK_S => ffi::XK_s,
        ffi::XK_T => ffi::XK_t,
        ffi::XK_U => ffi::XK_u,
        ffi::XK_V => ffi::XK_v,
        ffi::XK_W => ffi::XK_w,
        ffi::XK_X => ffi::XK_x,
        ffi::XK_Y => ffi::XK_y,
        ffi::XK_Z => ffi::XK_z,
        other => other,
    };

    match keysym {
        // Letters
        ffi::XK_a => Some(KeyCode::A),
        ffi::XK_b => Some(KeyCode::B),
        ffi::XK_c => Some(KeyCode::C),
        ffi::XK_d => Some(KeyCode::D),
        ffi::XK_e => Some(KeyCode::E),
        ffi::XK_f => Some(KeyCode::F),
        ffi::XK_g => Some(KeyCode::G),
        ffi::XK_h => Some(KeyCode::H),
        ffi::XK_i => Some(KeyCode::I),
        ffi::XK_j => Some(KeyCode::J),
        ffi::XK_k => Some(KeyCode::K),
        ffi::XK_l => Some(KeyCode::L),
        ffi::XK_m => Some(KeyCode::M),
        ffi::XK_n => Some(KeyCode::N),
        ffi::XK_o => Some(KeyCode::O),
        ffi::XK_p => Some(KeyCode::P),
        ffi::XK_q => Some(KeyCode::Q),
        ffi::XK_r => Some(KeyCode::R),
        ffi::XK_s => Some(KeyCode::S),
        ffi::XK_t => Some(KeyCode::T),
        ffi::XK_u => Some(KeyCode::U),
        ffi::XK_v => Some(KeyCode::V),
        ffi::XK_w => Some(KeyCode::W),
        ffi::XK_x => Some(KeyCode::X),
        ffi::XK_y => Some(KeyCode::Y),
        ffi::XK_z => Some(KeyCode::Z),

        // Digits
        ffi::XK_0 => Some(KeyCode::Digit0),
        ffi::XK_1 => Some(KeyCode::Digit1),
        ffi::XK_2 => Some(KeyCode::Digit2),
        ffi::XK_3 => Some(KeyCode::Digit3),
        ffi::XK_4 => Some(KeyCode::Digit4),
        ffi::XK_5 => Some(KeyCode::Digit5),
        ffi::XK_6 => Some(KeyCode::Digit6),
        ffi::XK_7 => Some(KeyCode::Digit7),
        ffi::XK_8 => Some(KeyCode::Digit8),
        ffi::XK_9 => Some(KeyCode::Digit9),

        // Function keys
        ffi::XK_F1 => Some(KeyCode::F1),
        ffi::XK_F2 => Some(KeyCode::F2),
        ffi::XK_F3 => Some(KeyCode::F3),
        ffi::XK_F4 => Some(KeyCode::F4),
        ffi::XK_F5 => Some(KeyCode::F5),
        ffi::XK_F6 => Some(KeyCode::F6),
        ffi::XK_F7 => Some(KeyCode::F7),
        ffi::XK_F8 => Some(KeyCode::F8),
        ffi::XK_F9 => Some(KeyCode::F9),
        ffi::XK_F10 => Some(KeyCode::F10),
        ffi::XK_F11 => Some(KeyCode::F11),
        ffi::XK_F12 => Some(KeyCode::F12),

        // Special keys
        ffi::XK_Escape => Some(KeyCode::Escape),
        ffi::XK_Return => Some(KeyCode::Enter),
        ffi::XK_Tab => Some(KeyCode::Tab),
        ffi::XK_BackSpace => Some(KeyCode::Backspace),
        ffi::XK_space => Some(KeyCode::Space),

        // Navigation
        ffi::XK_Up => Some(KeyCode::ArrowUp),
        ffi::XK_Down => Some(KeyCode::ArrowDown),
        ffi::XK_Left => Some(KeyCode::ArrowLeft),
        ffi::XK_Right => Some(KeyCode::ArrowRight),
        ffi::XK_Home => Some(KeyCode::Home),
        ffi::XK_End => Some(KeyCode::End),
        ffi::XK_Page_Up => Some(KeyCode::PageUp),
        ffi::XK_Page_Down => Some(KeyCode::PageDown),
        ffi::XK_Insert => Some(KeyCode::Insert),
        ffi::XK_Delete => Some(KeyCode::Delete),

        // Lock keys
        ffi::XK_Caps_Lock => Some(KeyCode::CapsLock),
        ffi::XK_Num_Lock => Some(KeyCode::NumLock),
        ffi::XK_Scroll_Lock => Some(KeyCode::ScrollLock),

        // System keys
        ffi::XK_Print => Some(KeyCode::PrintScreen),
        ffi::XK_Pause => Some(KeyCode::Pause),
        ffi::XK_Menu => Some(KeyCode::ContextMenu),

        // Modifier keys
        ffi::XK_Shift_L => Some(KeyCode::LeftShift),
        ffi::XK_Shift_R => Some(KeyCode::RightShift),
        ffi::XK_Control_L => Some(KeyCode::LeftCtrl),
        ffi::XK_Control_R => Some(KeyCode::RightCtrl),
        ffi::XK_Alt_L => Some(KeyCode::LeftAlt),
        ffi::XK_Alt_R => Some(KeyCode::RightAlt),
        ffi::XK_Super_L => Some(KeyCode::LeftSuper),
        ffi::XK_Super_R => Some(KeyCode::RightSuper),

        // Punctuation / symbols
        ffi::XK_comma => Some(KeyCode::Comma),
        ffi::XK_period => Some(KeyCode::Period),
        ffi::XK_slash => Some(KeyCode::Slash),
        ffi::XK_semicolon => Some(KeyCode::Semicolon),
        ffi::XK_apostrophe => Some(KeyCode::Quote),
        ffi::XK_bracketleft => Some(KeyCode::BracketLeft),
        ffi::XK_bracketright => Some(KeyCode::BracketRight),
        ffi::XK_backslash => Some(KeyCode::Backslash),
        ffi::XK_minus => Some(KeyCode::Minus),
        ffi::XK_equal => Some(KeyCode::Equal),
        ffi::XK_grave => Some(KeyCode::Grave),

        _ => None,
    }
}

/// Convert an X11 modifier state bitmask to `Modifiers`.
pub fn x11_modifiers_to_modifiers(state: u32) -> Modifiers {
    let mut bits: u8 = 0;

    if state & ffi::ShiftMask != 0 {
        bits |= Modifiers::SHIFT;
    }
    if state & ffi::ControlMask != 0 {
        bits |= Modifiers::CTRL;
    }
    if state & ffi::Mod1Mask != 0 {
        bits |= Modifiers::ALT;
    }
    if state & ffi::Mod4Mask != 0 {
        bits |= Modifiers::SUPER;
    }
    if state & ffi::LockMask != 0 {
        bits |= Modifiers::CAPS_LOCK;
    }
    if state & ffi::Mod2Mask != 0 {
        bits |= Modifiers::NUM_LOCK;
    }

    Modifiers::from_bits(bits)
}

/// Convert an X11 button number to a `MouseButton`.
///
/// Returns `None` for button numbers that represent scroll events
/// (Button4 / Button5 on traditional X11), since those are handled
/// separately as scroll events.
pub fn x11_button_to_mouse_button(button: u32) -> Option<MouseButton> {
    match button {
        ffi::Button1 => Some(MouseButton::Left),
        ffi::Button2 => Some(MouseButton::Middle),
        ffi::Button3 => Some(MouseButton::Right),
        // Button4 / Button5 are vertical scroll — not mapped to mouse buttons.
        ffi::Button4 | ffi::Button5 => None,
        // X11 buttons 6..7 are horizontal scroll on some setups, 8+ are extra.
        6 | 7 => None,
        8 => Some(MouseButton::Back),
        9 => Some(MouseButton::Forward),
        other => Some(MouseButton::Other(other as u8)),
    }
}
