//! Service management types for the Services tab (spec section 9).
//!
//! Provides complete service management interface showing all system services
//! (systemd units / Windows services / launchd plists), including status,
//! startup configuration, resource usage, recovery actions, and dependency info.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Current operational status of a system service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    Running,
    Stopped,
    Paused,
    StartPending,
    StopPending,
    PausePending,
    ContinuePending,
}

impl ServiceStatus {
    /// Returns the string representation of this status.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Stopped => "Stopped",
            Self::Paused => "Paused",
            Self::StartPending => "Start Pending",
            Self::StopPending => "Stop Pending",
            Self::PausePending => "Pause Pending",
            Self::ContinuePending => "Continue Pending",
        }
    }
}

impl fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a service is configured to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStartupType {
    Automatic,
    AutomaticDelayed,
    Manual,
    Disabled,
    Boot,
}

impl ServiceStartupType {
    /// Returns the string representation of this startup type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::AutomaticDelayed => "Automatic (Delayed)",
            Self::Manual => "Manual",
            Self::Disabled => "Disabled",
            Self::Boot => "Boot",
        }
    }
}

impl fmt::Display for ServiceStartupType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The execution model of a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceType {
    KernelDriver,
    FileSystemDriver,
    OwnProcess,
    ShareProcess,
    Interactive,
}

impl ServiceType {
    /// Returns the string representation of this service type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::KernelDriver => "Kernel Driver",
            Self::FileSystemDriver => "File System Driver",
            Self::OwnProcess => "Own Process",
            Self::ShareProcess => "Share Process",
            Self::Interactive => "Interactive",
        }
    }
}

impl fmt::Display for ServiceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Action to take when a service fails (first, second, or subsequent failure).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryAction {
    /// The recovery action type (e.g. "restart_service", "reboot", "run_program", "none").
    pub action: String,
    /// Delay in milliseconds before executing the recovery action.
    pub delay_ms: u64,
    /// Optional command to run (for "run_program" action type).
    pub command: Option<String>,
}

/// An action that can be performed on a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAction {
    Start,
    Stop,
    Pause,
    Resume,
    Restart,
    Configure,
}

impl ServiceAction {
    /// Returns the string representation of this action.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Stop => "Stop",
            Self::Pause => "Pause",
            Self::Resume => "Resume",
            Self::Restart => "Restart",
            Self::Configure => "Configure",
        }
    }
}

impl fmt::Display for ServiceAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Comprehensive information about a system service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Short service name identifier.
    pub name: String,
    /// Friendly display name.
    pub display_name: String,
    /// Service description text.
    pub description: String,
    /// Current operational status.
    pub status: ServiceStatus,
    /// How the service is configured to start.
    pub startup_type: ServiceStartupType,
    /// The execution model of the service.
    pub service_type: ServiceType,
    /// Process ID if the service is currently running.
    pub pid: Option<u32>,
    /// Absolute path to the service binary on disk.
    pub binary_path: String,
    /// The user account the service runs under.
    pub account: String,
    /// List of services this service depends on.
    pub dependencies: Vec<String>,
    /// List of services that depend on this service.
    pub dependent_services: Vec<String>,
    /// Timestamp of when the service last started.
    pub start_time: Option<String>,
    /// Current CPU usage as a percentage.
    pub cpu_percent: f64,
    /// Current memory usage in bytes.
    pub mem_bytes: u64,
    /// Current disk read rate in bytes per second.
    pub disk_read_bytes_sec: u64,
    /// Current disk write rate in bytes per second.
    pub disk_write_bytes_sec: u64,
    /// Number of open handles held by the service process.
    pub handles: u32,
    /// Number of threads in the service process.
    pub threads: u32,
    /// Error control level (Ignore / Normal / Severe / Critical).
    pub error_control: String,
    /// Load ordering group for boot-time service sequencing.
    pub load_order_group: Option<String>,
    /// Recovery action for the first service failure.
    pub recovery_first: Option<RecoveryAction>,
    /// Recovery action for the second service failure.
    pub recovery_second: Option<RecoveryAction>,
    /// Recovery action for subsequent service failures.
    pub recovery_subsequent: Option<RecoveryAction>,
    /// Seconds after which the failure count resets.
    pub reset_period_secs: Option<u64>,
}

impl Default for ServiceInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            display_name: String::new(),
            description: String::new(),
            status: ServiceStatus::Stopped,
            startup_type: ServiceStartupType::Manual,
            service_type: ServiceType::OwnProcess,
            pid: None,
            binary_path: String::new(),
            account: String::new(),
            dependencies: Vec::new(),
            dependent_services: Vec::new(),
            start_time: None,
            cpu_percent: 0.0,
            mem_bytes: 0,
            disk_read_bytes_sec: 0,
            disk_write_bytes_sec: 0,
            handles: 0,
            threads: 0,
            error_control: String::from("Normal"),
            load_order_group: None,
            recovery_first: None,
            recovery_second: None,
            recovery_subsequent: None,
            reset_period_secs: None,
        }
    }
}
