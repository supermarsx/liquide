//! XDG Autostart directories.
//!
//! Provides enumeration and management of autostart entries following the
//! freedesktop.org Autostart specification. Autostart entries are `.desktop`
//! files placed in `$XDG_CONFIG_HOME/autostart/` (user) and
//! `$XDG_CONFIG_DIRS/autostart/` (system-wide).

use std::path::{Path, PathBuf};

use crate::base_dirs::{self, XdgDirs};
use crate::desktop_entry::{DesktopEntry, ParseError};

/// An autostart entry with its file path and enabled/disabled state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutostartEntry {
    /// Path to the `.desktop` file.
    pub path: PathBuf,
    /// The parsed desktop entry.
    pub entry: DesktopEntry,
    /// Whether autostart is enabled.
    ///
    /// An entry is disabled if it has `Hidden=true` or `X-GNOME-Autostart-enabled=false`.
    pub enabled: bool,
}

/// Errors that can occur in autostart operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutostartError {
    /// Failed to read a desktop file.
    ReadFailed(String),
    /// Failed to parse a desktop file.
    Parse(ParseError),
    /// Failed to write a desktop file.
    WriteFailed(String),
    /// The autostart directory could not be determined.
    NoDirs,
}

impl std::fmt::Display for AutostartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AutostartError::ReadFailed(p) => write!(f, "failed to read: {p}"),
            AutostartError::Parse(e) => write!(f, "parse error: {e}"),
            AutostartError::WriteFailed(p) => write!(f, "failed to write: {p}"),
            AutostartError::NoDirs => write!(f, "autostart directories not available"),
        }
    }
}

impl std::error::Error for AutostartError {}

impl From<ParseError> for AutostartError {
    fn from(e: ParseError) -> Self {
        AutostartError::Parse(e)
    }
}

/// Return the user autostart directory (`$XDG_CONFIG_HOME/autostart`).
pub fn user_autostart_dir(dirs: &XdgDirs) -> PathBuf {
    dirs.config_home.join("autostart")
}

/// Return all system-wide autostart directories.
pub fn system_autostart_dirs() -> Vec<PathBuf> {
    base_dirs::config_dirs()
        .into_iter()
        .map(|d| d.join("autostart"))
        .collect()
}

/// Enumerate autostart entries from both user and system directories.
///
/// User entries override system entries with the same filename.
/// Entries with `Hidden=true` are included but marked as disabled.
pub fn list_autostart_entries(dirs: &XdgDirs) -> Vec<AutostartEntry> {
    let mut entries = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    // User entries take priority.
    let user_dir = user_autostart_dir(dirs);
    collect_entries_from(&user_dir, &mut entries, &mut seen_names);

    // Then system directories.
    for sys_dir in system_autostart_dirs() {
        collect_entries_from(&sys_dir, &mut entries, &mut seen_names);
    }

    entries
}

/// Add an autostart entry for the current user.
///
/// Writes the desktop entry to `$XDG_CONFIG_HOME/autostart/<filename>`.
pub fn add_autostart_entry(
    dirs: &XdgDirs,
    filename: &str,
    entry: &DesktopEntry,
) -> Result<PathBuf, AutostartError> {
    let dir = user_autostart_dir(dirs);
    std::fs::create_dir_all(&dir)
        .map_err(|_| AutostartError::WriteFailed(dir.display().to_string()))?;

    let path = dir.join(filename);
    let content = entry.to_desktop_string();
    std::fs::write(&path, content)
        .map_err(|_| AutostartError::WriteFailed(path.display().to_string()))?;

    Ok(path)
}

/// Remove an autostart entry by filename.
///
/// If the entry exists in a system directory, a user-level override with
/// `Hidden=true` is written instead of deleting the system file.
pub fn remove_autostart_entry(dirs: &XdgDirs, filename: &str) -> Result<(), AutostartError> {
    let user_path = user_autostart_dir(dirs).join(filename);

    if user_path.exists() {
        std::fs::remove_file(&user_path)
            .map_err(|_| AutostartError::WriteFailed(user_path.display().to_string()))?;
        return Ok(());
    }

    // If the entry only exists in system dirs, create a user override.
    let system_exists = system_autostart_dirs()
        .iter()
        .any(|d| d.join(filename).exists());

    if system_exists {
        let override_entry = DesktopEntry {
            name: filename.trim_end_matches(".desktop").to_string(),
            hidden: true,
            ..Default::default()
        };
        add_autostart_entry(dirs, filename, &override_entry)?;
    }

    Ok(())
}

/// Parse a single autostart entry from a file path.
pub fn parse_autostart_entry(path: &Path) -> Result<AutostartEntry, AutostartError> {
    let content = std::fs::read_to_string(path)
        .map_err(|_| AutostartError::ReadFailed(path.display().to_string()))?;
    let entry = DesktopEntry::parse(&content)?;
    let enabled = is_enabled(&entry);
    Ok(AutostartEntry {
        path: path.to_path_buf(),
        entry,
        enabled,
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn is_enabled(entry: &DesktopEntry) -> bool {
    if entry.hidden {
        return false;
    }
    // Check X-GNOME-Autostart-enabled (widely used convention).
    for (k, v) in &entry.extra {
        if k == "X-GNOME-Autostart-enabled" && v == "false" {
            return false;
        }
    }
    true
}

fn collect_entries_from(
    dir: &Path,
    entries: &mut Vec<AutostartEntry>,
    seen: &mut std::collections::HashSet<String>,
) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for dir_entry in read_dir.flatten() {
        let path = dir_entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
            continue;
        }
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !seen.insert(filename) {
            continue; // Already seen from a higher-priority directory.
        }
        if let Ok(ae) = parse_autostart_entry(&path) {
            entries.push(ae);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_xdg(name: &str) -> (PathBuf, XdgDirs) {
        let base = env::temp_dir().join(format!("liquide_xdg_autostart_{name}"));
        let _ = std::fs::remove_dir_all(&base);
        let dirs = XdgDirs::with_home(&base);
        (base, dirs)
    }

    #[test]
    fn user_autostart_dir_path() {
        let dirs = XdgDirs::with_home(Path::new("/home/user"));
        let d = user_autostart_dir(&dirs);
        assert_eq!(d, PathBuf::from("/home/user/.config/autostart"));
    }

    #[test]
    fn system_autostart_dirs_non_empty() {
        let dirs = system_autostart_dirs();
        assert!(!dirs.is_empty());
    }

    #[test]
    fn add_and_parse_entry() {
        let (base, dirs) = temp_xdg("add_parse");
        let entry = DesktopEntry {
            name: "MyApp".into(),
            exec: Some("myapp".into()),
            ..Default::default()
        };
        let path = add_autostart_entry(&dirs, "myapp.desktop", &entry).unwrap();
        assert!(path.exists());

        let parsed = parse_autostart_entry(&path).unwrap();
        assert_eq!(parsed.entry.name, "MyApp");
        assert!(parsed.enabled);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn remove_user_entry() {
        let (base, dirs) = temp_xdg("remove");
        let entry = DesktopEntry {
            name: "RmApp".into(),
            ..Default::default()
        };
        let path = add_autostart_entry(&dirs, "rmapp.desktop", &entry).unwrap();
        assert!(path.exists());

        remove_autostart_entry(&dirs, "rmapp.desktop").unwrap();
        assert!(!path.exists());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn list_entries_from_user_dir() {
        let (base, dirs) = temp_xdg("list");
        let entry = DesktopEntry {
            name: "ListApp".into(),
            exec: Some("listapp".into()),
            ..Default::default()
        };
        add_autostart_entry(&dirs, "listapp.desktop", &entry).unwrap();

        let entries = list_autostart_entries(&dirs);
        let found = entries.iter().any(|e| e.entry.name == "ListApp");
        assert!(found, "expected to find ListApp in autostart entries");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn hidden_entry_is_disabled() {
        let entry = DesktopEntry {
            name: "HiddenApp".into(),
            hidden: true,
            ..Default::default()
        };
        assert!(!is_enabled(&entry));
    }

    #[test]
    fn gnome_autostart_enabled_false_is_disabled() {
        let entry = DesktopEntry {
            name: "GnomeDisabled".into(),
            extra: vec![("X-GNOME-Autostart-enabled".into(), "false".into())],
            ..Default::default()
        };
        assert!(!is_enabled(&entry));
    }

    #[test]
    fn normal_entry_is_enabled() {
        let entry = DesktopEntry {
            name: "NormalApp".into(),
            ..Default::default()
        };
        assert!(is_enabled(&entry));
    }

    #[test]
    fn parse_nonexistent_file_fails() {
        let result = parse_autostart_entry(Path::new("/nonexistent/path/app.desktop"));
        assert!(result.is_err());
    }

    #[test]
    fn autostart_error_display() {
        let e = AutostartError::ReadFailed("/x".into());
        assert_eq!(e.to_string(), "failed to read: /x");
        let e = AutostartError::WriteFailed("/y".into());
        assert_eq!(e.to_string(), "failed to write: /y");
        let e = AutostartError::NoDirs;
        assert_eq!(e.to_string(), "autostart directories not available");
    }

    #[test]
    fn add_creates_directory_if_missing() {
        let (base, dirs) = temp_xdg("mkdir");
        let autostart_dir = user_autostart_dir(&dirs);
        assert!(!autostart_dir.exists());

        let entry = DesktopEntry {
            name: "MkdirApp".into(),
            ..Default::default()
        };
        add_autostart_entry(&dirs, "mkdirapp.desktop", &entry).unwrap();
        assert!(autostart_dir.exists());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn remove_nonexistent_entry_succeeds() {
        let (base, dirs) = temp_xdg("rm_nonexist");
        // Should not error even if the file doesn't exist.
        let result = remove_autostart_entry(&dirs, "nonexistent.desktop");
        assert!(result.is_ok());
        let _ = std::fs::remove_dir_all(&base);
    }
}
