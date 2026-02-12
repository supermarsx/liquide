//! IPC request and response types (spec section 23).
//!
//! Defines the message protocol used between the task manager and other
//! Liquide desktop applications over the D-Bus / named-pipe IPC channel.

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// IpcRequest
// ---------------------------------------------------------------------------

/// A request message sent to the task manager over the IPC channel.
///
/// Because many variants carry heap-allocated data this enum intentionally
/// does **not** derive `Copy`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcRequest {
    // -- Process requests ---------------------------------------------------
    /// List all running processes.
    ListProcesses,
    /// Get detailed information for a single process.
    GetProcess { pid: u32 },
    /// Terminate a process.
    EndProcess { pid: u32 },
    /// Terminate a process and all of its descendants.
    EndProcessTree { pid: u32 },
    /// Suspend a process.
    SuspendProcess { pid: u32 },
    /// Resume a suspended process.
    ResumeProcess { pid: u32 },
    /// Set the scheduling priority of a process.
    SetPriority { pid: u32, priority: String },
    /// Set the CPU core affinity mask for a process.
    SetAffinity { pid: u32, mask: u64 },

    // -- Performance requests -----------------------------------------------
    /// Get current CPU statistics.
    GetCpuStats,
    /// Get current memory statistics.
    GetMemoryStats,
    /// Get current statistics for a specific disk.
    GetDiskStats { index: u8 },
    /// Get current statistics for a specific GPU.
    GetGpuStats { index: u8 },
    /// Get current network performance statistics.
    GetNetworkStats,
    /// Get current power and battery statistics.
    GetPowerStats,
    /// Get current audio subsystem statistics.
    GetAudioStats,

    // -- Service requests ---------------------------------------------------
    /// List all registered system services.
    ListServices,
    /// Start a system service.
    StartService { name: String },
    /// Stop a running system service.
    StopService { name: String },
    /// Restart a system service.
    RestartService { name: String },
    /// Enable a service for automatic start.
    EnableService { name: String },
    /// Disable a service from automatic start.
    DisableService { name: String },

    // -- Device requests ----------------------------------------------------
    /// List all detected hardware devices.
    ListDevices,
    /// Get detailed information for a specific device.
    GetDeviceInfo { device_id: String },

    // -- Network requests ---------------------------------------------------
    /// List all active network connections.
    ListConnections,
    /// Close a specific network connection.
    CloseConnection { pid: u32, local_port: u16 },
    /// List recent DNS query log entries.
    ListDnsQueries,
    /// List firewall rules.
    ListFirewallRules,
    /// Add a new firewall rule from a JSON specification.
    AddFirewallRule { rule_json: String },

    // -- Startup requests ---------------------------------------------------
    /// List all startup (boot) entries.
    ListStartupEntries,
    /// Enable or disable a startup entry.
    SetStartupEnabled { name: String, enabled: bool },

    // -- User / session requests --------------------------------------------
    /// List all logged-in users and their sessions.
    ListUsers,
    /// Disconnect a user session.
    DisconnectUser { session_id: u32 },
    /// Log off a user session.
    LogoffUser { session_id: u32 },

    // -- File handle requests -----------------------------------------------
    /// List all open file handles system-wide.
    ListOpenFiles,
    /// Forcibly close handles locking a file path.
    UnlockFile { path: String },

    // -- Energy / power requests --------------------------------------------
    /// Get current battery status.
    GetBatteryStatus,
    /// List all thermal sensor readings.
    ListThermalSensors,
    /// List all cooling fans and their status.
    ListFans,
    /// Set the operating mode for a cooling fan.
    SetFanMode { fan: String, mode: String },

    // -- Process tree requests ----------------------------------------------
    /// Get the full process hierarchy tree.
    GetProcessTree,

    // -- Audio requests -----------------------------------------------------
    /// List all audio output devices.
    ListAudioOutputDevices,
    /// List all audio input devices.
    ListAudioInputDevices,
    /// List all active audio streams.
    ListAudioStreams,
    /// Set the volume of an audio device.
    SetVolume { device_id: String, percent: f64 },
    /// Mute or unmute an audio device.
    SetMute { device_id: String, muted: bool },

    // -- Configuration requests ---------------------------------------------
    /// Get the current task manager configuration.
    GetConfig,
    /// Set a configuration value.
    SetConfig { key: String, value: String },

    // -- Export requests -----------------------------------------------------
    /// Export data from a tab in the specified format.
    ExportData { tab: String, format: String },

    // -- Subscription requests ----------------------------------------------
    /// Subscribe to a set of event types.
    Subscribe { events: Vec<String> },
    /// Cancel the current event subscription.
    Unsubscribe,

    // -- Health check -------------------------------------------------------
    /// Simple liveness check.
    Ping,
}

impl IpcRequest {
    /// Return a human-readable label for this request type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ListProcesses => "List Processes",
            Self::GetProcess { .. } => "Get Process",
            Self::EndProcess { .. } => "End Process",
            Self::EndProcessTree { .. } => "End Process Tree",
            Self::SuspendProcess { .. } => "Suspend Process",
            Self::ResumeProcess { .. } => "Resume Process",
            Self::SetPriority { .. } => "Set Priority",
            Self::SetAffinity { .. } => "Set Affinity",
            Self::GetCpuStats => "Get CPU Stats",
            Self::GetMemoryStats => "Get Memory Stats",
            Self::GetDiskStats { .. } => "Get Disk Stats",
            Self::GetGpuStats { .. } => "Get GPU Stats",
            Self::GetNetworkStats => "Get Network Stats",
            Self::GetPowerStats => "Get Power Stats",
            Self::GetAudioStats => "Get Audio Stats",
            Self::ListServices => "List Services",
            Self::StartService { .. } => "Start Service",
            Self::StopService { .. } => "Stop Service",
            Self::RestartService { .. } => "Restart Service",
            Self::EnableService { .. } => "Enable Service",
            Self::DisableService { .. } => "Disable Service",
            Self::ListDevices => "List Devices",
            Self::GetDeviceInfo { .. } => "Get Device Info",
            Self::ListConnections => "List Connections",
            Self::CloseConnection { .. } => "Close Connection",
            Self::ListDnsQueries => "List DNS Queries",
            Self::ListFirewallRules => "List Firewall Rules",
            Self::AddFirewallRule { .. } => "Add Firewall Rule",
            Self::ListStartupEntries => "List Startup Entries",
            Self::SetStartupEnabled { .. } => "Set Startup Enabled",
            Self::ListUsers => "List Users",
            Self::DisconnectUser { .. } => "Disconnect User",
            Self::LogoffUser { .. } => "Logoff User",
            Self::ListOpenFiles => "List Open Files",
            Self::UnlockFile { .. } => "Unlock File",
            Self::GetBatteryStatus => "Get Battery Status",
            Self::ListThermalSensors => "List Thermal Sensors",
            Self::ListFans => "List Fans",
            Self::SetFanMode { .. } => "Set Fan Mode",
            Self::GetProcessTree => "Get Process Tree",
            Self::ListAudioOutputDevices => "List Audio Output Devices",
            Self::ListAudioInputDevices => "List Audio Input Devices",
            Self::ListAudioStreams => "List Audio Streams",
            Self::SetVolume { .. } => "Set Volume",
            Self::SetMute { .. } => "Set Mute",
            Self::GetConfig => "Get Config",
            Self::SetConfig { .. } => "Set Config",
            Self::ExportData { .. } => "Export Data",
            Self::Subscribe { .. } => "Subscribe",
            Self::Unsubscribe => "Unsubscribe",
            Self::Ping => "Ping",
        }
    }
}

impl fmt::Display for IpcRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// IpcResponse
// ---------------------------------------------------------------------------

/// A response message returned from the task manager over the IPC channel.
///
/// Because variants carry heap-allocated data this enum intentionally does
/// **not** derive `Copy`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcResponse {
    /// The request was handled successfully; `data` contains a
    /// JSON-serialised payload.
    Success { data: String },
    /// The request failed.
    Error { code: i32, message: String },
    /// A pushed event for active subscribers.
    Event { event_json: String },
    /// Reply to a `Ping` request.
    Pong,
}

impl IpcResponse {
    /// Return a human-readable label for this response type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success { .. } => "Success",
            Self::Error { .. } => "Error",
            Self::Event { .. } => "Event",
            Self::Pong => "Pong",
        }
    }
}

impl fmt::Display for IpcResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
