//! Startup and boot types for the Startup tab (spec section 7).
//!
//! Manages applications and services that run at system boot or user login,
//! tracks their impact on boot time, and records boot history.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Measured impact of a startup entry on boot time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupImpact {
    High,
    Medium,
    Low,
    None,
    NotMeasured,
}

impl StartupImpact {
    /// Returns the string representation of this impact level.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
            Self::None => "None",
            Self::NotMeasured => "Not Measured",
        }
    }
}

impl fmt::Display for StartupImpact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a startup entry is registered with the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupType {
    Registry,
    Folder,
    Task,
    Service,
}

impl StartupType {
    /// Returns the string representation of this startup type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Registry => "Registry",
            Self::Folder => "Folder",
            Self::Task => "Task",
            Self::Service => "Service",
        }
    }
}

impl fmt::Display for StartupType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Phase of the boot process where a startup entry runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootPhase {
    PreBoot,
    Boot,
    PostBoot,
    Login,
}

impl BootPhase {
    /// Returns the string representation of this boot phase.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreBoot => "Pre-Boot",
            Self::Boot => "Boot",
            Self::PostBoot => "Post-Boot",
            Self::Login => "Login",
        }
    }
}

impl fmt::Display for BootPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How the previous session ended before a boot event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownType {
    NormalShutdown,
    NormalReboot,
    UnexpectedShutdown,
    Bsod,
}

impl ShutdownType {
    /// Returns the string representation of this shutdown type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NormalShutdown => "Normal Shutdown",
            Self::NormalReboot => "Normal Reboot",
            Self::UnexpectedShutdown => "Unexpected Shutdown",
            Self::Bsod => "BSOD",
        }
    }
}

impl fmt::Display for ShutdownType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An application or service entry that runs at system startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupEntry {
    /// Friendly name of the startup entry.
    pub name: String,
    /// Developer or publisher name.
    pub publisher: Option<String>,
    /// Full command line executed at startup.
    pub command: String,
    /// How this entry is registered (registry, folder, task, service).
    pub startup_type: StartupType,
    /// Whether this startup entry is currently enabled.
    pub status_enabled: bool,
    /// Measured impact on boot time.
    pub impact: StartupImpact,
    /// Phase of the boot process when this entry runs.
    pub boot_phase: BootPhase,
    /// Disk I/O caused during boot in bytes.
    pub disk_impact_bytes: u64,
    /// CPU time consumed during boot in milliseconds.
    pub cpu_impact_ms: u64,
    /// Estimated delay added to boot in milliseconds.
    pub startup_delay_ms: u64,
    /// Timestamp when this entry was last disabled.
    pub last_disabled: Option<String>,
    /// Absolute path to the executable file.
    pub file_path: Option<String>,
    /// Size of the executable file in bytes.
    pub file_size_bytes: Option<u64>,
    /// Digital signature status (e.g. "Signed", "Unsigned", "Invalid").
    pub digital_signature: Option<String>,
    /// Description of the startup entry.
    pub description: Option<String>,
}

impl Default for StartupEntry {
    fn default() -> Self {
        Self {
            name: String::new(),
            publisher: None,
            command: String::new(),
            startup_type: StartupType::Registry,
            status_enabled: true,
            impact: StartupImpact::NotMeasured,
            boot_phase: BootPhase::Login,
            disk_impact_bytes: 0,
            cpu_impact_ms: 0,
            startup_delay_ms: 0,
            last_disabled: None,
            file_path: None,
            file_size_bytes: None,
            digital_signature: None,
            description: None,
        }
    }
}

/// A record of a single system boot event with timing breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootHistoryEntry {
    /// Date and time of the boot event.
    pub date: String,
    /// Total boot duration in milliseconds.
    pub boot_time_ms: u64,
    /// How the previous session ended.
    pub shutdown_type: ShutdownType,
    /// Duration of the pre-boot phase (BIOS/UEFI) in milliseconds.
    pub pre_boot_ms: u64,
    /// Duration of the kernel and service boot phase in milliseconds.
    pub boot_ms: u64,
    /// Duration of the post-boot phase in milliseconds.
    pub post_boot_ms: u64,
    /// Duration of the user login to desktop-ready phase in milliseconds.
    pub login_ms: u64,
    /// Total wall-clock time from power-on to desktop-ready in milliseconds.
    pub total_ms: u64,
}

impl Default for BootHistoryEntry {
    fn default() -> Self {
        Self {
            date: String::new(),
            boot_time_ms: 0,
            shutdown_type: ShutdownType::NormalShutdown,
            pre_boot_ms: 0,
            boot_ms: 0,
            post_boot_ms: 0,
            login_ms: 0,
            total_ms: 0,
        }
    }
}
