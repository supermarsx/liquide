//! Search through terminal scrollback and grid.

/// A search match in the terminal buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Line index (0-based).
    pub line: usize,
    /// Start column of the match.
    pub start_col: usize,
    /// End column of the match (exclusive).
    pub end_col: usize,
}

/// Search state for the terminal.
pub struct SearchState {
    query: String,
    matches: Vec<SearchMatch>,
    current_index: usize,
    case_sensitive: bool,
}

impl SearchState {
    /// Create a new empty search state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            query: String::new(),
            matches: Vec::new(),
            current_index: 0,
            case_sensitive: false,
        }
    }

    /// Set case sensitivity.
    pub fn set_case_sensitive(&mut self, sensitive: bool) {
        self.case_sensitive = sensitive;
    }

    /// Whether search is case sensitive.
    #[must_use]
    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Execute a search on provided lines (flattened text).
    pub fn search(&mut self, query: &str, lines: &[String]) {
        self.query = query.to_string();
        self.matches.clear();
        self.current_index = 0;

        if query.is_empty() {
            return;
        }

        let needle = if self.case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        for (line_idx, line) in lines.iter().enumerate() {
            let haystack = if self.case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };
            let mut start = 0;
            while let Some(pos) = haystack[start..].find(&needle) {
                let abs_pos = start + pos;
                self.matches.push(SearchMatch {
                    line: line_idx,
                    start_col: abs_pos,
                    end_col: abs_pos + needle.len(),
                });
                start = abs_pos + 1;
            }
        }
    }

    /// Current query.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// All matches.
    #[must_use]
    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    /// Total match count.
    #[must_use]
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Current match index.
    #[must_use]
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Get the current match.
    #[must_use]
    pub fn current_match(&self) -> Option<&SearchMatch> {
        self.matches.get(self.current_index)
    }

    /// Move to the next match.
    pub fn next_match(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }
        self.current_index = (self.current_index + 1) % self.matches.len();
        self.matches.get(self.current_index)
    }

    /// Move to the previous match.
    pub fn prev_match(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }
        if self.current_index == 0 {
            self.current_index = self.matches.len() - 1;
        } else {
            self.current_index -= 1;
        }
        self.matches.get(self.current_index)
    }

    /// Clear the search.
    pub fn clear(&mut self) {
        self.query.clear();
        self.matches.clear();
        self.current_index = 0;
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}
