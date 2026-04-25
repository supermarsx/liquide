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

    /// Apply a new configuration, triggering hot-reload if fonts changed.
    pub fn apply_config(&mut self, new_config: FontConfig) {
        let roles_changed = self.config.roles != new_config.roles;
        self.config = new_config;
        self.watcher = hot_reload::FontWatcher::new(self.config.watch_dirs.clone());
        if roles_changed {
            tracing::info!("font role assignments changed — hot-reload triggered");
        }
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
