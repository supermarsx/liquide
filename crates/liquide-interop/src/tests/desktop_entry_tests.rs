use crate::desktop_entry::{DesktopEntry, DesktopEntryType};

const BASIC_DESKTOP: &str = "\
[Desktop Entry]
Type=Application
Name=Firefox
GenericName=Web Browser
Comment=Browse the Web
Icon=firefox
Exec=firefox %u
TryExec=firefox
Terminal=false
Categories=Network;WebBrowser;
MimeType=text/html;application/xhtml+xml;
Keywords=browser;web;internet;
StartupNotify=true
";

#[test]
fn test_parse_basic() {
    let entry = DesktopEntry::parse(BASIC_DESKTOP).unwrap();
    assert_eq!(entry.entry_type, DesktopEntryType::Application);
    assert_eq!(entry.name, "Firefox");
    assert_eq!(entry.generic_name.as_deref(), Some("Web Browser"));
    assert_eq!(entry.comment.as_deref(), Some("Browse the Web"));
    assert_eq!(entry.icon.as_deref(), Some("firefox"));
    assert_eq!(entry.exec.as_deref(), Some("firefox %u"));
    assert!(entry.startup_notify);
}

#[test]
fn test_parse_with_actions() {
    let content = "\
[Desktop Entry]
Type=Application
Name=Files
Exec=nautilus
Icon=org.gnome.Nautilus

[Desktop Action new-window]
Name=New Window
Exec=nautilus --new-window

[Desktop Action connect]
Name=Connect to Server
Exec=nautilus --connect
Icon=network
";
    let entry = DesktopEntry::parse(content).unwrap();
    assert_eq!(entry.actions.len(), 2);
    assert_eq!(entry.actions[0].name, "New Window");
    assert_eq!(entry.actions[0].exec, "nautilus --new-window");
    assert!(entry.actions[0].icon.is_none());
    assert_eq!(entry.actions[1].name, "Connect to Server");
    assert_eq!(entry.actions[1].icon.as_deref(), Some("network"));
}

#[test]
fn test_parse_link_type() {
    let content = "\
[Desktop Entry]
Type=Link
Name=Google
URL=https://www.google.com
Icon=web-browser
";
    let entry = DesktopEntry::parse(content).unwrap();
    assert_eq!(entry.entry_type, DesktopEntryType::Link);
    assert_eq!(entry.name, "Google");
}

#[test]
fn test_categories() {
    let entry = DesktopEntry::parse(BASIC_DESKTOP).unwrap();
    assert_eq!(entry.categories, vec!["Network", "WebBrowser"]);
    assert!(entry.matches_category("Network"));
    assert!(!entry.matches_category("Office"));
}

#[test]
fn test_mime_matching() {
    let entry = DesktopEntry::parse(BASIC_DESKTOP).unwrap();
    assert!(entry.matches_mime("text/html"));
    assert!(entry.matches_mime("application/xhtml+xml"));
    assert!(!entry.matches_mime("image/png"));
}

#[test]
fn test_no_display() {
    let content = "\
[Desktop Entry]
Type=Application
Name=Hidden App
NoDisplay=true
";
    let entry = DesktopEntry::parse(content).unwrap();
    assert!(entry.no_display);
    assert!(!entry.hidden);
}

#[test]
fn test_hidden() {
    let content = "\
[Desktop Entry]
Type=Application
Name=Hidden App
Hidden=true
";
    let entry = DesktopEntry::parse(content).unwrap();
    assert!(entry.hidden);
}

#[test]
fn test_exec_path() {
    let content = "\
[Desktop Entry]
Type=Application
Name=MyApp
Exec=/usr/bin/myapp
Path=/usr/share/myapp
Terminal=true
";
    let entry = DesktopEntry::parse(content).unwrap();
    assert_eq!(entry.exec.as_deref(), Some("/usr/bin/myapp"));
    assert_eq!(entry.path.as_deref(), Some("/usr/share/myapp"));
    assert!(entry.terminal);
}

#[test]
fn test_to_desktop_string_roundtrip() {
    let entry = DesktopEntry::parse(BASIC_DESKTOP).unwrap();
    let output = entry.to_desktop_string();
    assert!(output.contains("Name=Firefox"));
    assert!(output.contains("Type=Application"));
    assert!(output.contains("Exec=firefox %u"));
}

#[test]
fn test_display() {
    let entry = DesktopEntry::parse(BASIC_DESKTOP).unwrap();
    let s = format!("{entry}");
    assert!(s.contains("Firefox"));
    assert!(s.contains("Application"));
}
