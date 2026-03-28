//! Multi-key compose sequences (X11 Compose / XKB compose).
//!
//! Implements the freedesktop compose input method: the user presses a
//! compose key (Multi_key) followed by a sequence of keysyms to produce
//! a composed character. For example, Compose + ' + e = e with acute (é).

use std::collections::HashMap;

/// Status of a compose operation after feeding a keysym.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposeStatus {
    /// The keysym was not part of any compose sequence.
    Nothing,
    /// A compose sequence is in progress (more keysyms needed).
    Composing,
    /// A compose sequence completed successfully with the given result.
    Composed(String),
    /// A compose sequence was started but the keysym doesn't match
    /// any known continuation — sequence cancelled.
    Cancelled,
}

/// A node in the compose trie. Each node represents a point in a sequence.
#[derive(Debug, Clone)]
struct ComposeNode {
    /// Children: keysym -> next node index.
    children: HashMap<u32, usize>,
    /// If this node completes a sequence, the composed result.
    result: Option<String>,
}

impl ComposeNode {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            result: None,
        }
    }
}

/// Compose sequence table stored as a trie (prefix tree) for efficient
/// multi-key matching.
#[derive(Debug, Clone)]
pub struct ComposeTable {
    nodes: Vec<ComposeNode>,
}

impl ComposeTable {
    /// Create an empty compose table.
    pub fn new() -> Self {
        Self {
            nodes: vec![ComposeNode::new()], // root node at index 0
        }
    }

    /// Create a compose table pre-loaded with standard X11 compose sequences.
    pub fn with_defaults() -> Self {
        let mut table = Self::new();
        table.load_defaults();
        table
    }

    /// Add a compose sequence: a list of keysyms that produce the given result.
    ///
    /// If the sequence already exists, the result is replaced.
    pub fn add_sequence(&mut self, keysyms: &[u32], result: &str) {
        if keysyms.is_empty() {
            return;
        }

        let mut current = 0; // root
        for &ks in keysyms {
            let next = if let Some(&idx) = self.nodes[current].children.get(&ks) {
                idx
            } else {
                let idx = self.nodes.len();
                self.nodes.push(ComposeNode::new());
                self.nodes[current].children.insert(ks, idx);
                idx
            };
            current = next;
        }
        self.nodes[current].result = Some(result.to_string());
    }

    /// Number of complete sequences in the table.
    pub fn sequence_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.result.is_some()).count()
    }

    /// Check if a keysym has any compose sequence starting from a given node.
    fn has_child(&self, node_idx: usize, keysym: u32) -> Option<usize> {
        self.nodes[node_idx].children.get(&keysym).copied()
    }

    /// Get the result at a node, if any.
    fn result_at(&self, node_idx: usize) -> Option<&str> {
        self.nodes[node_idx].result.as_deref()
    }

    /// Load the default compose table with common X11 compose sequences.
    ///
    /// Includes 50+ sequences for accented characters, currency symbols,
    /// mathematical symbols, and typographic characters.
    fn load_defaults(&mut self) {
        // Accented characters — acute accent (')
        // Compose + ' + vowel = acute vowel
        self.add_sequence(&[0x0027, 0x0061], "\u{00e1}"); // 'a -> á
        self.add_sequence(&[0x0027, 0x0065], "\u{00e9}"); // 'e -> é
        self.add_sequence(&[0x0027, 0x0069], "\u{00ed}"); // 'i -> í
        self.add_sequence(&[0x0027, 0x006f], "\u{00f3}"); // 'o -> ó
        self.add_sequence(&[0x0027, 0x0075], "\u{00fa}"); // 'u -> ú
        self.add_sequence(&[0x0027, 0x0041], "\u{00c1}"); // 'A -> Á
        self.add_sequence(&[0x0027, 0x0045], "\u{00c9}"); // 'E -> É
        self.add_sequence(&[0x0027, 0x0049], "\u{00cd}"); // 'I -> Í
        self.add_sequence(&[0x0027, 0x004f], "\u{00d3}"); // 'O -> Ó
        self.add_sequence(&[0x0027, 0x0055], "\u{00da}"); // 'U -> Ú

        // Grave accent (`)
        self.add_sequence(&[0x0060, 0x0061], "\u{00e0}"); // `a -> à
        self.add_sequence(&[0x0060, 0x0065], "\u{00e8}"); // `e -> è
        self.add_sequence(&[0x0060, 0x0069], "\u{00ec}"); // `i -> ì
        self.add_sequence(&[0x0060, 0x006f], "\u{00f2}"); // `o -> ò
        self.add_sequence(&[0x0060, 0x0075], "\u{00f9}"); // `u -> ù
        self.add_sequence(&[0x0060, 0x0041], "\u{00c0}"); // `A -> À
        self.add_sequence(&[0x0060, 0x0045], "\u{00c8}"); // `E -> È

        // Circumflex (^)
        self.add_sequence(&[0x005e, 0x0061], "\u{00e2}"); // ^a -> â
        self.add_sequence(&[0x005e, 0x0065], "\u{00ea}"); // ^e -> ê
        self.add_sequence(&[0x005e, 0x0069], "\u{00ee}"); // ^i -> î
        self.add_sequence(&[0x005e, 0x006f], "\u{00f4}"); // ^o -> ô
        self.add_sequence(&[0x005e, 0x0075], "\u{00fb}"); // ^u -> û
        self.add_sequence(&[0x005e, 0x0041], "\u{00c2}"); // ^A -> Â
        self.add_sequence(&[0x005e, 0x0045], "\u{00ca}"); // ^E -> Ê

        // Tilde (~)
        self.add_sequence(&[0x007e, 0x006e], "\u{00f1}"); // ~n -> ñ
        self.add_sequence(&[0x007e, 0x004e], "\u{00d1}"); // ~N -> Ñ
        self.add_sequence(&[0x007e, 0x0061], "\u{00e3}"); // ~a -> ã
        self.add_sequence(&[0x007e, 0x006f], "\u{00f5}"); // ~o -> õ

        // Diaeresis/umlaut (")
        self.add_sequence(&[0x0022, 0x0061], "\u{00e4}"); // "a -> ä
        self.add_sequence(&[0x0022, 0x0065], "\u{00eb}"); // "e -> ë
        self.add_sequence(&[0x0022, 0x0069], "\u{00ef}"); // "i -> ï
        self.add_sequence(&[0x0022, 0x006f], "\u{00f6}"); // "o -> ö
        self.add_sequence(&[0x0022, 0x0075], "\u{00fc}"); // "u -> ü
        self.add_sequence(&[0x0022, 0x0079], "\u{00ff}"); // "y -> ÿ
        self.add_sequence(&[0x0022, 0x0041], "\u{00c4}"); // "A -> Ä
        self.add_sequence(&[0x0022, 0x004f], "\u{00d6}"); // "O -> Ö
        self.add_sequence(&[0x0022, 0x0055], "\u{00dc}"); // "U -> Ü

        // Cedilla (,)
        self.add_sequence(&[0x002c, 0x0063], "\u{00e7}"); // ,c -> ç
        self.add_sequence(&[0x002c, 0x0043], "\u{00c7}"); // ,C -> Ç

        // Currency symbols
        self.add_sequence(&[0x003d, 0x0065], "\u{20ac}"); // =e -> €
        self.add_sequence(&[0x003d, 0x0045], "\u{20ac}"); // =E -> €
        self.add_sequence(&[0x002d, 0x004c], "\u{00a3}"); // -L -> £
        self.add_sequence(&[0x002f, 0x002f], "\u{00f7}"); // // -> ÷
        self.add_sequence(&[0x0078, 0x0078], "\u{00d7}"); // xx -> ×

        // Special symbols
        self.add_sequence(&[0x006f, 0x0063], "\u{00a9}"); // oc -> ©
        self.add_sequence(&[0x006f, 0x0072], "\u{00ae}"); // or -> ®
        self.add_sequence(&[0x002e, 0x002e], "\u{2026}"); // .. -> …
        self.add_sequence(&[0x002d, 0x002d], "\u{2014}"); // -- -> —
        self.add_sequence(&[0x003c, 0x003c], "\u{00ab}"); // << -> «
        self.add_sequence(&[0x003e, 0x003e], "\u{00bb}"); // >> -> »
        self.add_sequence(&[0x0021, 0x0021], "\u{00a1}"); // !! -> ¡
        self.add_sequence(&[0x003f, 0x003f], "\u{00bf}"); // ?? -> ¿

        // Typographic quotes
        self.add_sequence(&[0x0060, 0x0060], "\u{201c}"); // `` -> "
        self.add_sequence(&[0x0027, 0x0027], "\u{201d}"); // '' -> "

        // Degree, superscripts
        self.add_sequence(&[0x006f, 0x006f], "\u{00b0}"); // oo -> °
        self.add_sequence(&[0x005e, 0x0031], "\u{00b9}"); // ^1 -> ¹
        self.add_sequence(&[0x005e, 0x0032], "\u{00b2}"); // ^2 -> ²
        self.add_sequence(&[0x005e, 0x0033], "\u{00b3}"); // ^3 -> ³

        // Slashed letters
        self.add_sequence(&[0x002f, 0x006f], "\u{00f8}"); // /o -> ø
        self.add_sequence(&[0x002f, 0x004f], "\u{00d8}"); // /O -> Ø

        // Ring above
        self.add_sequence(&[0x006f, 0x0061], "\u{00e5}"); // oa -> å
        self.add_sequence(&[0x006f, 0x0041], "\u{00c5}"); // oA -> Å

        // Ligatures
        self.add_sequence(&[0x0061, 0x0065], "\u{00e6}"); // ae -> æ
        self.add_sequence(&[0x0041, 0x0045], "\u{00c6}"); // AE -> Æ

        // German sharp s
        self.add_sequence(&[0x0073, 0x0073], "\u{00df}"); // ss -> ß
    }
}

impl Default for ComposeTable {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Tracks the state of an in-progress compose sequence.
#[derive(Debug, Clone)]
pub struct ComposeState {
    table: ComposeTable,
    /// Current position in the compose trie (node index), or None if idle.
    current_node: Option<usize>,
}

impl ComposeState {
    /// Create a new compose state with the given table.
    pub fn new(table: ComposeTable) -> Self {
        Self {
            table,
            current_node: None,
        }
    }

    /// Create with the default compose table.
    pub fn with_defaults() -> Self {
        Self::new(ComposeTable::with_defaults())
    }

    /// Whether a compose sequence is currently in progress.
    pub fn is_composing(&self) -> bool {
        self.current_node.is_some()
    }

    /// Reset the compose state (cancel any in-progress sequence).
    pub fn reset(&mut self) {
        self.current_node = None;
    }

    /// Get a reference to the compose table.
    pub fn table(&self) -> &ComposeTable {
        &self.table
    }

    /// Get a mutable reference to the compose table (for adding custom
    /// sequences).
    pub fn table_mut(&mut self) -> &mut ComposeTable {
        &mut self.table
    }

    /// Feed a keysym into the compose engine.
    ///
    /// Returns the compose status after processing this keysym.
    pub fn feed(&mut self, keysym: u32) -> ComposeStatus {
        let node_idx = self.current_node.unwrap_or(0);

        if let Some(next_idx) = self.table.has_child(node_idx, keysym) {
            // We have a matching continuation.
            if let Some(result) = self.table.result_at(next_idx) {
                // This completes a sequence.
                self.current_node = None;
                ComposeStatus::Composed(result.to_string())
            } else {
                // Sequence continues — need more keysyms.
                self.current_node = Some(next_idx);
                ComposeStatus::Composing
            }
        } else if self.current_node.is_some() {
            // We were in a sequence but this keysym doesn't match — cancel.
            self.current_node = None;
            ComposeStatus::Cancelled
        } else {
            // Not in a sequence and no sequence starts with this keysym from root.
            ComposeStatus::Nothing
        }
    }
}
