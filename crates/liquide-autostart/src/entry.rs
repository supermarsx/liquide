/// Where a startup entry originates from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntrySource {
    /// System-wide entry (e.g. /etc/xdg/autostart/).
    /// Cannot be deleted, only disabled.
    System,
    /// User-created entry (e.g. ~/.config/autostart/).
    User,
    /// Session-specific, temporary entry that does not persist across reboots.
    Session,
}

impl std::fmt::Display for EntrySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntrySource::System => write!(f, "system"),
            EntrySource::User => write!(f, "user"),
            EntrySource::Session => write!(f, "session"),
        }
    }
}

/// An autostart application entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupEntry {
    /// Unique identifier (filename stem or registry key).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Command line to execute.
    pub command: String,
    /// Optional description / comment.
    pub comment: Option<String>,
    /// Optional icon name or path.
    pub icon: Option<String>,
    /// Whether this entry is enabled.
    pub enabled: bool,
    /// Delay in seconds before launching (default 0).
    pub delay_seconds: u32,
    /// Desktop environments this entry should only appear in (freedesktop OnlyShowIn).
    pub only_show_in: Vec<String>,
    /// Desktop environments this entry should NOT appear in (freedesktop NotShowIn).
    pub not_show_in: Vec<String>,
    /// Origin of this entry.
    pub source: EntrySource,
}

impl StartupEntry {
    /// Create a new enabled user entry with the given id, name, and command.
    pub fn new(id: impl Into<String>, name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            command: command.into(),
            comment: None,
            icon: None,
            enabled: true,
            delay_seconds: 0,
            only_show_in: Vec::new(),
            not_show_in: Vec::new(),
            source: EntrySource::User,
        }
    }

    /// Builder: set the comment.
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Builder: set the icon.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Builder: set the delay.
    pub fn with_delay(mut self, seconds: u32) -> Self {
        self.delay_seconds = seconds;
        self
    }

    /// Builder: set the source.
    pub fn with_source(mut self, source: EntrySource) -> Self {
        self.source = source;
        self
    }

    /// Builder: set enabled state.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Builder: set only_show_in list.
    pub fn with_only_show_in(mut self, desktops: Vec<String>) -> Self {
        self.only_show_in = desktops;
        self
    }

    /// Builder: set not_show_in list.
    pub fn with_not_show_in(mut self, desktops: Vec<String>) -> Self {
        self.not_show_in = desktops;
        self
    }

    /// Whether this entry should be shown in the given desktop environment.
    /// If `only_show_in` is non-empty, the desktop must be in it.
    /// If `not_show_in` is non-empty, the desktop must NOT be in it.
    /// If both are empty, the entry is shown everywhere.
    pub fn should_show_in(&self, desktop: &str) -> bool {
        if !self.only_show_in.is_empty() {
            return self.only_show_in.iter().any(|d| d == desktop);
        }
        if !self.not_show_in.is_empty() {
            return !self.not_show_in.iter().any(|d| d == desktop);
        }
        true
    }

    /// Estimated startup time in milliseconds for this entry.
    /// This is a rough heuristic: delay * 1000 + 500ms per entry for process spawn overhead.
    pub fn estimated_startup_ms(&self) -> u32 {
        self.delay_seconds * 1000 + 500
    }
}
