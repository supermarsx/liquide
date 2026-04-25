//! Font catalog — the authoritative database of known fonts.
//!
//! Each entry describes a font face: its family, style, weight, file path,
//! and metadata.  The catalog is populated by directory scanning and by
//! Google Fonts downloads.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Metadata for a single font face file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontEntry {
    /// Font family name (e.g. "Manrope", "JetBrains Mono").
    pub family: String,
    /// Style name within the family (e.g. "Regular", "Bold", "Italic").
    pub style: String,
    /// Weight (100–900).
    pub weight: u16,
    /// Whether the face is italic.
    pub italic: bool,
    /// Path to the font file on disk.
    pub path: PathBuf,
    /// Font format: "ttf", "otf", "woff2", "ttc".
    pub format: String,
    /// File size in bytes.
    pub file_size: u64,
    /// Source of the font (system, user-installed, google-fonts, url, git).
    pub source: FontSource,
    /// User-assigned tags.
    pub tags: Vec<String>,
    /// Whether this font is currently activated (available for use).
    pub activated: bool,
    /// Number of glyphs in the font.
    pub glyph_count: u32,
    /// Unicode script coverage summary.
    pub script_coverage: Vec<String>,
    /// Font version string.
    pub version: String,
    /// Font license identifier.
    pub license: String,
    /// Optional designer name.
    pub designer: String,
}

/// Where a font was sourced from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSource {
    /// Pre-installed system font.
    System,
    /// User manually installed (drag-and-drop, file picker, etc.).
    UserInstalled,
    /// Downloaded from Google Fonts.
    GoogleFonts,
    /// Imported from a URL.
    Url { url: String },
    /// Imported from a Git repository.
    Git { repo: String },
    /// Part of a collection import.
    Collection { collection_name: String },
}

impl std::fmt::Display for FontSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "System"),
            Self::UserInstalled => write!(f, "User Installed"),
            Self::GoogleFonts => write!(f, "Google Fonts"),
            Self::Url { url } => write!(f, "URL: {url}"),
            Self::Git { repo } => write!(f, "Git: {repo}"),
            Self::Collection { collection_name } => write!(f, "Collection: {collection_name}"),
        }
    }
}

/// The font catalog — holds all known font entries.
pub struct FontCatalog {
    /// All known font entries.
    pub entries: Vec<FontEntry>,
    /// Family name → indices into `entries`.
    family_index: HashMap<String, Vec<usize>>,
}

impl FontCatalog {
    /// Create an empty catalog.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            family_index: HashMap::new(),
        }
    }

    /// Add a font entry to the catalog.
    pub fn add(&mut self, entry: FontEntry) {
        let family = entry.family.clone();
        let idx = self.entries.len();
        self.entries.push(entry);
        self.family_index.entry(family).or_default().push(idx);
    }

    /// Check if a family exists in the catalog.
    #[must_use]
    pub fn has_family(&self, family: &str) -> bool {
        self.family_index.contains_key(family)
    }

    /// Get all entries for a family.
    #[must_use]
    pub fn family_entries(&self, family: &str) -> Vec<&FontEntry> {
        self.family_index
            .get(family)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&i| self.entries.get(i))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all unique family names, sorted alphabetically.
    #[must_use]
    pub fn families(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.family_index.keys().map(|s| s.as_str()).collect();
        names.sort_unstable();
        names
    }

    /// Get the total number of font entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the catalog is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the number of unique families.
    #[must_use]
    pub fn family_count(&self) -> usize {
        self.family_index.len()
    }

    /// Remove all entries matching a family name.
    pub fn remove_family(&mut self, family: &str) -> Vec<FontEntry> {
        let indices = self.family_index.remove(family).unwrap_or_default();
        // Remove in reverse order to preserve indices.
        let mut removed = Vec::new();
        let mut sorted_indices = indices;
        sorted_indices.sort_unstable();
        for &idx in sorted_indices.iter().rev() {
            if idx < self.entries.len() {
                removed.push(self.entries.remove(idx));
            }
        }
        // Rebuild the family index.
        self.rebuild_index();
        removed.reverse();
        removed
    }

    /// Rebuild the family index from scratch.
    fn rebuild_index(&mut self) {
        self.family_index.clear();
        for (i, entry) in self.entries.iter().enumerate() {
            self.family_index
                .entry(entry.family.clone())
                .or_default()
                .push(i);
        }
    }

    /// Find entries matching a tag.
    #[must_use]
    pub fn entries_with_tag(&self, tag: &str) -> Vec<&FontEntry> {
        self.entries
            .iter()
            .filter(|e| e.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Find entries from a specific source.
    #[must_use]
    pub fn entries_from_source(&self, source: &FontSource) -> Vec<&FontEntry> {
        self.entries
            .iter()
            .filter(|e| &e.source == source)
            .collect()
    }

    /// Get activated entries only.
    #[must_use]
    pub fn activated_entries(&self) -> Vec<&FontEntry> {
        self.entries.iter().filter(|e| e.activated).collect()
    }

    /// Activate all entries in a family.
    pub fn activate_family(&mut self, family: &str) {
        if let Some(indices) = self.family_index.get(family) {
            for &idx in indices {
                if let Some(entry) = self.entries.get_mut(idx) {
                    entry.activated = true;
                }
            }
        }
    }

    /// Deactivate all entries in a family.
    pub fn deactivate_family(&mut self, family: &str) {
        if let Some(indices) = self.family_index.get(family) {
            for &idx in indices {
                if let Some(entry) = self.entries.get_mut(idx) {
                    entry.activated = false;
                }
            }
        }
    }
}

impl Default for FontCatalog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(family: &str, style: &str, weight: u16, italic: bool) -> FontEntry {
        FontEntry {
            family: family.into(),
            style: style.into(),
            weight,
            italic,
            path: PathBuf::from(format!("/fonts/{family}-{style}.ttf")),
            format: "ttf".into(),
            file_size: 50_000,
            source: FontSource::System,
            tags: Vec::new(),
            activated: true,
            glyph_count: 200,
            script_coverage: vec!["latin".into()],
            version: "1.0".into(),
            license: "OFL".into(),
            designer: "Test Designer".into(),
        }
    }

    #[test]
    fn empty_catalog() {
        let catalog = FontCatalog::new();
        assert!(catalog.is_empty());
        assert_eq!(catalog.len(), 0);
        assert_eq!(catalog.family_count(), 0);
        assert!(catalog.families().is_empty());
    }

    #[test]
    fn add_and_query_entries() {
        let mut catalog = FontCatalog::new();
        catalog.add(make_entry("Manrope", "Regular", 400, false));
        catalog.add(make_entry("Manrope", "Bold", 700, false));
        catalog.add(make_entry("JetBrains Mono", "Regular", 400, false));

        assert_eq!(catalog.len(), 3);
        assert_eq!(catalog.family_count(), 2);
        assert!(catalog.has_family("Manrope"));
        assert!(catalog.has_family("JetBrains Mono"));
        assert!(!catalog.has_family("Inter"));

        let manrope = catalog.family_entries("Manrope");
        assert_eq!(manrope.len(), 2);
    }

    #[test]
    fn families_sorted_alphabetically() {
        let mut catalog = FontCatalog::new();
        catalog.add(make_entry("Zilla Slab", "Regular", 400, false));
        catalog.add(make_entry("Arial", "Regular", 400, false));
        catalog.add(make_entry("Manrope", "Regular", 400, false));

        let families = catalog.families();
        assert_eq!(families, vec!["Arial", "Manrope", "Zilla Slab"]);
    }

    #[test]
    fn remove_family() {
        let mut catalog = FontCatalog::new();
        catalog.add(make_entry("Manrope", "Regular", 400, false));
        catalog.add(make_entry("Manrope", "Bold", 700, false));
        catalog.add(make_entry("Inter", "Regular", 400, false));

        let removed = catalog.remove_family("Manrope");
        assert_eq!(removed.len(), 2);
        assert!(!catalog.has_family("Manrope"));
        assert!(catalog.has_family("Inter"));
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn remove_nonexistent_family() {
        let mut catalog = FontCatalog::new();
        catalog.add(make_entry("Inter", "Regular", 400, false));

        let removed = catalog.remove_family("NoSuchFont");
        assert!(removed.is_empty());
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn entries_with_tag() {
        let mut catalog = FontCatalog::new();
        let mut entry = make_entry("Manrope", "Regular", 400, false);
        entry.tags = vec!["ui".into(), "sans".into()];
        catalog.add(entry);
        catalog.add(make_entry("Inter", "Regular", 400, false));

        let tagged = catalog.entries_with_tag("ui");
        assert_eq!(tagged.len(), 1);
        assert_eq!(tagged[0].family, "Manrope");

        assert!(catalog.entries_with_tag("mono").is_empty());
    }

    #[test]
    fn entries_from_source() {
        let mut catalog = FontCatalog::new();
        catalog.add(make_entry("Manrope", "Regular", 400, false));
        let mut gf_entry = make_entry("Roboto", "Regular", 400, false);
        gf_entry.source = FontSource::GoogleFonts;
        catalog.add(gf_entry);

        let system = catalog.entries_from_source(&FontSource::System);
        assert_eq!(system.len(), 1);
        let google = catalog.entries_from_source(&FontSource::GoogleFonts);
        assert_eq!(google.len(), 1);
        assert_eq!(google[0].family, "Roboto");
    }

    #[test]
    fn activate_deactivate_family() {
        let mut catalog = FontCatalog::new();
        catalog.add(make_entry("Manrope", "Regular", 400, false));
        catalog.add(make_entry("Manrope", "Bold", 700, false));

        assert_eq!(catalog.activated_entries().len(), 2);

        catalog.deactivate_family("Manrope");
        assert!(catalog.activated_entries().is_empty());

        catalog.activate_family("Manrope");
        assert_eq!(catalog.activated_entries().len(), 2);
    }

    #[test]
    fn font_source_display() {
        assert_eq!(FontSource::System.to_string(), "System");
        assert_eq!(FontSource::GoogleFonts.to_string(), "Google Fonts");
        assert_eq!(
            FontSource::Url {
                url: "https://example.com/font.ttf".into()
            }
            .to_string(),
            "URL: https://example.com/font.ttf"
        );
    }
}
