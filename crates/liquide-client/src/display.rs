//! Display modes, monitor management, and seamless window tracking.

use std::collections::HashMap;
use std::fmt;

/// How the remote desktop is presented locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    SingleWindow,
    Fullscreen,
    Tabbed,
    MultiWindow,
    Seamless,
}

impl fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::SingleWindow => "SingleWindow",
            Self::Fullscreen => "Fullscreen",
            Self::Tabbed => "Tabbed",
            Self::MultiWindow => "MultiWindow",
            Self::Seamless => "Seamless",
        };
        f.write_str(label)
    }
}

/// Strategy for mapping remote monitors to local ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorStrategy {
    MatchLocal,
    SingleMonitor,
    Custom,
}

/// Describes a single physical monitor.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub primary: bool,
    pub name: String,
}

/// A remote application window rendered as a native local window.
#[derive(Debug, Clone)]
pub struct SeamlessWindow {
    pub window_id: u64,
    pub app_id: String,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
    pub focused: bool,
}

/// Manages display modes, monitors, and seamless windows.
pub struct DisplayManager {
    mode: DisplayMode,
    monitors: Vec<MonitorInfo>,
    active_monitor_id: Option<u32>,
    fullscreen_monitor_id: Option<u32>,
    seamless_windows: HashMap<u64, SeamlessWindow>,
}

impl DisplayManager {
    /// Create a new display manager with the given initial mode.
    #[must_use]
    pub fn new(mode: DisplayMode) -> Self {
        Self {
            mode,
            monitors: Vec::new(),
            active_monitor_id: None,
            fullscreen_monitor_id: None,
            seamless_windows: HashMap::new(),
        }
    }

    /// Switch display mode.
    pub fn set_mode(&mut self, mode: DisplayMode) {
        self.mode = mode;
    }

    /// Current display mode.
    #[must_use]
    pub fn current_mode(&self) -> DisplayMode {
        self.mode
    }

    /// Register a local monitor.
    pub fn add_monitor(&mut self, info: MonitorInfo) {
        if info.primary && self.active_monitor_id.is_none() {
            self.active_monitor_id = Some(info.id);
        }
        self.monitors.push(info);
    }

    /// Remove a monitor by id. Returns `true` if found and removed.
    pub fn remove_monitor(&mut self, id: u32) -> bool {
        let before = self.monitors.len();
        self.monitors.retain(|m| m.id != id);
        if self.active_monitor_id == Some(id) {
            self.active_monitor_id = self.monitors.first().map(|m| m.id);
        }
        self.monitors.len() < before
    }

    /// The currently active monitor, if any.
    #[must_use]
    pub fn active_monitor(&self) -> Option<&MonitorInfo> {
        let id = self.active_monitor_id?;
        self.monitors.iter().find(|m| m.id == id)
    }

    /// All known monitors.
    #[must_use]
    pub fn monitors(&self) -> &[MonitorInfo] {
        &self.monitors
    }

    /// Request a resolution change for the active monitor.
    /// Returns `true` if the monitor was found and updated.
    pub fn request_resolution(&mut self, monitor_id: u32, width: u32, height: u32) -> bool {
        if let Some(m) = self.monitors.iter_mut().find(|m| m.id == monitor_id) {
            m.width = width;
            m.height = height;
            true
        } else {
            false
        }
    }

    /// Create and track a new seamless window.
    pub fn create_seamless_window(&mut self, window: SeamlessWindow) {
        self.seamless_windows.insert(window.window_id, window);
    }

    /// Destroy a seamless window by id. Returns `true` if found.
    pub fn destroy_seamless_window(&mut self, window_id: u64) -> bool {
        self.seamless_windows.remove(&window_id).is_some()
    }

    /// Number of tracked seamless windows.
    #[must_use]
    pub fn seamless_window_count(&self) -> usize {
        self.seamless_windows.len()
    }
}
