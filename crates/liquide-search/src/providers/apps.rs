//! Application search provider.
//!
//! Searches installed applications by name, generic name, keywords, and exec
//! name.  Results are scored with fuzzy matching and boosted by launch
//! frequency.

use std::path::PathBuf;

use crate::provider::{
    SearchCategory, SearchProvider, SearchResult, SearchResultAction, fuzzy_score, clamp_score,
};

// ---------------------------------------------------------------------------
// AppEntry
// ---------------------------------------------------------------------------

/// Metadata for a single installed application.
#[derive(Debug, Clone)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub desktop_file: Option<PathBuf>,
    pub is_terminal: bool,
    pub no_display: bool,
    /// How many times this app has been launched (for frequency ranking).
    pub launch_count: u32,
}

impl AppEntry {
    /// Convenience constructor for tests and manual registration.
    pub fn new(id: &str, name: &str, exec: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            generic_name: None,
            comment: None,
            exec: exec.into(),
            icon: None,
            categories: Vec::new(),
            keywords: Vec::new(),
            desktop_file: None,
            is_terminal: false,
            no_display: false,
            launch_count: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// AppSearchProvider
// ---------------------------------------------------------------------------

/// Provider that searches application entries using fuzzy matching.
pub struct AppSearchProvider {
    apps: Vec<AppEntry>,
}

impl AppSearchProvider {
    pub fn new() -> Self {
        Self { apps: Vec::new() }
    }

    /// Add an application entry.
    pub fn add(&mut self, entry: AppEntry) {
        self.apps.push(entry);
    }

    /// Record a launch, incrementing the frequency counter.
    pub fn record_launch(&mut self, app_id: &str) {
        if let Some(app) = self.apps.iter_mut().find(|a| a.id == app_id) {
            app.launch_count += 1;
        }
    }

    /// Number of registered applications.
    pub fn app_count(&self) -> usize {
        self.apps.len()
    }

    /// Get all visible apps (for launcher grid display).
    pub fn all_apps(&self) -> Vec<&AppEntry> {
        self.apps.iter().filter(|a| !a.no_display).collect()
    }

    /// Get apps belonging to a category (case-insensitive).
    pub fn by_category(&self, category: &str) -> Vec<&AppEntry> {
        let cat = category.to_lowercase();
        self.apps
            .iter()
            .filter(|a| !a.no_display && a.categories.iter().any(|c| c.to_lowercase() == cat))
            .collect()
    }
}

impl Default for AppSearchProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchProvider for AppSearchProvider {
    fn id(&self) -> &str { "apps" }
    fn name(&self) -> &str { "Applications" }
    fn icon(&self) -> &str { "system-run" }
    fn priority(&self) -> u32 { 90 }

    fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
        let mut scored: Vec<(f32, &AppEntry)> = self
            .apps
            .iter()
            .filter(|a| !a.no_display)
            .filter_map(|app| {
                let score = score_app(app, query)?;
                Some((score, app))
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_results);

        scored
            .into_iter()
            .map(|(score, app)| SearchResult {
                id: app.id.clone(),
                title: app.name.clone(),
                description: app
                    .generic_name
                    .clone()
                    .or_else(|| app.comment.clone())
                    .unwrap_or_default(),
                icon: app.icon.clone().unwrap_or_default(),
                category: SearchCategory::Application,
                relevance_score: clamp_score(score),
                action: SearchResultAction::Launch(app.exec.clone()),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Score an app against a query.  Returns `None` when there is no match at all.
fn score_app(app: &AppEntry, query: &str) -> Option<f32> {
    let mut best: Option<f32> = None;

    // Name (highest weight).
    if let Some(s) = fuzzy_score(&app.name, query) {
        best = Some(s);
    }

    // Exec basename.
    let exec_base = app
        .exec
        .rsplit('/')
        .next()
        .unwrap_or(&app.exec)
        .split_whitespace()
        .next()
        .unwrap_or("");
    if let Some(s) = fuzzy_score(exec_base, query) {
        let s = s * 0.9; // slightly lower weight than name
        best = Some(best.map_or(s, |b| b.max(s)));
    }

    // Keywords (medium weight).
    for kw in &app.keywords {
        if let Some(s) = fuzzy_score(kw, query) {
            let s = s * 0.7;
            best = Some(best.map_or(s, |b| b.max(s)));
        }
    }

    // Generic name.
    if let Some(ref gn) = app.generic_name {
        if let Some(s) = fuzzy_score(gn, query) {
            let s = s * 0.6;
            best = Some(best.map_or(s, |b| b.max(s)));
        }
    }

    // Comment (lowest weight).
    if let Some(ref c) = app.comment {
        if let Some(s) = fuzzy_score(c, query) {
            let s = s * 0.3;
            best = Some(best.map_or(s, |b| b.max(s)));
        }
    }

    // Frequency boost: up to +0.15 for heavily-used apps.
    if let Some(base) = best {
        let freq_boost = ((app.launch_count as f32).ln_1p() / 10.0).min(0.15);
        return Some(base + freq_boost);
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, name: &str) -> AppEntry {
        AppEntry::new(id, name, &format!("/usr/bin/{}", id))
    }

    // -- AppEntry -------------------------------------------------------------

    #[test]
    fn app_entry_new() {
        let e = AppEntry::new("ff", "Firefox", "/usr/bin/firefox");
        assert_eq!(e.id, "ff");
        assert_eq!(e.name, "Firefox");
        assert_eq!(e.exec, "/usr/bin/firefox");
        assert!(!e.no_display);
        assert_eq!(e.launch_count, 0);
    }

    // -- AppSearchProvider basics --------------------------------------------

    #[test]
    fn provider_metadata() {
        let p = AppSearchProvider::new();
        assert_eq!(p.id(), "apps");
        assert_eq!(p.name(), "Applications");
        assert_eq!(p.priority(), 90);
    }

    #[test]
    fn provider_add_and_count() {
        let mut p = AppSearchProvider::new();
        assert_eq!(p.app_count(), 0);
        p.add(app("ff", "Firefox"));
        assert_eq!(p.app_count(), 1);
    }

    #[test]
    fn provider_record_launch() {
        let mut p = AppSearchProvider::new();
        p.add(app("ff", "Firefox"));
        p.record_launch("ff");
        p.record_launch("ff");
        assert_eq!(p.apps[0].launch_count, 2);
    }

    #[test]
    fn provider_record_launch_unknown() {
        let mut p = AppSearchProvider::new();
        p.add(app("ff", "Firefox"));
        p.record_launch("nonexistent"); // no panic
        assert_eq!(p.apps[0].launch_count, 0);
    }

    // -- search ---------------------------------------------------------------

    #[test]
    fn search_exact_name() {
        let mut p = AppSearchProvider::new();
        p.add(app("ff", "Firefox"));
        p.add(app("ch", "Chromium"));
        let r = p.search("firefox", 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "Firefox");
        assert_eq!(r[0].category, SearchCategory::Application);
    }

    #[test]
    fn search_prefix_name() {
        let mut p = AppSearchProvider::new();
        p.add(app("ff", "Firefox"));
        p.add(app("fi", "Files"));
        let r = p.search("fi", 10);
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn search_contains_name() {
        let mut p = AppSearchProvider::new();
        p.add(app("lo", "LibreOffice Writer"));
        let r = p.search("office", 10);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn search_no_match() {
        let mut p = AppSearchProvider::new();
        p.add(app("ff", "Firefox"));
        assert!(p.search("zzz", 10).is_empty());
    }

    #[test]
    fn search_keyword_match() {
        let mut p = AppSearchProvider::new();
        let mut e = app("nau", "Files");
        e.keywords = vec!["file".into(), "manager".into()];
        p.add(e);
        let r = p.search("manager", 10);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn search_generic_name_match() {
        let mut p = AppSearchProvider::new();
        let mut e = app("nau", "Files");
        e.generic_name = Some("File Manager".into());
        p.add(e);
        let r = p.search("file manager", 10);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn search_comment_match() {
        let mut p = AppSearchProvider::new();
        let mut e = app("vlc", "VLC");
        e.comment = Some("Media player and streaming".into());
        p.add(e);
        let r = p.search("streaming", 10);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn search_exec_match() {
        let mut p = AppSearchProvider::new();
        p.add(AppEntry::new("term", "GNOME Terminal", "/usr/bin/gnome-terminal"));
        let r = p.search("gnome-terminal", 10);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn search_no_display_hidden() {
        let mut p = AppSearchProvider::new();
        let mut e = app("hid", "HiddenApp");
        e.no_display = true;
        p.add(e);
        p.add(app("vis", "VisibleApp"));
        assert!(p.search("hidden", 10).is_empty());
        assert_eq!(p.all_apps().len(), 1);
    }

    #[test]
    fn search_case_insensitive() {
        let mut p = AppSearchProvider::new();
        p.add(app("ff", "Firefox"));
        assert_eq!(p.search("FIREFOX", 10).len(), 1);
        assert_eq!(p.search("firefox", 10).len(), 1);
        assert_eq!(p.search("Firefox", 10).len(), 1);
    }

    #[test]
    fn search_exact_ranks_above_prefix() {
        let mut p = AppSearchProvider::new();
        p.add(app("te", "Terminal Emulator"));
        p.add(app("t", "Terminal"));
        let r = p.search("terminal", 10);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].title, "Terminal"); // exact wins
    }

    #[test]
    fn search_max_results() {
        let mut p = AppSearchProvider::new();
        for i in 0..20 {
            p.add(app(&format!("a{}", i), &format!("App{}", i)));
        }
        let r = p.search("app", 5);
        assert_eq!(r.len(), 5);
    }

    #[test]
    fn search_frequency_boost() {
        let mut p = AppSearchProvider::new();
        p.add(app("ff", "Firefox"));
        p.add(app("fi", "Files"));
        for _ in 0..20 {
            p.record_launch("ff");
        }
        let r = p.search("fi", 10);
        // Firefox should be first because of frequency.
        assert_eq!(r[0].title, "Firefox");
    }

    #[test]
    fn search_result_action_is_launch() {
        let mut p = AppSearchProvider::new();
        p.add(app("ff", "Firefox"));
        let r = p.search("firefox", 1);
        assert!(matches!(r[0].action, SearchResultAction::Launch(_)));
    }

    // -- by_category ----------------------------------------------------------

    #[test]
    fn by_category_match() {
        let mut p = AppSearchProvider::new();
        let mut e = app("ff", "Firefox");
        e.categories = vec!["Network".into(), "WebBrowser".into()];
        p.add(e);
        p.add(app("term", "Terminal"));

        let net = p.by_category("Network");
        assert_eq!(net.len(), 1);
        assert_eq!(net[0].name, "Firefox");
    }

    #[test]
    fn by_category_case_insensitive() {
        let mut p = AppSearchProvider::new();
        let mut e = app("ff", "Firefox");
        e.categories = vec!["Network".into()];
        p.add(e);
        assert_eq!(p.by_category("network").len(), 1);
    }

    #[test]
    fn by_category_no_display_hidden() {
        let mut p = AppSearchProvider::new();
        let mut e = app("hid", "Hidden");
        e.categories = vec!["System".into()];
        e.no_display = true;
        p.add(e);
        assert!(p.by_category("System").is_empty());
    }

    // -- all_apps -------------------------------------------------------------

    #[test]
    fn all_apps_excludes_no_display() {
        let mut p = AppSearchProvider::new();
        p.add(app("a", "A"));
        let mut h = app("b", "B");
        h.no_display = true;
        p.add(h);
        assert_eq!(p.all_apps().len(), 1);
    }
}
