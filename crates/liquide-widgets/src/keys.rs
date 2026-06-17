//! Self-contained keyboard key encoding for widget behaviors.
//!
//! [`KeyInput`](crate::behavior::KeyInput) carries a raw `u32` key code so a
//! behavior can be driven by either a real platform keyboard event or a
//! synthesized one without depending on `liquide-input`. This module fixes the
//! encoding the widget toolkit uses:
//!
//! - **Printable keys** are encoded as their Unicode scalar value (`'a' as u32`,
//!   `'A' as u32`, `'5' as u32`, `' ' as u32`, …). A behavior that edits a text
//!   buffer can therefore turn a key directly into the character it inserts via
//!   [`printable_char`].
//! - **Named / control keys** (Enter, Backspace, the arrows, Home/End, …) live
//!   in a high private range starting at [`SPECIAL_BASE`] so they never collide
//!   with a printable codepoint.
//!
//! The shell input layer (`liquide-input::KeyCode`) is the production source of
//! these events; the seam that maps a `KeyCode` to this encoding lives at the
//! shell boundary (P8), out of this crate's lock. Keeping the encoding here makes
//! the widget tests self-contained and the contract explicit.

/// Modifier bitflags (match `liquide_input::Modifiers` bit positions).
pub mod modifiers {
    /// Shift held.
    pub const SHIFT: u32 = 0x01;
    /// Control held.
    pub const CTRL: u32 = 0x02;
    /// Alt held.
    pub const ALT: u32 = 0x04;
    /// Super / Meta held.
    pub const SUPER: u32 = 0x08;
}

/// First code point of the named-key range. Above the Unicode max scalar value
/// (`0x10_FFFF`) so named keys never alias a printable character.
pub const SPECIAL_BASE: u32 = 0x0011_0000;

/// Enter / Return.
pub const ENTER: u32 = SPECIAL_BASE + 1;
/// Tab.
pub const TAB: u32 = SPECIAL_BASE + 2;
/// Backspace.
pub const BACKSPACE: u32 = SPECIAL_BASE + 3;
/// Delete (forward delete).
pub const DELETE: u32 = SPECIAL_BASE + 4;
/// Escape.
pub const ESCAPE: u32 = SPECIAL_BASE + 5;
/// Left arrow.
pub const ARROW_LEFT: u32 = SPECIAL_BASE + 6;
/// Right arrow.
pub const ARROW_RIGHT: u32 = SPECIAL_BASE + 7;
/// Up arrow.
pub const ARROW_UP: u32 = SPECIAL_BASE + 8;
/// Down arrow.
pub const ARROW_DOWN: u32 = SPECIAL_BASE + 9;
/// Home.
pub const HOME: u32 = SPECIAL_BASE + 10;
/// End.
pub const END: u32 = SPECIAL_BASE + 11;
/// Page Up.
pub const PAGE_UP: u32 = SPECIAL_BASE + 12;
/// Page Down.
pub const PAGE_DOWN: u32 = SPECIAL_BASE + 13;

/// Space — also a printable character (`' '`), kept as a named alias for the
/// activation semantics shared by buttons/toggles ("Space activates").
pub const SPACE: u32 = ' ' as u32;

/// If `key` is a printable character (its codepoint is below [`SPECIAL_BASE`]
/// and is a valid, non-control Unicode scalar), return it. Backspace/Enter/etc.
/// return `None` because they are control actions, not insertable text.
pub fn printable_char(key: u32) -> Option<char> {
    if key >= SPECIAL_BASE {
        return None;
    }
    let c = char::from_u32(key)?;
    if c.is_control() {
        return None;
    }
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printables_decode_to_their_char() {
        assert_eq!(printable_char('a' as u32), Some('a'));
        assert_eq!(printable_char('Z' as u32), Some('Z'));
        assert_eq!(printable_char('5' as u32), Some('5'));
        assert_eq!(printable_char(' ' as u32), Some(' '));
    }

    #[test]
    fn named_keys_are_not_printable_and_dont_alias() {
        assert_eq!(printable_char(ENTER), None);
        assert_eq!(printable_char(BACKSPACE), None);
        assert_eq!(printable_char(ARROW_LEFT), None);
        // No named key collides with a printable codepoint.
        for k in [ENTER, BACKSPACE, DELETE, ARROW_LEFT, HOME, END, PAGE_UP] {
            assert!(k > 0x10_FFFF, "named key {k} must be above the Unicode max");
        }
    }
}
