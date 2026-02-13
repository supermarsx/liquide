//! Font provider abstraction — unified interface for obtaining fonts
//! from various sources (local filesystem, Google Fonts, URLs, Git repos).

use std::path::PathBuf;

use crate::catalog::FontEntry;
use crate::error::Result;

/// A font provider that can supply font files.
pub trait FontProvider: Send + Sync {
    /// Provider name for display.
    fn name(&self) -> &str;

    /// Whether this provider is available (e.g. network reachable).
    fn is_available(&self) -> bool;

    /// List available font families from this provider.
    fn list_families(&self) -> Result<Vec<String>>;

    /// Search for families matching a query.
    fn search(&self, query: &str) -> Result<Vec<String>>;

    /// Download / fetch a font family to a local path.
    fn fetch_family(&self, family: &str, dest: &PathBuf) -> Result<Vec<FontEntry>>;
}

/// Local filesystem font provider.
pub struct LocalProvider {
    /// Directories to scan.
    directories: Vec<PathBuf>,
}

impl LocalProvider {
    /// Create a new local font provider.
    #[must_use]
    pub fn new(directories: Vec<PathBuf>) -> Self {
        Self { directories }
    }

    /// Get the directories being scanned.
    #[must_use]
    pub fn directories(&self) -> &[PathBuf] {
        &self.directories
    }
}

impl FontProvider for LocalProvider {
    fn name(&self) -> &str {
        "Local Filesystem"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn list_families(&self) -> Result<Vec<String>> {
        let mut families = Vec::new();
        for dir in &self.directories {
            if let Ok(paths) = crate::install::scan_directory(dir) {
                for path in paths {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        let family = stem.split(|c: char| c == '-' || c == '_').next().unwrap_or(stem);
                        let f = family.to_string();
                        if !families.contains(&f) {
                            families.push(f);
                        }
                    }
                }
            }
        }
        families.sort();
        Ok(families)
    }

    fn search(&self, query: &str) -> Result<Vec<String>> {
        let all = self.list_families()?;
        let query_lower = query.to_lowercase();
        Ok(all
            .into_iter()
            .filter(|f| f.to_lowercase().contains(&query_lower))
            .collect())
    }

    fn fetch_family(&self, _family: &str, _dest: &PathBuf) -> Result<Vec<FontEntry>> {
        // Local provider doesn't need to fetch — fonts are already on disk.
        Ok(Vec::new())
    }
}
