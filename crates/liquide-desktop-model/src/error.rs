//! Error types for desktop model operations.

use crate::types::{DesktopId, WindowStationId};
use std::fmt;

/// Errors that can occur during desktop model operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopError {
    /// The specified window station was not found.
    StationNotFound(WindowStationId),
    /// The specified desktop was not found.
    DesktopNotFound(DesktopId),
    /// Cannot switch to a desktop that belongs to a different station
    /// than the currently active station.
    StationMismatch {
        desktop: DesktopId,
        expected_station: WindowStationId,
        actual_station: WindowStationId,
    },
    /// The desktop is locked and cannot be switched away from.
    InputLocked(DesktopId),
    /// Input can only be locked to the currently active secure desktop.
    InputLockRequiresActiveSecureDesktop {
        desktop: DesktopId,
        active_desktop: Option<DesktopId>,
    },
    /// Access denied for the requested operation.
    AccessDenied {
        desktop: DesktopId,
        thread_id: u64,
        required: String,
    },
    /// Desktop heap budget exceeded.
    HeapExhausted {
        desktop: DesktopId,
        requested: usize,
        available: usize,
    },
    /// Clipboard is not open (must call open() first).
    ClipboardNotOpen,
    /// Clipboard is already open by another window.
    ClipboardAlreadyOpen {
        current_owner: crate::types::WindowId,
    },
    /// A station with the given name already exists.
    StationNameExists(String),
    /// A desktop with the given name already exists in this station.
    DesktopNameExists {
        station: WindowStationId,
        name: String,
    },
    /// Cannot close the last station.
    LastStation,
}

impl fmt::Display for DesktopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StationNotFound(id) => write!(f, "window station {:?} not found", id),
            Self::DesktopNotFound(id) => write!(f, "desktop {:?} not found", id),
            Self::StationMismatch {
                desktop,
                expected_station,
                actual_station,
            } => write!(
                f,
                "desktop {:?} belongs to station {:?}, not active station {:?}",
                desktop, actual_station, expected_station
            ),
            Self::InputLocked(id) => {
                write!(f, "input is locked to desktop {:?}", id)
            }
            Self::InputLockRequiresActiveSecureDesktop {
                desktop,
                active_desktop,
            } => write!(
                f,
                "input can only be locked to the active secure desktop {:?} (current active: {:?})",
                desktop, active_desktop
            ),
            Self::AccessDenied {
                desktop,
                thread_id,
                required,
            } => write!(
                f,
                "thread {} denied {} access to desktop {:?}",
                thread_id, required, desktop
            ),
            Self::HeapExhausted {
                desktop,
                requested,
                available,
            } => write!(
                f,
                "desktop {:?} heap exhausted: requested {} bytes, {} available",
                desktop, requested, available
            ),
            Self::ClipboardNotOpen => write!(f, "clipboard is not open"),
            Self::ClipboardAlreadyOpen { current_owner } => {
                write!(f, "clipboard already open by window {:?}", current_owner)
            }
            Self::StationNameExists(name) => {
                write!(f, "window station '{}' already exists", name)
            }
            Self::DesktopNameExists { station, name } => {
                write!(
                    f,
                    "desktop '{}' already exists in station {:?}",
                    name, station
                )
            }
            Self::LastStation => write!(f, "cannot close the last window station"),
        }
    }
}

impl std::error::Error for DesktopError {}
