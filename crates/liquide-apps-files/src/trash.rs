//! Cross-platform trash management.
//!
//! Provides a `TrashManager` that moves files to the platform-specific trash
//! location instead of permanently deleting them, with restore support.

use serde::{Deserialize, Serialize};

/// An entry in the trash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashEntry {
    /// The original path before deletion.
    pub original_path: String,
    /// The path inside the trash directory.
    pub trash_path: String,
    /// Unix timestamp when the file was deleted.
    pub deleted_at: u64,
    /// Size of the file in bytes.
    pub size: u64,
}

impl TrashEntry {
    /// Create a new trash entry.
    #[must_use]
    pub fn new(original_path: String, trash_path: String, deleted_at: u64, size: u64) -> Self {
        Self { original_path, trash_path, deleted_at, size }
    }

    /// Get the file name from the original path.
    #[must_use]
    pub fn original_name(&self) -> &str {
        self.original_path
            .rsplit('/')
            .next()
            .or_else(|| self.original_path.rsplit('\\').next())
            .unwrap_or(&self.original_path)
    }
}

/// Cross-platform trash manager.
pub struct TrashManager {
    /// Trash directory path (platform-specific).
    trash_dir: String,
    /// In-memory list of trashed entries.
    entries: Vec<TrashEntry>,
    /// Counter for generating unique trash paths.
    counter: u64,
}

impl TrashManager {
    /// Create a new trash manager with the platform-specific trash directory.
    #[must_use]
    pub fn new() -> Self {
        Self {
            trash_dir: Self::platform_trash_dir(),
            entries: Vec::new(),
            counter: 0,
        }
    }

    /// Create a trash manager with a custom directory (for testing).
    #[must_use]
    pub fn with_dir(trash_dir: String) -> Self {
        Self {
            trash_dir,
            entries: Vec::new(),
            counter: 0,
        }
    }

    /// Get the platform-specific trash directory path.
    #[must_use]
    pub fn platform_trash_dir() -> String {
        #[cfg(target_os = "linux")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return format!("{}/.local/share/Trash", home);
            }
            "~/.local/share/Trash".to_string()
        }
        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return format!("{}/.Trash", home);
            }
            "~/.Trash".to_string()
        }
        #[cfg(target_os = "windows")]
        {
            // The actual $RECYCLE.BIN is per-drive and managed by the OS.
            // We provide a logical path; real integration goes through Win32 SHFileOperation.
            "C:\\$RECYCLE.BIN".to_string()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            "~/.local/share/Trash".to_string()
        }
    }

    /// Get the trash directory path.
    #[must_use]
    pub fn trash_dir(&self) -> &str {
        &self.trash_dir
    }

    /// Move a file to the trash.
    ///
    /// Returns the `TrashEntry` representing the trashed file.
    /// In this in-memory implementation, the file is recorded but not
    /// physically moved (that requires OS integration).
    pub fn trash(&mut self, path: &str, size: u64) -> crate::Result<TrashEntry> {
        // Generate a unique trash path.
        self.counter += 1;
        let name = path
            .rsplit('/')
            .next()
            .or_else(|| path.rsplit('\\').next())
            .unwrap_or(path);
        let trash_path = format!("{}/files/{}_{}", self.trash_dir, self.counter, name);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let entry = TrashEntry::new(
            path.to_string(),
            trash_path,
            now,
            size,
        );

        self.entries.push(entry.clone());
        Ok(entry)
    }

    /// Restore a file from the trash back to its original location.
    pub fn restore(&mut self, entry: &TrashEntry) -> crate::Result<()> {
        let idx = self.entries.iter().position(|e| e.trash_path == entry.trash_path);
        match idx {
            Some(i) => {
                self.entries.remove(i);
                Ok(())
            }
            None => Err(crate::FilesError::FileNotFound {
                path: entry.trash_path.clone(),
            }),
        }
    }

    /// Permanently delete all items in the trash.
    pub fn empty_trash(&mut self) {
        self.entries.clear();
    }

    /// List all items in the trash.
    #[must_use]
    pub fn list_trash(&self) -> &[TrashEntry] {
        &self.entries
    }

    /// Number of items in the trash.
    #[must_use]
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Total size of all trashed items.
    #[must_use]
    pub fn total_size(&self) -> u64 {
        self.entries.iter().map(|e| e.size).sum()
    }

    /// Find a trash entry by original path.
    #[must_use]
    pub fn find_by_original(&self, original_path: &str) -> Option<&TrashEntry> {
        self.entries.iter().find(|e| e.original_path == original_path)
    }
}

impl Default for TrashManager {
    fn default() -> Self {
        Self::new()
    }
}
