//! Platform-specific autostart discovery.
//!
//! Each platform module provides a `discover()` function that returns
//! startup entries found in standard system/user locations.

use crate::entry::{EntrySource, StartupEntry};

/// Discover autostart entries from platform-specific locations.
///
/// - **Linux**: reads `~/.config/autostart/` (User) and `/etc/xdg/autostart/` (System).
/// - **Windows**: reads `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (User)
///   and the user's Startup folder (User).
/// - **macOS**: reads `~/Library/LaunchAgents/` (User).
///
/// On unsupported platforms, returns an empty list.
pub fn discover() -> Vec<StartupEntry> {
    discover_impl()
}

/// Return the platform-specific autostart directories for display/diagnostic purposes.
pub fn autostart_directories() -> Vec<AutostartDirectory> {
    directories_impl()
}

/// Describes a platform autostart directory.
#[derive(Debug, Clone)]
pub struct AutostartDirectory {
    /// Filesystem path to the directory.
    pub path: String,
    /// Whether this is a system or user directory.
    pub source: EntrySource,
    /// Human-readable description.
    pub description: String,
}

// ── Linux ──────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn discover_impl() -> Vec<StartupEntry> {
    let mut entries = Vec::new();

    // System autostart: /etc/xdg/autostart/
    let system_dir = std::path::Path::new("/etc/xdg/autostart");
    if system_dir.is_dir() {
        entries.extend(read_desktop_dir(system_dir, EntrySource::System));
    }

    // User autostart: ~/.config/autostart/
    if let Some(config_dir) = home_config_dir() {
        let user_dir = config_dir.join("autostart");
        if user_dir.is_dir() {
            // User entries override system entries with the same filename.
            let user_entries = read_desktop_dir(&user_dir, EntrySource::User);
            for ue in user_entries {
                // If a system entry with the same id exists, replace it.
                if let Some(pos) = entries.iter().position(|e| e.id == ue.id) {
                    entries[pos] = ue;
                } else {
                    entries.push(ue);
                }
            }
        }
    }

    entries
}

#[cfg(target_os = "linux")]
fn directories_impl() -> Vec<AutostartDirectory> {
    let mut dirs = vec![AutostartDirectory {
        path: "/etc/xdg/autostart".into(),
        source: EntrySource::System,
        description: "System-wide autostart entries".into(),
    }];

    if let Some(config_dir) = home_config_dir() {
        dirs.push(AutostartDirectory {
            path: config_dir.join("autostart").to_string_lossy().into_owned(),
            source: EntrySource::User,
            description: "User autostart entries".into(),
        });
    }

    dirs
}

#[cfg(target_os = "linux")]
fn home_config_dir() -> Option<std::path::PathBuf> {
    // Prefer $XDG_CONFIG_HOME, fall back to ~/.config
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(std::path::PathBuf::from(xdg));
        }
    }
    std::env::var("HOME")
        .ok()
        .map(|h| std::path::PathBuf::from(h).join(".config"))
}

#[cfg(target_os = "linux")]
fn read_desktop_dir(dir: &std::path::Path, source: EntrySource) -> Vec<StartupEntry> {
    let mut entries = Vec::new();
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return entries,
    };

    for dir_entry in read_dir.flatten() {
        let path = dir_entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        match crate::desktop_file::parse_desktop_file(&content) {
            Ok(mut entry) => {
                // Use the filename stem as the id for deduplication.
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    entry.id = stem.to_string();
                }
                entry.source = source;
                entries.push(entry);
            }
            Err(_) => continue,
        }
    }

    entries
}

// ── Windows ────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn discover_impl() -> Vec<StartupEntry> {
    let mut entries = Vec::new();

    // Read from the Startup folder.
    if let Some(startup_dir) = windows_startup_folder() {
        if startup_dir.is_dir() {
            entries.extend(read_startup_folder(&startup_dir));
        }
    }

    // Read from the registry: HKCU\Software\Microsoft\Windows\CurrentVersion\Run
    entries.extend(read_windows_registry_run());

    entries
}

#[cfg(target_os = "windows")]
fn directories_impl() -> Vec<AutostartDirectory> {
    let mut dirs = Vec::new();

    if let Some(startup_dir) = windows_startup_folder() {
        dirs.push(AutostartDirectory {
            path: startup_dir.to_string_lossy().into_owned(),
            source: EntrySource::User,
            description: "Windows Startup folder".into(),
        });
    }

    dirs.push(AutostartDirectory {
        path: r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run".into(),
        source: EntrySource::User,
        description: "Windows registry Run key".into(),
    });

    dirs
}

#[cfg(target_os = "windows")]
fn windows_startup_folder() -> Option<std::path::PathBuf> {
    // %APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup
    std::env::var("APPDATA").ok().map(|appdata| {
        std::path::PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Startup")
    })
}

#[cfg(target_os = "windows")]
fn read_startup_folder(dir: &std::path::Path) -> Vec<StartupEntry> {
    let mut entries = Vec::new();
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return entries,
    };

    for dir_entry in read_dir.flatten() {
        let path = dir_entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Windows startup folder can contain .lnk (shortcuts), .bat, .exe, .cmd files.
        if !["lnk", "bat", "exe", "cmd"].contains(&ext) {
            continue;
        }

        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        entries.push(StartupEntry {
            id: stem.clone(),
            name: stem.clone(),
            command: path.to_string_lossy().into_owned(),
            comment: None,
            icon: None,
            enabled: true,
            delay_seconds: 0,
            only_show_in: Vec::new(),
            not_show_in: Vec::new(),
            source: EntrySource::User,
        });
    }

    entries
}

#[cfg(target_os = "windows")]
fn read_windows_registry_run() -> Vec<StartupEntry> {
    // Registry reading requires winreg or windows-sys. Since this project
    // avoids heavyweight dependencies, we parse the output of `reg query`.
    let output = match std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("HKEY_") {
            continue;
        }

        // Lines look like: "    AppName    REG_SZ    C:\path\to\app.exe --flag"
        let parts: Vec<&str> = trimmed.splitn(3, "    ").collect();
        if parts.len() < 3 {
            // Try splitting on any whitespace sequence with at least 2 spaces.
            let parts2: Vec<&str> = trimmed.splitn(3, "  ").collect();
            if parts2.len() >= 3 {
                let name = parts2[0].trim().to_string();
                let command = parts2[2].trim().to_string();
                if !command.is_empty() {
                    let id = name.to_lowercase().replace(' ', "-");
                    entries.push(StartupEntry {
                        id,
                        name: name.clone(),
                        command,
                        comment: None,
                        icon: None,
                        enabled: true,
                        delay_seconds: 0,
                        only_show_in: Vec::new(),
                        not_show_in: Vec::new(),
                        source: EntrySource::User,
                    });
                }
            }
            continue;
        }

        let name = parts[0].trim().to_string();
        // parts[1] is the type (REG_SZ), parts[2] is the value.
        let command = parts[2].trim().to_string();
        if command.is_empty() {
            continue;
        }

        let id = name.to_lowercase().replace(' ', "-");
        entries.push(StartupEntry {
            id,
            name: name.clone(),
            command,
            comment: None,
            icon: None,
            enabled: true,
            delay_seconds: 0,
            only_show_in: Vec::new(),
            not_show_in: Vec::new(),
            source: EntrySource::User,
        });
    }

    entries
}

// ── macOS ──────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn discover_impl() -> Vec<StartupEntry> {
    let mut entries = Vec::new();

    // ~/Library/LaunchAgents/
    if let Some(home) = std::env::var("HOME").ok() {
        let agents_dir = std::path::PathBuf::from(home).join("Library").join("LaunchAgents");
        if agents_dir.is_dir() {
            entries.extend(read_launch_agents(&agents_dir));
        }
    }

    entries
}

#[cfg(target_os = "macos")]
fn directories_impl() -> Vec<AutostartDirectory> {
    let mut dirs = Vec::new();

    if let Some(home) = std::env::var("HOME").ok() {
        dirs.push(AutostartDirectory {
            path: format!("{home}/Library/LaunchAgents"),
            source: EntrySource::User,
            description: "macOS LaunchAgents".into(),
        });
    }

    dirs.push(AutostartDirectory {
        path: "/Library/LaunchAgents".into(),
        source: EntrySource::System,
        description: "System-wide LaunchAgents".into(),
    });

    dirs
}

#[cfg(target_os = "macos")]
fn read_launch_agents(dir: &std::path::Path) -> Vec<StartupEntry> {
    let mut entries = Vec::new();
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return entries,
    };

    for dir_entry in read_dir.flatten() {
        let path = dir_entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("plist") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Simple XML plist parser for the most common fields.
        if let Some(entry) = parse_simple_plist(&content, &path) {
            entries.push(entry);
        }
    }

    entries
}

#[cfg(target_os = "macos")]
fn parse_simple_plist(content: &str, path: &std::path::Path) -> Option<StartupEntry> {
    // Very basic plist extraction: look for Label, ProgramArguments, Disabled.
    let label = extract_plist_string(content, "Label")?;
    let program = extract_plist_array_first(content, "ProgramArguments")
        .or_else(|| extract_plist_string(content, "Program"))?;

    let disabled = content.contains("<key>Disabled</key>")
        && content
            .split("<key>Disabled</key>")
            .nth(1)
            .map(|s| s.trim_start().starts_with("<true/>"))
            .unwrap_or(false);

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&label);

    Some(StartupEntry {
        id: stem.to_string(),
        name: label,
        command: program,
        comment: None,
        icon: None,
        enabled: !disabled,
        delay_seconds: 0,
        only_show_in: Vec::new(),
        not_show_in: Vec::new(),
        source: EntrySource::User,
    })
}

#[cfg(target_os = "macos")]
fn extract_plist_string(content: &str, key: &str) -> Option<String> {
    let key_tag = format!("<key>{key}</key>");
    let after = content.split(&key_tag).nth(1)?;
    let trimmed = after.trim_start();
    if trimmed.starts_with("<string>") {
        let start = "<string>".len();
        let end = trimmed.find("</string>")?;
        Some(trimmed[start..end].to_string())
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn extract_plist_array_first(content: &str, key: &str) -> Option<String> {
    let key_tag = format!("<key>{key}</key>");
    let after = content.split(&key_tag).nth(1)?;
    let trimmed = after.trim_start();
    if trimmed.starts_with("<array>") {
        // Find the first <string>...</string> inside the array.
        let arr_content = &trimmed["<array>".len()..];
        if let Some(s_start) = arr_content.find("<string>") {
            let rest = &arr_content[s_start + "<string>".len()..];
            let s_end = rest.find("</string>")?;
            return Some(rest[..s_end].to_string());
        }
    }
    None
}

// ── Fallback (unsupported platforms) ───────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn discover_impl() -> Vec<StartupEntry> {
    Vec::new()
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn directories_impl() -> Vec<AutostartDirectory> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_returns_vec() {
        // Just verify it doesn't panic. The actual contents depend on the platform.
        let entries = discover();
        // entries may be empty in CI or on a clean machine — that's fine.
        let _ = entries;
    }

    #[test]
    fn autostart_directories_returns_vec() {
        let dirs = autostart_directories();
        // On any supported platform we should get at least one directory.
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        assert!(!dirs.is_empty());
        let _ = dirs;
    }
}
