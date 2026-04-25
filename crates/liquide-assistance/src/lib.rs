//! Remote assistance framework — session shadowing, consent flows,
//! observer management, stealth monitoring, and audit trail.

pub mod audit;
pub mod chat;
pub mod config;
pub mod consent;
pub mod coordinator;
pub mod cursor;
pub mod input;
pub mod invite;
pub mod message;
pub mod mode;
pub mod observer;
pub mod policy;
pub mod session;
pub mod stealth;

#[cfg(test)]
mod tests;

pub use audit::{AssistanceAuditEvent, AuditLevel};
pub use chat::{ChatChannel, ChatMessage};
pub use config::{AssistanceConfig, ModeConfig, PermissionsConfig, RecordingConfig, StealthConfig};
pub use consent::{ConsentFlow, ConsentState};
pub use coordinator::AssistanceCoordinator;
pub use cursor::{CursorAppearance, GhostCursor, cursor_appearance_for_mode};
pub use input::{InputCoordinator, InputEvent, InputEventType, InputSource};
pub use invite::{InviteCode, InviteRegistry};
pub use message::{
    AnnotationAdd, AssistanceDenied, AssistanceEnd, AssistanceGranted, AssistanceInviteMsg,
    AssistanceRequest, ChatMsg, ConsentPromptMsg, ConsentResponseMsg, DenialReason, EndReason,
    EscalationGranted, EscalationPromptMsg, EscalationRequest, EscalationResponse,
    InviteCreatedMsg, JoinWithCode, OwnerReclaimControl,
};
pub use mode::{AssistanceMode, ModeCapabilities, Restriction};
pub use observer::{Observer, ObserverRole};
pub use policy::AssistancePolicy;
pub use session::{SessionInfo, ShadowSession, ShadowSessionState};
pub use stealth::StealthSession;

use thiserror::Error;

/// Errors produced by the assistance framework.
#[derive(Debug, Error)]
pub enum AssistanceError {
    #[error("assistance is disabled")]
    Disabled,
    #[error("mode not allowed: {mode}")]
    ModeNotAllowed { mode: String },
    #[error("maximum observers reached: {limit}")]
    MaxObserversReached { limit: u32 },
    #[error("consent denied")]
    ConsentDenied,
    #[error("consent timed out after {timeout_secs}s")]
    ConsentTimeout { timeout_secs: u64 },
    #[error("invalid invite code")]
    InvalidInviteCode,
    #[error("invite expired")]
    InviteExpired,
    #[error("stealth mode not enabled")]
    StealthNotEnabled,
    #[error("stealth requires role: {required_role}")]
    StealthRoleRequired { required_role: String },
    #[error("session not found: {session_id}")]
    SessionNotFound { session_id: String },
    #[error("observer not found: {observer_id}")]
    ObserverNotFound { observer_id: String },
    #[error("already in exclusive mode")]
    AlreadyExclusive,
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for assistance operations.
pub type Result<T> = std::result::Result<T, AssistanceError>;
