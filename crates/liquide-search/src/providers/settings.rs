//! Settings search provider.
//!
//! Matches against a registry of setting entries by label, description, and
//! keywords.

use crate::provider::{
    SearchCategory, SearchProvider, SearchResult, SearchResultAction, clamp_score, fuzzy_score,
};

// ---------------------------------------------------------------------------
// SettingEntry
// ---------------------------------------------------------------------------

/// A single searchable setting.
#[derive(Debug, Clone)]
pub struct SettingEntry {
    /// Unique key (e.g. `"display.resolution"`).
    pub key: String,
    /// Human-readable label shown in results.
    pub label: String,
    /// Longer explanation.
    pub description: String,
    /// Settings panel category (e.g. `"Display"`, `"Sound"`).
    pub category: String,
    /// Extra search keywords.
    pub keywords: Vec<String>,
}

impl SettingEntry {
    pub fn new(key: &str, label: &str, description: &str) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            description: description.into(),
            category: String::new(),
            keywords: Vec::new(),
        }
    }

    /// Builder: set category.
    pub fn with_category(mut self, cat: &str) -> Self {
        self.category = cat.into();
        self
    }

    /// Builder: add keywords.
    pub fn with_keywords(mut self, kw: &[&str]) -> Self {
        self.keywords = kw.iter().map(|s| s.to_string()).collect();
        self
    }
}

// ---------------------------------------------------------------------------
// SettingsSearchProvider
// ---------------------------------------------------------------------------

/// Provider that searches a list of [`SettingEntry`] values.
pub struct SettingsSearchProvider {
    entries: Vec<SettingEntry>,
}

impl SettingsSearchProvider {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a setting entry.
    pub fn add(&mut self, entry: SettingEntry) {
        self.entries.push(entry);
    }

    /// Number of registered settings.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for SettingsSearchProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchProvider for SettingsSearchProvider {
    fn id(&self) -> &str {
        "settings"
    }
    fn name(&self) -> &str {
        "Settings"
    }
    fn icon(&self) -> &str {
        "preferences-system"
    }
    fn priority(&self) -> u32 {
        70
    }

    fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
        let mut scored: Vec<(f32, &SettingEntry)> = self
            .entries
            .iter()
            .filter_map(|entry| {
                let s = score_setting(entry, query)?;
                Some((s, entry))
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_results);

        scored
            .into_iter()
            .map(|(score, entry)| SearchResult {
                id: entry.key.clone(),
                title: entry.label.clone(),
                description: entry.description.clone(),
                icon: "preferences-system".into(),
                category: SearchCategory::Setting,
                relevance_score: clamp_score(score),
                action: SearchResultAction::Navigate(format!("settings://{}", entry.key)),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

fn score_setting(entry: &SettingEntry, query: &str) -> Option<f32> {
    let mut best: Option<f32> = None;

    // Label (highest weight).
    if let Some(s) = fuzzy_score(&entry.label, query) {
        best = Some(s);
    }

    // Description (medium weight).
    if let Some(s) = fuzzy_score(&entry.description, query) {
        let s = s * 0.6;
        best = Some(best.map_or(s, |b| b.max(s)));
    }

    // Keywords (medium-high weight).
    for kw in &entry.keywords {
        if let Some(s) = fuzzy_score(kw, query) {
            let s = s * 0.8;
            best = Some(best.map_or(s, |b| b.max(s)));
        }
    }

    // Category name (low weight).
    if !entry.category.is_empty() {
        if let Some(s) = fuzzy_score(&entry.category, query) {
            let s = s * 0.4;
            best = Some(best.map_or(s, |b| b.max(s)));
        }
    }

    best
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<SettingEntry> {
        vec![
            SettingEntry::new(
                "display.resolution",
                "Screen Resolution",
                "Change display resolution",
            )
            .with_category("Display")
            .with_keywords(&["monitor", "screen", "dpi"]),
            SettingEntry::new("sound.volume", "Volume", "Adjust system volume")
                .with_category("Sound")
                .with_keywords(&["audio", "speaker", "mute"]),
            SettingEntry::new("network.wifi", "Wi-Fi", "Connect to wireless networks")
                .with_category("Network")
                .with_keywords(&["wireless", "internet", "ssid"]),
            SettingEntry::new("appearance.theme", "Theme", "Choose desktop theme")
                .with_category("Appearance")
                .with_keywords(&["dark", "light", "colors"]),
        ]
    }

    fn provider_with_samples() -> SettingsSearchProvider {
        let mut p = SettingsSearchProvider::new();
        for e in sample_entries() {
            p.add(e);
        }
        p
    }

    // -- SettingEntry ---------------------------------------------------------

    #[test]
    fn entry_new() {
        let e = SettingEntry::new("k", "Label", "Desc");
        assert_eq!(e.key, "k");
        assert_eq!(e.label, "Label");
        assert_eq!(e.description, "Desc");
        assert!(e.category.is_empty());
        assert!(e.keywords.is_empty());
    }

    #[test]
    fn entry_with_category() {
        let e = SettingEntry::new("k", "L", "D").with_category("Display");
        assert_eq!(e.category, "Display");
    }

    #[test]
    fn entry_with_keywords() {
        let e = SettingEntry::new("k", "L", "D").with_keywords(&["a", "b"]);
        assert_eq!(e.keywords, vec!["a", "b"]);
    }

    // -- SettingsSearchProvider basics ----------------------------------------

    #[test]
    fn provider_metadata() {
        let p = SettingsSearchProvider::new();
        assert_eq!(p.id(), "settings");
        assert_eq!(p.name(), "Settings");
        assert_eq!(p.priority(), 70);
    }

    #[test]
    fn provider_empty() {
        let p = SettingsSearchProvider::new();
        assert_eq!(p.len(), 0);
        assert!(p.is_empty());
        assert!(p.search("anything", 10).is_empty());
    }

    #[test]
    fn provider_add() {
        let mut p = SettingsSearchProvider::new();
        p.add(SettingEntry::new("k", "L", "D"));
        assert_eq!(p.len(), 1);
        assert!(!p.is_empty());
    }

    // -- search by label ------------------------------------------------------

    #[test]
    fn search_label_exact() {
        let p = provider_with_samples();
        let r = p.search("Volume", 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "Volume");
        assert_eq!(r[0].category, SearchCategory::Setting);
    }

    #[test]
    fn search_label_prefix() {
        let p = provider_with_samples();
        let r = p.search("vol", 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "Volume");
    }

    #[test]
    fn search_label_contains() {
        let p = provider_with_samples();
        let r = p.search("resolution", 10);
        assert!(!r.is_empty());
        assert_eq!(r[0].id, "display.resolution");
    }

    // -- search by keyword ----------------------------------------------------

    #[test]
    fn search_keyword() {
        let p = provider_with_samples();
        let r = p.search("wireless", 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "Wi-Fi");
    }

    #[test]
    fn search_keyword_dark() {
        let p = provider_with_samples();
        let r = p.search("dark", 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "Theme");
    }

    // -- search by description ------------------------------------------------

    #[test]
    fn search_description() {
        let p = provider_with_samples();
        // "Adjust system volume" contains "adjust"
        let r = p.search("adjust", 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "Volume");
    }

    // -- no match -------------------------------------------------------------

    #[test]
    fn search_no_match() {
        let p = provider_with_samples();
        assert!(p.search("zzzzz", 10).is_empty());
    }

    // -- max results ----------------------------------------------------------

    #[test]
    fn search_max_results() {
        let p = provider_with_samples();
        // All entries contain some word with "e" in it.
        let r = p.search("e", 2);
        assert!(r.len() <= 2);
    }

    // -- action ---------------------------------------------------------------

    #[test]
    fn result_action_navigate() {
        let p = provider_with_samples();
        let r = p.search("volume", 1);
        assert!(
            matches!(r[0].action, SearchResultAction::Navigate(ref s) if s.contains("sound.volume"))
        );
    }

    // -- case insensitive -----------------------------------------------------

    #[test]
    fn search_case_insensitive() {
        let p = provider_with_samples();
        assert_eq!(p.search("VOLUME", 10).len(), 1);
        assert_eq!(p.search("volume", 10).len(), 1);
    }
}
