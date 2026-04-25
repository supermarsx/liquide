//! Input method switching and per-window input method state.
//!
//! Manages a list of available input methods and tracks which method is active
//! globally and per-window.

use std::collections::HashMap;

/// Metadata describing an available input method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputMethodInfo {
    /// Unique identifier (e.g. "en-us", "ja-romaji", "zh-pinyin").
    pub id: String,
    /// Human-readable display name (e.g. "English (US)", "Japanese - Romaji").
    pub name: String,
    /// Optional icon name or path for the status indicator.
    pub icon: Option<String>,
    /// Language tag (e.g. "en", "ja", "zh").
    pub language: String,
}

impl InputMethodInfo {
    /// Create a new input method info entry.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: None,
            language: language.into(),
        }
    }

    /// Set icon (builder pattern).
    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// Input method switcher with per-window state tracking.
///
/// Maintains an ordered list of available input methods and tracks both a
/// global active method and per-window overrides.
pub struct InputMethodSwitcher {
    /// Available input methods in switching order.
    methods: Vec<InputMethodInfo>,
    /// Global active method index.
    active_index: usize,
    /// Per-window method index overrides.
    per_window: HashMap<u64, usize>,
}

impl InputMethodSwitcher {
    /// Create a new switcher with no methods registered.
    #[must_use]
    pub fn new() -> Self {
        Self {
            methods: Vec::new(),
            active_index: 0,
            per_window: HashMap::new(),
        }
    }

    /// Add an input method to the end of the list.
    pub fn add_method(&mut self, info: InputMethodInfo) {
        self.methods.push(info);
    }

    /// Number of registered methods.
    #[must_use]
    pub fn len(&self) -> usize {
        self.methods.len()
    }

    /// Whether the method list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.methods.is_empty()
    }

    /// Get the currently active global method, or `None` if no methods exist.
    #[must_use]
    pub fn active(&self) -> Option<&InputMethodInfo> {
        self.methods.get(self.active_index)
    }

    /// Get the index of the currently active global method.
    #[must_use]
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Cycle to the next input method and return it.
    /// Wraps around from the last method to the first.
    pub fn switch_next(&mut self) -> Option<&InputMethodInfo> {
        if self.methods.is_empty() {
            return None;
        }
        self.active_index = (self.active_index + 1) % self.methods.len();
        self.methods.get(self.active_index)
    }

    /// Switch to a specific method by index. Returns the method if the index is valid.
    pub fn switch_to(&mut self, index: usize) -> Option<&InputMethodInfo> {
        if index < self.methods.len() {
            self.active_index = index;
            Some(&self.methods[index])
        } else {
            None
        }
    }

    /// Set the active method for a specific window (overrides the global setting).
    pub fn set_for_window(&mut self, window_id: u64, index: usize) {
        if index < self.methods.len() {
            self.per_window.insert(window_id, index);
        }
    }

    /// Remove the per-window override for a specific window
    /// (falls back to the global method).
    pub fn clear_for_window(&mut self, window_id: u64) {
        self.per_window.remove(&window_id);
    }

    /// Get the effective input method for a specific window.
    /// Returns the per-window override if set, otherwise the global method.
    #[must_use]
    pub fn get_for_window(&self, window_id: u64) -> Option<&InputMethodInfo> {
        let idx = self
            .per_window
            .get(&window_id)
            .copied()
            .unwrap_or(self.active_index);
        self.methods.get(idx)
    }

    /// Get all registered methods.
    #[must_use]
    pub fn methods(&self) -> &[InputMethodInfo] {
        &self.methods
    }
}

impl Default for InputMethodSwitcher {
    fn default() -> Self {
        Self::new()
    }
}
