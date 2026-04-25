//! Font search index — background-built inverted index for fast
//! full-text search across all font metadata.

use std::collections::HashMap;

use crate::catalog::FontEntry;

/// Inverted index mapping search tokens to catalog entry indices.
pub struct FontIndex {
    /// Token → set of entry indices.
    token_index: HashMap<String, Vec<usize>>,
    /// Total number of indexed entries.
    entry_count: usize,
    /// Whether the index needs rebuilding.
    dirty: bool,
}

impl FontIndex {
    /// Create a new empty index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            token_index: HashMap::new(),
            entry_count: 0,
            dirty: false,
        }
    }

    /// Add a font entry to the index (by reference to the catalog).
    pub fn add_entry(&mut self, entry: &FontEntry) {
        let idx = self.entry_count;
        self.entry_count += 1;

        // Index family name tokens.
        for token in tokenize(&entry.family) {
            self.token_index.entry(token).or_default().push(idx);
        }

        // Index style.
        for token in tokenize(&entry.style) {
            self.token_index.entry(token).or_default().push(idx);
        }

        // Index tags.
        for tag in &entry.tags {
            for token in tokenize(tag) {
                self.token_index.entry(token).or_default().push(idx);
            }
        }

        // Index designer.
        if !entry.designer.is_empty() {
            for token in tokenize(&entry.designer) {
                self.token_index.entry(token).or_default().push(idx);
            }
        }

        // Index source.
        let source_str = entry.source.to_string();
        for token in tokenize(&source_str) {
            self.token_index.entry(token).or_default().push(idx);
        }

        // Index format.
        self.token_index
            .entry(entry.format.clone())
            .or_default()
            .push(idx);
    }

    /// Search the index for entries matching a query.
    ///
    /// Returns indices into the catalog, ranked by relevance (number of
    /// matching tokens).
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<usize> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }

        // Count how many query tokens each entry matches.
        let mut scores: HashMap<usize, usize> = HashMap::new();
        for token in &query_tokens {
            // Exact match.
            if let Some(entries) = self.token_index.get(token) {
                for &idx in entries {
                    *scores.entry(idx).or_default() += 2;
                }
            }
            // Prefix match.
            for (key, entries) in &self.token_index {
                if key.starts_with(token) && key != token {
                    for &idx in entries {
                        *scores.entry(idx).or_default() += 1;
                    }
                }
            }
        }

        // Sort by score descending.
        let mut results: Vec<(usize, usize)> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.into_iter().map(|(idx, _)| idx).collect()
    }

    /// Get the total number of indexed entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entry_count
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entry_count == 0
    }

    /// Clear the index.
    pub fn clear(&mut self) {
        self.token_index.clear();
        self.entry_count = 0;
        self.dirty = false;
    }

    /// Whether the index needs rebuilding.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the index as needing a rebuild.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Get the number of unique tokens in the index.
    #[must_use]
    pub fn token_count(&self) -> usize {
        self.token_index.len()
    }
}

impl Default for FontIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Tokenize a string into lowercase search tokens.
fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{FontEntry, FontSource};
    use std::path::PathBuf;

    fn entry(family: &str, style: &str) -> FontEntry {
        FontEntry {
            family: family.into(),
            style: style.into(),
            weight: 400,
            italic: false,
            path: PathBuf::from("/fonts/test.ttf"),
            format: "ttf".into(),
            file_size: 50_000,
            source: FontSource::System,
            tags: vec!["ui".into()],
            activated: true,
            glyph_count: 200,
            script_coverage: Vec::new(),
            version: "1.0".into(),
            license: "OFL".into(),
            designer: "Test".into(),
        }
    }

    #[test]
    fn empty_index() {
        let index = FontIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert!(index.search("anything").is_empty());
    }

    #[test]
    fn add_and_search_exact() {
        let mut index = FontIndex::new();
        index.add_entry(&entry("Manrope", "Regular"));
        index.add_entry(&entry("Inter", "Bold"));

        assert_eq!(index.len(), 2);

        let results = index.search("Manrope");
        assert!(!results.is_empty());
        assert_eq!(results[0], 0); // First entry added
    }

    #[test]
    fn search_by_style() {
        let mut index = FontIndex::new();
        index.add_entry(&entry("Manrope", "Regular"));
        index.add_entry(&entry("Inter", "Bold"));

        let results = index.search("Bold");
        assert!(results.contains(&1));
    }

    #[test]
    fn search_prefix_match() {
        let mut index = FontIndex::new();
        index.add_entry(&entry("Manrope", "Regular"));

        // "man" should prefix-match "manrope"
        let results = index.search("man");
        assert!(!results.is_empty());
    }

    #[test]
    fn search_empty_query() {
        let mut index = FontIndex::new();
        index.add_entry(&entry("Manrope", "Regular"));
        assert!(index.search("").is_empty());
    }

    #[test]
    fn clear_index() {
        let mut index = FontIndex::new();
        index.add_entry(&entry("Manrope", "Regular"));
        assert!(!index.is_empty());

        index.clear();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert_eq!(index.token_count(), 0);
    }

    #[test]
    fn dirty_flag() {
        let mut index = FontIndex::new();
        assert!(!index.is_dirty());
        index.mark_dirty();
        assert!(index.is_dirty());
        index.clear();
        assert!(!index.is_dirty());
    }

    #[test]
    fn tokenize_splits_on_non_alphanumeric() {
        let tokens = tokenize("JetBrains Mono - Regular");
        assert_eq!(tokens, vec!["jetbrains", "mono", "regular"]);
    }

    #[test]
    fn tokenize_empty_string() {
        assert!(tokenize("").is_empty());
    }
}
