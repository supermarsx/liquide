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

pub mod apply;
pub mod bridge;
pub mod category;
pub mod config;
pub mod entry;
pub mod notify;
pub mod page;
pub mod policy;
pub mod runtime;
pub mod search;
pub mod widget;

#[cfg(test)]
mod tests;

use anyhow::Result as AnyhowResult;
use liquide_app_harness::{AppBootstrap, Size};
use liquide_ui_core::widget::Widget;
use thiserror::Error;
use tracing::info;

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

impl From<std::io::Error> for SettingsError {
    fn from(e: std::io::Error) -> Self {
        SettingsError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for SettingsError {
    fn from(e: serde_json::Error) -> Self {
        SettingsError::Serialization(e.to_string())
    }
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, SettingsError>;

pub const SETTINGS_APP_ID: &str = "com.liquide.apps.settings";
pub const SETTINGS_DISPLAY_NAME: &str = "Settings";
pub const SETTINGS_INITIAL_SIZE: Size = Size::new(1100, 760);

/// Minimal runtime state that downstream launch tests can assert after setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsLaunchContract {
    pub category_count: usize,
    pub entry_count: usize,
    pub disk_load_error: Option<String>,
}

#[must_use]
pub fn app_bootstrap() -> AppBootstrap {
    AppBootstrap::new(SETTINGS_APP_ID, SETTINGS_DISPLAY_NAME)
        .with_initial_size(SETTINGS_INITIAL_SIZE)
        .with_ime(false)
}

#[must_use]
pub fn prepare_launch(config: SettingsConfig) -> SettingsLaunchContract {
    let mut runtime = SettingsRuntime::new(config);
    let disk_load_error = runtime
        .load_from_disk()
        .err()
        .map(|error| error.to_string());

    SettingsLaunchContract {
        category_count: runtime.category_infos().len(),
        entry_count: runtime.total_entries(),
        disk_load_error,
    }
}

#[must_use]
pub fn build_root(_contract: &SettingsLaunchContract) -> Box<dyn Widget> {
    Box::new(SettingsRoot::new())
}

pub fn launch(config: SettingsConfig) -> AnyhowResult<()> {
    let contract = prepare_launch(config);

    if let Some(error) = &contract.disk_load_error {
        tracing::warn!("failed to load settings from disk: {error}");
    }
    info!(
        categories = contract.category_count,
        entries = contract.entry_count,
        "Starting liquid-settings",
    );

    app_bootstrap().run(move |_cx| build_root(&contract))
}

pub fn run_binary() -> AnyhowResult<()> {
    init_tracing();
    launch(SettingsConfig::default())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}

// Re-exports for convenience.
pub use bridge::{SettingsBridge, translate};
pub use config::SettingsConfig;
pub use runtime::{SettingDisplay, SettingsRuntime};
pub use widget::SettingsRoot;

#[cfg(test)]
mod launch_tests {
    use super::*;
    use liquide_ui_core::{Constraints, UiTheme};

    #[test]
    fn settings_launch_contract_reports_category_and_entry_counts() {
        let contract = prepare_launch(SettingsConfig::default());

        assert!(contract.category_count > 0);
        assert!(contract.entry_count > 0);
    }

    #[test]
    fn settings_root_measures_non_zero() {
        let contract = prepare_launch(SettingsConfig::default());
        let root = build_root(&contract);
        let result = root.measure(
            &Constraints::new(0.0, 0.0, 800.0, 600.0),
            &UiTheme::default(),
        );

        assert!(result.width > 0.0);
        assert!(result.height > 0.0);
    }
}
