//! Directory listing management.

use crate::config::{SortField, ViewMode};
use crate::entry::FileEntry;
use crate::sort::sort_entries;

/// A directory listing.
#[derive(Debug, Clone)]
pub struct DirectoryListing {
    /// Current path.
    pub path: String,
    /// Sorted entries (visible based on filters).
    pub entries: Vec<FileEntry>,
    /// Total count before filtering.
    pub total_count: usize,
    /// Whether hidden files are shown.
    pub show_hidden: bool,
    /// Current sort field.
    pub sort_field: SortField,
    /// Sort direction.
    pub sort_ascending: bool,
    /// Current view mode.
    pub view_mode: ViewMode,
}

impl DirectoryListing {
    /// Create a new empty listing.
    #[must_use]
    pub fn new(path: String) -> Self {
        Self {
            path,
            entries: Vec::new(),
            total_count: 0,
            show_hidden: false,
            sort_field: SortField::Name,
            sort_ascending: true,
            view_mode: ViewMode::List,
        }
    }

    /// Set entries and apply sorting/filtering.
    pub fn set_entries(&mut self, mut entries: Vec<FileEntry>) {
        self.total_count = entries.len();
        if !self.show_hidden {
            entries.retain(|e| !e.hidden);
        }
        sort_entries(&mut entries, self.sort_field, self.sort_ascending);
        self.entries = entries;
    }

    /// Toggle hidden file visibility.
    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
    }

    /// Set sort field and re-sort.
    pub fn set_sort(&mut self, field: SortField, ascending: bool) {
        self.sort_field = field;
        self.sort_ascending = ascending;
        sort_entries(&mut self.entries, field, ascending);
    }

    /// Get entry by index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&FileEntry> {
        self.entries.get(index)
    }

    /// Find entry by name.
    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Option<&FileEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Count of visible entries.
    #[must_use]
    pub fn visible_count(&self) -> usize { self.entries.len() }

    /// Count of directories in the listing.
    #[must_use]
    pub fn dir_count(&self) -> usize { self.entries.iter().filter(|e| e.is_dir()).count() }

    /// Count of files in the listing.
    #[must_use]
    pub fn file_count(&self) -> usize { self.entries.iter().filter(|e| !e.is_dir()).count() }

    /// Total size of all files.
    #[must_use]
    pub fn total_size(&self) -> u64 { self.entries.iter().map(|e| e.size).sum() }

    /// Parent path, if any.
    #[must_use]
    pub fn parent(&self) -> Option<String> {
        let path = self.path.trim_end_matches('/');
        path.rsplit_once('/').map(|(parent, _)| {
            if parent.is_empty() { "/".to_string() } else { parent.to_string() }
        })
    }
}
