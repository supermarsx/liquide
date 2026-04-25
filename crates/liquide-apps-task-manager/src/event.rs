//! Real-time event types emitted by the task manager (spec section 21).
//!
//! The task manager produces a stream of [`TaskManagerEvent`] values that
//! other Liquide applications can subscribe to via the IPC interface.
//! [`EventFilter`] allows subscribers to narrow the stream to events they
//! care about.

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// TaskManagerEvent
// ---------------------------------------------------------------------------

/// An observable event emitted by the task manager.
///
/// Because many variants carry heap-allocated data this enum intentionally
/// does **not** derive `Copy`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskManagerEvent {
    // -- Process events -----------------------------------------------------
    /// A new process was created.
    ProcessCreated { pid: u32, name: String },
    /// A process exited.
    ProcessExited { pid: u32, exit_code: i32 },
    /// A process CPU usage spiked above a threshold.
    ProcessCpuSpike { pid: u32, percent: f64 },
    /// A process memory usage spiked above a threshold.
    ProcessMemorySpike { pid: u32, bytes: u64 },
    /// A process became unresponsive (window message pump stalled > 5 s).
    ProcessNotResponding { pid: u32 },
    /// A previously suspended process was resumed.
    ProcessResumed { pid: u32 },

    // -- System threshold events --------------------------------------------
    /// Overall CPU usage exceeded the configured threshold.
    CpuThresholdExceeded { percent: f64 },
    /// Overall memory usage exceeded the configured threshold.
    MemoryThresholdExceeded { percent: f64 },
    /// Disk utilisation exceeded the configured threshold.
    DiskThresholdExceeded { percent: f64 },
    /// GPU utilisation exceeded the configured threshold.
    GpuThresholdExceeded { percent: f64 },
    /// Network throughput exceeded the configured threshold.
    NetworkThresholdExceeded { bytes_sec: u64 },

    // -- Service events -----------------------------------------------------
    /// A system service started.
    ServiceStarted { name: String },
    /// A system service stopped.
    ServiceStopped { name: String },
    /// A system service failed.
    ServiceFailed { name: String, error: String },

    // -- Device events ------------------------------------------------------
    /// A hardware device was connected.
    DeviceConnected { device_id: String },
    /// A hardware device was disconnected.
    DeviceDisconnected { device_id: String },
    /// A hardware device reported an error.
    DeviceError { device_id: String, error: String },

    // -- User / session events ----------------------------------------------
    /// A user logged in.
    UserLoggedIn { username: String },
    /// A user logged out.
    UserLoggedOut { username: String },
    /// A session was locked.
    SessionLocked { session_id: u32 },
    /// A session was unlocked.
    SessionUnlocked { session_id: u32 },

    // -- File lock events ---------------------------------------------------
    /// A file was locked by a process.
    FileLocked { path: String, pid: u32 },
    /// A file lock was released.
    FileUnlocked { path: String },

    // -- Network events -----------------------------------------------------
    /// A network connection was opened.
    NetworkConnectionOpened { pid: u32, remote: String },
    /// A network connection was closed.
    NetworkConnectionClosed { pid: u32, remote: String },
    /// A DNS query was blocked by policy.
    DnsQueryBlocked { domain: String },
    /// A firewall rule was triggered.
    FirewallRuleTriggered { rule_name: String },

    // -- Power / battery events ---------------------------------------------
    /// Battery charge dropped below a warning threshold.
    BatteryLow { percent: f64 },
    /// The battery began charging.
    BatteryCharging,
    /// The battery began discharging.
    BatteryDischarging,
    /// The system power source changed.
    PowerSourceChanged { source: String },

    // -- Thermal events -----------------------------------------------------
    /// A thermal sensor exceeded its warning threshold.
    ThermalWarning { sensor: String, celsius: f64 },
    /// A thermal sensor reached a critical temperature.
    ThermalCritical { sensor: String, celsius: f64 },
    /// A cooling fan changed speed.
    FanSpeedChanged { fan: String, rpm: u32 },

    // -- Audio events -------------------------------------------------------
    /// An audio device was connected.
    AudioDeviceAdded { device_id: String },
    /// An audio device was removed.
    AudioDeviceRemoved { device_id: String },
    /// An audio glitch (buffer underrun/overrun) was detected.
    AudioGlitch { device_id: String },
    /// The volume of an audio device changed.
    VolumeChanged { device_id: String, percent: f64 },

    // -- Plugin events ------------------------------------------------------
    /// A plugin was loaded.
    PluginLoaded { name: String },
    /// A plugin was unloaded.
    PluginUnloaded { name: String },

    // -- Configuration events -----------------------------------------------
    /// A configuration value was changed.
    ConfigChanged { key: String },

    // -- System event log events --------------------------------------------
    /// A critical or error event appeared in the system event log.
    SystemEventLogAlert {
        source: String,
        event_id: u32,
        message: String,
    },
    /// An event log was cleared.
    EventLogCleared { source: String },
}

impl TaskManagerEvent {
    /// Return a human-readable label for this event category.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ProcessCreated { .. } => "Process Created",
            Self::ProcessExited { .. } => "Process Exited",
            Self::ProcessCpuSpike { .. } => "Process CPU Spike",
            Self::ProcessMemorySpike { .. } => "Process Memory Spike",
            Self::ProcessNotResponding { .. } => "Process Not Responding",
            Self::ProcessResumed { .. } => "Process Resumed",
            Self::CpuThresholdExceeded { .. } => "CPU Threshold Exceeded",
            Self::MemoryThresholdExceeded { .. } => "Memory Threshold Exceeded",
            Self::DiskThresholdExceeded { .. } => "Disk Threshold Exceeded",
            Self::GpuThresholdExceeded { .. } => "GPU Threshold Exceeded",
            Self::NetworkThresholdExceeded { .. } => "Network Threshold Exceeded",
            Self::ServiceStarted { .. } => "Service Started",
            Self::ServiceStopped { .. } => "Service Stopped",
            Self::ServiceFailed { .. } => "Service Failed",
            Self::DeviceConnected { .. } => "Device Connected",
            Self::DeviceDisconnected { .. } => "Device Disconnected",
            Self::DeviceError { .. } => "Device Error",
            Self::UserLoggedIn { .. } => "User Logged In",
            Self::UserLoggedOut { .. } => "User Logged Out",
            Self::SessionLocked { .. } => "Session Locked",
            Self::SessionUnlocked { .. } => "Session Unlocked",
            Self::FileLocked { .. } => "File Locked",
            Self::FileUnlocked { .. } => "File Unlocked",
            Self::NetworkConnectionOpened { .. } => "Network Connection Opened",
            Self::NetworkConnectionClosed { .. } => "Network Connection Closed",
            Self::DnsQueryBlocked { .. } => "DNS Query Blocked",
            Self::FirewallRuleTriggered { .. } => "Firewall Rule Triggered",
            Self::BatteryLow { .. } => "Battery Low",
            Self::BatteryCharging => "Battery Charging",
            Self::BatteryDischarging => "Battery Discharging",
            Self::PowerSourceChanged { .. } => "Power Source Changed",
            Self::ThermalWarning { .. } => "Thermal Warning",
            Self::ThermalCritical { .. } => "Thermal Critical",
            Self::FanSpeedChanged { .. } => "Fan Speed Changed",
            Self::AudioDeviceAdded { .. } => "Audio Device Added",
            Self::AudioDeviceRemoved { .. } => "Audio Device Removed",
            Self::AudioGlitch { .. } => "Audio Glitch",
            Self::VolumeChanged { .. } => "Volume Changed",
            Self::PluginLoaded { .. } => "Plugin Loaded",
            Self::PluginUnloaded { .. } => "Plugin Unloaded",
            Self::ConfigChanged { .. } => "Config Changed",
            Self::SystemEventLogAlert { .. } => "System Event Log Alert",
            Self::EventLogCleared { .. } => "Event Log Cleared",
        }
    }
}

impl fmt::Display for TaskManagerEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// EventFilter
// ---------------------------------------------------------------------------

/// Criteria for filtering the task manager event stream.
///
/// All fields are optional; when `None` the corresponding dimension is
/// unfiltered (i.e. all values pass).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventFilter {
    /// Only emit events whose type name (snake_case) is in this list.
    pub event_types: Option<Vec<String>>,
    /// Only emit process-related events for these PIDs.
    pub pids: Option<Vec<u32>>,
    /// Only emit events at or above this severity level.
    pub min_severity: Option<String>,
}
