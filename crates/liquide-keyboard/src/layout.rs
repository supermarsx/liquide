use std::collections::HashMap;

/// Unique identifier for a dead key accent.
pub type DeadKeyId = u32;

/// A keyboard layout definition mapping scancodes to character output.
#[derive(Debug, Clone)]
pub struct KeyboardLayout {
    /// Layout identifier (e.g., "us", "de", "fr", "jp").
    pub id: String,
    /// Human-readable display name (e.g., "English (US)", "German").
    pub name: String,
    /// ISO 639-1 language code (e.g., "en", "de", "fr").
    pub language: String,
    /// Optional layout variant (e.g., "dvorak", "colemak").
    pub variant: Option<String>,
    /// Scancode to key mapping table.
    pub keymap: HashMap<u32, KeyMapping>,
    /// Dead key definitions indexed by their ID.
    pub dead_keys: HashMap<DeadKeyId, DeadKey>,
}

/// What a single key produces under various modifier combinations.
#[derive(Debug, Clone)]
pub struct KeyMapping {
    /// Character produced with no modifiers.
    pub normal: char,
    /// Character produced with Shift held.
    pub shift: Option<char>,
    /// Character produced with AltGr (Right Alt) held.
    pub alt_gr: Option<char>,
    /// Character produced with Shift+AltGr held.
    pub shift_alt_gr: Option<char>,
    /// If this key is a dead key, its dead key ID.
    pub dead_key: Option<DeadKeyId>,
}

impl KeyMapping {
    /// Create a simple key mapping with normal and shift characters.
    pub fn simple(normal: char, shift: char) -> Self {
        Self {
            normal,
            shift: Some(shift),
            alt_gr: None,
            shift_alt_gr: None,
            dead_key: None,
        }
    }

    /// Create a key mapping with normal, shift, and AltGr characters.
    pub fn with_alt_gr(normal: char, shift: char, alt_gr: char) -> Self {
        Self {
            normal,
            shift: Some(shift),
            alt_gr: Some(alt_gr),
            shift_alt_gr: None,
            dead_key: None,
        }
    }

    /// Create a key mapping that is a dead key.
    pub fn dead(normal: char, shift: Option<char>, dead_key_id: DeadKeyId) -> Self {
        Self {
            normal,
            shift,
            alt_gr: None,
            shift_alt_gr: None,
            dead_key: Some(dead_key_id),
        }
    }

    /// Create a key that produces the same character regardless of Shift.
    pub fn uniform(ch: char) -> Self {
        Self {
            normal: ch,
            shift: Some(ch),
            alt_gr: None,
            shift_alt_gr: None,
            dead_key: None,
        }
    }
}

/// A dead key accent definition for character composition.
#[derive(Debug, Clone)]
pub struct DeadKey {
    /// Unique identifier for this dead key.
    pub id: DeadKeyId,
    /// The accent character itself (e.g., '`', '\'', '^').
    pub base_char: char,
    /// Map from base character to composed character (e.g., 'a' -> 'a').
    pub combinations: HashMap<char, char>,
    /// Output when no combination matches (typically the accent itself).
    pub fallback: char,
}

impl DeadKey {
    /// Attempt to compose a base character with this dead key's accent.
    /// Returns the composed character, or the fallback if no combination exists.
    pub fn compose(&self, base: char) -> char {
        self.combinations
            .get(&base)
            .copied()
            .unwrap_or(self.fallback)
    }

    /// Check whether a composition exists for the given base character.
    pub fn has_combination(&self, base: char) -> bool {
        self.combinations.contains_key(&base)
    }
}

impl KeyboardLayout {
    /// Create a new empty keyboard layout.
    pub fn new(id: &str, name: &str, language: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            language: language.to_string(),
            variant: None,
            keymap: HashMap::new(),
            dead_keys: HashMap::new(),
        }
    }

    /// Set the layout variant.
    pub fn with_variant(mut self, variant: &str) -> Self {
        self.variant = Some(variant.to_string());
        self
    }

    /// Insert a key mapping for a scancode.
    pub fn insert(&mut self, scancode: u32, mapping: KeyMapping) {
        self.keymap.insert(scancode, mapping);
    }

    /// Insert a dead key definition.
    pub fn insert_dead_key(&mut self, dead_key: DeadKey) {
        self.dead_keys.insert(dead_key.id, dead_key);
    }

    /// Look up the mapping for a scancode.
    pub fn get(&self, scancode: u32) -> Option<&KeyMapping> {
        self.keymap.get(&scancode)
    }

    /// Look up a dead key by its ID.
    pub fn get_dead_key(&self, id: DeadKeyId) -> Option<&DeadKey> {
        self.dead_keys.get(&id)
    }

    /// Number of mapped keys in this layout.
    pub fn key_count(&self) -> usize {
        self.keymap.len()
    }
}
