//! Terminal emulator application for the LiquiDE desktop environment.
//!
//! Provides VT sequence parsing, character grid management, scrollback
//! buffers, PTY abstraction, shell integration, and tab/pane management.

pub mod config;
pub mod grid;
pub mod pty;
pub mod runtime;
pub mod scrollback;
pub mod search;
pub mod shell_integration;
pub mod tab;
pub mod url_detect;
pub mod vt;

#[cfg(test)]
mod tests;

use anyhow::Result as AnyhowResult;
use liquide_app_harness::{AppBootstrap, Size};
use liquide_ui_core::widget::Widget;
use liquide_ui_widgets::Label;
use thiserror::Error;
use tracing::info;

/// Errors produced by the terminal emulator.
#[derive(Debug, Error)]
pub enum TerminalError {
    /// PTY spawn failed.
    #[error("failed to spawn PTY: {reason}")]
    PtySpawnFailed { reason: String },

    /// Shell exited.
    #[error("shell exited with code {code}")]
    ShellExited { code: i32 },

    /// Tab not found.
    #[error("tab not found: {id}")]
    TabNotFound { id: u32 },

    /// Pane not found.
    #[error("pane not found: {id}")]
    PaneNotFound { id: u32 },

    /// Invalid grid coordinate.
    #[error("coordinate out of bounds: ({row}, {col})")]
    OutOfBounds { row: u32, col: u32 },

    /// Scrollback buffer error.
    #[error("scrollback error: {0}")]
    ScrollbackError(String),

    /// Search regex invalid.
    #[error("invalid search pattern: {0}")]
    InvalidPattern(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    ConfigError(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, TerminalError>;

pub const TERMINAL_APP_ID: &str = "com.liquide.apps.terminal";
pub const TERMINAL_DISPLAY_NAME: &str = "Terminal";
pub const TERMINAL_INITIAL_SIZE: Size = Size::new(960, 640);

/// PTY strategy used when preparing a terminal launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLaunchMode {
    PlatformPty,
    StubPty,
}

/// Minimal runtime state that downstream launch tests can assert after setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLaunchContract {
    pub mode: TerminalLaunchMode,
    pub rows: u32,
    pub cols: u32,
    pub tab_count: usize,
    pub shell_label: String,
}

#[must_use]
pub fn app_bootstrap() -> AppBootstrap {
    AppBootstrap::new(TERMINAL_APP_ID, TERMINAL_DISPLAY_NAME)
        .with_initial_size(TERMINAL_INITIAL_SIZE)
        .with_ime(true)
}

pub fn prepare_launch(config: TerminalConfig, mode: TerminalLaunchMode) -> crate::Result<TerminalLaunchContract> {
    let shell_label = match mode {
        TerminalLaunchMode::PlatformPty if config.shell.is_empty() => "auto".to_string(),
        TerminalLaunchMode::PlatformPty => config.shell.clone(),
        TerminalLaunchMode::StubPty => "stub".to_string(),
    };

    match mode {
        TerminalLaunchMode::PlatformPty => {
            let mut runtime = TerminalRuntime::new(config);
            runtime.new_tab(None)?;
            let grid = runtime.active_grid();

            Ok(TerminalLaunchContract {
                mode,
                rows: grid.rows(),
                cols: grid.cols(),
                tab_count: runtime.tab_count(),
                shell_label,
            })
        }
        TerminalLaunchMode::StubPty => {
            let size = pty::PtySize::new(config.rows, config.cols);
            let tab = tab::Tab::new_with_pty(
                1,
                config.rows,
                config.cols,
                config.scrollback_lines,
                pty::PtyBackend::new_stub(size),
            )?;
            let grid = tab.grid();

            Ok(TerminalLaunchContract {
                mode,
                rows: grid.rows(),
                cols: grid.cols(),
                tab_count: 1,
                shell_label,
            })
        }
    }
}

#[must_use]
pub fn build_root(contract: &TerminalLaunchContract) -> Box<dyn Widget> {
    Box::new(Label::new(format!(
        "liquid-terminal — {}x{}",
        contract.rows, contract.cols
    )))
}

pub fn launch(config: TerminalConfig) -> AnyhowResult<()> {
    launch_with_mode(config, TerminalLaunchMode::PlatformPty)
}

pub fn launch_with_mode(config: TerminalConfig, mode: TerminalLaunchMode) -> AnyhowResult<()> {
    let contract = prepare_launch(config, mode)?;

    app_bootstrap().run(move |_cx| {
        info!(
            rows = contract.rows,
            cols = contract.cols,
            shell = %contract.shell_label,
            "Terminal runtime ready"
        );
        build_root(&contract)
    })
}

pub fn run_binary() -> AnyhowResult<()> {
    init_tracing();
    info!("Starting liquid-terminal");
    launch(TerminalConfig::default())
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
pub use config::TerminalConfig;
pub use runtime::{RenderedLine, TerminalRuntime, TextSpan};

#[cfg(test)]
mod launch_tests {
    use super::*;
    use liquide_ui_core::{Constraints, UiTheme};

    #[test]
    fn terminal_stub_launch_contract_is_deterministic() {
        let contract = prepare_launch(TerminalConfig::default(), TerminalLaunchMode::StubPty)
            .expect("stub PTY launch should succeed");

        assert_eq!(contract.mode, TerminalLaunchMode::StubPty);
        assert_eq!(contract.rows, TerminalConfig::default().rows);
        assert_eq!(contract.cols, TerminalConfig::default().cols);
        assert_eq!(contract.tab_count, 1);
        assert_eq!(contract.shell_label, "stub");
    }

    #[test]
    fn terminal_root_measures_non_zero() {
        let contract = prepare_launch(TerminalConfig::default(), TerminalLaunchMode::StubPty)
            .expect("stub PTY launch should succeed");
        let root = build_root(&contract);
        let result = root.measure(&Constraints::new(0.0, 0.0, 800.0, 600.0), &UiTheme::default());

        assert!(result.width > 0.0);
        assert!(result.height > 0.0);
    }
}
