//! FreeDesktop Trash specification implementation.
//!
//! Implements the freedesktop.org Trash specification v1.0. Trashed files
//! are stored in `$XDG_DATA_HOME/Trash/` with two sub-directories:
//! - `files/` — the actual trashed file data
//! - `info/` — `.trashinfo` metadata files
//!
//! Each `.trashinfo` file records the original path and deletion date.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::base_dirs::XdgDirs;

/// Metadata about a trashed file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashInfo {
    /// Unique trash ID (the filename stem in the `files/` and `info/` dirs).
    pub id: String,
    /// Original absolute path before deletion.
    pub original_path: PathBuf,
    /// ISO 8601 deletion date string (e.g. `"2025-01-15T14:30:00"`).
    pub deletion_date: String,
}

/// Errors that can occur during trash operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrashError {
    /// The file to be trashed does not exist.
    FileNotFound(String),
    /// The trash directory could not be created or accessed.
    TrashDirFailed(String),
    /// The trash info file could not be written.
    InfoWriteFailed(String),
    /// The file move operation failed.
    MoveFailed(String),
    /// The trash entry was not found.
    EntryNotFound(String),
    /// The restore operation failed.
    RestoreFailed(String),
    /// Failed to read trash info file.
    InfoReadFailed(String),
    /// Failed to parse a trash info file.
    InfoParseFailed(String),
}

impl fmt::Display for TrashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrashError::FileNotFound(p) => write!(f, "file not found: {p}"),
            TrashError::TrashDirFailed(p) => write!(f, "trash dir failed: {p}"),
            TrashError::InfoWriteFailed(p) => write!(f, "trash info write failed: {p}"),
            TrashError::MoveFailed(p) => write!(f, "move failed: {p}"),
            TrashError::EntryNotFound(id) => write!(f, "trash entry not found: {id}"),
            TrashError::RestoreFailed(p) => write!(f, "restore failed: {p}"),
            TrashError::InfoReadFailed(p) => write!(f, "info read failed: {p}"),
            TrashError::InfoParseFailed(msg) => write!(f, "info parse failed: {msg}"),
        }
    }
}

impl std::error::Error for TrashError {}

/// Return the base trash directory (`$XDG_DATA_HOME/Trash`).
pub fn trash_dir(dirs: &XdgDirs) -> PathBuf {
    dirs.data_home.join("Trash")
}

/// Return the `files/` subdirectory of the trash.
pub fn trash_files_dir(dirs: &XdgDirs) -> PathBuf {
    trash_dir(dirs).join("files")
}

/// Return the `info/` subdirectory of the trash.
pub fn trash_info_dir(dirs: &XdgDirs) -> PathBuf {
    trash_dir(dirs).join("info")
}

/// Ensure the trash directory structure exists.
pub fn ensure_trash_dirs(dirs: &XdgDirs) -> Result<(), TrashError> {
    let files = trash_files_dir(dirs);
    let info = trash_info_dir(dirs);
    std::fs::create_dir_all(&files)
        .map_err(|_| TrashError::TrashDirFailed(files.display().to_string()))?;
    std::fs::create_dir_all(&info)
        .map_err(|_| TrashError::TrashDirFailed(info.display().to_string()))?;
    Ok(())
}

/// Move a file or directory to the trash.
///
/// The file is moved to `$XDG_DATA_HOME/Trash/files/<id>` and a
/// corresponding `.trashinfo` file is created in `Trash/info/`.
pub fn trash_file(dirs: &XdgDirs, path: &Path) -> Result<TrashInfo, TrashError> {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };

    if !abs_path.exists() {
        return Err(TrashError::FileNotFound(abs_path.display().to_string()));
    }

    ensure_trash_dirs(dirs)?;

    let filename = abs_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed");

    // Generate a unique trash ID to avoid collisions.
    let id = generate_trash_id(dirs, filename);
    let deletion_date = format_deletion_date(SystemTime::now());

    let info = TrashInfo {
        id: id.clone(),
        original_path: abs_path.clone(),
        deletion_date: deletion_date.clone(),
    };

    // Write .trashinfo file.
    let info_path = trash_info_dir(dirs).join(format!("{id}.trashinfo"));
    let info_content = format_trash_info(&abs_path, &deletion_date);
    std::fs::write(&info_path, info_content)
        .map_err(|_| TrashError::InfoWriteFailed(info_path.display().to_string()))?;

    // Move the file.
    let dest = trash_files_dir(dirs).join(&id);
    std::fs::rename(&abs_path, &dest)
        .map_err(|_| TrashError::MoveFailed(abs_path.display().to_string()))?;

    Ok(info)
}

/// Restore a file from the trash to its original location.
pub fn restore_file(dirs: &XdgDirs, trash_id: &str) -> Result<PathBuf, TrashError> {
    let info = read_trash_info(dirs, trash_id)?;

    let trashed_path = trash_files_dir(dirs).join(trash_id);
    if !trashed_path.exists() {
        return Err(TrashError::EntryNotFound(trash_id.to_string()));
    }

    // Ensure the parent directory of the original path exists.
    if let Some(parent) = info.original_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    std::fs::rename(&trashed_path, &info.original_path)
        .map_err(|_| TrashError::RestoreFailed(info.original_path.display().to_string()))?;

    // Remove the .trashinfo file.
    let info_path = trash_info_dir(dirs).join(format!("{trash_id}.trashinfo"));
    let _ = std::fs::remove_file(info_path);

    Ok(info.original_path)
}

/// List all entries currently in the trash.
pub fn list_trash(dirs: &XdgDirs) -> Vec<TrashInfo> {
    let info_dir = trash_info_dir(dirs);
    let mut entries = Vec::new();

    let read_dir = match std::fs::read_dir(&info_dir) {
        Ok(rd) => rd,
        Err(_) => return entries,
    };

    for dir_entry in read_dir.flatten() {
        let path = dir_entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("trashinfo") {
            continue;
        }
        let id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        if let Ok(info) = read_trash_info(dirs, &id) {
            entries.push(info);
        }
    }

    entries
}

/// Permanently delete all files in the trash.
pub fn empty_trash(dirs: &XdgDirs) -> Result<usize, TrashError> {
    let files_dir = trash_files_dir(dirs);
    let info_dir = trash_info_dir(dirs);
    let mut count = 0;

    // Remove all files.
    if let Ok(rd) = std::fs::read_dir(&files_dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
            count += 1;
        }
    }

    // Remove all .trashinfo files.
    if let Ok(rd) = std::fs::read_dir(&info_dir) {
        for entry in rd.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }

    Ok(count)
}

/// Parse a `.trashinfo` file content string.
pub fn parse_trash_info(id: &str, content: &str) -> Result<TrashInfo, TrashError> {
    let mut original_path = None;
    let mut deletion_date = None;
    let mut found_header = false;

    for line in content.lines() {
        let line = line.trim();
        if line == "[Trash Info]" {
            found_header = true;
            continue;
        }
        if !found_header {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "Path" => {
                    // The path is percent-encoded per the spec.
                    original_path = Some(PathBuf::from(percent_decode(value.trim())));
                }
                "DeletionDate" => {
                    deletion_date = Some(value.trim().to_string());
                }
                _ => {}
            }
        }
    }

    if !found_header {
        return Err(TrashError::InfoParseFailed(
            "missing [Trash Info] header".into(),
        ));
    }

    Ok(TrashInfo {
        id: id.to_string(),
        original_path: original_path
            .ok_or_else(|| TrashError::InfoParseFailed("missing Path".into()))?,
        deletion_date: deletion_date
            .ok_or_else(|| TrashError::InfoParseFailed("missing DeletionDate".into()))?,
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn read_trash_info(dirs: &XdgDirs, trash_id: &str) -> Result<TrashInfo, TrashError> {
    let info_path = trash_info_dir(dirs).join(format!("{trash_id}.trashinfo"));
    let content = std::fs::read_to_string(&info_path)
        .map_err(|_| TrashError::InfoReadFailed(info_path.display().to_string()))?;
    parse_trash_info(trash_id, &content)
}

fn generate_trash_id(dirs: &XdgDirs, base_name: &str) -> String {
    let files_dir = trash_files_dir(dirs);

    // Try the plain name first, then append a counter.
    if !files_dir.join(base_name).exists() {
        return base_name.to_string();
    }

    let (stem, ext) = match base_name.rfind('.') {
        Some(i) => (&base_name[..i], Some(&base_name[i..])),
        None => (base_name, None),
    };

    for n in 2..u32::MAX {
        let candidate = match ext {
            Some(ext) => format!("{stem}.{n}{ext}"),
            None => format!("{stem}.{n}"),
        };
        if !files_dir.join(&candidate).exists() {
            return candidate;
        }
    }

    // Extremely unlikely fallback.
    format!("{base_name}.{}", std::process::id())
}

fn format_trash_info(original_path: &Path, deletion_date: &str) -> String {
    format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        percent_encode(&original_path.to_string_lossy()),
        deletion_date
    )
}

fn format_deletion_date(time: SystemTime) -> String {
    let secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Convert to a simple ISO 8601 date-time (UTC).
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;

    let (year, month, day) = days_to_ymd(days_since_epoch);
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}")
}

/// Simple civil date from days since 1970-01-01 (Euclidean affine algorithm).
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from Howard Hinnant's date library (public domain).
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Percent-encode a path string (space -> %20, etc).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push(HEX_UPPER[(byte >> 4) as usize] as char);
                out.push(HEX_UPPER[(byte & 0xf) as usize] as char);
            }
        }
    }
    out
}

/// Percent-decode a string (%20 -> space, etc).
fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((hi << 4 | lo) as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_xdg(name: &str) -> (PathBuf, XdgDirs) {
        let base = env::temp_dir().join(format!("liquide_xdg_trash_{name}"));
        let _ = std::fs::remove_dir_all(&base);
        let dirs = XdgDirs::with_home(&base);
        (base, dirs)
    }

    #[test]
    fn trash_dir_path() {
        let dirs = XdgDirs::with_home(Path::new("/home/user"));
        assert_eq!(
            trash_dir(&dirs),
            PathBuf::from("/home/user/.local/share/Trash")
        );
    }

    #[test]
    fn trash_files_dir_path() {
        let dirs = XdgDirs::with_home(Path::new("/home/user"));
        assert_eq!(
            trash_files_dir(&dirs),
            PathBuf::from("/home/user/.local/share/Trash/files")
        );
    }

    #[test]
    fn trash_info_dir_path() {
        let dirs = XdgDirs::with_home(Path::new("/home/user"));
        assert_eq!(
            trash_info_dir(&dirs),
            PathBuf::from("/home/user/.local/share/Trash/info")
        );
    }

    #[test]
    fn ensure_trash_dirs_creates_structure() {
        let (base, dirs) = temp_xdg("ensure");
        ensure_trash_dirs(&dirs).unwrap();
        assert!(trash_files_dir(&dirs).exists());
        assert!(trash_info_dir(&dirs).exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn parse_trash_info_valid() {
        let content = "\
[Trash Info]
Path=/home/user/Documents/test.txt
DeletionDate=2025-06-15T10:30:00
";
        let info = parse_trash_info("test.txt", content).unwrap();
        assert_eq!(info.id, "test.txt");
        assert_eq!(
            info.original_path,
            PathBuf::from("/home/user/Documents/test.txt")
        );
        assert_eq!(info.deletion_date, "2025-06-15T10:30:00");
    }

    #[test]
    fn parse_trash_info_percent_encoded_path() {
        let content = "\
[Trash Info]
Path=/home/user/My%20Documents/file%20name.txt
DeletionDate=2025-01-01T00:00:00
";
        let info = parse_trash_info("file", content).unwrap();
        assert_eq!(
            info.original_path,
            PathBuf::from("/home/user/My Documents/file name.txt")
        );
    }

    #[test]
    fn parse_trash_info_missing_header() {
        let content = "Path=/home/user/x\nDeletionDate=2025-01-01T00:00:00\n";
        let err = parse_trash_info("x", content).unwrap_err();
        assert_eq!(
            err,
            TrashError::InfoParseFailed("missing [Trash Info] header".into())
        );
    }

    #[test]
    fn parse_trash_info_missing_path() {
        let content = "[Trash Info]\nDeletionDate=2025-01-01T00:00:00\n";
        let err = parse_trash_info("x", content).unwrap_err();
        assert_eq!(err, TrashError::InfoParseFailed("missing Path".into()));
    }

    #[test]
    fn parse_trash_info_missing_deletion_date() {
        let content = "[Trash Info]\nPath=/home/user/x\n";
        let err = parse_trash_info("x", content).unwrap_err();
        assert_eq!(
            err,
            TrashError::InfoParseFailed("missing DeletionDate".into())
        );
    }

    #[test]
    fn percent_encode_decode_roundtrip() {
        let original = "/home/user/My Documents/file (1).txt";
        let encoded = percent_encode(original);
        let decoded = percent_decode(&encoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn percent_encode_preserves_safe_chars() {
        let s = "/home/user/file.txt";
        assert_eq!(percent_encode(s), s);
    }

    #[test]
    fn percent_encode_encodes_spaces() {
        assert!(percent_encode("a b").contains("%20"));
    }

    #[test]
    fn trash_file_and_list() {
        let (base, dirs) = temp_xdg("trash_list");

        // Create a file to trash.
        let source_dir = base.join("source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source_file = source_dir.join("hello.txt");
        std::fs::write(&source_file, "hello world").unwrap();

        let info = trash_file(&dirs, &source_file).unwrap();
        assert!(!source_file.exists(), "original should be gone");
        assert_eq!(info.original_path, source_file);

        // It should appear in list.
        let entries = list_trash(&dirs);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, info.id);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn trash_file_restore() {
        let (base, dirs) = temp_xdg("trash_restore");

        let source_dir = base.join("source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source_file = source_dir.join("restore_me.txt");
        std::fs::write(&source_file, "data").unwrap();

        let info = trash_file(&dirs, &source_file).unwrap();
        assert!(!source_file.exists());

        let restored = restore_file(&dirs, &info.id).unwrap();
        assert_eq!(restored, source_file);
        assert!(source_file.exists());
        assert_eq!(std::fs::read_to_string(&source_file).unwrap(), "data");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn trash_file_not_found() {
        let (base, dirs) = temp_xdg("not_found");
        let result = trash_file(&dirs, Path::new("/nonexistent/file.txt"));
        assert!(matches!(result, Err(TrashError::FileNotFound(_))));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn empty_trash_removes_all() {
        let (base, dirs) = temp_xdg("empty");

        // Create and trash two files.
        let source_dir = base.join("source");
        std::fs::create_dir_all(&source_dir).unwrap();
        for name in &["a.txt", "b.txt"] {
            let f = source_dir.join(name);
            std::fs::write(&f, "content").unwrap();
            trash_file(&dirs, &f).unwrap();
        }

        assert_eq!(list_trash(&dirs).len(), 2);
        let count = empty_trash(&dirs).unwrap();
        assert_eq!(count, 2);
        assert_eq!(list_trash(&dirs).len(), 0);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn duplicate_names_get_unique_ids() {
        let (base, dirs) = temp_xdg("duplicates");

        let source_dir = base.join("src");
        std::fs::create_dir_all(&source_dir).unwrap();

        // Trash two files with the same name (from different locations).
        let f1 = source_dir.join("dup.txt");
        std::fs::write(&f1, "first").unwrap();
        let info1 = trash_file(&dirs, &f1).unwrap();

        std::fs::write(&f1, "second").unwrap();
        let info2 = trash_file(&dirs, &f1).unwrap();

        assert_ne!(info1.id, info2.id, "IDs should be unique");
        assert_eq!(list_trash(&dirs).len(), 2);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn format_deletion_date_produces_iso8601() {
        let date = format_deletion_date(SystemTime::UNIX_EPOCH);
        assert_eq!(date, "1970-01-01T00:00:00");
    }

    #[test]
    fn days_to_ymd_epoch() {
        let (y, m, d) = days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn days_to_ymd_known_date() {
        // 2025-06-15 is 20254 days after 1970-01-01 (approx).
        // Let's test 2000-01-01 = day 10957.
        let (y, m, d) = days_to_ymd(10957);
        assert_eq!((y, m, d), (2000, 1, 1));
    }

    #[test]
    fn trash_error_display_variants() {
        assert_eq!(
            TrashError::FileNotFound("x".into()).to_string(),
            "file not found: x"
        );
        assert_eq!(
            TrashError::EntryNotFound("y".into()).to_string(),
            "trash entry not found: y"
        );
        assert_eq!(
            TrashError::RestoreFailed("z".into()).to_string(),
            "restore failed: z"
        );
        assert_eq!(
            TrashError::InfoParseFailed("bad".into()).to_string(),
            "info parse failed: bad"
        );
    }

    #[test]
    fn restore_nonexistent_entry_fails() {
        let (base, dirs) = temp_xdg("restore_bad");
        ensure_trash_dirs(&dirs).unwrap();
        let result = restore_file(&dirs, "nonexistent");
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}
