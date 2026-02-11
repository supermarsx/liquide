//! Full-text search across all settings.

use crate::entry::SettingEntry;

/// A search result pointing to a specific setting.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The setting key.
    pub key: String,
    /// The setting label.
    pub label: String,
    /// Which category owns the setting.
    pub category_id: String,
    /// Relevance score (higher = better).
    pub score: u32,
}

/// Settings search engine.
pub struct SettingsSearch {
    query: String,
    results: Vec<SearchResult>,
    history: Vec<String>,
    history_limit: usize,
}

impl SettingsSearch {
    #[must_use]
    pub fn new(history_limit: usize) -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            history: Vec::new(),
            history_limit,
        }
    }

    /// Run a search over all entries.
    pub fn search(&mut self, query: &str, entries: &[SettingEntry]) {
        self.query = query.to_string();
        let q = query.to_lowercase();
        self.results.clear();

        if q.is_empty() {
            return;
        }

        for entry in entries {
            let mut score = 0u32;

            let label_lower = entry.label.to_lowercase();
            let desc_lower = entry.description.to_lowercase();
            let key_lower = entry.key.to_lowercase();

            if label_lower == q {
                score += 100;
            } else if label_lower.starts_with(&q) {
                score += 80;
            } else if label_lower.contains(&q) {
                score += 50;
            }

            if key_lower.contains(&q) {
                score += 30;
            }

            if desc_lower.contains(&q) {
                score += 20;
            }

            for kw in &entry.keywords {
                if kw.to_lowercase().contains(&q) {
                    score += 10;
                }
            }

            if score > 0 {
                self.results.push(SearchResult {
                    key: entry.key.clone(),
                    label: entry.label.clone(),
                    category_id: entry.category.id().to_string(),
                    score,
                });
            }
        }

        self.results.sort_by(|a, b| b.score.cmp(&a.score));
    }

    /// Record the current query in the search history.
    pub fn commit_to_history(&mut self) {
        if self.query.is_empty() {
            return;
        }
        // Remove duplicate if present.
        self.history.retain(|h| h != &self.query);
        self.history.push(self.query.clone());
        if self.history.len() > self.history_limit {
            self.history.remove(0);
        }
    }

    /// Clear the current search.
    pub fn clear(&mut self) {
        self.query.clear();
        self.results.clear();
    }

    #[must_use]
    pub fn query(&self) -> &str { &self.query }
    #[must_use]
    pub fn results(&self) -> &[SearchResult] { &self.results }
    #[must_use]
    pub fn result_count(&self) -> usize { self.results.len() }
    #[must_use]
    pub fn history(&self) -> &[String] { &self.history }
}
