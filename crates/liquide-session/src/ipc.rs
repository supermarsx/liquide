//! IPC types for supervisor-session communication.

use std::sync::mpsc;

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

/// The supervisor-side handle returned by [`IpcChannel::create`].
///
/// Holds the receiving end for session events and the sending end for
/// supervisor commands.
pub struct SupervisorHandle {
    event_rx: mpsc::Receiver<SessionEvent>,
    command_tx: mpsc::Sender<SupervisorCommand>,
}

impl SupervisorHandle {
    /// Send a command to the session.
    pub fn send_command(&self, cmd: SupervisorCommand) -> crate::Result<()> {
        self.command_tx
            .send(cmd)
            .map_err(|e| crate::SessionError::Internal(format!("command send failed: {e}")))
    }

    /// Try to receive the next event from the session without blocking.
    pub fn try_recv_event(&self) -> crate::Result<Option<SessionEvent>> {
        match self.event_rx.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(crate::SessionError::Internal(
                "event channel disconnected".into(),
            )),
        }
    }

    /// Block until the next event arrives from the session.
    pub fn recv_event(&self) -> crate::Result<SessionEvent> {
        self.event_rx
            .recv()
            .map_err(|e| crate::SessionError::Internal(format!("event recv failed: {e}")))
    }
}

/// IPC channel between the supervisor and session processes.
pub struct IpcChannel {
    socket_path: String,
    event_tx: mpsc::Sender<SessionEvent>,
    command_rx: mpsc::Receiver<SupervisorCommand>,
}

impl IpcChannel {
    /// Create a new IPC channel with pre-existing channel endpoints.
    #[must_use]
    pub fn new(
        socket_path: String,
        event_tx: mpsc::Sender<SessionEvent>,
        command_rx: mpsc::Receiver<SupervisorCommand>,
    ) -> Self {
        Self {
            socket_path,
            event_tx,
            command_rx,
        }
    }

    /// Create a linked pair of `(IpcChannel, SupervisorHandle)`.
    ///
    /// The `IpcChannel` is used by the session side to send events and
    /// receive commands.  The [`SupervisorHandle`] is used by the
    /// supervisor side to send commands and receive events.
    #[must_use]
    pub fn create(socket_path: String) -> (Self, SupervisorHandle) {
        let (event_tx, event_rx) = mpsc::channel();
        let (command_tx, command_rx) = mpsc::channel();

        let channel = Self {
            socket_path,
            event_tx,
            command_rx,
        };
        let handle = SupervisorHandle {
            event_rx,
            command_tx,
        };
        (channel, handle)
    }

    /// The path to the IPC socket.
    #[must_use]
    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }

    /// Send an event to the supervisor.
    pub fn send_event(&self, event: &SessionEvent) -> crate::Result<()> {
        self.event_tx
            .send(event.clone())
            .map_err(|e| crate::SessionError::Internal(format!("event send failed: {e}")))
    }

    /// Try to receive a command from the supervisor without blocking.
    pub fn receive_command(&self) -> crate::Result<Option<SupervisorCommand>> {
        match self.command_rx.try_recv() {
            Ok(cmd) => Ok(Some(cmd)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(crate::SessionError::Internal(
                "command channel disconnected".into(),
            )),
        }
    }
}
