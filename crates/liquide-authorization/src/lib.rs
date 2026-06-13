#![doc = "Privilege escalation and authorization framework for the Liquide desktop."]
#![doc = ""]
#![doc = "Provides a privilege-escalation authorization model where privileged"]
#![doc = "actions are defined with required authentication levels, matched"]
#![doc = "against configurable policy rules, and authorized through"]
#![doc = "platform-specific credential verification."]

pub mod action;
pub mod agent;
pub mod audit;
pub mod auth_agent;
pub mod builtin;
pub mod level;
pub mod platform;
pub mod policy;
pub mod policy_db;
pub mod rules;
pub mod security_descriptor;
pub mod store;
pub mod subject;

pub use action::AuthorizationAction;
pub use agent::{AuthResult, AuthorizationAgent};
pub use audit::{AuditEntry, AuditLog, AuditPolicy};
pub use auth_agent::{AuthAgent, AuthAgentError, AuthIdentity, AuthPrompt, AuthSession};
pub use builtin::builtin_actions;
pub use level::AuthLevel;
pub use policy::{AuthorizationPolicy, PolicyRule};
pub use policy_db::{ActionId, AuthDecision, AuthType, ImpliedAuth, PolicyDatabase, PolicyEntry};
pub use rules::{Rule, RuleSet, SubjectMatch};
pub use security_descriptor::{
    AccessCheckResult, AccessControlEntry, AceEffect, CapabilitySet, Principal, SecurityDescriptor,
};
pub use store::AuthorizationStore;
pub use subject::{Resource, Subject, SubjectKind};

use thiserror::Error;

/// Errors produced by the authorization subsystem.
#[derive(Debug, Error)]
pub enum AuthorizationError {
    /// The requested action is not registered.
    #[error("unknown action: {0}")]
    UnknownAction(String),

    /// No policy rule matched the requested action.
    #[error("no policy rule matched action: {0}")]
    NoPolicyMatch(String),

    /// The platform credential verification failed.
    #[error("credential verification failed: {0}")]
    CredentialVerification(String),

    /// The authorization was denied by policy.
    #[error("authorization denied: {0}")]
    Denied(String),

    /// An I/O or subprocess error occurred during credential verification.
    #[error("platform error: {0}")]
    PlatformError(String),

    /// An internal error.
    #[error("internal authorization error: {0}")]
    Internal(String),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, AuthorizationError>;
