//! Task manager application for the LiquiDE desktop environment.
//!
//! Provides real-time visibility into running processes, system resource
//! utilization, hardware performance, active services, device states,
//! file locks, user sessions, and boot-time applications.

pub mod action;
pub mod aggregator;
pub mod app_history;
pub mod audio;
pub mod collector;
pub mod config;
pub mod devices;
pub mod energy;
pub mod event;
pub mod export;
pub mod files;
pub mod filter;
pub mod ipc;
pub mod network;
pub mod performance;
pub mod plugin;
pub mod process;
pub mod process_tree;
pub mod runtime;
pub mod services;
pub mod shortcut;
pub mod startup;
pub mod system_events;
pub mod system_monitor;
pub mod ui;
pub mod unlock;
pub mod users;

use liquide_app_harness::{AppBootstrap, Size};
use liquide_ui_widgets::Label;
use thiserror::Error;
use tracing::info;

/// Errors produced by the task manager.
#[derive(Debug, Error)]
pub enum TaskManagerError {
    /// Process not found.
    #[error("process not found: pid {pid}")]
    ProcessNotFound { pid: u32 },

    /// Service not found.
    #[error("service not found: {name}")]
    ServiceNotFound { name: String },

    /// Device not found.
    #[error("device not found: {id}")]
    DeviceNotFound { id: String },

    /// Permission denied for a privileged operation.
    #[error("permission denied: {action}")]
    PermissionDenied { action: String },

    /// Collector failed to gather data.
    #[error("collector error: {detail}")]
    CollectorError { detail: String },

    /// Configuration error.
    #[error("configuration error: {0}")]
    ConfigError(String),

    /// Export failed.
    #[error("export error: {0}")]
    ExportError(String),

    /// IPC communication failure.
    #[error("IPC error: {0}")]
    IpcError(String),

    /// Plugin error.
    #[error("plugin error: {0}")]
    PluginError(String),

    /// Filter parse error.
    #[error("invalid filter expression: {0}")]
    FilterParseError(String),

    /// File unlock failed.
    #[error("unlock failed: {0}")]
    UnlockFailed(String),

    /// Audio subsystem error.
    #[error("audio error: {0}")]
    AudioError(String),

    /// Network diagnostics error.
    #[error("network error: {0}")]
    NetworkError(String),

    /// Event log query failure.
    #[error("event log error: {0}")]
    EventLogError(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, TaskManagerError>;

// Re-exports for convenience.
pub use config::TaskManagerConfig;
pub use runtime::TaskManagerRuntime;

/// Reverse-DNS application identifier for the task manager.
pub const APP_ID: &str = "com.liquide.apps.task-manager";

/// Display name used for the default task manager window.
pub const DISPLAY_NAME: &str = "Task Manager";

/// Display name used for task manager widget mode.
pub const WIDGET_DISPLAY_NAME: &str = "Task Manager Widget";

/// Initial window size for the default GUI launch path.
pub const DEFAULT_WINDOW_SIZE: Size = Size::new(1240, 820);

/// Initial window size for widget mode.
pub const DEFAULT_WIDGET_SIZE: Size = Size::new(420, 260);

/// Graphical launch mode for the task manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskManagerLaunchMode {
    Window,
    Widget,
}

impl TaskManagerLaunchMode {
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Window => DISPLAY_NAME,
            Self::Widget => WIDGET_DISPLAY_NAME,
        }
    }

    #[must_use]
    pub const fn initial_size(self) -> Size {
        match self {
            Self::Window => DEFAULT_WINDOW_SIZE,
            Self::Widget => DEFAULT_WIDGET_SIZE,
        }
    }
}

/// Runtime summary produced by the graphical launch path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskManagerLaunchState {
    pub mode: TaskManagerLaunchMode,
    pub active_tab: ui::TabId,
    pub summary: String,
}

/// Build the task manager bootstrap used by the production binary.
#[must_use]
pub fn bootstrap_for_mode(mode: TaskManagerLaunchMode) -> AppBootstrap {
    AppBootstrap::new(APP_ID, mode.display_name())
        .with_initial_size(mode.initial_size())
        .with_ime(false)
}

/// Build the runtime state surfaced by the graphical launch path.
pub async fn graphical_launch_state(
    config: TaskManagerConfig,
    active_tab: Option<ui::TabId>,
    mode: TaskManagerLaunchMode,
) -> TaskManagerLaunchState {
    let runtime = TaskManagerRuntime::new(config);
    if let Some(tab) = active_tab {
        runtime.set_active_tab(tab).await;
    }

    let active_tab = runtime.active_tab().await;
    let summary = match mode {
        TaskManagerLaunchMode::Widget => {
            format!("liquid-taskmanager widget — active tab: {active_tab}")
        }
        TaskManagerLaunchMode::Window => {
            format!("liquid-taskmanager — active tab: {active_tab}")
        }
    };

    TaskManagerLaunchState {
        mode,
        active_tab,
        summary,
    }
}

/// Build the placeholder root widget from a previously computed launch state.
#[must_use]
pub fn build_graphical_root(state: &TaskManagerLaunchState) -> Label {
    Label::new(state.summary.clone())
}

async fn run_graphical_app(
    config: TaskManagerConfig,
    active_tab: Option<ui::TabId>,
    mode: TaskManagerLaunchMode,
) -> anyhow::Result<()> {
    let state = graphical_launch_state(config, active_tab, mode).await;

    match state.mode {
        TaskManagerLaunchMode::Widget => {
            info!(active_tab = %state.active_tab, "starting in floating widget mode");
        }
        TaskManagerLaunchMode::Window => {
            info!(active_tab = %state.active_tab, "starting GUI");
        }
    }

    bootstrap_for_mode(state.mode).run(move |_cx| Box::new(build_graphical_root(&state)))
}

/// Run the default task manager GUI path.
pub async fn run_default_app(
    config: TaskManagerConfig,
    active_tab: Option<ui::TabId>,
) -> anyhow::Result<()> {
    run_graphical_app(config, active_tab, TaskManagerLaunchMode::Window).await
}

/// Run the task manager widget GUI path.
pub async fn run_widget_app(
    config: TaskManagerConfig,
    active_tab: Option<ui::TabId>,
) -> anyhow::Result<()> {
    run_graphical_app(config, active_tab, TaskManagerLaunchMode::Widget).await
}

#[cfg(test)]
mod launch_tests {
    use super::*;

    #[tokio::test]
    async fn default_launch_state_uses_requested_tab() {
        let state = graphical_launch_state(
            TaskManagerConfig::default(),
            Some(ui::TabId::Performance),
            TaskManagerLaunchMode::Window,
        )
        .await;

        assert_eq!(state.mode, TaskManagerLaunchMode::Window);
        assert_eq!(state.active_tab, ui::TabId::Performance);
        assert_eq!(state.summary, "liquid-taskmanager — active tab: Performance");
    }

    #[tokio::test]
    async fn widget_launch_state_uses_widget_mode_defaults() {
        let state = graphical_launch_state(
            TaskManagerConfig::default(),
            None,
            TaskManagerLaunchMode::Widget,
        )
        .await;

        assert_eq!(state.mode, TaskManagerLaunchMode::Widget);
        assert_eq!(state.active_tab, ui::TabId::Processes);
        assert_eq!(
            state.summary,
            "liquid-taskmanager widget — active tab: Processes"
        );
    }
}
