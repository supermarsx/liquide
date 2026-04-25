use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{InteropError, Result};

/// Type of a .desktop entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesktopEntryType {
    Application,
    Link,
    Directory,
}

impl fmt::Display for DesktopEntryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application => write!(f, "Application"),
            Self::Link => write!(f, "Link"),
            Self::Directory => write!(f, "Directory"),
        }
    }
}

/// An action within a desktop entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopAction {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
}

impl DesktopAction {
    #[must_use]
    pub fn new(name: &str, exec: &str) -> Self {
        Self {
            name: name.to_string(),
            exec: exec.to_string(),
            icon: None,
        }
    }
}

/// A parsed .desktop file (freedesktop specification).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopEntry {
    pub entry_type: DesktopEntryType,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub icon: Option<String>,
    pub exec: Option<String>,
    pub try_exec: Option<String>,
    pub path: Option<String>,
    pub terminal: bool,
    pub categories: Vec<String>,
    pub mime_types: Vec<String>,
    pub keywords: Vec<String>,
    pub no_display: bool,
    pub hidden: bool,
    pub startup_notify: bool,
    pub actions: Vec<DesktopAction>,
}

impl DesktopEntry {
    /// Parse a .desktop file from its text content.
    pub fn parse(content: &str) -> Result<Self> {
        let mut entry_type = None;
        let mut name = None;
        let mut generic_name = None;
        let mut comment = None;
        let mut icon = None;
        let mut exec = None;
        let mut try_exec = None;
        let mut path = None;
        let mut terminal = false;
        let mut categories = Vec::new();
        let mut mime_types = Vec::new();
        let mut keywords = Vec::new();
        let mut no_display = false;
        let mut hidden = false;
        let mut startup_notify = false;
        let mut actions = Vec::new();
        let mut in_desktop_entry = false;
        let mut action_sections: Vec<(String, Vec<(String, String)>)> = Vec::new();
        let mut current_action: Option<(String, Vec<(String, String)>)> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line == "[Desktop Entry]" {
                if let Some(act) = current_action.take() {
                    action_sections.push(act);
                }
                in_desktop_entry = true;
                continue;
            }

            if line.starts_with("[Desktop Action ") && line.ends_with(']') {
                if let Some(act) = current_action.take() {
                    action_sections.push(act);
                }
                in_desktop_entry = false;
                let act_name = &line["[Desktop Action ".len()..line.len() - 1];
                current_action = Some((act_name.to_string(), Vec::new()));
                continue;
            }

            if line.starts_with('[') {
                if let Some(act) = current_action.take() {
                    action_sections.push(act);
                }
                in_desktop_entry = false;
                continue;
            }

            if let Some((_, ref mut pairs)) = current_action {
                if let Some((k, v)) = line.split_once('=') {
                    pairs.push((k.trim().to_string(), v.trim().to_string()));
                }
                continue;
            }

            if !in_desktop_entry {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "Type" => {
                        entry_type = Some(match value {
                            "Application" => DesktopEntryType::Application,
                            "Link" => DesktopEntryType::Link,
                            "Directory" => DesktopEntryType::Directory,
                            other => {
                                return Err(InteropError::InvalidDesktopEntry(format!(
                                    "unknown type: {other}"
                                )));
                            }
                        });
                    }
                    "Name" => name = Some(value.to_string()),
                    "GenericName" => generic_name = Some(value.to_string()),
                    "Comment" => comment = Some(value.to_string()),
                    "Icon" => icon = Some(value.to_string()),
                    "Exec" => exec = Some(value.to_string()),
                    "TryExec" => try_exec = Some(value.to_string()),
                    "Path" => path = Some(value.to_string()),
                    "Terminal" => terminal = value == "true",
                    "NoDisplay" => no_display = value == "true",
                    "Hidden" => hidden = value == "true",
                    "StartupNotify" => startup_notify = value == "true",
                    "Categories" => {
                        categories = value
                            .split(';')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect();
                    }
                    "MimeType" => {
                        mime_types = value
                            .split(';')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect();
                    }
                    "Keywords" => {
                        keywords = value
                            .split(';')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect();
                    }
                    _ => {}
                }
            }
        }

        if let Some(act) = current_action.take() {
            action_sections.push(act);
        }

        for (act_name, pairs) in action_sections {
            let mut a_exec = String::new();
            let mut a_icon = None;
            let mut a_label = act_name.clone();
            for (k, v) in pairs {
                match k.as_str() {
                    "Name" => a_label = v,
                    "Exec" => a_exec = v,
                    "Icon" => a_icon = Some(v),
                    _ => {}
                }
            }
            let mut action = DesktopAction::new(&a_label, &a_exec);
            action.icon = a_icon;
            actions.push(action);
        }

        let name = name
            .ok_or_else(|| InteropError::InvalidDesktopEntry("missing Name field".to_string()))?;

        Ok(Self {
            entry_type: entry_type.unwrap_or(DesktopEntryType::Application),
            name,
            generic_name,
            comment,
            icon,
            exec,
            try_exec,
            path,
            terminal,
            categories,
            mime_types,
            keywords,
            no_display,
            hidden,
            startup_notify,
            actions,
        })
    }

    /// Check if this entry handles the given MIME type.
    #[must_use]
    pub fn matches_mime(&self, mime: &str) -> bool {
        self.mime_types.iter().any(|m| m == mime)
    }

    /// Check if this entry belongs to the given category.
    #[must_use]
    pub fn matches_category(&self, category: &str) -> bool {
        self.categories.iter().any(|c| c == category)
    }

    /// Serialize this entry back to .desktop format.
    #[must_use]
    pub fn to_desktop_string(&self) -> String {
        let mut out = String::from("[Desktop Entry]\n");
        out.push_str(&format!("Type={}\n", self.entry_type));
        out.push_str(&format!("Name={}\n", self.name));
        if let Some(ref gn) = self.generic_name {
            out.push_str(&format!("GenericName={gn}\n"));
        }
        if let Some(ref c) = self.comment {
            out.push_str(&format!("Comment={c}\n"));
        }
        if let Some(ref i) = self.icon {
            out.push_str(&format!("Icon={i}\n"));
        }
        if let Some(ref e) = self.exec {
            out.push_str(&format!("Exec={e}\n"));
        }
        if let Some(ref te) = self.try_exec {
            out.push_str(&format!("TryExec={te}\n"));
        }
        if let Some(ref p) = self.path {
            out.push_str(&format!("Path={p}\n"));
        }
        if self.terminal {
            out.push_str("Terminal=true\n");
        }
        if self.no_display {
            out.push_str("NoDisplay=true\n");
        }
        if self.hidden {
            out.push_str("Hidden=true\n");
        }
        if self.startup_notify {
            out.push_str("StartupNotify=true\n");
        }
        if !self.categories.is_empty() {
            out.push_str(&format!("Categories={}\n", self.categories.join(";")));
        }
        if !self.mime_types.is_empty() {
            out.push_str(&format!("MimeType={}\n", self.mime_types.join(";")));
        }
        if !self.keywords.is_empty() {
            out.push_str(&format!("Keywords={}\n", self.keywords.join(";")));
        }
        out
    }
}

impl fmt::Display for DesktopEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DesktopEntry({}, type={}, exec={:?})",
            self.name, self.entry_type, self.exec
        )
    }
}
