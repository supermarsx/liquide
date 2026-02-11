//! File search within directory trees.

/// A search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Full path of the matching entry.
    pub path: String,
    /// File name.
    pub name: String,
    /// Whether it is a directory.
    pub is_dir: bool,
    /// Size in bytes.
    pub size: u64,
    /// Relevance score (higher = better match).
    pub score: u32,
}

/// Search state.
pub struct FileSearch {
    query: String,
    results: Vec<SearchResult>,
    searching: bool,
    case_sensitive: bool,
    search_contents: bool,
}

impl FileSearch {
    /// Create a new search state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            searching: false,
            case_sensitive: false,
            search_contents: false,
        }
    }

    /// Set whether to search within file contents.
    pub fn set_search_contents(&mut self, enabled: bool) { self.search_contents = enabled; }

    /// Set case sensitivity.
    pub fn set_case_sensitive(&mut self, sensitive: bool) { self.case_sensitive = sensitive; }

    /// Start a search with the given query and file list.
    pub fn search(&mut self, query: &str, files: &[(String, String, bool, u64)]) {
        self.query = query.to_string();
        self.results.clear();
        self.searching = true;

        if query.is_empty() {
            self.searching = false;
            return;
        }

        let needle = if self.case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        for (path, name, is_dir, size) in files {
            let haystack = if self.case_sensitive {
                name.to_string()
            } else {
                name.to_lowercase()
            };
            if haystack.contains(&needle) {
                let score = if haystack == needle {
                    100
                } else if haystack.starts_with(&needle) {
                    75
                } else {
                    50
                };
                self.results.push(SearchResult {
                    path: path.clone(),
                    name: name.clone(),
                    is_dir: *is_dir,
                    size: *size,
                    score,
                });
            }
        }

        self.results.sort_by(|a, b| b.score.cmp(&a.score));
        self.searching = false;
    }

    /// Current query.
    #[must_use]
    pub fn query(&self) -> &str { &self.query }

    /// Search results.
    #[must_use]
    pub fn results(&self) -> &[SearchResult] { &self.results }

    /// Result count.
    #[must_use]
    pub fn result_count(&self) -> usize { self.results.len() }

    /// Whether a search is in progress.
    #[must_use]
    pub fn is_searching(&self) -> bool { self.searching }

    /// Clear the search.
    pub fn clear(&mut self) {
        self.query.clear();
        self.results.clear();
        self.searching = false;
    }
}

impl Default for FileSearch {
    fn default() -> Self { Self::new() }
}
