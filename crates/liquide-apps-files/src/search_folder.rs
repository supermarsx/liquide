//! Saved / virtual search folders.
//!
//! A search folder is a virtual directory whose contents are determined by a
//! set of filter criteria rather than a physical path on disk.  This is the
//! same concept as GNOME Files "smart folders" or macOS Finder smart folders.

use crate::entry::FileEntry;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SearchFilter
// ---------------------------------------------------------------------------

/// A set of criteria that a `FileEntry` must satisfy to be included in a
/// search folder.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilter {
    /// Glob-style name pattern (e.g. `"*.rs"`).  Empty means "match any name".
    pub name_pattern: String,
    /// If non-empty, only files with this extension are included.
    pub extension_filter: String,
    /// Minimum file size in bytes (0 = no minimum).
    pub min_size: u64,
    /// Maximum file size in bytes (0 = no maximum).
    pub max_size: u64,
    /// Only include files modified after this timestamp (epoch seconds,
    /// 0 = no constraint).
    pub modified_after: u64,
    /// Only include files modified before this timestamp (epoch seconds,
    /// 0 = no constraint).
    pub modified_before: u64,
    /// MIME type prefix filter (e.g. `"image/"` matches `"image/png"`,
    /// `"image/jpeg"`, etc.).  Empty means "match any MIME type".
    pub mime_type: String,
}

impl SearchFilter {
    /// Create an empty filter that matches everything.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluate whether a `FileEntry` satisfies all active criteria.
    #[must_use]
    pub fn matches(&self, entry: &FileEntry) -> bool {
        // Name pattern (simple glob: `*` prefix/suffix).
        if !self.name_pattern.is_empty() {
            if !glob_match(&self.name_pattern, &entry.name) {
                return false;
            }
        }

        // Extension filter.
        if !self.extension_filter.is_empty() {
            if entry.extension.to_lowercase() != self.extension_filter.to_lowercase() {
                return false;
            }
        }

        // Size range.
        if self.min_size > 0 && entry.size < self.min_size {
            return false;
        }
        if self.max_size > 0 && entry.size > self.max_size {
            return false;
        }

        // Modified time range.
        if self.modified_after > 0 && entry.modified < self.modified_after {
            return false;
        }
        if self.modified_before > 0 && entry.modified > self.modified_before {
            return false;
        }

        // MIME type prefix.
        if !self.mime_type.is_empty() {
            if !entry.mime_type.starts_with(&self.mime_type) {
                return false;
            }
        }

        true
    }

    /// Whether this filter has any active constraints.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.name_pattern.is_empty()
            || !self.extension_filter.is_empty()
            || self.min_size > 0
            || self.max_size > 0
            || self.modified_after > 0
            || self.modified_before > 0
            || !self.mime_type.is_empty()
    }
}

/// Minimal glob matcher supporting `*` as a wildcard prefix/suffix and
/// `*.ext` patterns.
fn glob_match(pattern: &str, name: &str) -> bool {
    let pat = pattern.to_lowercase();
    let hay = name.to_lowercase();

    if pat == "*" {
        return true;
    }

    if let Some(suffix) = pat.strip_prefix('*') {
        if let Some(prefix) = suffix.strip_suffix('*') {
            // *text* — contains
            return hay.contains(prefix);
        }
        // *suffix — ends-with
        return hay.ends_with(suffix.as_ref() as &str);
    }

    if let Some(prefix) = pat.strip_suffix('*') {
        // prefix* — starts-with
        return hay.starts_with(prefix.as_ref() as &str);
    }

    // Exact match.
    hay == pat
}

// ---------------------------------------------------------------------------
// SearchFolder
// ---------------------------------------------------------------------------

/// A named, saved search (virtual folder).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFolder {
    /// User-visible name of the search folder.
    pub name: String,
    /// Free-text query string (for full-text / name search).
    pub query: String,
    /// Base location URI to search within (empty = everywhere).
    pub location: String,
    /// Structured filter criteria.
    pub filters: SearchFilter,
}

impl SearchFolder {
    /// Create a new search folder.
    #[must_use]
    pub fn new(name: &str, query: &str, location: &str, filters: SearchFilter) -> Self {
        Self {
            name: name.to_string(),
            query: query.to_string(),
            location: location.to_string(),
            filters,
        }
    }

    /// Evaluate whether a file entry matches both the free-text query and all
    /// structured filters.
    #[must_use]
    pub fn matches(&self, entry: &FileEntry) -> bool {
        // Free-text query (case-insensitive substring on name).
        if !self.query.is_empty() {
            let needle = self.query.to_lowercase();
            if !entry.name.to_lowercase().contains(&needle) {
                return false;
            }
        }
        self.filters.matches(entry)
    }
}

// ---------------------------------------------------------------------------
// SearchFolderStore
// ---------------------------------------------------------------------------

/// Persistent collection of saved search folders.
pub struct SearchFolderStore {
    folders: Vec<SearchFolder>,
}

impl SearchFolderStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            folders: Vec::new(),
        }
    }

    /// Save (add or update) a search folder.
    pub fn save(&mut self, folder: SearchFolder) {
        if let Some(existing) = self.folders.iter_mut().find(|f| f.name == folder.name) {
            *existing = folder;
        } else {
            self.folders.push(folder);
        }
    }

    /// Delete a search folder by name.
    pub fn delete(&mut self, name: &str) -> bool {
        let before = self.folders.len();
        self.folders.retain(|f| f.name != name);
        self.folders.len() < before
    }

    /// Load (find) a search folder by name.
    #[must_use]
    pub fn load(&self, name: &str) -> Option<&SearchFolder> {
        self.folders.iter().find(|f| f.name == name)
    }

    /// All saved search folders.
    #[must_use]
    pub fn list(&self) -> &[SearchFolder] {
        &self.folders
    }

    /// Number of saved search folders.
    #[must_use]
    pub fn len(&self) -> usize {
        self.folders.len()
    }

    /// Whether the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }
}

impl Default for SearchFolderStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Built-in smart folders
// ---------------------------------------------------------------------------

/// Return the set of built-in "smart folders".
///
/// - **Large Files** — files larger than 100 MB
/// - **Recent Documents** — files modified within the last 7 days
/// - **Images** — files with an `image/*` MIME type
/// - **Videos** — files with a `video/*` MIME type
#[must_use]
pub fn smart_folders() -> Vec<SearchFolder> {
    vec![
        SearchFolder::new(
            "Large Files",
            "",
            "",
            SearchFilter {
                min_size: 100 * 1024 * 1024, // 100 MB
                ..SearchFilter::default()
            },
        ),
        SearchFolder::new(
            "Recent Documents",
            "",
            "",
            SearchFilter {
                // Caller should set `modified_after` relative to "now".
                // We use a sentinel that `matches()` can evaluate: 7 days
                // before a reasonable epoch is meaningless, so we store 0
                // and let the caller pass a concrete timestamp when
                // constructing the folder at runtime.  For the static
                // definition we set a `modified_after` of 0 (disabled) and
                // document that consumers should call
                // `set_recent_window(now_secs)` to activate it.
                modified_after: 0,
                ..SearchFilter::default()
            },
        ),
        SearchFolder::new(
            "Images",
            "",
            "",
            SearchFilter {
                mime_type: "image/".to_string(),
                ..SearchFilter::default()
            },
        ),
        SearchFolder::new(
            "Videos",
            "",
            "",
            SearchFilter {
                mime_type: "video/".to_string(),
                ..SearchFilter::default()
            },
        ),
    ]
}

/// Helper: update the "Recent Documents" smart folder so that `modified_after`
/// is set to `now_secs - 7 days`.
pub fn set_recent_window(folder: &mut SearchFolder, now_secs: u64) {
    let seven_days = 7 * 86_400;
    folder.filters.modified_after = now_secs.saturating_sub(seven_days);
}
