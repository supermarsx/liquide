//! Quick file filter for directory listings.

use crate::entry::FileEntry;
use serde::{Deserialize, Serialize};

/// A filter for file listings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFilter {
    /// Text to search in file names (case-insensitive substring match).
    pub text: String,
    /// Whether to show hidden files.
    pub show_hidden: bool,
    /// File type extensions to include (empty = all types).
    pub file_types: Vec<String>,
}

impl FileFilter {
    /// Create a new empty filter that matches everything (except hidden files).
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: String::new(),
            show_hidden: false,
            file_types: Vec::new(),
        }
    }

    /// Create a filter with a text query.
    #[must_use]
    pub fn with_text(text: &str) -> Self {
        Self {
            text: text.to_string(),
            show_hidden: false,
            file_types: Vec::new(),
        }
    }

    /// Set the text filter.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
    }

    /// Toggle hidden file visibility.
    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
    }

    /// Add a file type extension to the filter.
    pub fn add_type(&mut self, ext: &str) {
        let lower = ext.to_lowercase();
        if !self.file_types.contains(&lower) {
            self.file_types.push(lower);
        }
    }

    /// Remove a file type extension from the filter.
    pub fn remove_type(&mut self, ext: &str) {
        let lower = ext.to_lowercase();
        self.file_types.retain(|t| t != &lower);
    }

    /// Clear all file type filters.
    pub fn clear_types(&mut self) {
        self.file_types.clear();
    }

    /// Check if a file entry matches this filter.
    #[must_use]
    pub fn matches(&self, entry: &FileEntry) -> bool {
        // Hidden file check.
        if !self.show_hidden && entry.hidden {
            return false;
        }

        // Text filter (case-insensitive substring match on the name).
        if !self.text.is_empty() {
            let needle = self.text.to_lowercase();
            let haystack = entry.name.to_lowercase();
            if !haystack.contains(&needle) {
                return false;
            }
        }

        // File type filter (directories always pass type filter).
        if !self.file_types.is_empty() && !entry.is_dir() {
            let ext_lower = entry.extension.to_lowercase();
            if !self.file_types.contains(&ext_lower) {
                return false;
            }
        }

        true
    }

    /// Filter a slice of entries, returning only those that match.
    #[must_use]
    pub fn apply<'a>(&self, entries: &'a [FileEntry]) -> Vec<&'a FileEntry> {
        entries.iter().filter(|e| self.matches(e)).collect()
    }

    /// Whether this filter is active (has any non-default settings).
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.text.is_empty() || !self.file_types.is_empty()
    }

    /// Reset to default state.
    pub fn reset(&mut self) {
        self.text.clear();
        self.file_types.clear();
        self.show_hidden = false;
    }
}

impl Default for FileFilter {
    fn default() -> Self {
        Self::new()
    }
}
