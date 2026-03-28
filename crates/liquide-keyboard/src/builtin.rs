//! Built-in keyboard layout definitions.
//!
//! Each function returns a complete `KeyboardLayout` with scancode mappings
//! for the main alphabetic, numeric, and punctuation keys (47+ keys each).
//!
//! Scancodes follow the USB HID usage table (essentially the physical key
//! position, independent of layout):
//!
//! | Scancode | Physical key (ANSI) |
//! |----------|---------------------|
//! | 0x02-0x0B | 1 2 3 4 5 6 7 8 9 0 |
//! | 0x0C-0x0D | - = |
//! | 0x10-0x19 | Q W E R T Y U I O P (row 1) |
//! | 0x1A-0x1B | [ ] |
//! | 0x1E-0x26 | A S D F G H J K L (row 2) |
//! | 0x27-0x29 | ; ' ` |
//! | 0x2B      | \ (backslash) |
//! | 0x2C-0x32 | Z X C V B N M (row 3) |
//! | 0x33-0x35 | , . / |
//! | 0x39      | Space |

use std::collections::HashMap;

use crate::layout::{DeadKey, KeyMapping, KeyboardLayout};

// ── Scancode constants (AT set 1) ──────────────────────────────────────────

// Number row
const SC_1: u32 = 0x02;
const SC_2: u32 = 0x03;
const SC_3: u32 = 0x04;
const SC_4: u32 = 0x05;
const SC_5: u32 = 0x06;
const SC_6: u32 = 0x07;
const SC_7: u32 = 0x08;
const SC_8: u32 = 0x09;
const SC_9: u32 = 0x0A;
const SC_0: u32 = 0x0B;
const SC_MINUS: u32 = 0x0C;
const SC_EQUALS: u32 = 0x0D;

// Top alpha row (QWERTY positions)
const SC_Q: u32 = 0x10;
const SC_W: u32 = 0x11;
const SC_E: u32 = 0x12;
const SC_R: u32 = 0x13;
const SC_T: u32 = 0x14;
const SC_Y: u32 = 0x15;
const SC_U: u32 = 0x16;
const SC_I: u32 = 0x17;
const SC_O: u32 = 0x18;
const SC_P: u32 = 0x19;
const SC_LBRACKET: u32 = 0x1A;
const SC_RBRACKET: u32 = 0x1B;

// Home row
const SC_A: u32 = 0x1E;
const SC_S: u32 = 0x1F;
const SC_D: u32 = 0x20;
const SC_F: u32 = 0x21;
const SC_G: u32 = 0x22;
const SC_H: u32 = 0x23;
const SC_J: u32 = 0x24;
const SC_K: u32 = 0x25;
const SC_L: u32 = 0x26;
const SC_SEMICOLON: u32 = 0x27;
const SC_APOSTROPHE: u32 = 0x28;
const SC_GRAVE: u32 = 0x29;

// Backslash
const SC_BACKSLASH: u32 = 0x2B;

// Bottom row
const SC_Z: u32 = 0x2C;
const SC_X: u32 = 0x2D;
const SC_C: u32 = 0x2E;
const SC_V: u32 = 0x2F;
const SC_B: u32 = 0x30;
const SC_N: u32 = 0x31;
const SC_M: u32 = 0x32;
const SC_COMMA: u32 = 0x33;
const SC_PERIOD: u32 = 0x34;
const SC_SLASH: u32 = 0x35;

// Space
const SC_SPACE: u32 = 0x39;

/// US QWERTY keyboard layout (ANSI 104-key).
pub fn layout_us_qwerty() -> KeyboardLayout {
    let mut layout = KeyboardLayout::new("us", "English (US)", "en");

    // Number row
    layout.insert(SC_1, KeyMapping::simple('1', '!'));
    layout.insert(SC_2, KeyMapping::simple('2', '@'));
    layout.insert(SC_3, KeyMapping::simple('3', '#'));
    layout.insert(SC_4, KeyMapping::simple('4', '$'));
    layout.insert(SC_5, KeyMapping::simple('5', '%'));
    layout.insert(SC_6, KeyMapping::simple('6', '^'));
    layout.insert(SC_7, KeyMapping::simple('7', '&'));
    layout.insert(SC_8, KeyMapping::simple('8', '*'));
    layout.insert(SC_9, KeyMapping::simple('9', '('));
    layout.insert(SC_0, KeyMapping::simple('0', ')'));
    layout.insert(SC_MINUS, KeyMapping::simple('-', '_'));
    layout.insert(SC_EQUALS, KeyMapping::simple('=', '+'));

    // Top row (QWERTY)
    layout.insert(SC_Q, KeyMapping::simple('q', 'Q'));
    layout.insert(SC_W, KeyMapping::simple('w', 'W'));
    layout.insert(SC_E, KeyMapping::simple('e', 'E'));
    layout.insert(SC_R, KeyMapping::simple('r', 'R'));
    layout.insert(SC_T, KeyMapping::simple('t', 'T'));
    layout.insert(SC_Y, KeyMapping::simple('y', 'Y'));
    layout.insert(SC_U, KeyMapping::simple('u', 'U'));
    layout.insert(SC_I, KeyMapping::simple('i', 'I'));
    layout.insert(SC_O, KeyMapping::simple('o', 'O'));
    layout.insert(SC_P, KeyMapping::simple('p', 'P'));
    layout.insert(SC_LBRACKET, KeyMapping::simple('[', '{'));
    layout.insert(SC_RBRACKET, KeyMapping::simple(']', '}'));

    // Home row
    layout.insert(SC_A, KeyMapping::simple('a', 'A'));
    layout.insert(SC_S, KeyMapping::simple('s', 'S'));
    layout.insert(SC_D, KeyMapping::simple('d', 'D'));
    layout.insert(SC_F, KeyMapping::simple('f', 'F'));
    layout.insert(SC_G, KeyMapping::simple('g', 'G'));
    layout.insert(SC_H, KeyMapping::simple('h', 'H'));
    layout.insert(SC_J, KeyMapping::simple('j', 'J'));
    layout.insert(SC_K, KeyMapping::simple('k', 'K'));
    layout.insert(SC_L, KeyMapping::simple('l', 'L'));
    layout.insert(SC_SEMICOLON, KeyMapping::simple(';', ':'));
    layout.insert(SC_APOSTROPHE, KeyMapping::simple('\'', '"'));
    layout.insert(SC_GRAVE, KeyMapping::simple('`', '~'));

    // Backslash
    layout.insert(SC_BACKSLASH, KeyMapping::simple('\\', '|'));

    // Bottom row
    layout.insert(SC_Z, KeyMapping::simple('z', 'Z'));
    layout.insert(SC_X, KeyMapping::simple('x', 'X'));
    layout.insert(SC_C, KeyMapping::simple('c', 'C'));
    layout.insert(SC_V, KeyMapping::simple('v', 'V'));
    layout.insert(SC_B, KeyMapping::simple('b', 'B'));
    layout.insert(SC_N, KeyMapping::simple('n', 'N'));
    layout.insert(SC_M, KeyMapping::simple('m', 'M'));
    layout.insert(SC_COMMA, KeyMapping::simple(',', '<'));
    layout.insert(SC_PERIOD, KeyMapping::simple('.', '>'));
    layout.insert(SC_SLASH, KeyMapping::simple('/', '?'));

    // Space
    layout.insert(SC_SPACE, KeyMapping::uniform(' '));

    layout
}

/// UK QWERTY keyboard layout (ISO 105-key).
///
/// Differences from US: shifted 2 = ", 3 = \u{00a3}, `~ replaced by \u{00ac}\u{00a6},
/// hash key on SC_BACKSLASH, AltGr on several keys.
pub fn layout_uk_qwerty() -> KeyboardLayout {
    let mut layout = KeyboardLayout::new("uk", "English (UK)", "en");
    layout.variant = Some("uk".to_string());

    // Number row
    layout.insert(SC_1, KeyMapping::simple('1', '!'));
    layout.insert(SC_2, KeyMapping::simple('2', '"'));
    layout.insert(SC_3, KeyMapping::with_alt_gr('3', '\u{00a3}', '#')); // £, AltGr=#
    layout.insert(SC_4, KeyMapping::with_alt_gr('4', '$', '\u{20ac}')); // AltGr=€
    layout.insert(SC_5, KeyMapping::simple('5', '%'));
    layout.insert(SC_6, KeyMapping::simple('6', '^'));
    layout.insert(SC_7, KeyMapping::simple('7', '&'));
    layout.insert(SC_8, KeyMapping::simple('8', '*'));
    layout.insert(SC_9, KeyMapping::simple('9', '('));
    layout.insert(SC_0, KeyMapping::simple('0', ')'));
    layout.insert(SC_MINUS, KeyMapping::simple('-', '_'));
    layout.insert(SC_EQUALS, KeyMapping::simple('=', '+'));

    // Top row (QWERTY)
    layout.insert(SC_Q, KeyMapping::simple('q', 'Q'));
    layout.insert(SC_W, KeyMapping::simple('w', 'W'));
    layout.insert(SC_E, KeyMapping::with_alt_gr('e', 'E', '\u{00e9}')); // AltGr=é
    layout.insert(SC_R, KeyMapping::simple('r', 'R'));
    layout.insert(SC_T, KeyMapping::simple('t', 'T'));
    layout.insert(SC_Y, KeyMapping::simple('y', 'Y'));
    layout.insert(SC_U, KeyMapping::with_alt_gr('u', 'U', '\u{00fa}')); // AltGr=ú
    layout.insert(SC_I, KeyMapping::with_alt_gr('i', 'I', '\u{00ed}')); // AltGr=í
    layout.insert(SC_O, KeyMapping::with_alt_gr('o', 'O', '\u{00f3}')); // AltGr=ó
    layout.insert(SC_P, KeyMapping::simple('p', 'P'));
    layout.insert(SC_LBRACKET, KeyMapping::simple('[', '{'));
    layout.insert(SC_RBRACKET, KeyMapping::simple(']', '}'));

    // Home row
    layout.insert(SC_A, KeyMapping::with_alt_gr('a', 'A', '\u{00e1}')); // AltGr=á
    layout.insert(SC_S, KeyMapping::simple('s', 'S'));
    layout.insert(SC_D, KeyMapping::simple('d', 'D'));
    layout.insert(SC_F, KeyMapping::simple('f', 'F'));
    layout.insert(SC_G, KeyMapping::simple('g', 'G'));
    layout.insert(SC_H, KeyMapping::simple('h', 'H'));
    layout.insert(SC_J, KeyMapping::simple('j', 'J'));
    layout.insert(SC_K, KeyMapping::simple('k', 'K'));
    layout.insert(SC_L, KeyMapping::simple('l', 'L'));
    layout.insert(SC_SEMICOLON, KeyMapping::simple(';', ':'));
    layout.insert(SC_APOSTROPHE, KeyMapping::simple('\'', '@'));
    layout.insert(SC_GRAVE, KeyMapping::simple('`', '\u{00ac}')); // ¬

    // Hash / backslash
    layout.insert(SC_BACKSLASH, KeyMapping::simple('#', '~'));

    // Bottom row
    layout.insert(SC_Z, KeyMapping::simple('z', 'Z'));
    layout.insert(SC_X, KeyMapping::simple('x', 'X'));
    layout.insert(SC_C, KeyMapping::simple('c', 'C'));
    layout.insert(SC_V, KeyMapping::simple('v', 'V'));
    layout.insert(SC_B, KeyMapping::simple('b', 'B'));
    layout.insert(SC_N, KeyMapping::simple('n', 'N'));
    layout.insert(SC_M, KeyMapping::simple('m', 'M'));
    layout.insert(SC_COMMA, KeyMapping::simple(',', '<'));
    layout.insert(SC_PERIOD, KeyMapping::simple('.', '>'));
    layout.insert(SC_SLASH, KeyMapping::simple('/', '?'));

    // Space
    layout.insert(SC_SPACE, KeyMapping::uniform(' '));

    layout
}

/// German QWERTZ keyboard layout.
///
/// Z and Y are swapped, umlauts on bracket/semicolon/apostrophe positions,
/// AltGr produces @, €, etc.
pub fn layout_de_qwertz() -> KeyboardLayout {
    let mut layout = KeyboardLayout::new("de", "German", "de");

    // Dead key definitions for German layout
    let dk_circumflex = DeadKey {
        id: 1,
        base_char: '^',
        combinations: {
            let mut m = HashMap::new();
            m.insert('a', '\u{00e2}'); // â
            m.insert('e', '\u{00ea}'); // ê
            m.insert('i', '\u{00ee}'); // î
            m.insert('o', '\u{00f4}'); // ô
            m.insert('u', '\u{00fb}'); // û
            m.insert('A', '\u{00c2}'); // Â
            m.insert('E', '\u{00ca}'); // Ê
            m.insert('I', '\u{00ce}'); // Î
            m.insert('O', '\u{00d4}'); // Ô
            m.insert('U', '\u{00db}'); // Û
            m
        },
        fallback: '^',
    };
    let dk_acute = DeadKey {
        id: 2,
        base_char: '\u{00b4}', // ´
        combinations: {
            let mut m = HashMap::new();
            m.insert('a', '\u{00e1}'); // á
            m.insert('e', '\u{00e9}'); // é
            m.insert('i', '\u{00ed}'); // í
            m.insert('o', '\u{00f3}'); // ó
            m.insert('u', '\u{00fa}'); // ú
            m.insert('A', '\u{00c1}'); // Á
            m.insert('E', '\u{00c9}'); // É
            m.insert('I', '\u{00cd}'); // Í
            m.insert('O', '\u{00d3}'); // Ó
            m.insert('U', '\u{00da}'); // Ú
            m
        },
        fallback: '\u{00b4}',
    };
    let dk_grave = DeadKey {
        id: 3,
        base_char: '`',
        combinations: {
            let mut m = HashMap::new();
            m.insert('a', '\u{00e0}'); // à
            m.insert('e', '\u{00e8}'); // è
            m.insert('i', '\u{00ec}'); // ì
            m.insert('o', '\u{00f2}'); // ò
            m.insert('u', '\u{00f9}'); // ù
            m.insert('A', '\u{00c0}'); // À
            m.insert('E', '\u{00c8}'); // È
            m.insert('I', '\u{00cc}'); // Ì
            m.insert('O', '\u{00d2}'); // Ò
            m.insert('U', '\u{00d9}'); // Ù
            m
        },
        fallback: '`',
    };
    layout.insert_dead_key(dk_circumflex);
    layout.insert_dead_key(dk_acute);
    layout.insert_dead_key(dk_grave);

    // Number row
    layout.insert(SC_1, KeyMapping::simple('1', '!'));
    layout.insert(SC_2, KeyMapping::with_alt_gr('2', '"', '\u{00b2}')); // ²
    layout.insert(SC_3, KeyMapping::with_alt_gr('3', '\u{00a7}', '\u{00b3}')); // §, ³
    layout.insert(SC_4, KeyMapping::simple('4', '$'));
    layout.insert(SC_5, KeyMapping::simple('5', '%'));
    layout.insert(SC_6, KeyMapping::simple('6', '&'));
    layout.insert(SC_7, KeyMapping::with_alt_gr('7', '/', '{')); // AltGr={
    layout.insert(SC_8, KeyMapping::with_alt_gr('8', '(', '[')); // AltGr=[
    layout.insert(SC_9, KeyMapping::with_alt_gr('9', ')', ']')); // AltGr=]
    layout.insert(SC_0, KeyMapping::with_alt_gr('0', '=', '}')); // AltGr=}
    layout.insert(SC_MINUS, KeyMapping::with_alt_gr('\u{00df}', '?', '\\')); // ß, AltGr=backslash
    layout.insert(SC_EQUALS, KeyMapping::dead('\u{00b4}', Some('`'), 2)); // dead acute, shift=dead grave

    // Top row (QWERTZ: note Y is at SC_Z position, Z at SC_Y)
    layout.insert(SC_Q, KeyMapping::with_alt_gr('q', 'Q', '@')); // AltGr=@
    layout.insert(SC_W, KeyMapping::simple('w', 'W'));
    layout.insert(SC_E, KeyMapping::with_alt_gr('e', 'E', '\u{20ac}')); // AltGr=€
    layout.insert(SC_R, KeyMapping::simple('r', 'R'));
    layout.insert(SC_T, KeyMapping::simple('t', 'T'));
    layout.insert(SC_Y, KeyMapping::simple('z', 'Z')); // Z on QWERTY-Y position
    layout.insert(SC_U, KeyMapping::simple('u', 'U'));
    layout.insert(SC_I, KeyMapping::simple('i', 'I'));
    layout.insert(SC_O, KeyMapping::simple('o', 'O'));
    layout.insert(SC_P, KeyMapping::simple('p', 'P'));
    layout.insert(SC_LBRACKET, KeyMapping::simple('\u{00fc}', '\u{00dc}')); // ü, Ü
    layout.insert(SC_RBRACKET, KeyMapping::with_alt_gr('+', '*', '~')); // AltGr=~

    // Home row
    layout.insert(SC_A, KeyMapping::simple('a', 'A'));
    layout.insert(SC_S, KeyMapping::simple('s', 'S'));
    layout.insert(SC_D, KeyMapping::simple('d', 'D'));
    layout.insert(SC_F, KeyMapping::simple('f', 'F'));
    layout.insert(SC_G, KeyMapping::simple('g', 'G'));
    layout.insert(SC_H, KeyMapping::simple('h', 'H'));
    layout.insert(SC_J, KeyMapping::simple('j', 'J'));
    layout.insert(SC_K, KeyMapping::simple('k', 'K'));
    layout.insert(SC_L, KeyMapping::simple('l', 'L'));
    layout.insert(SC_SEMICOLON, KeyMapping::simple('\u{00f6}', '\u{00d6}')); // ö, Ö
    layout.insert(SC_APOSTROPHE, KeyMapping::simple('\u{00e4}', '\u{00c4}')); // ä, Ä
    layout.insert(SC_GRAVE, KeyMapping::dead('^', Some('\u{00b0}'), 1)); // dead circumflex, shift=°

    // Backslash position (German: # ')
    layout.insert(SC_BACKSLASH, KeyMapping::simple('#', '\''));

    // Bottom row (Z and Y swapped)
    layout.insert(SC_Z, KeyMapping::simple('y', 'Y')); // Y on QWERTY-Z position
    layout.insert(SC_X, KeyMapping::simple('x', 'X'));
    layout.insert(SC_C, KeyMapping::simple('c', 'C'));
    layout.insert(SC_V, KeyMapping::simple('v', 'V'));
    layout.insert(SC_B, KeyMapping::simple('b', 'B'));
    layout.insert(SC_N, KeyMapping::simple('n', 'N'));
    layout.insert(SC_M, KeyMapping::with_alt_gr('m', 'M', '\u{00b5}')); // AltGr=µ
    layout.insert(SC_COMMA, KeyMapping::simple(',', ';'));
    layout.insert(SC_PERIOD, KeyMapping::simple('.', ':'));
    layout.insert(SC_SLASH, KeyMapping::simple('-', '_'));

    // Space
    layout.insert(SC_SPACE, KeyMapping::uniform(' '));

    layout
}

/// French AZERTY keyboard layout.
///
/// Top row starts A-Z-E-R-T-Y, number row needs Shift for digits,
/// unshifted produces accented/special characters.
pub fn layout_fr_azerty() -> KeyboardLayout {
    let mut layout = KeyboardLayout::new("fr", "French (AZERTY)", "fr");

    // Dead keys for French
    let dk_circumflex = DeadKey {
        id: 1,
        base_char: '^',
        combinations: {
            let mut m = HashMap::new();
            m.insert('a', '\u{00e2}');
            m.insert('e', '\u{00ea}');
            m.insert('i', '\u{00ee}');
            m.insert('o', '\u{00f4}');
            m.insert('u', '\u{00fb}');
            m.insert('A', '\u{00c2}');
            m.insert('E', '\u{00ca}');
            m.insert('I', '\u{00ce}');
            m.insert('O', '\u{00d4}');
            m.insert('U', '\u{00db}');
            m
        },
        fallback: '^',
    };
    let dk_diaeresis = DeadKey {
        id: 4,
        base_char: '\u{00a8}', // ¨
        combinations: {
            let mut m = HashMap::new();
            m.insert('a', '\u{00e4}');
            m.insert('e', '\u{00eb}');
            m.insert('i', '\u{00ef}');
            m.insert('o', '\u{00f6}');
            m.insert('u', '\u{00fc}');
            m.insert('y', '\u{00ff}');
            m.insert('A', '\u{00c4}');
            m.insert('E', '\u{00cb}');
            m.insert('I', '\u{00cf}');
            m.insert('O', '\u{00d6}');
            m.insert('U', '\u{00dc}');
            m
        },
        fallback: '\u{00a8}',
    };
    layout.insert_dead_key(dk_circumflex);
    layout.insert_dead_key(dk_diaeresis);

    // Number row — unshifted gives symbols/accents, Shift gives digits
    layout.insert(SC_1, KeyMapping::simple('&', '1'));
    layout.insert(SC_2, KeyMapping::with_alt_gr('\u{00e9}', '2', '~')); // é, AltGr=~
    layout.insert(SC_3, KeyMapping::with_alt_gr('"', '3', '#')); // AltGr=#
    layout.insert(SC_4, KeyMapping::with_alt_gr('\'', '4', '{')); // AltGr={
    layout.insert(SC_5, KeyMapping::with_alt_gr('(', '5', '[')); // AltGr=[
    layout.insert(SC_6, KeyMapping::with_alt_gr('-', '6', '|')); // AltGr=|
    layout.insert(SC_7, KeyMapping::with_alt_gr('\u{00e8}', '7', '`')); // è, AltGr=`
    layout.insert(SC_8, KeyMapping::with_alt_gr('_', '8', '\\')); // AltGr=backslash
    layout.insert(SC_9, KeyMapping::with_alt_gr('\u{00e7}', '9', '^')); // ç, AltGr=^
    layout.insert(SC_0, KeyMapping::with_alt_gr('\u{00e0}', '0', '@')); // à, AltGr=@
    layout.insert(SC_MINUS, KeyMapping::with_alt_gr(')', '\u{00b0}', ']')); // °, AltGr=]
    layout.insert(SC_EQUALS, KeyMapping::with_alt_gr('=', '+', '}')); // AltGr=}

    // Top row (AZERTY)
    layout.insert(SC_Q, KeyMapping::simple('a', 'A')); // A on QWERTY-Q
    layout.insert(SC_W, KeyMapping::simple('z', 'Z')); // Z on QWERTY-W
    layout.insert(SC_E, KeyMapping::with_alt_gr('e', 'E', '\u{20ac}')); // AltGr=€
    layout.insert(SC_R, KeyMapping::simple('r', 'R'));
    layout.insert(SC_T, KeyMapping::simple('t', 'T'));
    layout.insert(SC_Y, KeyMapping::simple('y', 'Y'));
    layout.insert(SC_U, KeyMapping::simple('u', 'U'));
    layout.insert(SC_I, KeyMapping::simple('i', 'I'));
    layout.insert(SC_O, KeyMapping::simple('o', 'O'));
    layout.insert(SC_P, KeyMapping::simple('p', 'P'));
    layout.insert(SC_LBRACKET, KeyMapping::dead('^', Some('\u{00a8}'), 1)); // dead ^, shift=dead ¨
    layout.insert(SC_RBRACKET, KeyMapping::with_alt_gr('$', '\u{00a3}', '\u{00a4}')); // £, AltGr=¤

    // Home row (AZERTY: Q and M differ)
    layout.insert(SC_A, KeyMapping::simple('q', 'Q')); // Q on QWERTY-A
    layout.insert(SC_S, KeyMapping::simple('s', 'S'));
    layout.insert(SC_D, KeyMapping::simple('d', 'D'));
    layout.insert(SC_F, KeyMapping::simple('f', 'F'));
    layout.insert(SC_G, KeyMapping::simple('g', 'G'));
    layout.insert(SC_H, KeyMapping::simple('h', 'H'));
    layout.insert(SC_J, KeyMapping::simple('j', 'J'));
    layout.insert(SC_K, KeyMapping::simple('k', 'K'));
    layout.insert(SC_L, KeyMapping::simple('l', 'L'));
    layout.insert(SC_SEMICOLON, KeyMapping::simple('m', 'M')); // M on semicolon position
    layout.insert(SC_APOSTROPHE, KeyMapping::simple('\u{00f9}', '%')); // ù
    layout.insert(SC_GRAVE, KeyMapping::simple('\u{00b2}', '\u{00b2}')); // ²

    // Backslash position
    layout.insert(SC_BACKSLASH, KeyMapping::simple('*', '\u{00b5}')); // µ

    // Bottom row (AZERTY: W on Z position)
    layout.insert(SC_Z, KeyMapping::simple('w', 'W')); // W on QWERTY-Z
    layout.insert(SC_X, KeyMapping::simple('x', 'X'));
    layout.insert(SC_C, KeyMapping::simple('c', 'C'));
    layout.insert(SC_V, KeyMapping::simple('v', 'V'));
    layout.insert(SC_B, KeyMapping::simple('b', 'B'));
    layout.insert(SC_N, KeyMapping::simple('n', 'N'));
    layout.insert(SC_M, KeyMapping::simple(',', '?')); // comma on QWERTY-M
    layout.insert(SC_COMMA, KeyMapping::simple(';', '.'));
    layout.insert(SC_PERIOD, KeyMapping::simple(':', '/'));
    layout.insert(SC_SLASH, KeyMapping::simple('!', '\u{00a7}')); // §

    // Space
    layout.insert(SC_SPACE, KeyMapping::uniform(' '));

    layout
}

/// US Dvorak keyboard layout.
///
/// Rearranged letter positions for ergonomic typing. The number row is
/// the same as US QWERTY, but the alpha and punctuation keys differ.
pub fn layout_us_dvorak() -> KeyboardLayout {
    let mut layout = KeyboardLayout::new("us-dvorak", "English (Dvorak)", "en");
    layout.variant = Some("dvorak".to_string());

    // Number row (same as US QWERTY)
    layout.insert(SC_1, KeyMapping::simple('1', '!'));
    layout.insert(SC_2, KeyMapping::simple('2', '@'));
    layout.insert(SC_3, KeyMapping::simple('3', '#'));
    layout.insert(SC_4, KeyMapping::simple('4', '$'));
    layout.insert(SC_5, KeyMapping::simple('5', '%'));
    layout.insert(SC_6, KeyMapping::simple('6', '^'));
    layout.insert(SC_7, KeyMapping::simple('7', '&'));
    layout.insert(SC_8, KeyMapping::simple('8', '*'));
    layout.insert(SC_9, KeyMapping::simple('9', '('));
    layout.insert(SC_0, KeyMapping::simple('0', ')'));
    layout.insert(SC_MINUS, KeyMapping::simple('[', '{'));
    layout.insert(SC_EQUALS, KeyMapping::simple(']', '}'));

    // Top row: ' , . p y f g c r l / =
    layout.insert(SC_Q, KeyMapping::simple('\'', '"'));
    layout.insert(SC_W, KeyMapping::simple(',', '<'));
    layout.insert(SC_E, KeyMapping::simple('.', '>'));
    layout.insert(SC_R, KeyMapping::simple('p', 'P'));
    layout.insert(SC_T, KeyMapping::simple('y', 'Y'));
    layout.insert(SC_Y, KeyMapping::simple('f', 'F'));
    layout.insert(SC_U, KeyMapping::simple('g', 'G'));
    layout.insert(SC_I, KeyMapping::simple('c', 'C'));
    layout.insert(SC_O, KeyMapping::simple('r', 'R'));
    layout.insert(SC_P, KeyMapping::simple('l', 'L'));
    layout.insert(SC_LBRACKET, KeyMapping::simple('/', '?'));
    layout.insert(SC_RBRACKET, KeyMapping::simple('=', '+'));

    // Home row: a o e u i d h t n s - \
    layout.insert(SC_A, KeyMapping::simple('a', 'A'));
    layout.insert(SC_S, KeyMapping::simple('o', 'O'));
    layout.insert(SC_D, KeyMapping::simple('e', 'E'));
    layout.insert(SC_F, KeyMapping::simple('u', 'U'));
    layout.insert(SC_G, KeyMapping::simple('i', 'I'));
    layout.insert(SC_H, KeyMapping::simple('d', 'D'));
    layout.insert(SC_J, KeyMapping::simple('h', 'H'));
    layout.insert(SC_K, KeyMapping::simple('t', 'T'));
    layout.insert(SC_L, KeyMapping::simple('n', 'N'));
    layout.insert(SC_SEMICOLON, KeyMapping::simple('s', 'S'));
    layout.insert(SC_APOSTROPHE, KeyMapping::simple('-', '_'));
    layout.insert(SC_GRAVE, KeyMapping::simple('`', '~'));

    // Backslash
    layout.insert(SC_BACKSLASH, KeyMapping::simple('\\', '|'));

    // Bottom row: ; q j k x b m w v z
    layout.insert(SC_Z, KeyMapping::simple(';', ':'));
    layout.insert(SC_X, KeyMapping::simple('q', 'Q'));
    layout.insert(SC_C, KeyMapping::simple('j', 'J'));
    layout.insert(SC_V, KeyMapping::simple('k', 'K'));
    layout.insert(SC_B, KeyMapping::simple('x', 'X'));
    layout.insert(SC_N, KeyMapping::simple('b', 'B'));
    layout.insert(SC_M, KeyMapping::simple('m', 'M'));
    layout.insert(SC_COMMA, KeyMapping::simple('w', 'W'));
    layout.insert(SC_PERIOD, KeyMapping::simple('v', 'V'));
    layout.insert(SC_SLASH, KeyMapping::simple('z', 'Z'));

    // Space
    layout.insert(SC_SPACE, KeyMapping::uniform(' '));

    layout
}

/// Return all built-in layouts.
pub fn all_builtin_layouts() -> Vec<KeyboardLayout> {
    vec![
        layout_us_qwerty(),
        layout_uk_qwerty(),
        layout_de_qwertz(),
        layout_fr_azerty(),
        layout_us_dvorak(),
    ]
}
