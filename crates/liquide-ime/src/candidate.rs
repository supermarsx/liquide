//! IME candidate list window.
//!
//! When the IME presents conversion choices (e.g., pinyin → hanzi candidates),
//! this module provides the data model for displaying those candidates.

use serde::{Deserialize, Serialize};

/// A single candidate item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateItem {
    /// The candidate text.
    pub text: String,
    /// Optional label (e.g., "1", "2", etc.).
    pub label: Option<String>,
    /// Optional annotation (e.g., reading, meaning).
    pub annotation: Option<String>,
}

impl CandidateItem {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            label: None,
            annotation: None,
        }
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn with_annotation(mut self, annotation: impl Into<String>) -> Self {
        self.annotation = Some(annotation.into());
        self
    }
}

/// Page info for paginated candidate lists.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CandidatePageInfo {
    /// Current page index (zero-based).
    pub current_page: usize,
    /// Total number of pages.
    pub total_pages: usize,
    /// Items per page.
    pub page_size: usize,
}

/// The candidate list shown to the user during IME conversion.
#[derive(Debug, Clone)]
pub struct CandidateList {
    /// All candidates on the current page.
    pub candidates: Vec<CandidateItem>,
    /// Index of the currently selected candidate.
    pub selected_index: usize,
    /// Page info (if paginated).
    pub page_info: Option<CandidatePageInfo>,
    /// Whether the candidate window is visible.
    pub visible: bool,
    /// Layout: horizontal or vertical.
    pub horizontal: bool,
}

impl CandidateList {
    #[must_use]
    pub fn new() -> Self {
        Self {
            candidates: Vec::new(),
            selected_index: 0,
            page_info: None,
            visible: false,
            horizontal: false,
        }
    }

    /// Show candidates.
    pub fn show(&mut self, candidates: Vec<CandidateItem>) {
        self.candidates = candidates;
        self.selected_index = 0;
        self.visible = !self.candidates.is_empty();
    }

    /// Hide the candidate window.
    pub fn hide(&mut self) {
        self.visible = false;
        self.candidates.clear();
        self.selected_index = 0;
    }

    /// Select the next candidate.
    pub fn next(&mut self) {
        if !self.candidates.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.candidates.len();
        }
    }

    /// Select the previous candidate.
    pub fn prev(&mut self) {
        if !self.candidates.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.candidates.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    /// Select a candidate by index.
    pub fn select(&mut self, index: usize) {
        if index < self.candidates.len() {
            self.selected_index = index;
        }
    }

    /// Get the currently selected candidate.
    #[must_use]
    pub fn selected(&self) -> Option<&CandidateItem> {
        self.candidates.get(self.selected_index)
    }

    /// Number of candidates.
    #[must_use]
    pub fn count(&self) -> usize {
        self.candidates.len()
    }
}

impl Default for CandidateList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_list() {
        let mut cl = CandidateList::new();
        cl.show(vec![
            CandidateItem::new("漢字").with_label("1"),
            CandidateItem::new("感じ").with_label("2"),
            CandidateItem::new("幹事").with_label("3"),
        ]);
        assert!(cl.visible);
        assert_eq!(cl.count(), 3);
        assert_eq!(cl.selected().unwrap().text, "漢字");
    }

    #[test]
    fn test_navigation() {
        let mut cl = CandidateList::new();
        cl.show(vec![
            CandidateItem::new("A"),
            CandidateItem::new("B"),
            CandidateItem::new("C"),
        ]);
        cl.next();
        assert_eq!(cl.selected_index, 1);
        cl.next();
        assert_eq!(cl.selected_index, 2);
        cl.next();
        assert_eq!(cl.selected_index, 0); // wraps
    }

    #[test]
    fn test_hide() {
        let mut cl = CandidateList::new();
        cl.show(vec![CandidateItem::new("test")]);
        assert!(cl.visible);
        cl.hide();
        assert!(!cl.visible);
        assert!(cl.candidates.is_empty());
    }
}
