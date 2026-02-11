//! Management web UI backend for administering LiquiDE deployments.
//!
//! Provides a REST API and real-time WebSocket interface for monitoring
//! sessions, managing servers, editing policies, and viewing audit logs.

pub mod api;
pub mod audit;
pub mod config;
pub mod dashboard;
pub mod gateway_mgmt;
pub mod metrics;
pub mod policy_mgmt;
pub mod runtime;
pub mod server_mgmt;
pub mod session_mgmt;
pub mod user_mgmt;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the management backend.
#[derive(Debug, Error)]
pub enum ManagerError {
    /// The requested server was not found.
    #[error("server not found: {name}")]
    ServerNotFound { name: String },

    /// The requested session was not found.
    #[error("session not found: {session_id}")]
    SessionNotFound { session_id: String },

    /// The requested user was not found.
    #[error("user not found: {username}")]
    UserNotFound { username: String },

    /// Authentication failed.
    #[error("authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    /// The caller lacks permission for the requested action.
    #[error("access denied: {action} requires role {required_role}")]
    AccessDenied {
        action: String,
        required_role: String,
    },

    /// A policy operation failed.
    #[error("policy error: {0}")]
    PolicyError(String),

    /// Rate limit exceeded for the caller.
    #[error("rate limit exceeded for {ip}")]
    RateLimitExceeded { ip: String },

    /// The configuration is invalid.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// A gateway operation failed.
    #[error("gateway error: {name}: {reason}")]
    GatewayError { name: String, reason: String },

    /// An internal error occurred.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, ManagerError>;

// Re-exports for convenience.
pub use config::ManagerConfig;
pub use runtime::ManagerRuntime;
