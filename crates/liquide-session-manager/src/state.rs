//! Session state tracking: snapshots of open windows, named sessions, save/restore.

use std::collections::HashMap;
use std::fmt;

/// Overall session state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// Session is initializing (services starting, autostart launching).
    Starting,
    /// Normal operation.
    Running,
    /// Screen lock requested, transitioning to locked.
    Locking,
    /// Screen is locked.
    Locked,
    /// Shutdown sequence in progress.
    ShuttingDown,
    /// Logout sequence in progress.
    LoggingOut,
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Locking => write!(f, "locking"),
            Self::Locked => write!(f, "locked"),
            Self::ShuttingDown => write!(f, "shutting-down"),
            Self::LoggingOut => write!(f, "logging-out"),
        }
    }
}

/// A window captured in a session snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionWindow {
    /// Application identifier (e.g. desktop file id or executable name).
    pub app_id: String,
    /// Window title at the time of capture.
    pub title: String,
    /// Geometry: (x, y, width, height).
    pub geometry: (i32, i32, u32, u32),
    /// Workspace index the window was on.
    pub workspace: u32,
    /// Whether the window was maximized.
    pub is_maximized: bool,
    /// Whether the window was minimized.
    pub is_minimized: bool,
}

/// A complete snapshot of a session, suitable for save/restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    /// Name of this saved session (e.g. "default", "work", "gaming").
    pub name: String,
    /// Unix-epoch milliseconds when the snapshot was taken.
    pub timestamp_ms: u64,
    /// Windows that were open.
    pub windows: Vec<SessionWindow>,
    /// Which workspace was active.
    pub active_workspace: u32,
    /// App id of the window that had keyboard focus, if any.
    pub focused_window: Option<String>,
}

impl SessionSnapshot {
    /// Create an empty snapshot with the given name and timestamp.
    pub fn new(name: impl Into<String>, timestamp_ms: u64) -> Self {
        Self {
            name: name.into(),
            timestamp_ms,
            windows: Vec::new(),
            active_workspace: 0,
            focused_window: None,
        }
    }
}

/// Errors from session state operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// The serialized data is malformed.
    DeserializationFailed(String),
    /// A named session was not found.
    SessionNotFound(String),
    /// Cannot perform the operation in the current state.
    InvalidStateTransition {
        from: SessionState,
        to: SessionState,
    },
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeserializationFailed(msg) => write!(f, "deserialization failed: {}", msg),
            Self::SessionNotFound(name) => write!(f, "session not found: {}", name),
            Self::InvalidStateTransition { from, to } => {
                write!(f, "invalid state transition: {} -> {}", from, to)
            }
        }
    }
}

impl std::error::Error for SessionError {}

/// Serialize a snapshot to a simple text format.
///
/// Format:
/// ```text
/// SESSION:<name>
/// TIMESTAMP:<ms>
/// WORKSPACE:<n>
/// FOCUSED:<app_id or empty>
/// WINDOW:<app_id>\t<title>\t<x>,<y>,<w>,<h>\t<workspace>\t<maximized>\t<minimized>
/// ...
/// ```
pub fn serialize_snapshot(snap: &SessionSnapshot) -> String {
    let mut out = String::with_capacity(256);
    out.push_str("SESSION:");
    out.push_str(&escape_field(&snap.name));
    out.push('\n');
    out.push_str("TIMESTAMP:");
    out.push_str(&snap.timestamp_ms.to_string());
    out.push('\n');
    out.push_str("WORKSPACE:");
    out.push_str(&snap.active_workspace.to_string());
    out.push('\n');
    out.push_str("FOCUSED:");
    if let Some(ref focused) = snap.focused_window {
        out.push_str(&escape_field(focused));
    }
    out.push('\n');
    for win in &snap.windows {
        out.push_str("WINDOW:");
        out.push_str(&escape_field(&win.app_id));
        out.push('\t');
        out.push_str(&escape_field(&win.title));
        out.push('\t');
        let (x, y, w, h) = win.geometry;
        out.push_str(&format!("{},{},{},{}", x, y, w, h));
        out.push('\t');
        out.push_str(&win.workspace.to_string());
        out.push('\t');
        out.push_str(if win.is_maximized { "1" } else { "0" });
        out.push('\t');
        out.push_str(if win.is_minimized { "1" } else { "0" });
        out.push('\n');
    }
    out
}

/// Deserialize a snapshot from the text format produced by [`serialize_snapshot`].
pub fn deserialize_snapshot(s: &str) -> Result<SessionSnapshot, SessionError> {
    let err = |msg: &str| SessionError::DeserializationFailed(msg.to_string());

    let mut name: Option<String> = None;
    let mut timestamp_ms: u64 = 0;
    let mut active_workspace: u32 = 0;
    let mut focused_window: Option<String> = None;
    let mut windows = Vec::new();

    for line in s.lines() {
        if line.is_empty() {
            continue;
        }
        if let Some(val) = line.strip_prefix("SESSION:") {
            name = Some(unescape_field(val));
        } else if let Some(val) = line.strip_prefix("TIMESTAMP:") {
            timestamp_ms = val.parse::<u64>().map_err(|_| err("invalid timestamp"))?;
        } else if let Some(val) = line.strip_prefix("WORKSPACE:") {
            active_workspace = val.parse::<u32>().map_err(|_| err("invalid workspace"))?;
        } else if let Some(val) = line.strip_prefix("FOCUSED:") {
            let v = unescape_field(val);
            focused_window = if v.is_empty() { None } else { Some(v) };
        } else if let Some(val) = line.strip_prefix("WINDOW:") {
            let parts: Vec<&str> = val.split('\t').collect();
            if parts.len() < 6 {
                return Err(err("window line has too few fields"));
            }
            let geom_parts: Vec<&str> = parts[2].split(',').collect();
            if geom_parts.len() != 4 {
                return Err(err("invalid geometry"));
            }
            let x = geom_parts[0]
                .parse::<i32>()
                .map_err(|_| err("invalid geometry x"))?;
            let y = geom_parts[1]
                .parse::<i32>()
                .map_err(|_| err("invalid geometry y"))?;
            let w = geom_parts[2]
                .parse::<u32>()
                .map_err(|_| err("invalid geometry w"))?;
            let h = geom_parts[3]
                .parse::<u32>()
                .map_err(|_| err("invalid geometry h"))?;
            let workspace = parts[3]
                .parse::<u32>()
                .map_err(|_| err("invalid window workspace"))?;
            let is_maximized = parts[4] == "1";
            let is_minimized = parts[5] == "1";
            windows.push(SessionWindow {
                app_id: unescape_field(parts[0]),
                title: unescape_field(parts[1]),
                geometry: (x, y, w, h),
                workspace,
                is_maximized,
                is_minimized,
            });
        }
        // Unknown lines are silently ignored for forward compatibility.
    }

    let session_name = name.ok_or_else(|| err("missing SESSION: header"))?;

    Ok(SessionSnapshot {
        name: session_name,
        timestamp_ms,
        windows,
        active_workspace,
        focused_window,
    })
}

/// Simple escaping: replace `\` with `\\`, `\t` with `\T`, `\n` with `\N`.
fn escape_field(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\t', "\\T")
        .replace('\n', "\\N")
}

/// Reverse of [`escape_field`].
fn unescape_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('T') => out.push('\t'),
                Some('N') => out.push('\n'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Manages multiple named sessions and tracks the current session state.
pub struct SessionStore {
    /// Current state of the session.
    pub state: SessionState,
    /// Named session snapshots, keyed by name.
    sessions: HashMap<String, SessionSnapshot>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            state: SessionState::Starting,
            sessions: HashMap::new(),
        }
    }

    /// Transition to a new state. Returns an error if the transition is not allowed.
    pub fn transition(&mut self, to: SessionState) -> Result<(), SessionError> {
        let allowed = match (self.state, to) {
            (SessionState::Starting, SessionState::Running) => true,
            (SessionState::Running, SessionState::Locking) => true,
            (SessionState::Running, SessionState::ShuttingDown) => true,
            (SessionState::Running, SessionState::LoggingOut) => true,
            (SessionState::Locking, SessionState::Locked) => true,
            (SessionState::Locked, SessionState::Running) => true,
            (SessionState::LoggingOut, SessionState::ShuttingDown) => true,
            // Same state is always allowed (no-op).
            (a, b) if a == b => true,
            _ => false,
        };
        if !allowed {
            return Err(SessionError::InvalidStateTransition {
                from: self.state,
                to,
            });
        }
        self.state = to;
        Ok(())
    }

    /// Save a session snapshot under its name.
    pub fn save_session(&mut self, snapshot: SessionSnapshot) {
        self.sessions.insert(snapshot.name.clone(), snapshot);
    }

    /// Load a session snapshot by name.
    pub fn load_session(&self, name: &str) -> Result<&SessionSnapshot, SessionError> {
        self.sessions
            .get(name)
            .ok_or_else(|| SessionError::SessionNotFound(name.to_string()))
    }

    /// Delete a named session.
    pub fn delete_session(&mut self, name: &str) -> Result<SessionSnapshot, SessionError> {
        self.sessions
            .remove(name)
            .ok_or_else(|| SessionError::SessionNotFound(name.to_string()))
    }

    /// List all saved session names.
    pub fn session_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.sessions.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Number of saved sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}
