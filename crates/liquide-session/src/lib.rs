//! Session management for the LiquiDE remote desktop environment.
//!
//! Provides per-user session lifecycle, state machine, worker supervision,
//! heartbeat monitoring, crash recovery, session resume, sandboxing, IPC,
//! and audit trail.

pub mod audit;
pub mod config;
pub mod crash;
pub mod desktop;
pub mod heartbeat;
pub mod ipc;
pub mod resume;
pub mod runtime;
pub mod sandbox;
pub mod state;
pub mod telemetry;
pub mod worker;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the session subsystem.
#[derive(Debug, Error)]
pub enum SessionError {
    /// An invalid state transition was attempted.
    #[error("invalid state transition from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },

    /// Heartbeat timeout threshold exceeded.
    #[error("heartbeat timeout: {missed_count} consecutive heartbeats missed")]
    HeartbeatTimeout { missed_count: u32 },

    /// A managed worker process failed.
    #[error("worker failed: {worker}: {reason}")]
    WorkerFailed { worker: String, reason: String },

    /// A resource limit was exceeded.
    #[error("resource limit exceeded: {resource} (limit={limit}, actual={actual})")]
    ResourceLimitExceeded {
        resource: String,
        limit: String,
        actual: String,
    },

    /// Maximum restart attempts exceeded.
    #[error("restart limit exceeded: max {max_restarts} restarts")]
    RestartLimitExceeded { max_restarts: u32 },

    /// A plugin was quarantined due to repeated failures.
    #[error("plugin quarantined: {plugin_id}")]
    PluginQuarantined { plugin_id: String },

    /// A sandbox violation was detected.
    #[error("sandbox violation: {detail}")]
    SandboxViolation { detail: String },

    /// The resume token has expired.
    #[error("resume token expired")]
    ResumeTokenExpired,

    /// The resume token is invalid.
    #[error("resume token invalid")]
    ResumeTokenInvalid,

    /// A configuration error.
    #[error("configuration error: {detail}")]
    ConfigError { detail: String },

    /// An internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for session operations.
pub type Result<T> = std::result::Result<T, SessionError>;

// Re-exports
pub use audit::{AuditLevel, SessionAuditEvent};
pub use config::{
    JailConfig, JailNetwork, MultiClientConfig, MultiClientMode, ResourceLimits, ResumeConfig,
    SessionConfig, SupervisorConfig,
};
pub use crash::{
    CrashInfo, CrashMetadata, DisabledFeature, ResourceSnapshot, RestartAction, RestartTracker,
    SafeMode,
};
pub use heartbeat::{HeartbeatConfig, HeartbeatMonitor, HeartbeatState, HeartbeatStatus};
pub use ipc::{IpcChannel, SessionEvent, SupervisorCommand};
pub use resume::{PersistenceState, ResumeManager, ResumeToken, SessionPersistence};
pub use runtime::SessionRuntime;
pub use sandbox::{JailType, NamespaceConfig, SandboxEnforcer};
pub use state::{SessionState, SessionStateMachine};
pub use worker::{WorkerHandle, WorkerKind, WorkerManager, WorkerStatus};
