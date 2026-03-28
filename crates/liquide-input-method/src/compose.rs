//! Compose sequence and dead key handler.
//!
//! Implements XKB-style compose sequences: a series of keysyms maps to a
//! single output character. For example, `<dead_acute> <a>` produces `á`.

/// Result of feeding a key into the compose table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposeResult {
    /// A compose sequence is in progress (more keys needed).
    Composing,
    /// A complete sequence was matched, producing this character.
    Committed(char),
    /// The sequence was invalid and has been cancelled.
    Cancelled,
}

/// A compose table mapping key sequences to output characters.
///
/// Each entry is a pair of `(keysym_sequence, output_char)`. When the user
/// types a sequence of keysyms that matches an entry, the corresponding
/// character is produced.
#[derive(Debug, Clone)]
pub struct ComposeTable {
    /// The table of compose sequences.
    sequences: Vec<(Vec<u32>, char)>,
    /// Current in-progress sequence buffer.
    buffer: Vec<u32>,
}

impl ComposeTable {
    /// Create an empty compose table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sequences: Vec::new(),
            buffer: Vec::new(),
        }
    }

    /// Create a compose table from a list of sequences.
    #[must_use]
    pub fn from_sequences(sequences: Vec<(Vec<u32>, char)>) -> Self {
        Self {
            sequences,
            buffer: Vec::new(),
        }
    }

    /// Add a compose sequence.
    pub fn add_sequence(&mut self, keys: Vec<u32>, output: char) {
        self.sequences.push((keys, output));
    }

    /// Number of sequences in the table.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sequences.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sequences.is_empty()
    }

    /// Feed a keysym into the compose state machine.
    ///
    /// Returns `Composing` if more keys are needed, `Committed(ch)` if a
    /// sequence was completed, or `Cancelled` if no sequence can match.
    pub fn feed_key(&mut self, keysym: u32) -> ComposeResult {
        self.buffer.push(keysym);

        // Check for exact match first.
        for (seq, ch) in &self.sequences {
            if *seq == self.buffer {
                let result = *ch;
                self.buffer.clear();
                return ComposeResult::Committed(result);
            }
        }

        // Check if any sequence has this as a prefix.
        let is_prefix = self.sequences.iter().any(|(seq, _)| {
            seq.len() > self.buffer.len()
                && seq[..self.buffer.len()] == self.buffer[..]
        });

        if is_prefix {
            ComposeResult::Composing
        } else {
            self.buffer.clear();
            ComposeResult::Cancelled
        }
    }

    /// Reset the compose state, discarding any in-progress sequence.
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    /// Whether a compose sequence is currently in progress.
    #[must_use]
    pub fn is_composing(&self) -> bool {
        !self.buffer.is_empty()
    }

    /// Get the current compose buffer (keys pressed so far).
    #[must_use]
    pub fn buffer(&self) -> &[u32] {
        &self.buffer
    }
}

impl Default for ComposeTable {
    fn default() -> Self {
        Self::new()
    }
}

// XKB keysym constants used in compose sequences.
// These match the X11 keysym definitions (keysymdef.h).
// Not all constants are used in the default table, but they are kept for
// extensibility and as a reference.
#[allow(dead_code)]
const XK_DEAD_ACUTE: u32 = 0xfe51;
const XK_DEAD_GRAVE: u32 = 0xfe50;
const XK_DEAD_DIAERESIS: u32 = 0xfe57;
const XK_DEAD_TILDE: u32 = 0xfe53;
const XK_DEAD_CIRCUMFLEX: u32 = 0xfe52;
const XK_DEAD_CEDILLA: u32 = 0xfe55;
const XK_DEAD_RING_ABOVE: u32 = 0xfe58;
const XK_DEAD_STROKE: u32 = 0xfe60;
const XK_DEAD_CARON: u32 = 0xfe5a;
const XK_DEAD_MACRON: u32 = 0xfe54;
const XK_MULTI_KEY: u32 = 0xff20; // Compose key

// Latin letter keysyms (ASCII range).
const XK_A_LOWER: u32 = 0x0061;
const XK_A_UPPER: u32 = 0x0041;
const XK_C_LOWER: u32 = 0x0063;
const XK_C_UPPER: u32 = 0x0043;
const XK_D_LOWER: u32 = 0x0064;
const XK_D_UPPER: u32 = 0x0044;
const XK_E_LOWER: u32 = 0x0065;
const XK_E_UPPER: u32 = 0x0045;
const XK_I_LOWER: u32 = 0x0069;
const XK_I_UPPER: u32 = 0x0049;
const XK_L_LOWER: u32 = 0x006c;
const XK_L_UPPER: u32 = 0x004c;
const XK_N_LOWER: u32 = 0x006e;
const XK_N_UPPER: u32 = 0x004e;
const XK_O_LOWER: u32 = 0x006f;
const XK_O_UPPER: u32 = 0x004f;
const XK_R_LOWER: u32 = 0x0072;
const XK_R_UPPER: u32 = 0x0052;
const XK_S_LOWER: u32 = 0x0073;
const XK_S_UPPER: u32 = 0x0053;
const XK_U_LOWER: u32 = 0x0075;
const XK_U_UPPER: u32 = 0x0055;
const XK_Y_LOWER: u32 = 0x0079;
const XK_Y_UPPER: u32 = 0x0059;
const XK_Z_LOWER: u32 = 0x007a;
const XK_Z_UPPER: u32 = 0x005a;

// Punctuation / symbol keysyms.
const XK_MINUS: u32 = 0x002d;
const XK_EQUAL: u32 = 0x003d;
const XK_SLASH: u32 = 0x002f;
const XK_EXCLAM: u32 = 0x0021;
const XK_QUESTION: u32 = 0x003f;
const XK_LESS: u32 = 0x003c;
const XK_GREATER: u32 = 0x003e;
const XK_PERIOD: u32 = 0x002e;
const XK_COLON: u32 = 0x003a;
const XK_PARENLEFT: u32 = 0x0028;
const XK_PARENRIGHT: u32 = 0x0029;
const XK_PLUS: u32 = 0x002b;
const XK_SPACE: u32 = 0x0020;
const XK_0: u32 = 0x0030;
const XK_1: u32 = 0x0031;
const XK_2: u32 = 0x0032;
const XK_3: u32 = 0x0033;
const XK_4: u32 = 0x0034;

/// Create the default compose table with ~50 common sequences.
///
/// Includes:
/// - Dead key + letter sequences for Latin accented characters
/// - Multi_key (Compose) sequences for special characters, currency, and math
#[must_use]
pub fn default_compose_table() -> ComposeTable {
    let sequences: Vec<(Vec<u32>, char)> = vec![
        // ===== Dead key sequences =====

        // Acute accent: ´ + vowel
        (vec![XK_DEAD_ACUTE, XK_A_LOWER], '\u{00e1}'), // á
        (vec![XK_DEAD_ACUTE, XK_A_UPPER], '\u{00c1}'), // Á
        (vec![XK_DEAD_ACUTE, XK_E_LOWER], '\u{00e9}'), // é
        (vec![XK_DEAD_ACUTE, XK_E_UPPER], '\u{00c9}'), // É
        (vec![XK_DEAD_ACUTE, XK_I_LOWER], '\u{00ed}'), // í
        (vec![XK_DEAD_ACUTE, XK_I_UPPER], '\u{00cd}'), // Í
        (vec![XK_DEAD_ACUTE, XK_O_LOWER], '\u{00f3}'), // ó
        (vec![XK_DEAD_ACUTE, XK_O_UPPER], '\u{00d3}'), // Ó
        (vec![XK_DEAD_ACUTE, XK_U_LOWER], '\u{00fa}'), // ú
        (vec![XK_DEAD_ACUTE, XK_U_UPPER], '\u{00da}'), // Ú
        (vec![XK_DEAD_ACUTE, XK_Y_LOWER], '\u{00fd}'), // ý
        (vec![XK_DEAD_ACUTE, XK_Y_UPPER], '\u{00dd}'), // Ý

        // Grave accent: ` + vowel
        (vec![XK_DEAD_GRAVE, XK_A_LOWER], '\u{00e0}'), // à
        (vec![XK_DEAD_GRAVE, XK_E_LOWER], '\u{00e8}'), // è
        (vec![XK_DEAD_GRAVE, XK_I_LOWER], '\u{00ec}'), // ì
        (vec![XK_DEAD_GRAVE, XK_O_LOWER], '\u{00f2}'), // ò
        (vec![XK_DEAD_GRAVE, XK_U_LOWER], '\u{00f9}'), // ù

        // Diaeresis (umlaut): ¨ + vowel
        (vec![XK_DEAD_DIAERESIS, XK_A_LOWER], '\u{00e4}'), // ä
        (vec![XK_DEAD_DIAERESIS, XK_A_UPPER], '\u{00c4}'), // Ä
        (vec![XK_DEAD_DIAERESIS, XK_E_LOWER], '\u{00eb}'), // ë
        (vec![XK_DEAD_DIAERESIS, XK_I_LOWER], '\u{00ef}'), // ï
        (vec![XK_DEAD_DIAERESIS, XK_O_LOWER], '\u{00f6}'), // ö
        (vec![XK_DEAD_DIAERESIS, XK_O_UPPER], '\u{00d6}'), // Ö
        (vec![XK_DEAD_DIAERESIS, XK_U_LOWER], '\u{00fc}'), // ü
        (vec![XK_DEAD_DIAERESIS, XK_U_UPPER], '\u{00dc}'), // Ü
        (vec![XK_DEAD_DIAERESIS, XK_Y_LOWER], '\u{00ff}'), // ÿ

        // Tilde: ~ + letter
        (vec![XK_DEAD_TILDE, XK_N_LOWER], '\u{00f1}'), // ñ
        (vec![XK_DEAD_TILDE, XK_N_UPPER], '\u{00d1}'), // Ñ
        (vec![XK_DEAD_TILDE, XK_A_LOWER], '\u{00e3}'), // ã
        (vec![XK_DEAD_TILDE, XK_O_LOWER], '\u{00f5}'), // õ

        // Circumflex: ^ + vowel
        (vec![XK_DEAD_CIRCUMFLEX, XK_A_LOWER], '\u{00e2}'), // â
        (vec![XK_DEAD_CIRCUMFLEX, XK_E_LOWER], '\u{00ea}'), // ê
        (vec![XK_DEAD_CIRCUMFLEX, XK_I_LOWER], '\u{00ee}'), // î
        (vec![XK_DEAD_CIRCUMFLEX, XK_O_LOWER], '\u{00f4}'), // ô
        (vec![XK_DEAD_CIRCUMFLEX, XK_U_LOWER], '\u{00fb}'), // û

        // Cedilla: ¸ + letter
        (vec![XK_DEAD_CEDILLA, XK_C_LOWER], '\u{00e7}'), // ç
        (vec![XK_DEAD_CEDILLA, XK_C_UPPER], '\u{00c7}'), // Ç

        // Ring above: ° + letter
        (vec![XK_DEAD_RING_ABOVE, XK_A_LOWER], '\u{00e5}'), // å
        (vec![XK_DEAD_RING_ABOVE, XK_A_UPPER], '\u{00c5}'), // Å
        (vec![XK_DEAD_RING_ABOVE, XK_U_LOWER], '\u{016f}'), // ů

        // Stroke: - through letter
        (vec![XK_DEAD_STROKE, XK_O_LOWER], '\u{00f8}'), // ø
        (vec![XK_DEAD_STROKE, XK_O_UPPER], '\u{00d8}'), // Ø
        (vec![XK_DEAD_STROKE, XK_L_LOWER], '\u{0142}'), // ł
        (vec![XK_DEAD_STROKE, XK_D_LOWER], '\u{0111}'), // đ

        // Caron (háček): ˇ + letter
        (vec![XK_DEAD_CARON, XK_C_LOWER], '\u{010d}'), // č
        (vec![XK_DEAD_CARON, XK_S_LOWER], '\u{0161}'), // š
        (vec![XK_DEAD_CARON, XK_Z_LOWER], '\u{017e}'), // ž
        (vec![XK_DEAD_CARON, XK_R_LOWER], '\u{0159}'), // ř

        // Macron: ¯ + vowel
        (vec![XK_DEAD_MACRON, XK_A_LOWER], '\u{0101}'), // ā
        (vec![XK_DEAD_MACRON, XK_E_LOWER], '\u{0113}'), // ē
        (vec![XK_DEAD_MACRON, XK_I_LOWER], '\u{012b}'), // ī
        (vec![XK_DEAD_MACRON, XK_O_LOWER], '\u{014d}'), // ō
        (vec![XK_DEAD_MACRON, XK_U_LOWER], '\u{016b}'), // ū

        // ===== Multi_key (Compose) sequences =====

        // Currency symbols
        (vec![XK_MULTI_KEY, XK_E_LOWER, XK_EQUAL], '\u{20ac}'),       // €
        (vec![XK_MULTI_KEY, XK_L_LOWER, XK_MINUS], '\u{00a3}'),       // £
        (vec![XK_MULTI_KEY, XK_Y_LOWER, XK_EQUAL], '\u{00a5}'),       // ¥
        (vec![XK_MULTI_KEY, XK_C_LOWER, XK_SLASH], '\u{00a2}'),       // ¢
        (vec![XK_MULTI_KEY, XK_C_LOWER, XK_EQUAL], '\u{20ac}'),       // € (alt)

        // Math symbols
        (vec![XK_MULTI_KEY, XK_PLUS, XK_MINUS], '\u{00b1}'),          // ±
        (vec![XK_MULTI_KEY, XK_MINUS, XK_COLON], '\u{00f7}'),         // ÷
        (vec![XK_MULTI_KEY, XK_LESS, XK_EQUAL], '\u{2264}'),          // ≤
        (vec![XK_MULTI_KEY, XK_GREATER, XK_EQUAL], '\u{2265}'),       // ≥
        (vec![XK_MULTI_KEY, XK_SLASH, XK_EQUAL], '\u{2260}'),         // ≠
        (vec![XK_MULTI_KEY, XK_PERIOD, XK_PERIOD], '\u{2026}'),       // …
        (vec![XK_MULTI_KEY, XK_MINUS, XK_MINUS], '\u{2014}'),         // — (em dash)
        (vec![XK_MULTI_KEY, XK_PERIOD, XK_MINUS], '\u{2013}'),        // – (en dash)
        (vec![XK_MULTI_KEY, XK_LESS, XK_LESS], '\u{00ab}'),           // «
        (vec![XK_MULTI_KEY, XK_GREATER, XK_GREATER], '\u{00bb}'),     // »

        // Special characters
        (vec![XK_MULTI_KEY, XK_S_LOWER, XK_S_LOWER], '\u{00df}'),     // ß
        (vec![XK_MULTI_KEY, XK_EXCLAM, XK_EXCLAM], '\u{00a1}'),       // ¡
        (vec![XK_MULTI_KEY, XK_QUESTION, XK_QUESTION], '\u{00bf}'),   // ¿
        (vec![XK_MULTI_KEY, XK_O_LOWER, XK_C_LOWER], '\u{00a9}'),     // ©
        (vec![XK_MULTI_KEY, XK_O_LOWER, XK_R_LOWER], '\u{00ae}'),     // ®
        (vec![XK_MULTI_KEY, XK_PARENLEFT, XK_C_LOWER], '\u{00a9}'),   // © (alt)
        (vec![XK_MULTI_KEY, XK_PARENLEFT, XK_R_LOWER], '\u{00ae}'),   // ® (alt)
        (vec![XK_MULTI_KEY, XK_SPACE, XK_SPACE], '\u{00a0}'),         // non-breaking space
        (vec![XK_MULTI_KEY, XK_0_LOWER, XK_C_LOWER], '\u{00b0}'),     // °

        // Superscripts
        (vec![XK_MULTI_KEY, XK_CIRCUMFLEX, XK_0], '\u{2070}'),        // ⁰
        (vec![XK_MULTI_KEY, XK_CIRCUMFLEX, XK_1], '\u{00b9}'),        // ¹
        (vec![XK_MULTI_KEY, XK_CIRCUMFLEX, XK_2], '\u{00b2}'),        // ²
        (vec![XK_MULTI_KEY, XK_CIRCUMFLEX, XK_3], '\u{00b3}'),        // ³

        // Fractions
        (vec![XK_MULTI_KEY, XK_1, XK_2], '\u{00bd}'),                 // ½
        (vec![XK_MULTI_KEY, XK_1, XK_4], '\u{00bc}'),                 // ¼
        (vec![XK_MULTI_KEY, XK_3, XK_4], '\u{00be}'),                 // ¾
    ];

    ComposeTable::from_sequences(sequences)
}

// Alias constants used in Multi_key sequences that reference other keysyms.
const XK_CIRCUMFLEX: u32 = 0x005e;
const XK_0_LOWER: u32 = 0x0030;
