//! Management web UI frontend for LiquiDE deployments.
//!
//! Provides the application shell, navigation, view models, and API client
//! types for the browser-based management interface.

pub mod api_client;
pub mod auth;
pub mod component;
pub mod config;
pub mod nav;
pub mod page;
pub mod runtime;
pub mod theme;
pub mod view_model;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the management frontend.
#[derive(Debug, Error)]
pub enum FrontendError {
    /// No user is currently authenticated.
    #[error("not authenticated")]
    NotAuthenticated,

    /// The current user lacks the required permission level.
    #[error("insufficient permissions: requires {required}")]
    InsufficientPermissions { required: String },

    /// The requested page path does not exist.
    #[error("page not found: {path}")]
    PageNotFound { path: String },

    /// An API communication error.
    #[error("API error: {0}")]
    ApiError(String),

    /// An invalid theme was requested.
    #[error("invalid theme: {name}")]
    InvalidTheme { name: String },

    /// A navigation operation failed.
    #[error("navigation error: {0}")]
    NavigationError(String),

    /// A serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, FrontendError>;

// Re-exports for convenience.
pub use config::FrontendConfig;
pub use runtime::FrontendRuntime;
