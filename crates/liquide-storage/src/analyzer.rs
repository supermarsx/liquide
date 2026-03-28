//! Disk usage analysis tools.

use std::fs;
use std::path::Path;

/// Usage information for a single directory.
#[derive(Debug, Clone)]
pub struct DirUsage {
    /// Absolute path of the directory.
    pub path: String,
    /// Total size of all files in this directory and its children, in bytes.
    pub size_bytes: u64,
    /// Number of files in this directory and its children.
    pub file_count: u32,
    /// Child directory usage entries (populated up to `max_depth`).
    pub children: Vec<DirUsage>,
}

/// Information about a single file.
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// Absolute path of the file.
    pub path: String,
    /// File size in bytes.
    pub size_bytes: u64,
}

/// Disk usage summary for a path.
#[derive(Debug, Clone, Copy)]
pub struct DiskUsage {
    /// Total capacity in bytes.
    pub total_bytes: u64,
    /// Bytes currently used.
    pub used_bytes: u64,
    /// Bytes available.
    pub available_bytes: u64,
    /// Usage as a percentage (0.0 to 100.0).
    pub usage_percent: f32,
}

impl DiskUsage {
    /// Construct a `DiskUsage` from total and available bytes.
    pub fn from_total_available(total_bytes: u64, available_bytes: u64) -> Self {
        let used_bytes = total_bytes.saturating_sub(available_bytes);
        let usage_percent = if total_bytes == 0 {
            0.0
        } else {
            (used_bytes as f64 / total_bytes as f64 * 100.0) as f32
        };
        Self {
            total_bytes,
            used_bytes,
            available_bytes,
            usage_percent,
        }
    }
}

/// Analyze disk usage of a directory recursively.
///
/// Walks the directory tree up to `max_depth` levels deep. A `max_depth` of 0
/// means only the target directory itself (no children). The returned
/// `DirUsage` always contains the total recursive size regardless of depth;
/// the `children` field is only populated for depths less than `max_depth`.
///
/// Inaccessible entries (permission denied, broken symlinks) are silently
/// skipped.
pub fn analyze_directory(path: &str, max_depth: u32) -> DirUsage {
    analyze_dir_recursive(path, 0, max_depth)
}

fn analyze_dir_recursive(path: &str, current_depth: u32, max_depth: u32) -> DirUsage {
    let mut total_size: u64 = 0;
    let mut total_files: u32 = 0;
    let mut children = Vec::new();

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => {
            return DirUsage {
                path: path.to_string(),
                size_bytes: 0,
                file_count: 0,
                children: Vec::new(),
            };
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let entry_path = entry.path();
        let entry_path_str = entry_path.to_string_lossy().to_string();

        if metadata.is_file() {
            total_size += metadata.len();
            total_files += 1;
        } else if metadata.is_dir() {
            let child = analyze_dir_recursive(&entry_path_str, current_depth + 1, max_depth);
            total_size += child.size_bytes;
            total_files += child.file_count;
            if current_depth < max_depth {
                children.push(child);
            }
        }
    }

    // Sort children by size descending for convenience.
    children.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    DirUsage {
        path: path.to_string(),
        size_bytes: total_size,
        file_count: total_files,
        children,
    }
}

/// Find the `count` largest files under `path` (recursive).
///
/// Inaccessible entries are silently skipped.
pub fn largest_files(path: &str, count: usize) -> Vec<FileInfo> {
    let mut files = Vec::new();
    collect_files_recursive(Path::new(path), &mut files);

    // Sort by size descending and truncate.
    files.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    files.truncate(count);
    files
}

fn collect_files_recursive(dir: &Path, files: &mut Vec<FileInfo>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let entry_path = entry.path();

        if metadata.is_file() {
            files.push(FileInfo {
                path: entry_path.to_string_lossy().to_string(),
                size_bytes: metadata.len(),
            });
        } else if metadata.is_dir() {
            collect_files_recursive(&entry_path, files);
        }
    }
}

/// Format a byte count into a human-readable string.
///
/// Uses binary units (KiB, MiB, GiB, TiB) with one decimal place.
///
/// # Examples
///
/// ```
/// use liquide_storage::analyzer::format_size;
/// assert_eq!(format_size(0), "0 B");
/// assert_eq!(format_size(1023), "1023 B");
/// assert_eq!(format_size(1024), "1.0 KiB");
/// assert_eq!(format_size(1_048_576), "1.0 MiB");
/// ```
pub fn format_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;
    const PIB: u64 = 1024 * TIB;

    if bytes >= PIB {
        format!("{:.1} PiB", bytes as f64 / PIB as f64)
    } else if bytes >= TIB {
        format!("{:.1} TiB", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}
