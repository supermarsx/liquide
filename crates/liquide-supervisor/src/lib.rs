//! Supervisor daemon for the LiquiDE remote desktop environment.
//!
//! Provides session lifecycle management, user authentication, policy
//! enforcement, admission control, heartbeat monitoring, crash handling,
//! restart policies, auto-downgrade under host pressure, resource
//! monitoring, IPC control plane, and audit trail.

pub mod admission;
pub mod audit;
pub mod config;
pub mod crash;
pub mod downgrade;
pub mod heartbeat;
pub mod ipc;
pub mod resource;
pub mod restart;
pub mod runtime;
pub mod session;
pub mod spawn;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the supervisor subsystem.
#[derive(Debug, Error)]
pub enum SupervisorError {
    /// The requested session was not found.
    #[error("session not found: {session_id}")]
    SessionNotFound { session_id: String },

    /// A session spawn was rejected by admission control.
    #[error("admission rejected: {reason}")]
    AdmissionRejected { reason: String },

    /// User authentication failed.
    #[error("authentication failed for {user}: {reason}")]
    AuthenticationFailed { user: String, reason: String },

    /// A session state transition was invalid.
    #[error("invalid session state transition: {from} -> {to}")]
    InvalidStateTransition { from: String, to: String },

    /// The session process failed to spawn.
    #[error("spawn failed for session {session_id}: {reason}")]
    SpawnFailed { session_id: String, reason: String },

    /// A heartbeat timeout was detected.
    #[error("heartbeat timeout for session {session_id}: missed {missed_count} heartbeats")]
    HeartbeatTimeout {
        session_id: String,
        missed_count: u32,
    },

    /// Maximum restart attempts exceeded.
    #[error("restart limit exceeded for session {session_id}: max {max_restarts}")]
    RestartLimitExceeded {
        session_id: String,
        max_restarts: u32,
    },

    /// A policy violation was detected.
    #[error("policy violation: {detail}")]
    PolicyViolation { detail: String },

    /// A configuration error.
    #[error("configuration error: {detail}")]
    ConfigError { detail: String },

    /// IPC communication error.
    #[error("IPC error: {detail}")]
    IpcError { detail: String },

    /// A resource limit was exceeded.
    #[error("resource limit exceeded: {resource} (limit={limit}, actual={actual})")]
    ResourceLimitExceeded {
        resource: String,
        limit: String,
        actual: String,
    },

    /// An internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for supervisor operations.
pub type Result<T> = std::result::Result<T, SupervisorError>;

// Re-exports
pub use admission::{AdmissionController, AdmissionDecision, HostResources};
pub use audit::{AuditLevel, SupervisorAuditEvent};
pub use config::{
    AdmissionConfig, AuthBackend, DowngradeThresholds, ResourceDefaults, SupervisorConfig,
};
pub use crash::{CrashCategory, CrashHandler, CrashReport, CrashResourceSnapshot};
pub use downgrade::{DowngradeAction, DowngradeLevel, DowngradeManager};
pub use heartbeat::{
    HeartbeatAlert, HeartbeatConfig, HeartbeatEntry, HeartbeatState, HeartbeatTracker,
};
pub use ipc::{
    ControlChannel, ControlCommand, ControlResponse, SessionDetail, SessionSummary,
    SupervisorStatus,
};
pub use resource::{
    HostMetrics, ResourceMonitor, ResourceSeverity, ResourceSnapshot, ResourceType,
    ResourceWarning,
};
pub use restart::{RestartDecision, RestartPolicy};
pub use runtime::SupervisorRuntime;
pub use session::{CrashRecord, ResourceBudget, SessionRecord, SessionRegistry, SessionState};
pub use spawn::{SessionSpawner, SpawnRequest, SpawnResult};
