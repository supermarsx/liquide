//! Reusable search bar model with query, toggles, and result tracking.

use serde::{Deserialize, Serialize};

/// A search bar with query text, feature toggles, and result counts.
///
/// Used by: Files, Terminal, Settings, Software Center, Text Editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchBar {
    /// The current search query text.
    pub query: String,
    /// Placeholder text shown when the query is empty.
    pub placeholder: String,
    /// Whether the search bar currently has input focus.
    pub is_focused: bool,
    /// Show a clear/reset button when the query is non-empty.
    pub show_clear: bool,
    /// Show a regex mode toggle.
    pub show_regex_toggle: bool,
    /// Show a case-sensitivity toggle.
    pub show_case_toggle: bool,
    /// Whether regex mode is active.
    pub regex_enabled: bool,
    /// Whether case-sensitive mode is active.
    pub case_sensitive: bool,
    /// Current and total result counts, e.g. `(3, 42)` for "3 of 42".
    pub result_count: Option<(usize, usize)>,
}

impl SearchBar {
    /// Create a new search bar with the given placeholder text.
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            query: String::new(),
            placeholder: placeholder.into(),
            is_focused: false,
            show_clear: true,
            show_regex_toggle: false,
            show_case_toggle: false,
            regex_enabled: false,
            case_sensitive: false,
            result_count: None,
        }
    }

    /// Whether the query is non-empty.
    pub fn has_query(&self) -> bool {
        !self.query.is_empty()
    }

    /// Clear the query and result count.
    pub fn clear(&mut self) {
        self.query.clear();
        self.result_count = None;
    }
}

impl Default for SearchBar {
    fn default() -> Self {
        Self::new("Search...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_bar_default() {
        let bar = SearchBar::default();
        assert!(!bar.has_query());
        assert_eq!(bar.placeholder, "Search...");
        assert!(bar.result_count.is_none());
    }

    #[test]
    fn search_bar_clear() {
        let mut bar = SearchBar::default();
        bar.query = "test".into();
        bar.result_count = Some((1, 5));
        bar.clear();
        assert!(!bar.has_query());
        assert!(bar.result_count.is_none());
    }
}
