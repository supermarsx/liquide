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

use thiserror::Error;

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
