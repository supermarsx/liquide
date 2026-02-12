//! Resource unlocking types for the Resource Unlocking tab (spec section 12).
//!
//! Dedicated tool for finding and releasing locked resources, combining the
//! functionality of Handle.exe, lsof, and Unlocker with safety features
//! such as confirmation prompts, audit logging, and backup options.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The type of unlock operation to perform on a locked resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockOperation {
    CloseHandle,
    TerminateProcess,
    UnloadDll,
    DisconnectNetworkShare,
    ReleaseFileLock,
    ForceUnmount,
    KillLockingProcesses,
}

impl UnlockOperation {
    /// Returns the string representation of this unlock operation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CloseHandle => "Close Handle",
            Self::TerminateProcess => "Terminate Process",
            Self::UnloadDll => "Unload DLL",
            Self::DisconnectNetworkShare => "Disconnect Network Share",
            Self::ReleaseFileLock => "Release File Lock",
            Self::ForceUnmount => "Force Unmount",
            Self::KillLockingProcesses => "Kill Locking Processes",
        }
    }
}

impl fmt::Display for UnlockOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How to batch multiple unlock operations together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchMode {
    Individual,
    AllForFile,
    AllForProcess,
}

impl BatchMode {
    /// Returns the string representation of this batch mode.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Individual => "Individual",
            Self::AllForFile => "All for File",
            Self::AllForProcess => "All for Process",
        }
    }
}

impl fmt::Display for BatchMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Level of confirmation required before executing an unlock operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationLevel {
    None,
    Simple,
    Detailed,
}

impl ConfirmationLevel {
    /// Returns the string representation of this confirmation level.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Simple => "Simple",
            Self::Detailed => "Detailed",
        }
    }
}

impl fmt::Display for ConfirmationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A locked resource target with its list of handle holders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockTarget {
    /// Full path of the locked resource.
    pub path: String,
    /// Processes holding handles to this resource.
    pub holders: Vec<HandleHolder>,
}

/// A process that holds a handle to a locked resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleHolder {
    /// Process ID of the handle holder.
    pub pid: u32,
    /// Name of the process holding the handle.
    pub process_name: String,
    /// System handle value.
    pub handle_value: u64,
    /// Type of handle (e.g. "File", "Section", "Mutex").
    pub handle_type: String,
    /// Human-readable description of the access mode.
    pub access_description: String,
}

/// Safety configuration options for unlock operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockSafetyOptions {
    /// Level of confirmation required before unlocking.
    pub confirm_level: ConfirmationLevel,
    /// Whether to create a system restore point before unlocking.
    pub create_restore_point: bool,
    /// Whether to back up the file before unlocking.
    pub backup_before_unlock: bool,
    /// Whether to attempt a graceful close before forcing.
    pub close_gracefully_first: bool,
    /// Timeout in milliseconds for graceful close attempts.
    pub timeout_ms: u64,
}

impl Default for UnlockSafetyOptions {
    fn default() -> Self {
        Self {
            confirm_level: ConfirmationLevel::Simple,
            create_restore_point: false,
            backup_before_unlock: false,
            close_gracefully_first: true,
            timeout_ms: 5000,
        }
    }
}

/// An audit log entry recording an unlock operation and its outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Timestamp of the unlock operation.
    pub timestamp: String,
    /// Type of operation that was performed.
    pub operation: UnlockOperation,
    /// Path of the resource that was unlocked.
    pub target_path: String,
    /// Process ID of the handle holder targeted.
    pub pid: u32,
    /// Name of the process targeted.
    pub process_name: String,
    /// Whether the operation completed successfully.
    pub success: bool,
    /// Error message if the operation failed.
    pub error_message: Option<String>,
}
