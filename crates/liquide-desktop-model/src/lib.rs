//! Desktop and window station isolation model for LiquiDE.
//!
//! This crate implements the security-critical separation between different
//! login sessions and secure desktops using a window station / desktop
//! isolation architecture.
//!
//! # Architecture
//!
//! ```text
//! Session (logon)
//!   └── WindowStation ("WinSta0")
//!         ├── Desktop "Default"     ← where apps run
//!         ├── Desktop "Winlogon"    ← secure: login/lock screen
//!         ├── Desktop "Screensaver"
//!         └── Desktop "Disconnect"  ← remote disconnect screen
//! ```
//!
//! Each [`WindowStation`] owns:
//! - A set of [`Desktop`]s, each with its own window hierarchy
//! - A per-station [`ClipboardData`] (clipboard is shared within a station)
//! - An [`AtomTable`] for efficient string interning
//!
//! Each [`Desktop`] owns:
//! - A root window and a list of top-level windows
//! - A [`DesktopHeap`] for memory budget tracking
//! - Flags controlling active/locked/secure state
//!
//! The [`DesktopManager`] is the central orchestrator:
//! - Creates and destroys stations and desktops
//! - Tracks the active desktop and handles switching
//! - Manages thread-to-desktop assignments
//! - Implements the secure desktop pattern (input locking)
//!
//! [`DesktopSecurity`] provides per-thread access control: threads can only
//! interact with their assigned desktop unless explicitly granted access to
//! others.

pub mod atom_table;
pub mod clipboard;
pub mod desktop;
pub mod error;
pub mod heap;
pub mod manager;
pub mod security;
pub mod station;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export primary types at crate root.
pub use atom_table::AtomTable;
pub use clipboard::ClipboardData;
pub use desktop::Desktop;
pub use error::DesktopError;
pub use heap::DesktopHeap;
pub use manager::DesktopManager;
pub use security::{DesktopAccess, DesktopFlags, DesktopSecurity, WindowStationFlags};
pub use station::WindowStation;
pub use types::{Atom, DesktopId, WindowId, WindowStationId};
