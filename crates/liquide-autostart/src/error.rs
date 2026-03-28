/// Errors that can occur when managing autostart entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutostartError {
    /// The entry with the given id was not found.
    NotFound(String),
    /// Cannot remove a system entry — only disable it.
    SystemEntryCannotBeRemoved(String),
    /// An entry with this id already exists.
    DuplicateEntry(String),
    /// The entry has an invalid or empty command.
    InvalidCommand(String),
    /// The entry has an invalid or empty id.
    InvalidId,
}

impl std::fmt::Display for AutostartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AutostartError::NotFound(id) => write!(f, "autostart entry not found: {id}"),
            AutostartError::SystemEntryCannotBeRemoved(id) => {
                write!(f, "cannot remove system entry '{id}' — disable it instead")
            }
            AutostartError::DuplicateEntry(id) => {
                write!(f, "autostart entry already exists: {id}")
            }
            AutostartError::InvalidCommand(msg) => write!(f, "invalid command: {msg}"),
            AutostartError::InvalidId => write!(f, "entry id must not be empty"),
        }
    }
}

impl std::error::Error for AutostartError {}

/// Errors that can occur when parsing a .desktop file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The [Desktop Entry] section header is missing.
    MissingDesktopEntrySection,
    /// A required key is missing.
    MissingKey(String),
    /// A value could not be parsed.
    InvalidValue { key: String, reason: String },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::MissingDesktopEntrySection => {
                write!(f, "missing [Desktop Entry] section")
            }
            ParseError::MissingKey(key) => write!(f, "missing required key: {key}"),
            ParseError::InvalidValue { key, reason } => {
                write!(f, "invalid value for '{key}': {reason}")
            }
        }
    }
}

impl std::error::Error for ParseError {}
