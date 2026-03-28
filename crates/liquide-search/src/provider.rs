//! Search provider trait and result types.
//!
//! Every search backend (applications, files, calculator, settings, etc.)
//! implements [`SearchProvider`].  The [`SearchEngine`](crate::engine::SearchEngine)
//! collects results from all registered providers, ranks them, and returns a
//! unified result list.

use std::fmt;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// What happens when the user activates a search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultAction {
    /// Launch an application by its desktop-id or path.
    Launch(String),
    /// Open a file / URI.
    Open(String),
    /// Navigate to a view inside the desktop (e.g. a settings page).
    Navigate(String),
    /// Provider-specific action payload.
    Custom(String),
}

/// Broad category of a search result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchCategory {
    Application,
    File,
    Setting,
    Contact,
    Calculator,
    WebSearch,
    RecentFile,
    Bookmark,
}

impl fmt::Display for SearchCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application => write!(f, "Applications"),
            Self::File => write!(f, "Files"),
            Self::Setting => write!(f, "Settings"),
            Self::Contact => write!(f, "Contacts"),
            Self::Calculator => write!(f, "Calculator"),
            Self::WebSearch => write!(f, "Web"),
            Self::RecentFile => write!(f, "Recent Files"),
            Self::Bookmark => write!(f, "Bookmarks"),
        }
    }
}

/// A single search result produced by a provider.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Provider-scoped unique id.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Optional description / subtitle.
    pub description: String,
    /// Icon name or path.
    pub icon: String,
    /// Category used for grouping in the UI.
    pub category: SearchCategory,
    /// Relevance score in `[0.0, 1.0]` (higher = more relevant).
    pub relevance_score: f32,
    /// Action performed on activation.
    pub action: SearchResultAction,
}

impl SearchResult {
    /// Combined ranking key used by the engine.
    ///
    /// `provider_priority` is typically in `[0, 100]`.
    pub fn rank_key(&self, provider_priority: u32) -> f64 {
        (provider_priority as f64) * 0.01 + self.relevance_score as f64
    }
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Trait implemented by every search backend.
///
/// Providers are registered with [`SearchEngine`](crate::engine::SearchEngine)
/// and queried in parallel.  Each provider returns results scoped to its own
/// domain.
pub trait SearchProvider {
    /// Unique, stable identifier for this provider (e.g. `"apps"`, `"files"`).
    fn id(&self) -> &str;

    /// Human-readable name shown in the search UI.
    fn name(&self) -> &str;

    /// Icon name for the provider group header.
    fn icon(&self) -> &str;

    /// Priority weight.  Higher values push results towards the top of the
    /// merged list.  Typical range: 0 - 100.
    fn priority(&self) -> u32;

    /// Execute a search and return up to `max_results` results.
    fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult>;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Case-insensitive fuzzy match returning a score in `[0.0, 1.0]` or `None`.
///
/// Scoring:
/// - Exact match          -> 1.0
/// - Prefix match         -> 0.8
/// - Word-boundary match  -> 0.6
/// - Contains             -> 0.4
/// - No match             -> None
pub fn fuzzy_score(haystack: &str, needle: &str) -> Option<f32> {
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();

    if h == n {
        return Some(1.0);
    }
    if h.starts_with(&n) {
        return Some(0.8);
    }
    // Word-boundary: needle appears right after a space, hyphen, or underscore.
    if word_boundary_match(&h, &n) {
        return Some(0.6);
    }
    if h.contains(&n) {
        return Some(0.4);
    }
    None
}

/// Returns `true` when `needle` appears at a word boundary inside `haystack`.
fn word_boundary_match(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    for (i, _) in haystack.match_indices(needle) {
        if i == 0 {
            return true;
        }
        let prev = bytes[i - 1];
        if prev == b' ' || prev == b'-' || prev == b'_' || prev == b'/' || prev == b'.' {
            return true;
        }
    }
    false
}

/// Clamp a score to `[0.0, 1.0]`.
pub fn clamp_score(s: f32) -> f32 {
    s.clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- SearchCategory Display -------------------------------------------------

    #[test]
    fn category_display() {
        assert_eq!(SearchCategory::Application.to_string(), "Applications");
        assert_eq!(SearchCategory::File.to_string(), "Files");
        assert_eq!(SearchCategory::Setting.to_string(), "Settings");
        assert_eq!(SearchCategory::Contact.to_string(), "Contacts");
        assert_eq!(SearchCategory::Calculator.to_string(), "Calculator");
        assert_eq!(SearchCategory::WebSearch.to_string(), "Web");
        assert_eq!(SearchCategory::RecentFile.to_string(), "Recent Files");
        assert_eq!(SearchCategory::Bookmark.to_string(), "Bookmarks");
    }

    // -- SearchResult -----------------------------------------------------------

    #[test]
    fn result_rank_key_combines_priority_and_score() {
        let r = SearchResult {
            id: "x".into(),
            title: "X".into(),
            description: String::new(),
            icon: String::new(),
            category: SearchCategory::Application,
            relevance_score: 0.9,
            action: SearchResultAction::Launch("x".into()),
        };
        let key = r.rank_key(80);
        // 80*0.01 + 0.9 = 1.7
        assert!((key - 1.7).abs() < 1e-6);
    }

    #[test]
    fn result_rank_key_zero_priority() {
        let r = SearchResult {
            id: "y".into(),
            title: "Y".into(),
            description: String::new(),
            icon: String::new(),
            category: SearchCategory::File,
            relevance_score: 0.5,
            action: SearchResultAction::Open("/tmp/y".into()),
        };
        let key = r.rank_key(0);
        assert!((key - 0.5).abs() < 1e-6);
    }

    // -- SearchResultAction -----------------------------------------------------

    #[test]
    fn action_variants() {
        let a = SearchResultAction::Launch("firefox".into());
        let b = SearchResultAction::Open("/home/user/doc.pdf".into());
        let c = SearchResultAction::Navigate("settings://display".into());
        let d = SearchResultAction::Custom("do-thing".into());
        assert_ne!(a, b);
        assert_ne!(c, d);
        assert_eq!(a, SearchResultAction::Launch("firefox".into()));
    }

    // -- fuzzy_score ------------------------------------------------------------

    #[test]
    fn fuzzy_exact() {
        assert_eq!(fuzzy_score("Firefox", "firefox"), Some(1.0));
    }

    #[test]
    fn fuzzy_prefix() {
        assert_eq!(fuzzy_score("Firefox", "fire"), Some(0.8));
    }

    #[test]
    fn fuzzy_word_boundary() {
        assert_eq!(fuzzy_score("GNOME Terminal", "terminal"), Some(0.6));
        assert_eq!(fuzzy_score("file-manager", "manager"), Some(0.6));
        assert_eq!(fuzzy_score("file_browser", "browser"), Some(0.6));
    }

    #[test]
    fn fuzzy_contains() {
        assert_eq!(fuzzy_score("LibreOffice", "office"), Some(0.4));
    }

    #[test]
    fn fuzzy_no_match() {
        assert_eq!(fuzzy_score("Firefox", "zzz"), None);
    }

    #[test]
    fn fuzzy_empty_needle() {
        // Empty needle is contained in everything (prefix match).
        assert!(fuzzy_score("anything", "").is_some());
    }

    #[test]
    fn fuzzy_case_insensitive() {
        assert_eq!(fuzzy_score("FIREFOX", "Firefox"), Some(1.0));
    }

    // -- clamp_score ------------------------------------------------------------

    #[test]
    fn clamp_within_range() {
        assert_eq!(clamp_score(0.5), 0.5);
    }

    #[test]
    fn clamp_above() {
        assert_eq!(clamp_score(1.5), 1.0);
    }

    #[test]
    fn clamp_below() {
        assert_eq!(clamp_score(-0.3), 0.0);
    }

    // -- word_boundary_match ----------------------------------------------------

    #[test]
    fn word_boundary_at_start() {
        assert!(word_boundary_match("terminal", "terminal"));
    }

    #[test]
    fn word_boundary_after_space() {
        assert!(word_boundary_match("gnome terminal", "terminal"));
    }

    #[test]
    fn word_boundary_after_dot() {
        assert!(word_boundary_match("org.gnome.terminal", "terminal"));
    }

    #[test]
    fn word_boundary_none() {
        assert!(!word_boundary_match("xterminal", "terminal"));
    }
}
