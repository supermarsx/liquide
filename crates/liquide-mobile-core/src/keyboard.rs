//! Virtual keyboard and text input handling.

use serde::{Deserialize, Serialize};

/// A virtual key on the extended key bar or keyboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VirtualKey {
    Escape,
    Tab,
    Control,
    Alt,
    Super,
    Shift,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Enter,
    Backspace,
    Space,
    /// A printable character key.
    Character(char),
}

impl VirtualKey {
    /// Human-readable label for display on the key bar.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Escape => "Esc".to_string(),
            Self::Tab => "Tab".to_string(),
            Self::Control => "Ctrl".to_string(),
            Self::Alt => "Alt".to_string(),
            Self::Super => "Super".to_string(),
            Self::Shift => "Shift".to_string(),
            Self::F1 => "F1".to_string(),
            Self::F2 => "F2".to_string(),
            Self::F3 => "F3".to_string(),
            Self::F4 => "F4".to_string(),
            Self::F5 => "F5".to_string(),
            Self::F6 => "F6".to_string(),
            Self::F7 => "F7".to_string(),
            Self::F8 => "F8".to_string(),
            Self::F9 => "F9".to_string(),
            Self::F10 => "F10".to_string(),
            Self::F11 => "F11".to_string(),
            Self::F12 => "F12".to_string(),
            Self::Delete => "Del".to_string(),
            Self::Insert => "Ins".to_string(),
            Self::Home => "Home".to_string(),
            Self::End => "End".to_string(),
            Self::PageUp => "PgUp".to_string(),
            Self::PageDown => "PgDn".to_string(),
            Self::ArrowLeft => "Left".to_string(),
            Self::ArrowRight => "Right".to_string(),
            Self::ArrowUp => "Up".to_string(),
            Self::ArrowDown => "Down".to_string(),
            Self::Enter => "Enter".to_string(),
            Self::Backspace => "Bksp".to_string(),
            Self::Space => "Space".to_string(),
            Self::Character(c) => c.to_string(),
        }
    }
}

impl std::fmt::Display for VirtualKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Extended key bar shown above the on-screen keyboard.
#[derive(Debug, Clone)]
pub struct ExtendedKeyBar {
    keys: Vec<VirtualKey>,
}

impl ExtendedKeyBar {
    /// Create a key bar with the given keys.
    #[must_use]
    pub fn new(keys: Vec<VirtualKey>) -> Self {
        Self { keys }
    }

    /// Create the default extended key bar layout.
    #[must_use]
    pub fn default_keys() -> Self {
        Self {
            keys: vec![
                VirtualKey::Escape,
                VirtualKey::Tab,
                VirtualKey::Control,
                VirtualKey::Alt,
                VirtualKey::Super,
                VirtualKey::ArrowLeft,
                VirtualKey::ArrowUp,
                VirtualKey::ArrowDown,
                VirtualKey::ArrowRight,
                VirtualKey::F1,
                VirtualKey::F2,
                VirtualKey::F3,
                VirtualKey::F4,
                VirtualKey::F5,
                VirtualKey::F6,
                VirtualKey::F7,
                VirtualKey::F8,
                VirtualKey::F9,
                VirtualKey::F10,
                VirtualKey::F11,
                VirtualKey::F12,
            ],
        }
    }

    /// The ordered list of keys.
    #[must_use]
    pub fn keys(&self) -> &[VirtualKey] {
        &self.keys
    }
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Modifiers {
    /// Shift key is held.
    pub shift: bool,
    /// Control key is held.
    pub ctrl: bool,
    /// Alt/Option key is held.
    pub alt: bool,
    /// Super/Command key is held.
    pub super_key: bool,
}

impl Modifiers {
    /// No modifiers pressed.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
}

/// A key press or release event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEvent {
    /// The key being pressed or released.
    pub key: VirtualKey,
    /// `true` for key-down, `false` for key-up.
    pub pressed: bool,
    /// Active modifier keys.
    pub modifiers: Modifiers,
}

/// Tracks IME / on-screen keyboard text composition state.
#[derive(Debug, Clone)]
pub struct TextInput {
    /// Committed text buffer.
    committed: String,
    /// Text currently being composed (IME pre-edit).
    composing: String,
}

impl TextInput {
    /// Create a new empty text input state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            committed: String::new(),
            composing: String::new(),
        }
    }

    /// Insert committed text.
    pub fn insert_text(&mut self, text: &str) {
        self.committed.push_str(text);
    }

    /// Delete one character backwards from committed text.
    pub fn delete_backward(&mut self) {
        self.committed.pop();
    }

    /// Set the current composing (pre-edit) text.
    pub fn set_composing(&mut self, text: &str) {
        self.composing = text.to_string();
    }

    /// Commit the composing text to the committed buffer.
    pub fn commit_composing(&mut self) {
        self.committed.push_str(&self.composing);
        self.composing.clear();
    }

    /// Get the full current text (committed + composing).
    #[must_use]
    pub fn current_text(&self) -> String {
        let mut result = self.committed.clone();
        result.push_str(&self.composing);
        result
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}
