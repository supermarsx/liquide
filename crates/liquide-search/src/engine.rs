//! Search engine that orchestrates multiple [`SearchProvider`]s.
//!
//! The engine maintains a registry of providers, dispatches queries to all of
//! them, merges and ranks the results, and caches recent queries.

use crate::provider::{SearchCategory, SearchProvider, SearchResult};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Maximum number of cached query results.
const CACHE_CAPACITY: usize = 32;

/// Minimum interval (in microseconds) between two consecutive searches inside
/// a [`SearchSession`].
const DEBOUNCE_INTERVAL_US: u64 = 150_000; // 150 ms

// ---------------------------------------------------------------------------
// SearchEngine
// ---------------------------------------------------------------------------

/// Central search engine that fans out queries to registered providers.
pub struct SearchEngine {
    providers: Vec<Box<dyn SearchProvider>>,
    cache: QueryCache,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            cache: QueryCache::new(CACHE_CAPACITY),
        }
    }

    /// Register a search provider.
    pub fn register(&mut self, provider: Box<dyn SearchProvider>) {
        // Prevent duplicate ids.
        let id = provider.id().to_string();
        self.providers.retain(|p| p.id() != id);
        self.providers.push(provider);
    }

    /// Remove a provider by id.  Returns `true` if it was found.
    pub fn unregister(&mut self, id: &str) -> bool {
        let before = self.providers.len();
        self.providers.retain(|p| p.id() != id);
        self.providers.len() < before
    }

    /// Number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// List registered provider ids.
    pub fn provider_ids(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.id()).collect()
    }

    /// Run a search across **all** providers, merge and rank results.
    pub fn search(&mut self, query: &str, max_results: usize) -> Vec<SearchResult> {
        if query.is_empty() {
            return Vec::new();
        }

        // Check cache first.
        if let Some(cached) = self.cache.get(query, max_results) {
            return cached;
        }

        let results = self.query_providers(query, max_results, None);
        self.cache.put(query, &results);
        results
    }

    /// Search only within a specific category.
    pub fn search_category(
        &mut self,
        query: &str,
        category: SearchCategory,
        max_results: usize,
    ) -> Vec<SearchResult> {
        if query.is_empty() {
            return Vec::new();
        }
        self.query_providers(query, max_results, Some(category))
    }

    /// Clear the result cache (e.g. after index updates).
    pub fn invalidate_cache(&mut self) {
        self.cache.clear();
    }

    // -- internal -------------------------------------------------------------

    fn query_providers(
        &self,
        query: &str,
        max_results: usize,
        category_filter: Option<SearchCategory>,
    ) -> Vec<SearchResult> {
        let mut all: Vec<(f64, SearchResult)> = Vec::new();

        for provider in &self.providers {
            let priority = provider.priority();
            let results = provider.search(query, max_results);

            for r in results {
                if let Some(cat) = category_filter {
                    if r.category != cat {
                        continue;
                    }
                }
                let key = r.rank_key(priority);
                all.push((key, r));
            }
        }

        // Sort descending by rank key.
        all.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        all.truncate(max_results);
        all.into_iter().map(|(_, r)| r).collect()
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// QueryCache
// ---------------------------------------------------------------------------

/// Simple LRU-ish cache mapping query strings to result lists.
struct QueryCache {
    capacity: usize,
    /// Entries ordered from oldest to newest.
    entries: Vec<CacheEntry>,
}

struct CacheEntry {
    query: String,
    results: Vec<SearchResult>,
}

impl QueryCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Vec::with_capacity(capacity),
        }
    }

    fn get(&self, query: &str, max_results: usize) -> Option<Vec<SearchResult>> {
        for entry in self.entries.iter().rev() {
            if entry.query == query {
                let mut results = entry.results.clone();
                results.truncate(max_results);
                return Some(results);
            }
        }
        None
    }

    fn put(&mut self, query: &str, results: &[SearchResult]) {
        // Remove existing entry for same query.
        self.entries.retain(|e| e.query != query);

        if self.entries.len() >= self.capacity {
            self.entries.remove(0); // evict oldest
        }
        self.entries.push(CacheEntry {
            query: query.to_string(),
            results: results.to_vec(),
        });
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// SearchSession
// ---------------------------------------------------------------------------

/// Tracks an interactive search session with debouncing.
///
/// The caller feeds keystrokes via [`update`](SearchSession::update) and
/// receives results only when the debounce interval has elapsed.
pub struct SearchSession {
    last_query: String,
    last_search_us: u64,
    debounce_us: u64,
}

impl SearchSession {
    pub fn new() -> Self {
        Self {
            last_query: String::new(),
            last_search_us: 0,
            debounce_us: DEBOUNCE_INTERVAL_US,
        }
    }

    /// Create a session with a custom debounce interval (in microseconds).
    pub fn with_debounce(debounce_us: u64) -> Self {
        Self {
            last_query: String::new(),
            last_search_us: 0,
            debounce_us,
        }
    }

    /// Submit a query update.  Returns `true` if enough time has passed since
    /// the last search and the query has changed, meaning the caller should
    /// invoke [`SearchEngine::search`].
    pub fn update(&mut self, query: &str, now_us: u64) -> bool {
        if query == self.last_query {
            return false;
        }

        // Always accept the very first query (no previous search).
        let first = self.last_query.is_empty() && self.last_search_us == 0;
        let elapsed = now_us.saturating_sub(self.last_search_us);

        if first || elapsed >= self.debounce_us {
            self.last_query = query.to_string();
            self.last_search_us = now_us;
            true
        } else {
            false
        }
    }

    /// Force-accept the current query regardless of debounce (e.g. on Enter).
    pub fn flush(&mut self, query: &str, now_us: u64) {
        self.last_query = query.to_string();
        self.last_search_us = now_us;
    }

    /// The query that was last accepted by [`update`] or [`flush`].
    pub fn last_query(&self) -> &str {
        &self.last_query
    }
}

impl Default for SearchSession {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{SearchCategory, SearchResultAction};

    // -- helpers --------------------------------------------------------------

    /// Trivial provider that returns fixed results for any non-empty query.
    struct StubProvider {
        pid: &'static str,
        pname: &'static str,
        priority: u32,
        results: Vec<SearchResult>,
    }

    impl StubProvider {
        fn single(pid: &'static str, priority: u32, result: SearchResult) -> Self {
            Self {
                pid,
                pname: pid,
                priority,
                results: vec![result],
            }
        }
    }

    impl SearchProvider for StubProvider {
        fn id(&self) -> &str {
            self.pid
        }
        fn name(&self) -> &str {
            self.pname
        }
        fn icon(&self) -> &str {
            "stub-icon"
        }
        fn priority(&self) -> u32 {
            self.priority
        }

        fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
            if query.is_empty() {
                return Vec::new();
            }
            self.results
                .iter()
                .filter(|r| r.title.to_lowercase().contains(&query.to_lowercase()))
                .take(max_results)
                .cloned()
                .collect()
        }
    }

    fn make_result(title: &str, cat: SearchCategory, score: f32) -> SearchResult {
        SearchResult {
            id: title.to_lowercase(),
            title: title.into(),
            description: String::new(),
            icon: String::new(),
            category: cat,
            relevance_score: score,
            action: SearchResultAction::Custom(title.into()),
        }
    }

    // -- SearchEngine ---------------------------------------------------------

    #[test]
    fn engine_empty_returns_nothing() {
        let mut engine = SearchEngine::new();
        let results = engine.search("hello", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn engine_register_and_search() {
        let mut engine = SearchEngine::new();
        engine.register(Box::new(StubProvider::single(
            "apps",
            80,
            make_result("Firefox", SearchCategory::Application, 0.9),
        )));
        let results = engine.search("firefox", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Firefox");
    }

    #[test]
    fn engine_empty_query_returns_nothing() {
        let mut engine = SearchEngine::new();
        engine.register(Box::new(StubProvider::single(
            "apps",
            80,
            make_result("Firefox", SearchCategory::Application, 0.9),
        )));
        assert!(engine.search("", 10).is_empty());
    }

    #[test]
    fn engine_unregister() {
        let mut engine = SearchEngine::new();
        engine.register(Box::new(StubProvider::single(
            "apps",
            80,
            make_result("Firefox", SearchCategory::Application, 0.9),
        )));
        assert_eq!(engine.provider_count(), 1);
        assert!(engine.unregister("apps"));
        assert_eq!(engine.provider_count(), 0);
        assert!(!engine.unregister("apps")); // already gone
    }

    #[test]
    fn engine_duplicate_provider_id_replaces() {
        let mut engine = SearchEngine::new();
        engine.register(Box::new(StubProvider::single(
            "apps",
            80,
            make_result("Old", SearchCategory::Application, 0.5),
        )));
        engine.register(Box::new(StubProvider::single(
            "apps",
            90,
            make_result("New", SearchCategory::Application, 0.8),
        )));
        assert_eq!(engine.provider_count(), 1);
        let results = engine.search("new", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "New");
    }

    #[test]
    fn engine_multiple_providers_merged_and_ranked() {
        let mut engine = SearchEngine::new();

        // Low-priority provider with high relevance.
        engine.register(Box::new(StubProvider::single(
            "files",
            20,
            make_result("Test File", SearchCategory::File, 1.0),
        )));
        // High-priority provider with medium relevance.
        engine.register(Box::new(StubProvider::single(
            "apps",
            80,
            make_result("Test App", SearchCategory::Application, 0.5),
        )));

        let results = engine.search("test", 10);
        assert_eq!(results.len(), 2);
        // apps: 80*0.01 + 0.5 = 1.3  >  files: 20*0.01 + 1.0 = 1.2
        assert_eq!(results[0].title, "Test App");
        assert_eq!(results[1].title, "Test File");
    }

    #[test]
    fn engine_search_category_filter() {
        let mut engine = SearchEngine::new();
        engine.register(Box::new(StubProvider::single(
            "apps",
            80,
            make_result("Test App", SearchCategory::Application, 0.9),
        )));
        engine.register(Box::new(StubProvider::single(
            "files",
            60,
            make_result("Test File", SearchCategory::File, 0.7),
        )));

        let only_files = engine.search_category("test", SearchCategory::File, 10);
        assert_eq!(only_files.len(), 1);
        assert_eq!(only_files[0].title, "Test File");

        let only_apps = engine.search_category("test", SearchCategory::Application, 10);
        assert_eq!(only_apps.len(), 1);
        assert_eq!(only_apps[0].title, "Test App");
    }

    #[test]
    fn engine_max_results_honoured() {
        let mut engine = SearchEngine::new();
        let results_vec: Vec<SearchResult> = (0..20)
            .map(|i| make_result(&format!("Item {}", i), SearchCategory::File, 0.5))
            .collect();
        engine.register(Box::new(StubProvider {
            pid: "many",
            pname: "Many",
            priority: 50,
            results: results_vec,
        }));

        let results = engine.search("item", 5);
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn engine_provider_ids() {
        let mut engine = SearchEngine::new();
        engine.register(Box::new(StubProvider::single(
            "alpha",
            10,
            make_result("A", SearchCategory::File, 0.5),
        )));
        engine.register(Box::new(StubProvider::single(
            "beta",
            20,
            make_result("B", SearchCategory::File, 0.5),
        )));
        let ids = engine.provider_ids();
        assert!(ids.contains(&"alpha"));
        assert!(ids.contains(&"beta"));
    }

    // -- cache ----------------------------------------------------------------

    #[test]
    fn cache_hit_returns_results() {
        let mut engine = SearchEngine::new();
        engine.register(Box::new(StubProvider::single(
            "apps",
            80,
            make_result("Firefox", SearchCategory::Application, 0.9),
        )));

        // First search populates cache.
        let r1 = engine.search("firefox", 10);
        // Second should come from cache (same results).
        let r2 = engine.search("firefox", 10);
        assert_eq!(r1.len(), r2.len());
        assert_eq!(r1[0].title, r2[0].title);
    }

    #[test]
    fn cache_invalidation() {
        let mut engine = SearchEngine::new();
        engine.register(Box::new(StubProvider::single(
            "apps",
            80,
            make_result("Firefox", SearchCategory::Application, 0.9),
        )));

        let _ = engine.search("firefox", 10);
        engine.invalidate_cache();
        // After invalidation the search still works (just re-queries providers).
        let r = engine.search("firefox", 10);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn cache_evicts_oldest() {
        let mut cache = QueryCache::new(3);
        cache.put("a", &[make_result("A", SearchCategory::File, 0.5)]);
        cache.put("b", &[make_result("B", SearchCategory::File, 0.5)]);
        cache.put("c", &[make_result("C", SearchCategory::File, 0.5)]);
        // All three present.
        assert!(cache.get("a", 10).is_some());
        // Adding a 4th evicts "a".
        cache.put("d", &[make_result("D", SearchCategory::File, 0.5)]);
        assert!(cache.get("a", 10).is_none());
        assert!(cache.get("b", 10).is_some());
    }

    #[test]
    fn cache_dedup_on_reinsert() {
        let mut cache = QueryCache::new(4);
        cache.put("x", &[make_result("X1", SearchCategory::File, 0.5)]);
        cache.put("y", &[make_result("Y", SearchCategory::File, 0.5)]);
        // Re-insert "x" with updated results.
        cache.put("x", &[make_result("X2", SearchCategory::File, 0.8)]);
        assert_eq!(cache.entries.len(), 2);
        let got = cache.get("x", 10).unwrap();
        assert_eq!(got[0].title, "X2");
    }

    // -- SearchSession --------------------------------------------------------

    #[test]
    fn session_debounce_blocks_rapid_updates() {
        let mut session = SearchSession::with_debounce(100_000); // 100 ms
        assert!(session.update("a", 0));
        // 50 ms later -- too soon.
        assert!(!session.update("ab", 50_000));
        // 100 ms later -- ok.
        assert!(session.update("ab", 100_000));
    }

    #[test]
    fn session_same_query_ignored() {
        let mut session = SearchSession::new();
        assert!(session.update("hello", 0));
        assert!(!session.update("hello", 1_000_000)); // same query
    }

    #[test]
    fn session_flush_overrides_debounce() {
        let mut session = SearchSession::with_debounce(1_000_000); // 1 second
        session.update("a", 0);
        session.flush("abc", 10_000);
        assert_eq!(session.last_query(), "abc");
    }

    #[test]
    fn session_last_query_empty_initially() {
        let session = SearchSession::new();
        assert_eq!(session.last_query(), "");
    }

    #[test]
    fn session_default() {
        let s = SearchSession::default();
        assert_eq!(s.debounce_us, DEBOUNCE_INTERVAL_US);
    }
}
