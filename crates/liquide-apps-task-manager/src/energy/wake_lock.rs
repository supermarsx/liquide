//! Wake lock and sleep prevention audit data (spec section 15.11).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// WakeLockType
// ---------------------------------------------------------------------------

/// Type of wake lock preventing system sleep or display off (spec section 15.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeLockType {
    Display,
    System,
    PartialWake,
    ProximityWake,
}

impl WakeLockType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Display => "Display",
            Self::System => "System",
            Self::PartialWake => "Partial Wake",
            Self::ProximityWake => "Proximity Wake",
        }
    }
}

impl fmt::Display for WakeLockType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// WakeLock
// ---------------------------------------------------------------------------

/// An active wake lock preventing sleep or display off (spec section 15.11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeLock {
    /// Unique wake lock identifier.
    pub id: String,
    /// Type of wake lock held.
    pub lock_type: WakeLockType,
    /// PID of the process holding the lock.
    pub owner_pid: u32,
    /// Name of the process holding the lock.
    pub owner_name: String,
    /// Stated reason for the wake lock (provided by the process).
    pub reason: String,
    /// When the lock was acquired (ISO 8601 format).
    pub acquired_time: String,
    /// How long the lock has been held in seconds.
    pub duration_secs: u64,
}
