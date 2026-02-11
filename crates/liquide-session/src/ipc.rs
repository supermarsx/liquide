//! IPC types for supervisor-session communication.

use crate::crash::CrashInfo;
use crate::state::SessionState;

/// Commands sent from the supervisor to the session.
#[derive(Debug, Clone)]
pub enum SupervisorCommand {
    /// Shut down the session gracefully.
    Shutdown,
    /// Lock the session.
    Lock,
    /// Unlock the session.
    Unlock,
    /// Suspend the session.
    Suspend,
    /// Resume the session.
    Resume,
    /// Update the session policy.
    UpdatePolicy,
    /// Force-terminate the session immediately.
    ForceTerminate,
    /// Trigger a session restart.
    RestartSession,
}

impl std::fmt::Display for SupervisorCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shutdown => write!(f, "Shutdown"),
            Self::Lock => write!(f, "Lock"),
            Self::Unlock => write!(f, "Unlock"),
            Self::Suspend => write!(f, "Suspend"),
            Self::Resume => write!(f, "Resume"),
            Self::UpdatePolicy => write!(f, "UpdatePolicy"),
            Self::ForceTerminate => write!(f, "ForceTerminate"),
            Self::RestartSession => write!(f, "RestartSession"),
        }
    }
}

/// Events sent from the session to the supervisor.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// The session state changed.
    StateChanged {
        /// Previous state.
        from: SessionState,
        /// New state.
        to: SessionState,
    },
    /// A heartbeat was sent.
    HeartbeatSent,
    /// A worker process failed.
    WorkerFailed {
        /// Which worker.
        worker: String,
        /// Failure reason.
        reason: String,
    },
    /// A crash was detected.
    CrashDetected {
        /// Crash information.
        info: CrashInfo,
    },
    /// A resource usage warning.
    ResourceWarning {
        /// The resource that triggered the warning.
        resource: String,
        /// Current usage as a percentage of the limit.
        usage_percent: f64,
    },
}

/// IPC channel between the supervisor and session processes.
pub struct IpcChannel {
    socket_path: String,
}

impl IpcChannel {
    /// Create a new IPC channel with the given socket path.
    #[must_use]
    pub fn new(socket_path: String) -> Self {
        Self { socket_path }
    }

    /// The path to the IPC socket.
    #[must_use]
    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }

    /// Send an event to the supervisor.
    ///
    /// In a real implementation this would serialize and write to the socket.
    pub fn send_event(&self, _event: &SessionEvent) -> crate::Result<()> {
        // Stub: would write to the Unix domain socket / Windows named pipe.
        Ok(())
    }

    /// Try to receive a command from the supervisor.
    ///
    /// In a real implementation this would read from the socket.
    pub fn receive_command(&self) -> crate::Result<Option<SupervisorCommand>> {
        // Stub: would read from the socket.
        Ok(None)
    }
}
