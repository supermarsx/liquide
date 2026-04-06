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
    /// On Linux, follows the XDG Trash spec (moves to `~/.local/share/Trash/`
    /// with a `.trashinfo` metadata file).  On macOS, moves to `~/.Trash/`.
    /// On Windows, falls back to in-memory recording (real integration would
    /// use `SHFileOperationW` with `FOF_ALLOWUNDO`).
    ///
    /// If the physical move fails (e.g. cross-device), the file is still
    /// recorded in memory so the caller can decide what to do.
    pub fn trash(&mut self, path: &str, size: u64) -> crate::Result<TrashEntry> {
        // Generate a unique trash path.
        self.counter += 1;
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let trash_name = format!("{}_{}", self.counter, name);
        let trash_base = std::path::Path::new(&self.trash_dir);
        let trash_path = trash_base.join("files").join(&trash_name)
            .to_string_lossy().to_string();

        // Attempt real filesystem trash (best-effort).
        let _ = self.try_physical_trash(path, &trash_name, now);

        let entry = TrashEntry::new(
            path.to_string(),
            trash_path,
            now,
            size,
        );

        self.entries.push(entry.clone());
        Ok(entry)
    }

    /// Attempt to physically move a file into the trash directory.
    ///
    /// Returns `Ok(())` if the move succeeds, `Err` otherwise.
    /// The caller ([`trash`]) treats failure as non-fatal.
    fn try_physical_trash(&self, original: &str, trash_name: &str, deleted_at: u64) -> std::io::Result<()> {
        let src = std::path::Path::new(original);
        if !src.exists() {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "source not found"));
        }

        let trash_base = std::path::Path::new(&self.trash_dir);
        let files_dir = trash_base.join("files");
        let info_dir = trash_base.join("info");
        std::fs::create_dir_all(&files_dir)?;
        std::fs::create_dir_all(&info_dir)?;

        // Write .trashinfo (XDG Trash spec, also useful on macOS for our own restore).
        #[cfg(not(target_os = "windows"))]
        {
            let info_content = format!(
                "[Trash Info]\nPath={}\nDeletionDate={}\n",
                original,
                format_deletion_date(deleted_at),
            );
            std::fs::write(info_dir.join(format!("{trash_name}.trashinfo")), info_content)?;
        }
        #[cfg(target_os = "windows")]
        {
            let _ = (info_dir, deleted_at);
        }

        // Move the file.
        let dest = files_dir.join(trash_name);
        std::fs::rename(src, &dest)?;
        Ok(())
    }

    /// Restore a file from the trash back to its original location.
    ///
    /// Attempts to physically move the file back if the trash path exists on
    /// disk.  Always removes the entry from the in-memory list.
    pub fn restore(&mut self, entry: &TrashEntry) -> crate::Result<()> {
        let idx = self.entries.iter().position(|e| e.trash_path == entry.trash_path);
        match idx {
            Some(i) => {
                // Try physical restore.
                let trash_file = std::path::Path::new(&entry.trash_path);
                if trash_file.exists() {
                    let original = std::path::Path::new(&entry.original_path);
                    // Ensure parent directory exists.
                    if let Some(parent) = original.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    std::fs::rename(trash_file, original)
                        .map_err(|e| crate::FilesError::Io(e.to_string()))?;

                    // Remove .trashinfo file if it exists.
                    let trash_base = std::path::Path::new(&self.trash_dir);
                    let trash_name = trash_file.file_name().unwrap_or_default();
                    let info_file = trash_base.join("info")
                        .join(format!("{}.trashinfo", trash_name.to_string_lossy()));
                    let _ = std::fs::remove_file(info_file);
                }

                self.entries.remove(i);
                Ok(())
            }
            None => Err(crate::FilesError::FileNotFound {
                path: entry.trash_path.clone(),
            }),
        }
    }

    /// Permanently delete all items in the trash.
    ///
    /// Also attempts to remove the physical trash files from disk.
    pub fn empty_trash(&mut self) {
        for entry in &self.entries {
            let trash_file = std::path::Path::new(&entry.trash_path);
            if trash_file.exists() {
                if trash_file.is_dir() {
                    let _ = std::fs::remove_dir_all(trash_file);
                } else {
                    let _ = std::fs::remove_file(trash_file);
                }
            }
            // Also remove .trashinfo.
            let trash_base = std::path::Path::new(&self.trash_dir);
            if let Some(name) = trash_file.file_name() {
                let info = trash_base.join("info")
                    .join(format!("{}.trashinfo", name.to_string_lossy()));
                let _ = std::fs::remove_file(info);
            }
        }
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

    /// Scan the physical trash directory and populate the in-memory list.
    ///
    /// This reads the `files/` subdirectory and, on Unix, parses `.trashinfo`
    /// files for original paths and deletion dates.
    pub fn load_from_disk(&mut self) {
        let trash_base = std::path::Path::new(&self.trash_dir);
        let files_dir = trash_base.join("files");
        let info_dir = trash_base.join("info");

        let read_dir = match std::fs::read_dir(&files_dir) {
            Ok(rd) => rd,
            Err(_) => return,
        };

        self.entries.clear();
        self.counter = 0;

        for item in read_dir {
            let item = match item {
                Ok(i) => i,
                Err(_) => continue,
            };
            let trash_name = item.file_name().to_string_lossy().to_string();
            let trash_path = files_dir.join(&trash_name).to_string_lossy().to_string();
            let size = item.metadata().map(|m| m.len()).unwrap_or(0);

            // Try to read .trashinfo for original path and date.
            let mut original_path = trash_name.clone();
            let deleted_at = 0u64;

            let info_file = info_dir.join(format!("{trash_name}.trashinfo"));
            if let Ok(content) = std::fs::read_to_string(&info_file) {
                for line in content.lines() {
                    if let Some(p) = line.strip_prefix("Path=") {
                        original_path = p.to_string();
                    }
                    // DeletionDate is informational; we don't parse it back to epoch.
                }
            }

            self.counter += 1;
            self.entries.push(TrashEntry::new(
                original_path,
                trash_path,
                deleted_at,
                size,
            ));
        }
    }
}

impl Default for TrashManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a Unix epoch timestamp as an ISO-8601-ish string for `.trashinfo`.
#[cfg(not(target_os = "windows"))]
fn format_deletion_date(epoch_secs: u64) -> String {
    // Simple UTC formatting without pulling in chrono.
    // Format: YYYY-MM-DDTHH:MM:SS
    let secs_per_day: u64 = 86400;
    let days = epoch_secs / secs_per_day;
    let time_of_day = epoch_secs % secs_per_day;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Days since epoch -> year/month/day (simplified leap-year calculation).
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}")
}

/// Convert days since Unix epoch to (year, month, day).
#[cfg(not(target_os = "windows"))]
fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut month = 1u64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

#[cfg(not(target_os = "windows"))]
fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
