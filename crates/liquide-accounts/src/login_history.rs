//! Login history tracking.

use std::fmt;

/// How the user authenticated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoginMethod {
    /// Standard password authentication.
    Password,
    /// Fingerprint / biometric reader.
    Fingerprint,
    /// Smart-card / PIV / PKCS#11 token.
    SmartCard,
    /// Automatic login (no credentials required).
    AutoLogin,
    /// Remote desktop session (RDP, VNC, LiquiDE protocol, etc.).
    RemoteDesktop,
}

impl fmt::Display for LoginMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Password => write!(f, "Password"),
            Self::Fingerprint => write!(f, "Fingerprint"),
            Self::SmartCard => write!(f, "Smart Card"),
            Self::AutoLogin => write!(f, "Auto Login"),
            Self::RemoteDesktop => write!(f, "Remote Desktop"),
        }
    }
}

/// A single login attempt record.
#[derive(Debug, Clone)]
pub struct LoginEntry {
    /// UID of the user who attempted to log in.
    pub uid: u32,
    /// Unix timestamp of the attempt.
    pub timestamp: u64,
    /// Whether the login succeeded.
    pub success: bool,
    /// The authentication method used.
    pub method: LoginMethod,
    /// Remote IP address, if applicable.
    pub ip: Option<String>,
}

/// In-memory login history store.
///
/// On real systems the platform backend populates this from
/// `/var/log/wtmp` (Linux), the Windows Security Event Log, or
/// macOS's `last` command. The `LoginHistory` struct also supports
/// recording new entries programmatically.
#[derive(Debug, Clone)]
pub struct LoginHistory {
    entries: Vec<LoginEntry>,
}

impl LoginHistory {
    /// Create an empty login history.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Create a login history pre-populated with entries.
    pub fn with_entries(entries: Vec<LoginEntry>) -> Self {
        Self { entries }
    }

    /// Record a new login attempt.
    pub fn record(&mut self, entry: LoginEntry) {
        self.entries.push(entry);
    }

    /// Return the `count` most recent login entries for a given user,
    /// ordered newest-first.
    pub fn recent_logins(&self, uid: u32, count: usize) -> Vec<LoginEntry> {
        let mut user_entries: Vec<&LoginEntry> =
            self.entries.iter().filter(|e| e.uid == uid).collect();
        // Sort descending by timestamp.
        user_entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        user_entries
            .into_iter()
            .take(count)
            .cloned()
            .collect()
    }

    /// Return all entries (newest-first).
    pub fn all_entries(&self) -> Vec<LoginEntry> {
        let mut sorted = self.entries.clone();
        sorted.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        sorted
    }

    /// Total number of recorded entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Count failed login attempts for a user since a given timestamp.
    pub fn failed_attempts_since(&self, uid: u32, since: u64) -> usize {
        self.entries
            .iter()
            .filter(|e| e.uid == uid && !e.success && e.timestamp >= since)
            .count()
    }
}

impl Default for LoginHistory {
    fn default() -> Self {
        Self::new()
    }
}
