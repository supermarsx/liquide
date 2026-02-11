//! Built-in settings application for the LiquiDE desktop environment.
//!
//! This crate provides the logic for a graphical settings panel that lets
//! users configure display, input, audio, network, appearance, privacy,
//! and system options.
//!
//! # Modules
//!
//! - [`config`] — Application-level configuration (window size, defaults).
//! - [`category`] — Setting categories and their metadata.
//! - [`entry`] — Individual setting entries with typed values.
//! - [`page`] — Settings pages composed of sections and entries.
//! - [`search`] — Full-text search across all settings.
//! - [`apply`] — Change tracking, validation, and persistence.
//! - [`policy`] — Policy constraints on modifiable settings.
//! - [`notify`] — Change notifications for other system components.
//! - [`runtime`] — Top-level settings coordinator.

pub mod config;
pub mod category;
pub mod entry;
pub mod page;
pub mod search;
pub mod apply;
pub mod policy;
pub mod notify;
pub mod runtime;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the settings application.
#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("unknown category: {0}")]
    UnknownCategory(String),

    #[error("unknown setting: {key}")]
    UnknownSetting { key: String },

    #[error("invalid value for {key}: {reason}")]
    InvalidValue { key: String, reason: String },

    #[error("setting locked by policy: {key}")]
    LockedByPolicy { key: String },

    #[error("nothing to undo")]
    NothingToUndo,

    #[error("nothing to redo")]
    NothingToRedo,

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("I/O error: {0}")]
    Io(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, SettingsError>;

// Re-exports for convenience.
pub use config::SettingsConfig;
pub use runtime::SettingsRuntime;
