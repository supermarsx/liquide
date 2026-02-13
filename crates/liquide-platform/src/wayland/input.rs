//! Linux/Wayland scancode-to-keycode translation and modifier mapping.
//!
//! Maps Linux `input-event-codes.h` scancodes (as delivered by
//! `wl_keyboard::key`) to [`KeyCode`] values, and translates Wayland
//! modifier state bitmasks to [`Modifiers`].

use liquide_input::keyboard::{KeyCode, Modifiers};

/// Map a Linux input event scancode to our logical [`KeyCode`].
///
/// Scancodes correspond to `KEY_*` constants from
/// `<linux/input-event-codes.h>`.  The Wayland `wl_keyboard::key` event
/// delivers these values directly.
#[must_use]
pub fn linux_scancode_to_keycode(scancode: u32) -> Option<KeyCode> {
    match scancode {
        // Row 0: Escape, function keys
        1 => Some(KeyCode::Escape),
        59 => Some(KeyCode::F1),
        60 => Some(KeyCode::F2),
        61 => Some(KeyCode::F3),
        62 => Some(KeyCode::F4),
        63 => Some(KeyCode::F5),
        64 => Some(KeyCode::F6),
        65 => Some(KeyCode::F7),
        66 => Some(KeyCode::F8),
        67 => Some(KeyCode::F9),
        68 => Some(KeyCode::F10),
        87 => Some(KeyCode::F11),
        88 => Some(KeyCode::F12),
        99 => Some(KeyCode::PrintScreen),
        70 => Some(KeyCode::ScrollLock),
        119 => Some(KeyCode::Pause),

        // Row 1: Number row
        41 => Some(KeyCode::Grave),
        2 => Some(KeyCode::Digit1),
        3 => Some(KeyCode::Digit2),
        4 => Some(KeyCode::Digit3),
        5 => Some(KeyCode::Digit4),
        6 => Some(KeyCode::Digit5),
        7 => Some(KeyCode::Digit6),
        8 => Some(KeyCode::Digit7),
        9 => Some(KeyCode::Digit8),
        10 => Some(KeyCode::Digit9),
        11 => Some(KeyCode::Digit0),
        12 => Some(KeyCode::Minus),
        13 => Some(KeyCode::Equal),
        14 => Some(KeyCode::Backspace),

        // Row 2: Tab + QWERTY
        15 => Some(KeyCode::Tab),
        16 => Some(KeyCode::Q),
        17 => Some(KeyCode::W),
        18 => Some(KeyCode::E),
        19 => Some(KeyCode::R),
        20 => Some(KeyCode::T),
        21 => Some(KeyCode::Y),
        22 => Some(KeyCode::U),
        23 => Some(KeyCode::I),
        24 => Some(KeyCode::O),
        25 => Some(KeyCode::P),
        26 => Some(KeyCode::BracketLeft),
        27 => Some(KeyCode::BracketRight),
        28 => Some(KeyCode::Enter),

        // Row 3: Caps + ASDF
        58 => Some(KeyCode::CapsLock),
        30 => Some(KeyCode::A),
        31 => Some(KeyCode::S),
        32 => Some(KeyCode::D),
        33 => Some(KeyCode::F),
        34 => Some(KeyCode::G),
        35 => Some(KeyCode::H),
        36 => Some(KeyCode::J),
        37 => Some(KeyCode::K),
        38 => Some(KeyCode::L),
        39 => Some(KeyCode::Semicolon),
        40 => Some(KeyCode::Quote),
        43 => Some(KeyCode::Backslash),

        // Row 4: Shift + ZXCV
        42 => Some(KeyCode::LeftShift),
        44 => Some(KeyCode::Z),
        45 => Some(KeyCode::X),
        46 => Some(KeyCode::C),
        47 => Some(KeyCode::V),
        48 => Some(KeyCode::B),
        49 => Some(KeyCode::N),
        50 => Some(KeyCode::M),
        51 => Some(KeyCode::Comma),
        52 => Some(KeyCode::Period),
        53 => Some(KeyCode::Slash),
        54 => Some(KeyCode::RightShift),

        // Row 5: Control row
        29 => Some(KeyCode::LeftCtrl),
        125 => Some(KeyCode::LeftSuper),
        56 => Some(KeyCode::LeftAlt),
        57 => Some(KeyCode::Space),
        100 => Some(KeyCode::RightAlt),
        126 => Some(KeyCode::RightSuper),
        127 => Some(KeyCode::ContextMenu),
        97 => Some(KeyCode::RightCtrl),

        // Navigation cluster
        110 => Some(KeyCode::Insert),
        111 => Some(KeyCode::Delete),
        102 => Some(KeyCode::Home),
        107 => Some(KeyCode::End),
        104 => Some(KeyCode::PageUp),
        109 => Some(KeyCode::PageDown),

        // Arrow keys
        103 => Some(KeyCode::ArrowUp),
        108 => Some(KeyCode::ArrowDown),
        105 => Some(KeyCode::ArrowLeft),
        106 => Some(KeyCode::ArrowRight),

        // Numpad lock
        69 => Some(KeyCode::NumLock),

        _ => None,
    }
}

/// Convert Wayland modifier state bitmasks to our [`Modifiers`] flags.
///
/// The `mods_depressed` bitmask comes from `wl_keyboard::modifiers` and
/// uses the XKB modifier indices.  On a standard PC keymap the bits are:
///
/// - bit 0 (0x01): Shift
/// - bit 2 (0x04): Control
/// - bit 3 (0x08): Mod1 (Alt)
/// - bit 6 (0x40): Mod4 (Super/Logo)
///
/// `mods_locked` carries lock state:
///
/// - bit 1 (0x02): Caps Lock
/// - bit 4 (0x10): Num Lock
#[must_use]
pub fn wayland_modifiers_to_modifiers(mods_depressed: u32, mods_locked: u32) -> Modifiers {
    let mut bits: u8 = 0;

    if mods_depressed & 0x01 != 0 {
        bits |= Modifiers::SHIFT;
    }
    if mods_depressed & 0x04 != 0 {
        bits |= Modifiers::CTRL;
    }
    if mods_depressed & 0x08 != 0 {
        bits |= Modifiers::ALT;
    }
    if mods_depressed & 0x40 != 0 {
        bits |= Modifiers::SUPER;
    }
    if mods_locked & 0x02 != 0 {
        bits |= Modifiers::CAPS_LOCK;
    }
    if mods_locked & 0x10 != 0 {
        bits |= Modifiers::NUM_LOCK;
    }

    Modifiers::from_bits(bits)
}
