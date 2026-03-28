//! Desktop — an isolated window hierarchy within a window station.
//!
//! Each desktop contains its own window tree rooted at `root_window`, tracks
//! the foreground window, and has an associated memory heap budget. Desktops
//! can be flagged as secure (for lock screens, UAC prompts) which prevents
//! input from reaching other desktops.

use crate::heap::{DesktopHeap, DEFAULT_INTERACTIVE_HEAP_BUDGET};
use crate::security::DesktopFlags;
use crate::types::{DesktopId, WindowId, WindowStationId};

/// A desktop — an isolated window hierarchy.
#[derive(Debug, Clone)]
pub struct Desktop {
    /// Unique desktop ID.
    pub id: DesktopId,
    /// Human-readable name (e.g. "Default", "Winlogon").
    pub name: String,
    /// The station this desktop belongs to.
    pub station_id: WindowStationId,
    /// Root (background) window of this desktop.
    pub root_window: WindowId,
    /// Desktop state flags.
    pub flags: DesktopFlags,
    /// Memory usage tracking.
    pub heap: DesktopHeap,
    /// The window currently in the foreground, if any.
    pub foreground_window: Option<WindowId>,
    /// All top-level windows on this desktop, in z-order (front to back).
    pub windows: Vec<WindowId>,
}

impl Desktop {
    /// Creates a new desktop with default flags and the given heap budget.
    pub fn new(
        id: DesktopId,
        name: String,
        station_id: WindowStationId,
        root_window: WindowId,
        heap_budget: usize,
    ) -> Self {
        Self {
            id,
            name,
            station_id,
            root_window,
            flags: DesktopFlags::ALLOW_INPUT,
            heap: DesktopHeap::new(id, heap_budget),
            foreground_window: None,
            windows: vec![root_window],
        }
    }

    /// Creates a new desktop with the default interactive heap budget.
    pub fn new_interactive(
        id: DesktopId,
        name: String,
        station_id: WindowStationId,
        root_window: WindowId,
    ) -> Self {
        Self::new(id, name, station_id, root_window, DEFAULT_INTERACTIVE_HEAP_BUDGET)
    }

    /// Returns `true` if this desktop is currently active (visible).
    pub fn is_active(&self) -> bool {
        self.flags.contains(DesktopFlags::ACTIVE)
    }

    /// Returns `true` if this is a secure desktop.
    pub fn is_secure(&self) -> bool {
        self.flags.contains(DesktopFlags::SECURE)
    }

    /// Returns `true` if this desktop is locked (input-exclusive).
    pub fn is_locked(&self) -> bool {
        self.flags.contains(DesktopFlags::LOCKED)
    }

    /// Returns `true` if this desktop accepts input.
    pub fn allows_input(&self) -> bool {
        self.flags.contains(DesktopFlags::ALLOW_INPUT)
    }

    /// Adds a window to this desktop's window list.
    pub fn add_window(&mut self, window: WindowId) {
        if !self.windows.contains(&window) {
            self.windows.push(window);
        }
    }

    /// Removes a window from this desktop. If it was the foreground window,
    /// foreground is cleared.
    pub fn remove_window(&mut self, window: WindowId) {
        self.windows.retain(|&w| w != window);
        if self.foreground_window == Some(window) {
            self.foreground_window = None;
        }
    }

    /// Sets the foreground window. The window must belong to this desktop.
    pub fn set_foreground(&mut self, window: WindowId) -> bool {
        if self.windows.contains(&window) {
            self.foreground_window = Some(window);
            true
        } else {
            false
        }
    }

    /// Returns the number of top-level windows (including the root).
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }
}

/// Well-known desktop names.
pub const DESKTOP_DEFAULT: &str = "Default";
pub const DESKTOP_WINLOGON: &str = "Winlogon";
pub const DESKTOP_SCREENSAVER: &str = "Screensaver";
pub const DESKTOP_DISCONNECT: &str = "Disconnect";
