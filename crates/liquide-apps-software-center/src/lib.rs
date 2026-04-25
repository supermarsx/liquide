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

pub mod catalog;
pub mod config;
pub mod install;
pub mod package;
pub mod repository;
pub mod review;
pub mod runtime;
pub mod screenshot;
pub mod update;

#[cfg(test)]
mod tests;

use liquide_app_harness::{AppBootstrap, Size};
use liquide_ui_widgets::Label;
use thiserror::Error;
use tracing::info;

/// Reverse-DNS application identifier for the software center.
pub const APP_ID: &str = "com.liquide.apps.software-center";

/// Display name used for the default software center window.
pub const DISPLAY_NAME: &str = "Software Center";

/// Initial window size for the default GUI launch path.
pub const DEFAULT_WINDOW_SIZE: Size = Size::new(1180, 760);

/// Runtime summary produced by the default GUI launch path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareCenterLaunchState {
    pub repository_count: usize,
    pub package_count: usize,
    pub summary: String,
}

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

    #[error("transport error: {0}")]
    Transport(String),

    #[error("unsupported package backend: {0}")]
    UnsupportedBackend(String),

    #[error("backend command failed: {0}")]
    BackendCommand(String),

    #[error("invalid rating: {0}")]
    InvalidRating(u8),

    #[error("I/O error: {0}")]
    Io(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, SoftwareCenterError>;

// Re-exports for convenience.
pub use config::SoftwareCenterConfig;
pub use install::{CommandSpec, InstallAction, InstallQueue, PackageSource};
pub use runtime::SoftwareCenterRuntime;

/// Build the default application bootstrap used by the production binary.
#[must_use]
pub fn default_bootstrap() -> AppBootstrap {
    AppBootstrap::new(APP_ID, DISPLAY_NAME)
        .with_initial_size(DEFAULT_WINDOW_SIZE)
        .with_ime(false)
}

/// Build the runtime state surfaced by the default GUI launch path.
#[must_use]
pub fn default_launch_state(config: SoftwareCenterConfig) -> SoftwareCenterLaunchState {
    let runtime = SoftwareCenterRuntime::new(config);
    let repository_count = runtime.repos().count();
    let package_count = runtime.catalog().total_count();

    SoftwareCenterLaunchState {
        repository_count,
        package_count,
        summary: format!(
            "liquid-software-center — {repository_count} repositories, {package_count} packages"
        ),
    }
}

/// Build the default placeholder root widget.
#[must_use]
pub fn build_default_root(config: SoftwareCenterConfig) -> Label {
    let state = default_launch_state(config);
    build_root_from_state(&state)
}

/// Build the placeholder root widget from a previously computed launch state.
#[must_use]
pub fn build_root_from_state(state: &SoftwareCenterLaunchState) -> Label {
    Label::new(state.summary.clone())
}

/// Run the default software center GUI path.
pub fn run_default_app() -> anyhow::Result<()> {
    let config = SoftwareCenterConfig::default();
    let state = default_launch_state(config.clone());

    info!(
        auto_updates = config.auto_check_updates,
        repos = state.repository_count,
        "Starting liquid-software-center"
    );

    default_bootstrap().run(move |_cx| Box::new(build_root_from_state(&state)))
}

#[cfg(test)]
mod launch_tests {
    use super::*;

    #[test]
    fn default_launch_state_reports_repository_and_package_counts() {
        let state = default_launch_state(SoftwareCenterConfig::default());

        assert_eq!(state.repository_count, 3);
        assert_eq!(state.package_count, 0);
        assert_eq!(
            state.summary,
            "liquid-software-center — 3 repositories, 0 packages"
        );
    }
}
