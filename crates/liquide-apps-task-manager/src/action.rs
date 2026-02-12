//! Privileged action types and dispatch interface (spec sections 2.3, 4.5).
//!
//! Defines the commands that the elevated helper daemon can execute on
//! behalf of the task manager, together with their result type and the
//! trait used to dispatch them.

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// PrivilegedAction
// ---------------------------------------------------------------------------

/// A privileged operation that requires elevated (root / SYSTEM) permissions.
///
/// Because several variants carry heap-allocated `String` data this enum
/// intentionally does **not** derive `Copy`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegedAction {
    /// Terminate a single process.
    Kill { pid: u32 },
    /// Terminate a process and all of its descendants.
    KillTree { pid: u32 },
    /// Suspend (freeze) a process.
    Suspend { pid: u32 },
    /// Resume a previously suspended process.
    Resume { pid: u32 },
    /// Change the scheduling priority of a process.
    Renice { pid: u32, priority: i32 },
    /// Set the CPU core affinity mask for a process.
    SetAffinity { pid: u32, mask: u64 },
    /// Set the I/O scheduling priority for a process.
    SetIoPriority { pid: u32, priority: u8 },
    /// Forcibly close an open handle held by a process.
    UnlockHandle { pid: u32, handle: u64 },
    /// Enable a system service so it starts automatically.
    EnableService { name: String },
    /// Disable a system service so it no longer starts automatically.
    DisableService { name: String },
    /// Start a system service.
    StartService { name: String },
    /// Stop a running system service.
    StopService { name: String },
    /// Restart a system service (stop then start).
    RestartService { name: String },
    /// Enable or disable a startup entry.
    SetStartup { name: String, enabled: bool },
    /// Create a memory dump of a process.
    CreateDump { pid: u32, full: bool },
    /// Attach a debugger to a running process.
    AttachDebugger { pid: u32 },
    /// Forcibly unmount a filesystem path.
    ForceUnmount { path: String },
    /// Close a network connection owned by a process.
    CloseNetworkConnection { pid: u32, local_port: u16 },
    /// Switch to a different system power plan.
    SetPowerPlan { plan_id: String },
    /// Set the operating mode for a cooling fan.
    SetFanMode { fan_name: String, mode: String },
}

impl PrivilegedAction {
    /// Return a human-readable label describing this action category.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Kill { .. } => "Kill",
            Self::KillTree { .. } => "Kill Tree",
            Self::Suspend { .. } => "Suspend",
            Self::Resume { .. } => "Resume",
            Self::Renice { .. } => "Renice",
            Self::SetAffinity { .. } => "Set Affinity",
            Self::SetIoPriority { .. } => "Set I/O Priority",
            Self::UnlockHandle { .. } => "Unlock Handle",
            Self::EnableService { .. } => "Enable Service",
            Self::DisableService { .. } => "Disable Service",
            Self::StartService { .. } => "Start Service",
            Self::StopService { .. } => "Stop Service",
            Self::RestartService { .. } => "Restart Service",
            Self::SetStartup { .. } => "Set Startup",
            Self::CreateDump { .. } => "Create Dump",
            Self::AttachDebugger { .. } => "Attach Debugger",
            Self::ForceUnmount { .. } => "Force Unmount",
            Self::CloseNetworkConnection { .. } => "Close Network Connection",
            Self::SetPowerPlan { .. } => "Set Power Plan",
            Self::SetFanMode { .. } => "Set Fan Mode",
        }
    }
}

impl fmt::Display for PrivilegedAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ActionResult
// ---------------------------------------------------------------------------

/// Outcome of dispatching a [`PrivilegedAction`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// Whether the action completed successfully.
    pub success: bool,
    /// A human-readable message describing the outcome.
    pub message: String,
    /// An optional platform-specific error code (e.g. Win32 error, errno).
    pub error_code: Option<i32>,
}

// ---------------------------------------------------------------------------
// ActionDispatcher
// ---------------------------------------------------------------------------

/// Sends privileged commands to the elevated helper daemon.
///
/// Implementations communicate over a local Unix socket or named pipe to the
/// root/SYSTEM helper process (see spec section 2.3).
pub trait ActionDispatcher {
    /// Execute a privileged action and return the result.
    fn dispatch(&self, action: &PrivilegedAction) -> ActionResult;

    /// Check whether the given action requires elevation beyond the current
    /// privilege level.
    fn requires_elevation(&self, action: &PrivilegedAction) -> bool;
}
