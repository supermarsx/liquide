//! Comprehensive font management for the LiquiDE desktop environment.
//!
//! This crate provides:
//! - Font discovery, installation, and uninstallation
//! - Google Fonts integration for remote font fetching
//! - Font preview with Lorem Ipsum or custom text samples
//! - Font collections (create, export, import)
//! - Font family grouping and tagging
//! - Hot-reloadable font configuration with fallback chains
//! - Glyph inspection and manipulation
//! - Background indexing with a dedicated worker thread
//! - Drag-and-drop installation support
//! - Import from Git repos and URLs
//! - Per-role font assignments (UI, terminal, titles, data, accessibility, emoji)

pub mod catalog;
pub mod collection;
pub mod config;
pub mod error;
pub mod family;
pub mod glyph;
pub mod google_fonts;
pub mod hot_reload;
pub mod index;
pub mod install;
pub mod preview;
pub mod provider;
pub mod roles;
pub mod search;
pub mod tag;

pub use config::FontConfig;
pub use error::{FontError, Result};
pub use roles::{FontRole, FontStack};

/// System font discovery façade.
///
/// Re-exports the system-scan / metadata API from `liquide-font-manager`
/// so consumers can rely on `liquide-fonts` as the single font façade.
/// `SystemScanner` is an alias for `liquide_font_manager::FontManager`
/// (the discovery/preview/fallback hub) to avoid colliding with this
/// crate's own role-oriented `FontManager`.
pub mod system_scan {
    pub use liquide_font_manager::{
        FontError as SystemScanError, FontFallback, FontFormat, FontInfo,
        FontManager as SystemScanner, FontPreview, FontStretch, FontStyle, FontWeight,
        PreviewConfig, UnicodeBlock,
    };
}

/// The central font manager.
///
/// Coordinates font discovery, installation, indexing, preview, and
/// hot-reload.  Designed to be shared across threads via `Arc`.
pub struct FontManager {
    config: config::FontConfig,
    catalog: catalog::FontCatalog,
    index: index::FontIndex,
    collections: collection::CollectionStore,
    google: google_fonts::GoogleFontsClient,
    watcher: hot_reload::FontWatcher,
}

impl FontManager {
    /// Create a new font manager with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::from_config(FontConfig::default())
    }

    /// Create a new font manager from explicit configuration.
    #[must_use]
    pub fn from_config(config: FontConfig) -> Self {
        let catalog = catalog::FontCatalog::new();
        let index = index::FontIndex::new();
        let collections = collection::CollectionStore::new();
        let google = google_fonts::GoogleFontsClient::new(config.google_fonts.clone());
        let watcher = hot_reload::FontWatcher::new(config.watch_dirs.clone());
        Self {
            config,
            catalog,
            index,
            collections,
            google,
            watcher,
        }
    }

    /// Get the current font configuration.
    #[must_use]
    pub fn config(&self) -> &FontConfig {
        &self.config
    }

    /// Get the font catalog.
    #[must_use]
    pub fn catalog(&self) -> &catalog::FontCatalog {
        &self.catalog
    }

    /// Get the mutable font catalog.
    pub fn catalog_mut(&mut self) -> &mut catalog::FontCatalog {
        &mut self.catalog
    }

    /// Get the font index.
    #[must_use]
    pub fn index(&self) -> &index::FontIndex {
        &self.index
    }

    /// Get the collection store.
    #[must_use]
    pub fn collections(&self) -> &collection::CollectionStore {
        &self.collections
    }

    /// Get the collection store mutably.
    pub fn collections_mut(&mut self) -> &mut collection::CollectionStore {
        &mut self.collections
    }

    /// Get the Google Fonts client.
    #[must_use]
    pub fn google_fonts(&self) -> &google_fonts::GoogleFontsClient {
        &self.google
    }

    /// Resolve the effective font file path for a given role.
    ///
    /// Walks the fallback chain until a font that exists on disk is found.
    #[must_use]
    pub fn resolve_font_for_role(&self, role: FontRole) -> Option<&str> {
        let stack = self.config.stack_for_role(role);
        for family in &stack.families {
            if self.catalog.has_family(family) {
                return Some(family.as_str());
            }
        }
        None
    }

    /// Trigger a full re-index of all font directories.
    pub fn reindex(&mut self) {
        self.index.clear();
        for entry in &self.catalog.entries {
            self.index.add_entry(entry);
        }
        tracing::info!(count = self.index.len(), "font index rebuilt");
    }

    /// Apply a new configuration and rebuild the directory watcher.
    ///
    /// Returns `true` if the role assignments changed compared to the
    /// previous configuration, `false` otherwise.
    ///
    /// The rebuilt [`hot_reload::FontWatcher`] watches the new `watch_dirs` and
    /// performs **real** change detection on [`FontManager::poll_font_changes`]
    /// (or directly via `FontWatcher::scan`). Applying a detected change to
    /// already-rasterized faces is the renderer's responsibility: it owns the
    /// `FontDatabase` and calls `reload_face` / `invalidate_stale_fonts` (the
    /// reload API that landed for t49-e3-F15). This crate does not hold a
    /// `FontDatabase` handle, so `apply_config` updates config + watcher and
    /// surfaces changes; it does not itself mutate any glyph cache.
    pub fn apply_config(&mut self, new_config: FontConfig) -> bool {
        let roles_changed = self.config.roles != new_config.roles;
        self.config = new_config;
        let mut watcher = hot_reload::FontWatcher::new(self.config.watch_dirs.clone());
        watcher.start();
        self.watcher = watcher;
        if roles_changed {
            tracing::info!("font role assignments changed; watcher rebuilt over new watch_dirs");
        }
        roles_changed
    }

    /// Poll the directory watcher for font-file changes since the last poll.
    ///
    /// Performs a real directory scan (respecting the watcher's poll cadence)
    /// and returns the detected [`hot_reload::FontChange`] entries. An inactive
    /// watcher, or one polled before its interval elapses, returns an empty
    /// list. Callers (renderer / indexer) drive the actual reload from the
    /// returned changes.
    pub fn poll_font_changes(&mut self) -> Vec<hot_reload::FontChange> {
        if !self.watcher.should_poll() {
            return Vec::new();
        }
        self.watcher.scan()
    }

    /// Borrow the directory watcher (read-only).
    #[must_use]
    pub fn watcher(&self) -> &hot_reload::FontWatcher {
        &self.watcher
    }

    /// Get the default font stack.
    #[must_use]
    pub fn default_stack() -> FontConfig {
        FontConfig::default()
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roles::FontStack;

    /// Regression for t49-e3-F14: `apply_config` reports whether roles changed
    /// and rebuilds the watcher, but does not itself fabricate reload events —
    /// the rebuilt watcher has not yet scanned, so its pending queue is empty
    /// until a real `scan`/`poll_font_changes` runs.
    #[test]
    fn apply_config_reports_role_change_and_rebuilds_active_watcher() {
        let mut manager = FontManager::new();

        // Same config: no role change reported.
        let same = FontConfig::default();
        assert!(
            !manager.apply_config(same),
            "identical roles must report no change"
        );

        // Now mutate a role and re-apply: change is reported.
        let mut changed = FontConfig::default();
        changed.set_stack(
            FontStack::new(FontRole::StatusBar, vec!["BrandNewFont".into()], 11.0).with_weight(700),
        );
        assert!(
            manager.apply_config(changed),
            "differing roles must report a change"
        );
        // The new config is stored...
        assert_eq!(
            manager
                .config()
                .stack_for_role(FontRole::StatusBar)
                .families[0],
            "BrandNewFont"
        );
        // ...the rebuilt watcher is active (ready to scan)...
        assert!(
            manager.watcher().is_active(),
            "apply_config must start the rebuilt watcher"
        );
        // ...but no change is fabricated before any scan runs.
        assert!(
            manager.watcher.drain_changes().is_empty(),
            "apply_config must not fabricate reload events before scanning"
        );
    }

    /// `poll_font_changes` performs a real scan and surfaces detected file
    /// changes — the live seam that drives a downstream face reload.
    #[test]
    fn poll_font_changes_detects_real_directory_changes() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "liquide-fonts-poll-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Poll.ttf"), b"poll-font").unwrap();

        let mut config = FontConfig::default();
        config.watch_dirs = vec![dir.clone()];
        let mut manager = FontManager::from_config(config);
        manager.watcher.start();
        manager.watcher.set_poll_interval(0);

        let changes = manager.poll_font_changes();
        assert_eq!(changes.len(), 1, "the new font file is detected");
        assert_eq!(changes[0].kind, hot_reload::FontChangeKind::Added);

        let _ = std::fs::remove_dir_all(dir);
    }
}
