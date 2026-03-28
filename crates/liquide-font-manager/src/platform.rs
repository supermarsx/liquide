//! Platform-specific font directory enumeration.
//!
//! Provides [`system_font_dirs()`] and [`user_font_dir()`] functions that
//! return the correct paths for the current OS.

use std::path::PathBuf;

/// Return the list of directories where system fonts are installed.
///
/// The returned paths may or may not exist on the current machine — callers
/// should check existence before scanning.
#[must_use]
pub fn system_font_dirs() -> Vec<PathBuf> {
    platform_system_dirs()
}

/// Return the directory where user-installed fonts should be placed.
///
/// Returns `None` if the user's home directory cannot be determined.
#[must_use]
pub fn user_font_dir() -> Option<PathBuf> {
    platform_user_dir()
}

// ── Linux ────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn platform_system_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    // XDG data dirs may contain additional font directories.
    if let Ok(xdg) = std::env::var("XDG_DATA_DIRS") {
        for base in xdg.split(':') {
            let p = PathBuf::from(base).join("fonts");
            if !dirs.contains(&p) {
                dirs.push(p);
            }
        }
    }
    dirs
}

#[cfg(target_os = "linux")]
fn platform_user_dir() -> Option<PathBuf> {
    // Prefer XDG_DATA_HOME, fall back to ~/.local/share/fonts.
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        return Some(PathBuf::from(xdg_data).join("fonts"));
    }
    home_dir().map(|h| h.join(".local").join("share").join("fonts"))
}

// ── Windows ──────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn platform_system_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    // %WINDIR%\Fonts (typically C:\Windows\Fonts).
    if let Ok(windir) = std::env::var("WINDIR") {
        dirs.push(PathBuf::from(windir).join("Fonts"));
    } else {
        dirs.push(PathBuf::from(r"C:\Windows\Fonts"));
    }
    dirs
}

#[cfg(target_os = "windows")]
fn platform_user_dir() -> Option<PathBuf> {
    // %LOCALAPPDATA%\Microsoft\Windows\Fonts
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return Some(
            PathBuf::from(local)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
        );
    }
    home_dir().map(|h| {
        h.join("AppData")
            .join("Local")
            .join("Microsoft")
            .join("Windows")
            .join("Fonts")
    })
}

// ── macOS ────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn platform_system_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("/System/Library/Fonts/Supplemental"),
    ]
}

#[cfg(target_os = "macos")]
fn platform_user_dir() -> Option<PathBuf> {
    home_dir().map(|h| h.join("Library").join("Fonts"))
}

// ── Fallback (other OS) ──────────────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn platform_system_dirs() -> Vec<PathBuf> {
    // Best-effort: try common FHS paths.
    vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ]
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn platform_user_dir() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".fonts"))
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Portable home-directory lookup (avoids pulling in `dirs` crate).
fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

/// Scan a directory tree recursively for font files (by extension).
///
/// Returns the list of absolute paths to font files found.
pub fn scan_font_dir(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if !dir.is_dir() {
        return results;
    }
    scan_recursive(dir, &mut results);
    results.sort();
    results
}

fn scan_recursive(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_recursive(&path, out);
        } else if is_font_file(&path) {
            out.push(path);
        }
    }
}

fn is_font_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            crate::format::FontFormat::from_extension(ext).is_some()
        })
        .unwrap_or(false)
}
