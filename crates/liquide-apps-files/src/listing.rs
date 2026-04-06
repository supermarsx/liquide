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
        // Try std::path::Path first (handles both / and \ on Windows).
        let p = std::path::Path::new(&self.path);
        if let Some(parent) = p.parent() {
            let s = parent.to_string_lossy().to_string();
            if s.is_empty() || s == self.path {
                return None;
            }
            return Some(s);
        }
        // Fallback: string-based for virtual paths.
        let path = self.path.trim_end_matches('/');
        path.rsplit_once('/').map(|(parent, _)| {
            if parent.is_empty() { "/".to_string() } else { parent.to_string() }
        })
    }

    /// Load entries from the real filesystem at the given path.
    ///
    /// Reads the directory with `std::fs::read_dir`, constructs [`FileEntry`]
    /// items from metadata, and applies the current sort/filter settings.
    pub fn load_directory(&mut self, path: &std::path::Path) -> crate::Result<()> {
        use crate::entry::{EntryKind, guess_mime};

        if !path.is_dir() {
            return Err(crate::FilesError::DirectoryNotFound {
                path: path.display().to_string(),
            });
        }

        let read_dir = std::fs::read_dir(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                crate::FilesError::PermissionDenied { path: path.display().to_string() }
            } else {
                crate::FilesError::Io(e.to_string())
            }
        })?;

        let mut entries = Vec::new();
        for item in read_dir {
            let item = match item {
                Ok(i) => i,
                Err(_) => continue, // skip unreadable entries
            };
            let metadata = match item.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let name = item.file_name().to_string_lossy().to_string();
            let full_path = item.path().to_string_lossy().to_string();

            let kind = if metadata.is_dir() {
                EntryKind::Directory
            } else if metadata.is_symlink() {
                EntryKind::Symlink
            } else {
                EntryKind::File
            };

            let size = metadata.len();
            let modified = metadata.modified().ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let hidden = is_hidden(&item, &name);

            let extension = if kind == EntryKind::Directory {
                String::new()
            } else {
                name.rsplit('.')
                    .next()
                    .filter(|e| *e != name.as_str())
                    .unwrap_or("")
                    .to_string()
            };

            let mime_type = if kind == EntryKind::Directory {
                "inode/directory".to_string()
            } else {
                guess_mime(&extension)
            };

            let permissions = format_permissions(&metadata);

            let symlink_target = if kind == EntryKind::Symlink {
                std::fs::read_link(item.path())
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
            } else {
                None
            };

            entries.push(FileEntry {
                name,
                path: full_path,
                kind,
                size,
                modified,
                extension,
                hidden,
                permissions,
                symlink_target,
                mime_type,
            });
        }

        self.path = path.to_string_lossy().to_string();
        self.set_entries(entries);
        Ok(())
    }
}

/// Check whether a directory entry is hidden (platform-specific).
fn is_hidden(_entry: &std::fs::DirEntry, name: &str) -> bool {
    #[cfg(unix)]
    {
        let _ = _entry;
        name.starts_with('.')
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_HIDDEN = 0x02
        _entry.metadata()
            .map(|m| m.file_attributes() & 0x02 != 0)
            .unwrap_or_else(|_| name.starts_with('.'))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = _entry;
        name.starts_with('.')
    }
}

/// Format filesystem permissions into a [`Permissions`] struct.
fn format_permissions(meta: &std::fs::Metadata) -> crate::entry::Permissions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        crate::entry::Permissions::from_mode(meta.permissions().mode())
    }
    #[cfg(not(unix))]
    {
        let readonly = meta.permissions().readonly();
        crate::entry::Permissions {
            readable: true,
            writable: !readonly,
            executable: false,
            mode: if readonly { 0o444 } else { 0o644 },
        }
    }
}
