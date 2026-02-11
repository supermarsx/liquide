//! Built-in software center for the LiquiDE desktop environment.
//!
//! This crate provides a graphical storefront for discovering, installing,
//! updating, and removing applications.
//!
//! # Modules
//!
//! - [`config`] — Software center configuration.
//! - [`package`] — Package metadata and versioning.
//! - [`repository`] — Repository sources and management.
//! - [`catalog`] — App catalog with categories, featured, and search.
//! - [`install`] — Installation, removal, and progress tracking.
//! - [`update`] — Update checking and batch updates.
//! - [`review`] — App reviews and ratings.
//! - [`screenshot`] — Screenshot gallery handling.
//! - [`runtime`] — Software center coordinator.

pub mod config;
pub mod package;
pub mod repository;
pub mod catalog;
pub mod install;
pub mod update;
pub mod review;
pub mod screenshot;
pub mod runtime;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the software center.
#[derive(Debug, Error)]
pub enum SoftwareCenterError {
    #[error("package not found: {0}")]
    PackageNotFound(String),

    #[error("repository not found: {0}")]
    RepositoryNotFound(String),

    #[error("already installed: {0}")]
    AlreadyInstalled(String),

    #[error("not installed: {0}")]
    NotInstalled(String),

    #[error("version conflict: {0}")]
    VersionConflict(String),

    #[error("download failed: {0}")]
    DownloadFailed(String),

    #[error("invalid rating: {0}")]
    InvalidRating(u8),

    #[error("I/O error: {0}")]
    Io(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, SoftwareCenterError>;

// Re-exports for convenience.
pub use config::SoftwareCenterConfig;
pub use runtime::SoftwareCenterRuntime;
