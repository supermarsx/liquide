//! Session tracking, state machine, and registry.

use std::collections::HashMap;
use std::time::Instant;

/// State of a managed session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session process is being spawned.
    Spawning,
    /// Session is actively running.
    Running,
    /// Session is locked (user idle or policy lock).
    Locked,
    /// Client disconnected but session is alive.
    Disconnected,
    /// Session is suspended (low resource mode).
    Suspended,
    /// Session has crashed and is being evaluated for restart.
    Crashed,
    /// Session has exhausted restart attempts.
    Failed,
    /// Session has been terminated.
    Terminated,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawning => write!(f, "Spawning"),
            Self::Running => write!(f, "Running"),
            Self::Locked => write!(f, "Locked"),
            Self::Disconnected => write!(f, "Disconnected"),
            Self::Suspended => write!(f, "Suspended"),
            Self::Crashed => write!(f, "Crashed"),
            Self::Failed => write!(f, "Failed"),
            Self::Terminated => write!(f, "Terminated"),
        }
    }
}

/// Resource budget allocated to a session.
#[derive(Debug, Clone)]
pub struct ResourceBudget {
    /// Allocated CPU cores.
    pub cpu_cores: f64,
    /// Allocated memory in megabytes.
    pub memory_mb: u64,
    /// Maximum number of PIDs.
    pub max_pids: u32,
    /// I/O bandwidth in megabytes per second.
    pub io_mbps: u32,
    /// Network bandwidth in megabits per second.
    pub net_mbps: u32,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            cpu_cores: 2.0,
            memory_mb: 512,
            max_pids: 256,
            io_mbps: 10,
            net_mbps: 20,
        }
    }
}

/// Record of a crash event.
#[derive(Debug, Clone)]
pub struct CrashRecord {
    /// Unique identifier for this crash.
    pub crash_id: String,
    /// When the crash occurred.
    pub timestamp: Instant,
    /// Signal that caused the crash, if any.
    pub signal: Option<i32>,
    /// Exit code of the crashed process, if available.
    pub exit_code: Option<i32>,
    /// Path to the core dump file, if generated.
    pub coredump_path: Option<String>,
    /// Captured log lines from the crashed session.
    pub log_lines: Vec<String>,
}

/// A tracked session record.
#[derive(Debug)]
pub struct SessionRecord {
    /// Unique session identifier.
    pub session_id: String,
    /// User who owns this session.
    pub user: String,
    /// Process ID of the session process.
    pub pid: u32,
    /// Current session state.
    pub state: SessionState,
    /// When the session was started.
    pub started_at: Instant,
    /// Resource budget allocated to this session.
    pub resource_budget: ResourceBudget,
    /// Number of restart attempts.
    pub restart_count: u32,
    /// Timestamp of the last heartbeat received.
    pub last_heartbeat: Instant,
    /// History of crash events for this session.
    pub crash_history: Vec<CrashRecord>,
}

impl SessionRecord {
    /// Create a new session record.
    #[must_use]
    pub fn new(
        session_id: String,
        user: String,
        pid: u32,
        resource_budget: ResourceBudget,
    ) -> Self {
        let now = Instant::now();
        Self {
            session_id,
            user,
            pid,
            state: SessionState::Spawning,
            started_at: now,
            resource_budget,
            restart_count: 0,
            last_heartbeat: now,
            crash_history: Vec::new(),
        }
    }

    /// Seconds since the session was started.
    #[must_use]
    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}

/// Registry of active sessions.
pub struct SessionRegistry {
    sessions: HashMap<String, SessionRecord>,
}

impl SessionRegistry {
    /// Create an empty session registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Register a new session.
    pub fn register_session(&mut self, record: SessionRecord) {
        self.sessions.insert(record.session_id.clone(), record);
    }

    /// Remove a session by ID.
    pub fn remove_session(&mut self, session_id: &str) -> Option<SessionRecord> {
        self.sessions.remove(session_id)
    }

    /// Get an immutable reference to a session.
    #[must_use]
    pub fn get_session(&self, session_id: &str) -> Option<&SessionRecord> {
        self.sessions.get(session_id)
    }

    /// Get a mutable reference to a session.
    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut SessionRecord> {
        self.sessions.get_mut(session_id)
    }

    /// Count of sessions in an active state (not Terminated or Failed).
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|s| !matches!(s.state, SessionState::Terminated | SessionState::Failed))
            .count()
    }

    /// Get all session IDs for a given user.
    #[must_use]
    pub fn sessions_for_user(&self, user: &str) -> Vec<String> {
        self.sessions
            .values()
            .filter(|s| s.user == user)
            .map(|s| s.session_id.clone())
            .collect()
    }

    /// Get all session records.
    #[must_use]
    pub fn all_sessions(&self) -> &HashMap<String, SessionRecord> {
        &self.sessions
    }

    /// Total number of sessions (including terminated).
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
