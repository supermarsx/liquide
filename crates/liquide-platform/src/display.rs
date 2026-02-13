//! Display and monitor enumeration.
//!
//! Provides the [`DisplayBackend`] trait for querying connected monitors
//! and a [`NullDisplayBackend`] that returns empty results for testing.

use liquide_compositor::geometry::Rect;
use serde::{Deserialize, Serialize};

/// Information about a connected monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Unique identifier for this monitor.
    pub id: u32,
    /// Human-readable name (e.g. "HDMI-1").
    pub name: String,
    /// Full geometry of the monitor in virtual screen coordinates.
    pub geometry: Rect,
    /// Usable work area excluding taskbars / panels.
    pub work_area: Rect,
    /// DPI scaling factor (1.0 = 96 DPI).
    pub dpi_scale: f32,
    /// Whether this is the primary monitor.
    pub primary: bool,
    /// Refresh rate in hertz.
    pub refresh_rate_hz: u32,
}

/// Backend for querying display / monitor information.
pub trait DisplayBackend: Send {
    /// Return information about all connected monitors.
    fn monitors(&self) -> Vec<MonitorInfo>;

    /// Return information about the primary monitor, if any.
    fn primary_monitor(&self) -> Option<MonitorInfo>;

    /// Return the bounding rectangle of the entire virtual screen
    /// (the union of all monitors).
    fn virtual_screen_rect(&self) -> Rect;
}

/// A [`DisplayBackend`] that reports no monitors.
#[derive(Debug, Default)]
pub struct NullDisplayBackend;

impl DisplayBackend for NullDisplayBackend {
    fn monitors(&self) -> Vec<MonitorInfo> {
        Vec::new()
    }

    fn primary_monitor(&self) -> Option<MonitorInfo> {
        None
    }

    fn virtual_screen_rect(&self) -> Rect {
        Rect::ZERO
    }
}
