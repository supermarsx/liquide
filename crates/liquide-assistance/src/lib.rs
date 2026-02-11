//! Remote assistance framework — session shadowing, consent flows,
//! observer management, stealth monitoring, and audit trail.

pub mod mode;
pub mod config;
pub mod message;
pub mod session;
pub mod observer;
pub mod consent;
pub mod input;
pub mod cursor;
pub mod chat;
pub mod invite;
pub mod stealth;
pub mod audit;
pub mod coordinator;
pub mod policy;

#[cfg(test)]
mod tests;

pub use mode::{AssistanceMode, ModeCapabilities, Restriction};
pub use config::{AssistanceConfig, ModeConfig, StealthConfig, PermissionsConfig, RecordingConfig};
pub use message::{
    AssistanceRequest, ConsentPromptMsg, ConsentResponseMsg,
    AssistanceGranted, AssistanceDenied, DenialReason,
    AssistanceInviteMsg, InviteCreatedMsg, JoinWithCode,
    EscalationRequest, EscalationPromptMsg, EscalationResponse, EscalationGranted,
    EndReason, AssistanceEnd, ChatMsg, AnnotationAdd, OwnerReclaimControl,
};
pub use session::{ShadowSessionState, ShadowSession, SessionInfo};
pub use observer::{ObserverRole, Observer};
pub use consent::{ConsentState, ConsentFlow};
pub use input::{InputSource, InputEventType, InputEvent, InputCoordinator};
pub use cursor::{CursorAppearance, GhostCursor, cursor_appearance_for_mode};
pub use chat::{ChatMessage, ChatChannel};
pub use invite::{InviteCode, InviteRegistry};
pub use stealth::StealthSession;
pub use audit::{AuditLevel, AssistanceAuditEvent};
pub use coordinator::AssistanceCoordinator;
pub use policy::AssistancePolicy;

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
