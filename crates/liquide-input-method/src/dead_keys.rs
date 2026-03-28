//! Dead key and multi-key compose sequence handling at the character level.
//!
//! Unlike [`ComposeTable`](crate::compose::ComposeTable) which works with X11 keysyms,
//! this module operates on Unicode characters directly, making it suitable for
//! platform-agnostic dead key processing.

use std::collections::HashMap;

/// Result of feeding a key into the dead key state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadKeyResult {
    /// A dead key combined with the input to produce a composed character.
    Composed(char),
    /// A dead key is pending -- waiting for the next character.
    Pending,
    /// The input was not consumed (no dead key pending and not a dead key itself).
    PassThrough,
}

/// Character-level dead key state machine.
///
/// Tracks a single pending dead key accent character and combines it with the
/// next character typed. If the combination is not in the map, both characters
/// are emitted.
pub struct DeadKeyState {
    /// The currently pending dead key character, if any.
    pending_dead: Option<char>,
    /// Map from (dead_char, base_char) -> composed_char.
    map: HashMap<(char, char), char>,
    /// Set of characters recognized as dead keys.
    dead_chars: Vec<char>,
}

impl DeadKeyState {
    /// Create a new dead key handler with the default Western European dead key map.
    #[must_use]
    pub fn new() -> Self {
        let (map, dead_chars) = default_dead_key_map();
        Self {
            pending_dead: None,
            map,
            dead_chars,
        }
    }

    /// Create a dead key handler from a custom map.
    #[must_use]
    pub fn with_map(entries: Vec<(char, char, char)>) -> Self {
        let mut dead_chars_set = Vec::new();
        let mut map = HashMap::new();
        for (dead, base, composed) in entries {
            if !dead_chars_set.contains(&dead) {
                dead_chars_set.push(dead);
            }
            map.insert((dead, base), composed);
        }
        Self {
            pending_dead: None,
            map,
            dead_chars: dead_chars_set,
        }
    }

    /// Get the currently pending dead key, if any.
    #[must_use]
    pub fn pending(&self) -> Option<char> {
        self.pending_dead
    }

    /// Reset the dead key state, discarding any pending dead key.
    pub fn reset(&mut self) {
        self.pending_dead = None;
    }

    /// Feed a character into the dead key state machine.
    ///
    /// - If no dead key is pending and `key` is a dead key, it becomes pending -> `Pending`.
    /// - If no dead key is pending and `key` is not a dead key -> `PassThrough`.
    /// - If a dead key is pending and (dead, key) is in the map -> `Composed(result)`.
    /// - If a dead key is pending but no mapping exists -> `PassThrough` (the pending
    ///   dead key is cleared; the caller should emit both the dead char and `key`).
    pub fn feed_key(&mut self, key: char) -> DeadKeyResult {
        if let Some(dead) = self.pending_dead.take() {
            // Try to compose dead + key.
            if let Some(&composed) = self.map.get(&(dead, key)) {
                DeadKeyResult::Composed(composed)
            } else {
                // No composition -- pass through (caller emits dead char + key).
                DeadKeyResult::PassThrough
            }
        } else if self.dead_chars.contains(&key) {
            self.pending_dead = Some(key);
            DeadKeyResult::Pending
        } else {
            DeadKeyResult::PassThrough
        }
    }
}

impl Default for DeadKeyState {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of feeding a character into the compose state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposeResult {
    /// A compose sequence was completed, producing this string.
    Composed(String),
    /// A compose sequence is in progress -- more characters needed.
    InProgress,
    /// The sequence does not match any compose entry.
    NoMatch,
}

/// Multi-key compose sequence handler at the character level.
///
/// After a compose key trigger (detected by the caller), characters are fed
/// into this state machine until a known sequence completes or the sequence
/// fails to match.
pub struct ComposeState {
    /// Characters accumulated so far.
    sequence: Vec<char>,
    /// Map from character sequence to composed output.
    table: Vec<(Vec<char>, String)>,
}

impl ComposeState {
    /// Create a new compose handler with the default compose sequences.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sequence: Vec::new(),
            table: default_compose_sequences(),
        }
    }

    /// Create a compose handler with custom sequences.
    #[must_use]
    pub fn with_sequences(table: Vec<(Vec<char>, String)>) -> Self {
        Self {
            sequence: Vec::new(),
            table,
        }
    }

    /// Get the current in-progress sequence.
    #[must_use]
    pub fn sequence(&self) -> &[char] {
        &self.sequence
    }

    /// Whether a compose sequence is in progress.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.sequence.is_empty()
    }

    /// Reset the compose state, discarding any in-progress sequence.
    pub fn reset(&mut self) {
        self.sequence.clear();
    }

    /// Feed a character into the compose state machine.
    pub fn feed(&mut self, c: char) -> ComposeResult {
        self.sequence.push(c);

        // Check for exact match.
        for (seq, output) in &self.table {
            if *seq == self.sequence {
                let result = output.clone();
                self.sequence.clear();
                return ComposeResult::Composed(result);
            }
        }

        // Check if any sequence has this as a prefix.
        let is_prefix = self.table.iter().any(|(seq, _)| {
            seq.len() > self.sequence.len()
                && seq[..self.sequence.len()] == self.sequence[..]
        });

        if is_prefix {
            ComposeResult::InProgress
        } else {
            self.sequence.clear();
            ComposeResult::NoMatch
        }
    }
}

impl Default for ComposeState {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default dead key map with 30+ common Western European compositions.
fn default_dead_key_map() -> (HashMap<(char, char), char>, Vec<char>) {
    // Dead key characters: acute, grave, circumflex, tilde, diaeresis, cedilla,
    // ring above, caron, macron, stroke.
    let dead_chars = vec![
        '\u{00B4}', // acute accent ´
        '`',        // grave accent
        '^',        // circumflex
        '~',        // tilde
        '\u{00A8}', // diaeresis ¨
        '\u{00B8}', // cedilla ¸
        '\u{00B0}', // ring above °
        '\u{02C7}', // caron ˇ
        '\u{00AF}', // macron ¯
        '\u{002D}', // stroke/dash (when used as dead key)
    ];

    let entries: Vec<(char, char, char)> = vec![
        // Acute accent (´)
        ('\u{00B4}', 'a', '\u{00E1}'), // á
        ('\u{00B4}', 'A', '\u{00C1}'), // Á
        ('\u{00B4}', 'e', '\u{00E9}'), // é
        ('\u{00B4}', 'E', '\u{00C9}'), // É
        ('\u{00B4}', 'i', '\u{00ED}'), // í
        ('\u{00B4}', 'I', '\u{00CD}'), // Í
        ('\u{00B4}', 'o', '\u{00F3}'), // ó
        ('\u{00B4}', 'O', '\u{00D3}'), // Ó
        ('\u{00B4}', 'u', '\u{00FA}'), // ú
        ('\u{00B4}', 'U', '\u{00DA}'), // Ú
        ('\u{00B4}', 'y', '\u{00FD}'), // ý
        ('\u{00B4}', 'Y', '\u{00DD}'), // Ý

        // Grave accent (`)
        ('`', 'a', '\u{00E0}'), // à
        ('`', 'A', '\u{00C0}'), // À
        ('`', 'e', '\u{00E8}'), // è
        ('`', 'E', '\u{00C8}'), // È
        ('`', 'i', '\u{00EC}'), // ì
        ('`', 'o', '\u{00F2}'), // ò
        ('`', 'u', '\u{00F9}'), // ù

        // Circumflex (^)
        ('^', 'a', '\u{00E2}'), // â
        ('^', 'A', '\u{00C2}'), // Â
        ('^', 'e', '\u{00EA}'), // ê
        ('^', 'i', '\u{00EE}'), // î
        ('^', 'o', '\u{00F4}'), // ô
        ('^', 'u', '\u{00FB}'), // û

        // Tilde (~)
        ('~', 'n', '\u{00F1}'), // ñ
        ('~', 'N', '\u{00D1}'), // Ñ
        ('~', 'a', '\u{00E3}'), // ã
        ('~', 'o', '\u{00F5}'), // õ

        // Diaeresis / umlaut (¨)
        ('\u{00A8}', 'a', '\u{00E4}'), // ä
        ('\u{00A8}', 'A', '\u{00C4}'), // Ä
        ('\u{00A8}', 'e', '\u{00EB}'), // ë
        ('\u{00A8}', 'i', '\u{00EF}'), // ï
        ('\u{00A8}', 'o', '\u{00F6}'), // ö
        ('\u{00A8}', 'O', '\u{00D6}'), // Ö
        ('\u{00A8}', 'u', '\u{00FC}'), // ü
        ('\u{00A8}', 'U', '\u{00DC}'), // Ü
        ('\u{00A8}', 'y', '\u{00FF}'), // ÿ

        // Cedilla (¸)
        ('\u{00B8}', 'c', '\u{00E7}'), // ç
        ('\u{00B8}', 'C', '\u{00C7}'), // Ç

        // Ring above (°)
        ('\u{00B0}', 'a', '\u{00E5}'), // å
        ('\u{00B0}', 'A', '\u{00C5}'), // Å
        ('\u{00B0}', 'u', '\u{016F}'), // ů

        // Caron (ˇ)
        ('\u{02C7}', 'c', '\u{010D}'), // č
        ('\u{02C7}', 's', '\u{0161}'), // š
        ('\u{02C7}', 'z', '\u{017E}'), // ž
        ('\u{02C7}', 'r', '\u{0159}'), // ř

        // Macron (¯)
        ('\u{00AF}', 'a', '\u{0101}'), // ā
        ('\u{00AF}', 'e', '\u{0113}'), // ē
        ('\u{00AF}', 'i', '\u{012B}'), // ī
        ('\u{00AF}', 'o', '\u{014D}'), // ō
        ('\u{00AF}', 'u', '\u{016B}'), // ū
    ];

    let mut map = HashMap::new();
    for (dead, base, composed) in &entries {
        map.insert((*dead, *base), *composed);
    }

    (map, dead_chars)
}

/// Build the default multi-key compose sequences.
fn default_compose_sequences() -> Vec<(Vec<char>, String)> {
    vec![
        // Copyright / registered / trademark
        (vec!['o', 'c'], "\u{00A9}".into()),       // ©
        (vec!['o', 'r'], "\u{00AE}".into()),       // ®
        (vec!['t', 'm'], "\u{2122}".into()),       // ™
        (vec!['(', 'c', ')'], "\u{00A9}".into()),  // ©
        (vec!['(', 'r', ')'], "\u{00AE}".into()),  // ®

        // Currency
        (vec!['e', '='], "\u{20AC}".into()),       // €
        (vec!['l', '-'], "\u{00A3}".into()),       // £
        (vec!['y', '='], "\u{00A5}".into()),       // ¥
        (vec!['c', '/'], "\u{00A2}".into()),       // ¢

        // Fractions
        (vec!['1', '2'], "\u{00BD}".into()),       // ½
        (vec!['1', '4'], "\u{00BC}".into()),       // ¼
        (vec!['3', '4'], "\u{00BE}".into()),       // ¾

        // Math symbols
        (vec!['+', '-'], "\u{00B1}".into()),       // ±
        (vec!['-', ':'], "\u{00F7}".into()),       // ÷
        (vec!['<', '='], "\u{2264}".into()),       // ≤
        (vec!['>', '='], "\u{2265}".into()),       // ≥
        (vec!['/', '='], "\u{2260}".into()),       // ≠
        (vec!['~', '='], "\u{2248}".into()),       // ≈
        (vec!['i', 'n', 'f'], "\u{221E}".into()), // ∞

        // Punctuation and typography
        (vec!['.', '.'], "\u{2026}".into()),       // …
        (vec!['-', '-'], "\u{2014}".into()),       // — (em dash)
        (vec!['.', '-'], "\u{2013}".into()),       // – (en dash)
        (vec!['<', '<'], "\u{00AB}".into()),       // «
        (vec!['>', '>'], "\u{00BB}".into()),       // »
        (vec!['!', '!'], "\u{00A1}".into()),       // ¡
        (vec!['?', '?'], "\u{00BF}".into()),       // ¿
        (vec!['s', 's'], "\u{00DF}".into()),       // ß
        (vec![' ', ' '], "\u{00A0}".into()),       // non-breaking space

        // Superscripts
        (vec!['^', '0'], "\u{2070}".into()),       // ⁰
        (vec!['^', '1'], "\u{00B9}".into()),       // ¹
        (vec!['^', '2'], "\u{00B2}".into()),       // ²
        (vec!['^', '3'], "\u{00B3}".into()),       // ³

        // Degree and section
        (vec!['o', 'o'], "\u{00B0}".into()),       // °
        (vec!['s', 'o'], "\u{00A7}".into()),       // §
        (vec!['p', 'i'], "\u{03C0}".into()),       // π
        (vec!['m', 'u'], "\u{00B5}".into()),       // µ
    ]
}
