use crate::entry::{EntrySource, StartupEntry};
use crate::error::ParseError;
use std::collections::HashMap;

/// Parse a freedesktop .desktop file into a `StartupEntry`.
///
/// Reads the `[Desktop Entry]` section and extracts:
/// - `Name` (required)
/// - `Exec` (required)
/// - `Icon` (optional)
/// - `Comment` (optional)
/// - `Hidden` (optional, maps to `!enabled`)
/// - `X-GNOME-Autostart-enabled` (optional, maps to `enabled`)
/// - `OnlyShowIn` (optional, semicolon-separated)
/// - `NotShowIn` (optional, semicolon-separated)
/// - `X-GNOME-Autostart-Delay` (optional, maps to `delay_seconds`)
///
/// The `id` is derived from the `Name` field (lowercased, spaces replaced with dashes).
/// The `source` defaults to `User`.
pub fn parse_desktop_file(content: &str) -> Result<StartupEntry, ParseError> {
    let section = parse_desktop_entry_section(content)?;

    let name = section
        .get("Name")
        .ok_or_else(|| ParseError::MissingKey("Name".into()))?
        .clone();

    let command = section
        .get("Exec")
        .ok_or_else(|| ParseError::MissingKey("Exec".into()))?
        .clone();

    if command.trim().is_empty() {
        return Err(ParseError::InvalidValue {
            key: "Exec".into(),
            reason: "command must not be empty".into(),
        });
    }

    let id = name.to_lowercase().replace(' ', "-");

    let comment = section.get("Comment").cloned();
    let icon = section.get("Icon").cloned();

    // `Hidden=true` means the entry should be treated as deleted/disabled.
    let hidden = section
        .get("Hidden")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    // X-GNOME-Autostart-enabled takes precedence over Hidden for GNOME autostart entries.
    let gnome_enabled = section
        .get("X-GNOME-Autostart-enabled")
        .map(|v| v.eq_ignore_ascii_case("true"));

    let enabled = match gnome_enabled {
        Some(val) => val,
        None => !hidden,
    };

    let only_show_in = section
        .get("OnlyShowIn")
        .map(|v| parse_semicolon_list(v))
        .unwrap_or_default();

    let not_show_in = section
        .get("NotShowIn")
        .map(|v| parse_semicolon_list(v))
        .unwrap_or_default();

    let delay_seconds = section
        .get("X-GNOME-Autostart-Delay")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);

    Ok(StartupEntry {
        id,
        name,
        command,
        comment,
        icon,
        enabled,
        delay_seconds,
        only_show_in,
        not_show_in,
        source: EntrySource::User,
    })
}

/// Serialize a `StartupEntry` back to .desktop file format.
pub fn write_desktop_file(entry: &StartupEntry) -> String {
    let mut lines = Vec::with_capacity(12);
    lines.push("[Desktop Entry]".to_string());
    lines.push("Type=Application".to_string());
    lines.push(format!("Name={}", entry.name));
    lines.push(format!("Exec={}", entry.command));

    if let Some(ref comment) = entry.comment {
        lines.push(format!("Comment={comment}"));
    }

    if let Some(ref icon) = entry.icon {
        lines.push(format!("Icon={icon}"));
    }

    if !entry.enabled {
        lines.push("Hidden=true".to_string());
        lines.push("X-GNOME-Autostart-enabled=false".to_string());
    }

    if !entry.only_show_in.is_empty() {
        let val = entry.only_show_in.join(";");
        lines.push(format!("OnlyShowIn={val};"));
    }

    if !entry.not_show_in.is_empty() {
        let val = entry.not_show_in.join(";");
        lines.push(format!("NotShowIn={val};"));
    }

    if entry.delay_seconds > 0 {
        lines.push(format!("X-GNOME-Autostart-Delay={}", entry.delay_seconds));
    }

    lines.push(String::new()); // trailing newline
    lines.join("\n")
}

/// Parse the `[Desktop Entry]` section of a .desktop file into key-value pairs.
fn parse_desktop_entry_section(content: &str) -> Result<HashMap<String, String>, ParseError> {
    let mut in_section = false;
    let mut map = HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with('[') {
            if trimmed == "[Desktop Entry]" {
                in_section = true;
                continue;
            } else if in_section {
                // We've reached a different section — stop.
                break;
            }
            continue;
        }

        if in_section {
            if let Some((key, value)) = trimmed.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                // Skip locale-specific keys like Name[de]=...
                if !key.contains('[') {
                    map.insert(key.to_string(), value.to_string());
                }
            }
        }
    }

    if !in_section && map.is_empty() {
        return Err(ParseError::MissingDesktopEntrySection);
    }

    Ok(map)
}

/// Parse a semicolon-separated list (freedesktop convention).
fn parse_semicolon_list(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_desktop_file() {
        let content = "\
[Desktop Entry]
Name=My App
Exec=/usr/bin/myapp
";
        let entry = parse_desktop_file(content).unwrap();
        assert_eq!(entry.name, "My App");
        assert_eq!(entry.command, "/usr/bin/myapp");
        assert_eq!(entry.id, "my-app");
        assert!(entry.enabled);
        assert_eq!(entry.delay_seconds, 0);
        assert!(entry.comment.is_none());
        assert!(entry.icon.is_none());
    }

    #[test]
    fn parse_full_desktop_file() {
        let content = "\
[Desktop Entry]
Type=Application
Name=Slack
Exec=/usr/bin/slack --startup
Comment=Slack Messaging
Icon=slack
Hidden=false
OnlyShowIn=GNOME;KDE;
X-GNOME-Autostart-Delay=5
";
        let entry = parse_desktop_file(content).unwrap();
        assert_eq!(entry.name, "Slack");
        assert_eq!(entry.command, "/usr/bin/slack --startup");
        assert_eq!(entry.comment.as_deref(), Some("Slack Messaging"));
        assert_eq!(entry.icon.as_deref(), Some("slack"));
        assert!(entry.enabled);
        assert_eq!(entry.delay_seconds, 5);
        assert_eq!(entry.only_show_in, vec!["GNOME", "KDE"]);
        assert!(entry.not_show_in.is_empty());
    }

    #[test]
    fn parse_hidden_entry() {
        let content = "\
[Desktop Entry]
Name=Hidden App
Exec=/usr/bin/hidden
Hidden=true
";
        let entry = parse_desktop_file(content).unwrap();
        assert!(!entry.enabled);
    }

    #[test]
    fn parse_gnome_autostart_enabled_overrides_hidden() {
        let content = "\
[Desktop Entry]
Name=Gnome App
Exec=/usr/bin/gnome-app
Hidden=true
X-GNOME-Autostart-enabled=true
";
        let entry = parse_desktop_file(content).unwrap();
        assert!(entry.enabled);
    }

    #[test]
    fn parse_gnome_autostart_disabled() {
        let content = "\
[Desktop Entry]
Name=Disabled App
Exec=/usr/bin/disabled
X-GNOME-Autostart-enabled=false
";
        let entry = parse_desktop_file(content).unwrap();
        assert!(!entry.enabled);
    }

    #[test]
    fn parse_not_show_in() {
        let content = "\
[Desktop Entry]
Name=Test
Exec=/bin/test
NotShowIn=XFCE;LXDE;
";
        let entry = parse_desktop_file(content).unwrap();
        assert!(entry.only_show_in.is_empty());
        assert_eq!(entry.not_show_in, vec!["XFCE", "LXDE"]);
    }

    #[test]
    fn parse_missing_section() {
        let content = "Name=Bad\nExec=/bin/bad\n";
        let err = parse_desktop_file(content).unwrap_err();
        assert_eq!(err, ParseError::MissingDesktopEntrySection);
    }

    #[test]
    fn parse_missing_name() {
        let content = "[Desktop Entry]\nExec=/bin/test\n";
        let err = parse_desktop_file(content).unwrap_err();
        assert_eq!(err, ParseError::MissingKey("Name".into()));
    }

    #[test]
    fn parse_missing_exec() {
        let content = "[Desktop Entry]\nName=Test\n";
        let err = parse_desktop_file(content).unwrap_err();
        assert_eq!(err, ParseError::MissingKey("Exec".into()));
    }

    #[test]
    fn parse_empty_exec() {
        let content = "[Desktop Entry]\nName=Test\nExec=\n";
        let err = parse_desktop_file(content).unwrap_err();
        match err {
            ParseError::InvalidValue { key, .. } => assert_eq!(key, "Exec"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_ignores_locale_keys() {
        let content = "\
[Desktop Entry]
Name=App
Name[de]=Anwendung
Exec=/bin/app
Comment=English comment
Comment[fr]=Commentaire francais
";
        let entry = parse_desktop_file(content).unwrap();
        assert_eq!(entry.name, "App");
        assert_eq!(entry.comment.as_deref(), Some("English comment"));
    }

    #[test]
    fn parse_ignores_other_sections() {
        let content = "\
[Desktop Entry]
Name=App
Exec=/bin/app

[Desktop Action New]
Name=New Window
Exec=/bin/app --new
";
        let entry = parse_desktop_file(content).unwrap();
        assert_eq!(entry.command, "/bin/app");
    }

    #[test]
    fn parse_comments_and_blank_lines() {
        let content = "\
# This is a comment

[Desktop Entry]
# Another comment
Name=App

Exec=/bin/app
";
        let entry = parse_desktop_file(content).unwrap();
        assert_eq!(entry.name, "App");
        assert_eq!(entry.command, "/bin/app");
    }

    #[test]
    fn write_minimal_desktop_file() {
        let entry = StartupEntry::new("test", "Test App", "/bin/test");
        let output = write_desktop_file(&entry);
        assert!(output.contains("[Desktop Entry]"));
        assert!(output.contains("Name=Test App"));
        assert!(output.contains("Exec=/bin/test"));
        assert!(output.contains("Type=Application"));
        assert!(!output.contains("Hidden"));
        assert!(!output.contains("X-GNOME-Autostart-Delay"));
    }

    #[test]
    fn write_disabled_desktop_file() {
        let entry = StartupEntry::new("test", "Test", "/bin/test").with_enabled(false);
        let output = write_desktop_file(&entry);
        assert!(output.contains("Hidden=true"));
        assert!(output.contains("X-GNOME-Autostart-enabled=false"));
    }

    #[test]
    fn write_with_delay_and_icon() {
        let entry = StartupEntry::new("test", "Test", "/bin/test")
            .with_delay(3)
            .with_icon("my-icon")
            .with_comment("A comment");
        let output = write_desktop_file(&entry);
        assert!(output.contains("X-GNOME-Autostart-Delay=3"));
        assert!(output.contains("Icon=my-icon"));
        assert!(output.contains("Comment=A comment"));
    }

    #[test]
    fn write_with_show_in_lists() {
        let entry = StartupEntry::new("test", "Test", "/bin/test")
            .with_only_show_in(vec!["GNOME".into(), "KDE".into()]);
        let output = write_desktop_file(&entry);
        assert!(output.contains("OnlyShowIn=GNOME;KDE;"));
    }

    #[test]
    fn roundtrip_parse_write() {
        let original = StartupEntry::new("myapp", "My App", "/usr/bin/myapp")
            .with_comment("A great app")
            .with_icon("myapp-icon")
            .with_delay(10)
            .with_not_show_in(vec!["XFCE".into()]);

        let desktop = write_desktop_file(&original);
        let parsed = parse_desktop_file(&desktop).unwrap();

        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.command, original.command);
        assert_eq!(parsed.comment, original.comment);
        assert_eq!(parsed.icon, original.icon);
        assert_eq!(parsed.delay_seconds, original.delay_seconds);
        assert_eq!(parsed.not_show_in, original.not_show_in);
        assert_eq!(parsed.enabled, original.enabled);
    }
}
