//! Paginated candidate window for CJK input method candidate selection.
//!
//! Wraps a flat candidate list with page-based navigation, number-key selection,
//! and a highlight cursor.

/// A single entry in the candidate window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateEntry {
    /// The candidate text (the string that would be committed).
    pub text: String,
    /// Optional short label (e.g. "1", "a").
    pub label: Option<String>,
    /// Optional annotation (reading, meaning, etc.).
    pub annotation: Option<String>,
}

impl CandidateEntry {
    /// Create an entry with just text.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            label: None,
            annotation: None,
        }
    }

    /// Create an entry with text and a label.
    #[must_use]
    pub fn with_label(text: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            label: Some(label.into()),
            annotation: None,
        }
    }

    /// Set annotation (builder pattern).
    #[must_use]
    pub fn annotated(mut self, annotation: impl Into<String>) -> Self {
        self.annotation = Some(annotation.into());
        self
    }
}

/// A paginated candidate window for input method candidate selection.
///
/// Provides page-based navigation over a flat candidate list, with methods
/// for cursor movement, page flipping, number-key selection, and confirmation.
pub struct CandidateWindow {
    /// All candidates in the current conversion.
    candidates: Vec<CandidateEntry>,
    /// Number of candidates visible per page.
    page_size: usize,
    /// Current page index (0-based).
    current_page: usize,
    /// Index of the selected candidate within the current page (0-based).
    selected: usize,
}

impl CandidateWindow {
    /// Create a new empty candidate window with the given page size.
    #[must_use]
    pub fn new(page_size: usize) -> Self {
        let page_size = page_size.max(1);
        Self {
            candidates: Vec::new(),
            page_size,
            current_page: 0,
            selected: 0,
        }
    }

    /// Replace the candidate list. Resets page and selection to zero.
    pub fn set_candidates(&mut self, candidates: Vec<CandidateEntry>) {
        self.candidates = candidates;
        self.current_page = 0;
        self.selected = 0;
    }

    /// Clear all candidates.
    pub fn clear(&mut self) {
        self.candidates.clear();
        self.current_page = 0;
        self.selected = 0;
    }

    /// Whether there are any candidates.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Total number of candidates.
    #[must_use]
    pub fn total(&self) -> usize {
        self.candidates.len()
    }

    /// Total number of pages.
    #[must_use]
    pub fn total_pages(&self) -> usize {
        if self.candidates.is_empty() {
            0
        } else {
            (self.candidates.len() + self.page_size - 1) / self.page_size
        }
    }

    /// Current page index (0-based).
    #[must_use]
    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// Index of the selected item within the current page.
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Absolute index of the selected candidate in the full list.
    #[must_use]
    pub fn absolute_selected(&self) -> usize {
        self.current_page * self.page_size + self.selected
    }

    /// Get the candidates visible on the current page.
    #[must_use]
    pub fn visible_candidates(&self) -> &[CandidateEntry] {
        if self.candidates.is_empty() {
            return &[];
        }
        let start = self.current_page * self.page_size;
        let end = (start + self.page_size).min(self.candidates.len());
        &self.candidates[start..end]
    }

    /// Move selection to the next candidate. Wraps within the current page.
    pub fn next_candidate(&mut self) {
        if self.candidates.is_empty() {
            return;
        }
        let page_count = self.visible_candidates().len();
        self.selected = (self.selected + 1) % page_count;
    }

    /// Move selection to the previous candidate. Wraps within the current page.
    pub fn prev_candidate(&mut self) {
        if self.candidates.is_empty() {
            return;
        }
        let page_count = self.visible_candidates().len();
        self.selected = if self.selected == 0 {
            page_count - 1
        } else {
            self.selected - 1
        };
    }

    /// Advance to the next page. Wraps to first page after the last.
    pub fn next_page(&mut self) {
        if self.candidates.is_empty() {
            return;
        }
        let total = self.total_pages();
        self.current_page = (self.current_page + 1) % total;
        self.selected = 0;
    }

    /// Go to the previous page. Wraps to last page before the first.
    pub fn prev_page(&mut self) {
        if self.candidates.is_empty() {
            return;
        }
        let total = self.total_pages();
        self.current_page = if self.current_page == 0 {
            total - 1
        } else {
            self.current_page - 1
        };
        self.selected = 0;
    }

    /// Select a candidate by its number on the current page (0-based).
    /// Returns a reference to the entry if valid, or `None`.
    #[must_use]
    pub fn select_by_number(&mut self, n: usize) -> Option<&CandidateEntry> {
        let visible = self.visible_candidates().len();
        if n < visible {
            self.selected = n;
            let abs = self.current_page * self.page_size + n;
            Some(&self.candidates[abs])
        } else {
            None
        }
    }

    /// Confirm the currently selected candidate, removing it from view.
    /// Returns the confirmed entry, or `None` if empty.
    pub fn confirm(&mut self) -> Option<CandidateEntry> {
        if self.candidates.is_empty() {
            return None;
        }
        let abs = self.absolute_selected();
        if abs < self.candidates.len() {
            let entry = self.candidates[abs].clone();
            self.candidates.clear();
            self.current_page = 0;
            self.selected = 0;
            Some(entry)
        } else {
            None
        }
    }
}

impl Default for CandidateWindow {
    fn default() -> Self {
        Self::new(9)
    }
}
