//! IPC control plane for supervisor management.

use crate::session::SessionState;

/// Commands accepted on the supervisor control channel.
#[derive(Debug, Clone)]
pub enum ControlCommand {
    /// List all active sessions.
    ListSessions,
    /// Get detailed information about a specific session.
    GetSessionInfo {
        /// Session to query.
        session_id: String,
    },
    /// Spawn a new session for a user.
    SpawnSession {
        /// User to spawn a session for.
        user: String,
    },
    /// Terminate a session.
    TerminateSession {
        /// Session to terminate.
        session_id: String,
    },
    /// Restart a crashed or failed session.
    RestartSession {
        /// Session to restart.
        session_id: String,
    },
    /// Lock a session.
    LockSession {
        /// Session to lock.
        session_id: String,
    },
    /// Unlock a session.
    UnlockSession {
        /// Session to unlock.
        session_id: String,
    },
    /// Suspend a session.
    SuspendSession {
        /// Session to suspend.
        session_id: String,
    },
    /// Resume a suspended session.
    ResumeSession {
        /// Session to resume.
        session_id: String,
    },
    /// Update the active policy.
    SetPolicy {
        /// Policy configuration as a string (e.g., serialized TOML).
        policy: String,
    },
    /// Get the overall supervisor status.
    GetStatus,
    /// Shut down the supervisor.
    Shutdown,
}

impl std::fmt::Display for ControlCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ListSessions => write!(f, "ListSessions"),
            Self::GetSessionInfo { session_id } => {
                write!(f, "GetSessionInfo({})", session_id)
            }
            Self::SpawnSession { user } => write!(f, "SpawnSession({})", user),
            Self::TerminateSession { session_id } => {
                write!(f, "TerminateSession({})", session_id)
            }
            Self::RestartSession { session_id } => {
                write!(f, "RestartSession({})", session_id)
            }
            Self::LockSession { session_id } => write!(f, "LockSession({})", session_id),
            Self::UnlockSession { session_id } => {
                write!(f, "UnlockSession({})", session_id)
            }
            Self::SuspendSession { session_id } => {
                write!(f, "SuspendSession({})", session_id)
            }
            Self::ResumeSession { session_id } => {
                write!(f, "ResumeSession({})", session_id)
            }
            Self::SetPolicy { .. } => write!(f, "SetPolicy"),
            Self::GetStatus => write!(f, "GetStatus"),
            Self::Shutdown => write!(f, "Shutdown"),
        }
    }
}

/// Summary of a session for list commands.
#[derive(Debug, Clone)]
pub struct SessionSummary {
    /// Session identifier.
    pub session_id: String,
    /// User who owns the session.
    pub user: String,
    /// Current session state.
    pub state: SessionState,
    /// Uptime in seconds.
    pub uptime_sec: u64,
}

/// Detailed information about a session.
#[derive(Debug, Clone)]
pub struct SessionDetail {
    /// Session identifier.
    pub session_id: String,
    /// User who owns the session.
    pub user: String,
    /// Current session state.
    pub state: SessionState,
    /// Uptime in seconds.
    pub uptime_sec: u64,
    /// Process ID.
    pub pid: u32,
    /// CPU cores allocated.
    pub cpu_cores: f64,
    /// Memory allocated in megabytes.
    pub memory_mb: u64,
    /// Number of restart attempts.
    pub restart_count: u32,
    /// Number of crashes recorded.
    pub crash_count: usize,
}

/// Snapshot of the supervisor's overall status.
#[derive(Debug, Clone)]
pub struct SupervisorStatus {
    /// Uptime of the supervisor in seconds.
    pub uptime_sec: u64,
    /// Number of active sessions.
    pub active_sessions: usize,
    /// Host CPU usage percentage.
    pub host_cpu_pct: f64,
    /// Host memory usage percentage.
    pub host_memory_pct: f64,
}

/// Responses from the supervisor control plane.
#[derive(Debug, Clone)]
pub enum ControlResponse {
    /// Generic success.
    Ok,
    /// List of sessions.
    SessionList(Vec<SessionSummary>),
    /// Detailed session information.
    SessionInfo(SessionDetail),
    /// An error occurred.
    Error(String),
    /// Supervisor status.
    Status(SupervisorStatus),
}

/// The control channel for IPC communication.
pub struct ControlChannel {
    /// Path to the Unix domain socket.
    socket_path: String,
}

impl ControlChannel {
    /// Create a new control channel.
    #[must_use]
    pub fn new(socket_path: String) -> Self {
        Self { socket_path }
    }

    /// The socket path.
    #[must_use]
    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }
}
