//! Worker process management for session subsystems.

use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

use crate::audit::SessionAuditEvent;

/// Kinds of worker processes managed by the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkerKind {
    /// The Wayland/X11 compositor.
    Compositor,
    /// The software or GPU renderer.
    Renderer,
    /// The video encoder pipeline.
    Encoder,
    /// The network transport layer.
    Transport,
    /// The audio subsystem.
    Audio,
    /// The input routing subsystem.
    Input,
    /// The clipboard bridge.
    Clipboard,
    /// USB device redirection.
    Usb,
    /// A plugin worker.
    Plugin,
    /// Session recording.
    Recording,
    /// Accessibility bridge.
    Accessibility,
}

impl fmt::Display for WorkerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compositor => write!(f, "Compositor"),
            Self::Renderer => write!(f, "Renderer"),
            Self::Encoder => write!(f, "Encoder"),
            Self::Transport => write!(f, "Transport"),
            Self::Audio => write!(f, "Audio"),
            Self::Input => write!(f, "Input"),
            Self::Clipboard => write!(f, "Clipboard"),
            Self::Usb => write!(f, "Usb"),
            Self::Plugin => write!(f, "Plugin"),
            Self::Recording => write!(f, "Recording"),
            Self::Accessibility => write!(f, "Accessibility"),
        }
    }
}

/// Status of a worker process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStatus {
    /// The worker is starting up.
    Starting,
    /// The worker is running normally.
    Running,
    /// The worker is paused.
    Paused,
    /// The worker has been stopped.
    Stopped,
    /// The worker has failed.
    Failed,
}

impl fmt::Display for WorkerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Starting => write!(f, "Starting"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Stopped => write!(f, "Stopped"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

/// Handle to a running worker.
pub struct WorkerHandle {
    kind: WorkerKind,
    status: WorkerStatus,
    started_at: Instant,
}

impl WorkerHandle {
    /// Create a new worker handle.
    #[must_use]
    pub fn new(kind: WorkerKind) -> Self {
        Self {
            kind,
            status: WorkerStatus::Starting,
            started_at: Instant::now(),
        }
    }

    /// The kind of worker.
    #[must_use]
    pub fn kind(&self) -> WorkerKind {
        self.kind
    }

    /// The current status.
    #[must_use]
    pub fn status(&self) -> WorkerStatus {
        self.status
    }

    /// When the worker was started.
    #[must_use]
    pub fn started_at(&self) -> Instant {
        self.started_at
    }

    /// Seconds since the worker was started.
    #[must_use]
    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}

/// Manages all workers within a session.
pub struct WorkerManager {
    workers: HashMap<WorkerKind, WorkerHandle>,
    audit_events: Vec<SessionAuditEvent>,
}

impl WorkerManager {
    /// Create a new worker manager with no workers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            workers: HashMap::new(),
            audit_events: Vec::new(),
        }
    }

    /// Start a worker of the given kind.
    ///
    /// If a worker of this kind is already running, it is replaced.
    pub fn start_worker(&mut self, kind: WorkerKind) {
        let mut handle = WorkerHandle::new(kind);
        handle.status = WorkerStatus::Running;
        self.workers.insert(kind, handle);
        self.audit_events.push(SessionAuditEvent::WorkerStarted {
            worker: kind.to_string(),
        });
    }

    /// Stop a worker of the given kind.
    pub fn stop_worker(&mut self, kind: WorkerKind) {
        if let Some(handle) = self.workers.get_mut(&kind) {
            handle.status = WorkerStatus::Stopped;
            self.audit_events.push(SessionAuditEvent::WorkerStopped {
                worker: kind.to_string(),
            });
        }
    }

    /// Pause a worker of the given kind.
    pub fn pause_worker(&mut self, kind: WorkerKind) {
        if let Some(handle) = self.workers.get_mut(&kind) {
            if handle.status == WorkerStatus::Running {
                handle.status = WorkerStatus::Paused;
            }
        }
    }

    /// Mark a worker as failed.
    pub fn fail_worker(&mut self, kind: WorkerKind, reason: &str) {
        if let Some(handle) = self.workers.get_mut(&kind) {
            handle.status = WorkerStatus::Failed;
            self.audit_events.push(SessionAuditEvent::WorkerFailed {
                worker: kind.to_string(),
                reason: reason.to_string(),
            });
        }
    }

    /// Get the status of a specific worker.
    #[must_use]
    pub fn worker_status(&self, kind: WorkerKind) -> Option<WorkerStatus> {
        self.workers.get(&kind).map(|h| h.status)
    }

    /// Whether all registered workers are in the Running state.
    #[must_use]
    pub fn all_running(&self) -> bool {
        !self.workers.is_empty()
            && self
                .workers
                .values()
                .all(|h| h.status == WorkerStatus::Running)
    }

    /// Count of workers currently in the Running state.
    #[must_use]
    pub fn running_count(&self) -> usize {
        self.workers
            .values()
            .filter(|h| h.status == WorkerStatus::Running)
            .count()
    }

    /// Access the worker map.
    #[must_use]
    pub fn workers(&self) -> &HashMap<WorkerKind, WorkerHandle> {
        &self.workers
    }

    /// Drain all pending audit events.
    pub fn drain_events(&mut self) -> Vec<SessionAuditEvent> {
        std::mem::take(&mut self.audit_events)
    }
}

impl Default for WorkerManager {
    fn default() -> Self {
        Self::new()
    }
}
