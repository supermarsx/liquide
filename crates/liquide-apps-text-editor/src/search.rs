//! Search and replace.

use crate::cursor::Position;

/// A search match in the buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub start: Position,
    pub end: Position,
    pub line_text: String,
}

/// Search and replace state.
pub struct SearchReplace {
    query: String,
    replacement: String,
    matches: Vec<SearchMatch>,
    current: usize,
    case_sensitive: bool,
    whole_word: bool,
    use_regex: bool,
}

impl SearchReplace {
    #[must_use]
    pub fn new() -> Self {
        Self {
            query: String::new(),
            replacement: String::new(),
            matches: Vec::new(),
            current: 0,
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
        }
    }

    /// Search for a query in the buffer lines.
    pub fn search(&mut self, query: &str, lines: &[String]) {
        self.query = query.to_string();
        self.matches.clear();
        self.current = 0;

        if query.is_empty() { return; }

        let q = if self.case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        for (line_idx, line) in lines.iter().enumerate() {
            let haystack = if self.case_sensitive {
                line.clone()
            } else {
                line.to_lowercase()
            };

            let mut col = 0;
            while let Some(pos) = haystack[col..].find(&q) {
                let start_col = col + pos;
                let end_col = start_col + query.len();

                if self.whole_word {
                    let before_ok = start_col == 0 ||
                        !haystack.as_bytes()[start_col - 1].is_ascii_alphanumeric();
                    let after_ok = end_col >= haystack.len() ||
                        !haystack.as_bytes()[end_col].is_ascii_alphanumeric();
                    if !before_ok || !after_ok {
                        col = start_col + 1;
                        continue;
                    }
                }

                self.matches.push(SearchMatch {
                    start: Position::new(line_idx, start_col),
                    end: Position::new(line_idx, end_col),
                    line_text: line.clone(),
                });
                col = start_col + 1;
            }
        }
    }

    /// Set the replacement text.
    pub fn set_replacement(&mut self, replacement: impl Into<String>) {
        self.replacement = replacement.into();
    }

    /// Set case sensitivity.
    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        self.case_sensitive = case_sensitive;
    }

    /// Set whole word matching.
    pub fn set_whole_word(&mut self, whole_word: bool) {
        self.whole_word = whole_word;
    }

    /// Navigate to next match.
    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current = (self.current + 1) % self.matches.len();
        }
    }

    /// Navigate to previous match.
    pub fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            self.current = if self.current == 0 {
                self.matches.len() - 1
            } else {
                self.current - 1
            };
        }
    }

    /// Clear search state.
    pub fn clear(&mut self) {
        self.query.clear();
        self.replacement.clear();
        self.matches.clear();
        self.current = 0;
    }

    #[must_use]
    pub fn query(&self) -> &str { &self.query }
    #[must_use]
    pub fn replacement(&self) -> &str { &self.replacement }
    #[must_use]
    pub fn matches(&self) -> &[SearchMatch] { &self.matches }
    #[must_use]
    pub fn match_count(&self) -> usize { self.matches.len() }
    #[must_use]
    pub fn current_index(&self) -> usize { self.current }
    #[must_use]
    pub fn current_match(&self) -> Option<&SearchMatch> { self.matches.get(self.current) }
    #[must_use]
    pub fn is_case_sensitive(&self) -> bool { self.case_sensitive }
    #[must_use]
    pub fn is_whole_word(&self) -> bool { self.whole_word }
}

impl Default for SearchReplace {
    fn default() -> Self { Self::new() }
}
