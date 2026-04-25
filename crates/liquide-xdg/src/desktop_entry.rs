//! Desktop Entry (.desktop) file parser and writer.
//!
//! Implements parsing and serialization of `.desktop` files following the
//! freedesktop.org Desktop Entry Specification v1.5.

use std::fmt;

/// A parsed `.desktop` file entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesktopEntry {
    /// The name of the application (required).
    pub name: String,
    /// A generic name (e.g. "Web Browser").
    pub generic_name: Option<String>,
    /// A tooltip / description.
    pub comment: Option<String>,
    /// The command to execute (`Exec` key).
    pub exec: Option<String>,
    /// Icon name or path.
    pub icon: Option<String>,
    /// The entry type: Application, Link, or Directory.
    pub type_: EntryType,
    /// Semicolon-separated categories.
    pub categories: Vec<String>,
    /// MIME types the application can handle.
    pub mime_types: Vec<String>,
    /// Whether the application should run in a terminal.
    pub terminal: bool,
    /// If `true`, the entry should not be shown in menus.
    pub no_display: bool,
    /// If `true`, the entry has been deleted by the user.
    pub hidden: bool,
    /// Working directory for the application.
    pub path: Option<String>,
    /// Startup notification ID.
    pub startup_wm_class: Option<String>,
    /// Application actions (desktop actions).
    pub actions: Vec<String>,
    /// Raw extra keys not explicitly modeled, stored as (key, value).
    pub extra: Vec<(String, String)>,
}

/// The `Type` field of a desktop entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EntryType {
    #[default]
    Application,
    Link,
    Directory,
    /// An unrecognised type string.
    Other(String),
}

impl EntryType {
    fn as_str(&self) -> &str {
        match self {
            EntryType::Application => "Application",
            EntryType::Link => "Link",
            EntryType::Directory => "Directory",
            EntryType::Other(s) => s.as_str(),
        }
    }

    fn from_str(s: &str) -> Self {
        match s.trim() {
            "Application" => EntryType::Application,
            "Link" => EntryType::Link,
            "Directory" => EntryType::Directory,
            other => EntryType::Other(other.to_string()),
        }
    }
}

/// Errors that can occur when parsing a `.desktop` file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// The required `[Desktop Entry]` section header is missing.
    MissingSectionHeader,
    /// The required `Name` key is missing.
    MissingName,
    /// A line could not be parsed.
    InvalidLine(usize, String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::MissingSectionHeader => {
                write!(f, "missing [Desktop Entry] section header")
            }
            ParseError::MissingName => write!(f, "missing required 'Name' key"),
            ParseError::InvalidLine(n, line) => {
                write!(f, "invalid line {n}: {line}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl DesktopEntry {
    /// Parse a `.desktop` file from its text content.
    pub fn parse(content: &str) -> Result<Self, ParseError> {
        let mut entry = DesktopEntry::default();
        let mut in_desktop_section = false;
        let mut found_header = false;
        let mut has_name = false;

        for (line_no, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim();

            // Skip empty lines and comments.
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Section headers.
            if line.starts_with('[') && line.ends_with(']') {
                let section = &line[1..line.len() - 1];
                in_desktop_section = section == "Desktop Entry";
                if in_desktop_section {
                    found_header = true;
                }
                continue;
            }

            if !in_desktop_section {
                continue;
            }

            // Key=Value pairs.
            let Some((key, value)) = line.split_once('=') else {
                return Err(ParseError::InvalidLine(line_no + 1, line.to_string()));
            };
            let key = key.trim();
            let value = value.trim();

            match key {
                "Name" => {
                    entry.name = value.to_string();
                    has_name = true;
                }
                "GenericName" => entry.generic_name = Some(value.to_string()),
                "Comment" => entry.comment = Some(value.to_string()),
                "Exec" => entry.exec = Some(value.to_string()),
                "Icon" => entry.icon = Some(value.to_string()),
                "Type" => entry.type_ = EntryType::from_str(value),
                "Categories" => {
                    entry.categories = split_semicolon_list(value);
                }
                "MimeType" => {
                    entry.mime_types = split_semicolon_list(value);
                }
                "Terminal" => entry.terminal = value == "true",
                "NoDisplay" => entry.no_display = value == "true",
                "Hidden" => entry.hidden = value == "true",
                "Path" => entry.path = Some(value.to_string()),
                "StartupWMClass" => entry.startup_wm_class = Some(value.to_string()),
                "Actions" => {
                    entry.actions = split_semicolon_list(value);
                }
                _ => {
                    entry.extra.push((key.to_string(), value.to_string()));
                }
            }
        }

        if !found_header {
            return Err(ParseError::MissingSectionHeader);
        }
        if !has_name {
            return Err(ParseError::MissingName);
        }

        Ok(entry)
    }

    /// Serialize the entry back to `.desktop` file format.
    pub fn to_desktop_string(&self) -> String {
        let mut out = String::with_capacity(512);
        out.push_str("[Desktop Entry]\n");
        push_kv(&mut out, "Name", &self.name);
        push_kv(&mut out, "Type", self.type_.as_str());

        if let Some(ref v) = self.generic_name {
            push_kv(&mut out, "GenericName", v);
        }
        if let Some(ref v) = self.comment {
            push_kv(&mut out, "Comment", v);
        }
        if let Some(ref v) = self.exec {
            push_kv(&mut out, "Exec", v);
        }
        if let Some(ref v) = self.icon {
            push_kv(&mut out, "Icon", v);
        }
        if !self.categories.is_empty() {
            push_kv(
                &mut out,
                "Categories",
                &join_semicolon_list(&self.categories),
            );
        }
        if !self.mime_types.is_empty() {
            push_kv(&mut out, "MimeType", &join_semicolon_list(&self.mime_types));
        }
        if self.terminal {
            push_kv(&mut out, "Terminal", "true");
        }
        if self.no_display {
            push_kv(&mut out, "NoDisplay", "true");
        }
        if self.hidden {
            push_kv(&mut out, "Hidden", "true");
        }
        if let Some(ref v) = self.path {
            push_kv(&mut out, "Path", v);
        }
        if let Some(ref v) = self.startup_wm_class {
            push_kv(&mut out, "StartupWMClass", v);
        }
        if !self.actions.is_empty() {
            push_kv(&mut out, "Actions", &join_semicolon_list(&self.actions));
        }
        for (k, v) in &self.extra {
            push_kv(&mut out, k, v);
        }

        out
    }
}

impl fmt::Display for DesktopEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_desktop_string())
    }
}

/// Split a semicolon-delimited list, trimming whitespace and dropping empties.
fn split_semicolon_list(s: &str) -> Vec<String> {
    s.split(';')
        .map(|seg| seg.trim().to_string())
        .filter(|seg| !seg.is_empty())
        .collect()
}

/// Join a list into a semicolon-delimited string (with trailing semicolon).
fn join_semicolon_list(items: &[String]) -> String {
    let mut s = items.join(";");
    if !s.is_empty() {
        s.push(';');
    }
    s
}

fn push_kv(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push('=');
    out.push_str(value);
    out.push('\n');
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
[Desktop Entry]
Name=Firefox Web Browser
GenericName=Web Browser
Comment=Browse the World Wide Web
Exec=firefox %u
Icon=firefox
Type=Application
Categories=Network;WebBrowser;
MimeType=text/html;application/xhtml+xml;
Terminal=false
StartupWMClass=firefox
";

    #[test]
    fn parse_sample_name() {
        let entry = DesktopEntry::parse(SAMPLE).unwrap();
        assert_eq!(entry.name, "Firefox Web Browser");
    }

    #[test]
    fn parse_sample_generic_name() {
        let entry = DesktopEntry::parse(SAMPLE).unwrap();
        assert_eq!(entry.generic_name.as_deref(), Some("Web Browser"));
    }

    #[test]
    fn parse_sample_exec() {
        let entry = DesktopEntry::parse(SAMPLE).unwrap();
        assert_eq!(entry.exec.as_deref(), Some("firefox %u"));
    }

    #[test]
    fn parse_sample_categories() {
        let entry = DesktopEntry::parse(SAMPLE).unwrap();
        assert_eq!(entry.categories, vec!["Network", "WebBrowser"]);
    }

    #[test]
    fn parse_sample_mime_types() {
        let entry = DesktopEntry::parse(SAMPLE).unwrap();
        assert_eq!(entry.mime_types, vec!["text/html", "application/xhtml+xml"]);
    }

    #[test]
    fn parse_sample_type() {
        let entry = DesktopEntry::parse(SAMPLE).unwrap();
        assert_eq!(entry.type_, EntryType::Application);
    }

    #[test]
    fn parse_terminal_false() {
        let entry = DesktopEntry::parse(SAMPLE).unwrap();
        assert!(!entry.terminal);
    }

    #[test]
    fn parse_terminal_true() {
        let content = "[Desktop Entry]\nName=Htop\nType=Application\nExec=htop\nTerminal=true\n";
        let entry = DesktopEntry::parse(content).unwrap();
        assert!(entry.terminal);
    }

    #[test]
    fn parse_hidden_and_no_display() {
        let content =
            "[Desktop Entry]\nName=Secret\nHidden=true\nNoDisplay=true\nType=Application\n";
        let entry = DesktopEntry::parse(content).unwrap();
        assert!(entry.hidden);
        assert!(entry.no_display);
    }

    #[test]
    fn parse_missing_header() {
        let content = "Name=Bad\nExec=bad\n";
        assert_eq!(
            DesktopEntry::parse(content),
            Err(ParseError::MissingSectionHeader)
        );
    }

    #[test]
    fn parse_missing_name() {
        let content = "[Desktop Entry]\nExec=something\n";
        assert_eq!(DesktopEntry::parse(content), Err(ParseError::MissingName));
    }

    #[test]
    fn parse_comments_and_blanks() {
        let content = "\
# This is a comment
[Desktop Entry]

Name=Test App
# Another comment
Exec=testapp
Type=Application
";
        let entry = DesktopEntry::parse(content).unwrap();
        assert_eq!(entry.name, "Test App");
    }

    #[test]
    fn parse_link_type() {
        let content = "[Desktop Entry]\nName=Link\nType=Link\nURL=https://example.com\n";
        let entry = DesktopEntry::parse(content).unwrap();
        assert_eq!(entry.type_, EntryType::Link);
    }

    #[test]
    fn parse_directory_type() {
        let content = "[Desktop Entry]\nName=MyDir\nType=Directory\nIcon=folder\n";
        let entry = DesktopEntry::parse(content).unwrap();
        assert_eq!(entry.type_, EntryType::Directory);
    }

    #[test]
    fn parse_unknown_type() {
        let content = "[Desktop Entry]\nName=Custom\nType=Service\n";
        let entry = DesktopEntry::parse(content).unwrap();
        assert_eq!(entry.type_, EntryType::Other("Service".into()));
    }

    #[test]
    fn parse_extra_keys_preserved() {
        let content = "[Desktop Entry]\nName=App\nX-Custom-Key=hello\n";
        let entry = DesktopEntry::parse(content).unwrap();
        assert_eq!(entry.extra, vec![("X-Custom-Key".into(), "hello".into())]);
    }

    #[test]
    fn parse_actions() {
        let content = "[Desktop Entry]\nName=App\nActions=New;Open;\n";
        let entry = DesktopEntry::parse(content).unwrap();
        assert_eq!(entry.actions, vec!["New", "Open"]);
    }

    #[test]
    fn parse_path_key() {
        let content = "[Desktop Entry]\nName=App\nPath=/usr/share/myapp\n";
        let entry = DesktopEntry::parse(content).unwrap();
        assert_eq!(entry.path.as_deref(), Some("/usr/share/myapp"));
    }

    #[test]
    fn roundtrip_serialize_parse() {
        let entry = DesktopEntry::parse(SAMPLE).unwrap();
        let serialized = entry.to_desktop_string();
        let reparsed = DesktopEntry::parse(&serialized).unwrap();
        assert_eq!(entry.name, reparsed.name);
        assert_eq!(entry.exec, reparsed.exec);
        assert_eq!(entry.categories, reparsed.categories);
        assert_eq!(entry.mime_types, reparsed.mime_types);
        assert_eq!(entry.type_, reparsed.type_);
    }

    #[test]
    fn serialize_minimal() {
        let entry = DesktopEntry {
            name: "Min".into(),
            ..Default::default()
        };
        let s = entry.to_desktop_string();
        assert!(s.contains("[Desktop Entry]\n"));
        assert!(s.contains("Name=Min\n"));
        assert!(s.contains("Type=Application\n"));
    }

    #[test]
    fn serialize_booleans_only_when_true() {
        let entry = DesktopEntry {
            name: "X".into(),
            terminal: false,
            no_display: false,
            hidden: false,
            ..Default::default()
        };
        let s = entry.to_desktop_string();
        assert!(!s.contains("Terminal="));
        assert!(!s.contains("NoDisplay="));
        assert!(!s.contains("Hidden="));
    }

    #[test]
    fn display_impl() {
        let entry = DesktopEntry {
            name: "D".into(),
            ..Default::default()
        };
        let s = format!("{entry}");
        assert!(s.starts_with("[Desktop Entry]"));
    }

    #[test]
    fn parse_error_display() {
        assert_eq!(
            ParseError::MissingSectionHeader.to_string(),
            "missing [Desktop Entry] section header"
        );
        assert_eq!(
            ParseError::MissingName.to_string(),
            "missing required 'Name' key"
        );
        assert_eq!(
            ParseError::InvalidLine(5, "bad".into()).to_string(),
            "invalid line 5: bad"
        );
    }

    #[test]
    fn parse_ignores_other_sections() {
        let content = "\
[Desktop Entry]
Name=App
Exec=app

[Desktop Action New]
Name=New Window
Exec=app --new
";
        let entry = DesktopEntry::parse(content).unwrap();
        assert_eq!(entry.name, "App");
        // Keys from [Desktop Action New] are not captured.
        assert!(entry.extra.is_empty());
    }

    #[test]
    fn parse_startup_wm_class() {
        let entry = DesktopEntry::parse(SAMPLE).unwrap();
        assert_eq!(entry.startup_wm_class.as_deref(), Some("firefox"));
    }
}
