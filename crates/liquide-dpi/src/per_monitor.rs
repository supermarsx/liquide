//! Per-monitor DPI management.
//!
//! Extends the basic [`MonitorDpi`](crate::MonitorDpi) registry with higher-level
//! window-to-monitor scale resolution, change notifications, and multi-monitor
//! awareness.

use crate::geometry::LogicalRect;
use crate::monitor::MonitorId;
use std::collections::HashMap;

/// Detailed per-monitor scale information.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorScale {
    /// Unique monitor identifier.
    pub monitor_id: MonitorId,
    /// The UI scale factor (e.g. 1.0, 1.5, 2.0).
    pub scale_factor: f64,
    /// The physical DPI reported by the display hardware (e.g. 96, 144, 192).
    pub physical_dpi: f64,
}

impl MonitorScale {
    /// Create a new `MonitorScale`.
    pub fn new(monitor_id: MonitorId, scale_factor: f64, physical_dpi: f64) -> Self {
        Self {
            monitor_id,
            scale_factor: scale_factor.clamp(0.5, 8.0),
            physical_dpi: physical_dpi.max(1.0),
        }
    }

    /// Whether this monitor is HiDPI (scale > 1.0).
    #[inline]
    pub fn is_hidpi(&self) -> bool {
        self.scale_factor > 1.0
    }
}

impl std::fmt::Display for MonitorScale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Monitor {} ({:.2}x, {:.0} DPI)",
            self.monitor_id, self.scale_factor, self.physical_dpi
        )
    }
}

/// Events emitted when scaling changes.
#[derive(Debug, Clone, PartialEq)]
pub enum ScaleEvent {
    /// A monitor's scale factor changed (e.g. user changed settings).
    MonitorScaleChanged {
        monitor_id: MonitorId,
        old_scale: f64,
        new_scale: f64,
    },
    /// A window moved to a different monitor (or spans monitors differently).
    WindowMoved {
        window_id: u64,
        old_scale: f64,
        new_scale: f64,
        target_monitor: MonitorId,
    },
    /// The global/fallback scale was changed.
    GlobalScaleChanged {
        old_scale: f64,
        new_scale: f64,
    },
}

/// Manages per-monitor scale factors and resolves window-to-monitor scale mapping.
///
/// Tracks monitor geometries (bounds) alongside their scale factors so that
/// `scale_for_window` can determine which monitor "owns" a window by computing
/// overlap area.
#[derive(Debug, Clone)]
pub struct ScaleManager {
    /// Per-monitor scale info, keyed by monitor ID.
    monitors: HashMap<MonitorId, MonitorEntry>,
    /// Global/fallback scale factor.
    global_scale: f64,
    /// Collected events since last drain.
    pending_events: Vec<ScaleEvent>,
}

/// Internal entry for a tracked monitor.
#[derive(Debug, Clone, Copy)]
struct MonitorEntry {
    scale: MonitorScale,
    bounds: LogicalRect,
}

impl ScaleManager {
    /// Create a new `ScaleManager` with a global fallback scale of 1.0.
    pub fn new() -> Self {
        Self {
            monitors: HashMap::new(),
            global_scale: 1.0,
            pending_events: Vec::new(),
        }
    }

    /// Register a monitor with its scale and screen bounds (in logical pixels).
    pub fn add_monitor(&mut self, scale: MonitorScale, bounds: LogicalRect) {
        self.monitors.insert(
            scale.monitor_id,
            MonitorEntry { scale, bounds },
        );
    }

    /// Remove a monitor from tracking.
    pub fn remove_monitor(&mut self, id: MonitorId) -> Option<MonitorScale> {
        self.monitors.remove(&id).map(|e| e.scale)
    }

    /// Get the scale factor for a specific monitor.
    ///
    /// Returns the global fallback if the monitor is not tracked.
    pub fn scale_for_monitor(&self, id: MonitorId) -> f64 {
        self.monitors
            .get(&id)
            .map(|e| e.scale.scale_factor)
            .unwrap_or(self.global_scale)
    }

    /// Get the full [`MonitorScale`] for a monitor, if tracked.
    pub fn monitor_info(&self, id: MonitorId) -> Option<&MonitorScale> {
        self.monitors.get(&id).map(|e| &e.scale)
    }

    /// Determine the effective scale factor for a window based on which monitor
    /// contains the most area of the window rectangle.
    ///
    /// If the window doesn't overlap any monitor, returns the global fallback.
    pub fn scale_for_window(&self, window_rect: LogicalRect) -> f64 {
        let mut best_area: f32 = 0.0;
        let mut best_scale = self.global_scale;

        let mut best_monitor_id: MonitorId = 0;

        for entry in self.monitors.values() {
            if let Some(overlap) = window_rect.intersection(entry.bounds) {
                let area = overlap.area();
                // On tie, prefer the monitor with the lower ID for determinism.
                if area > best_area
                    || (area == best_area && entry.scale.monitor_id < best_monitor_id)
                {
                    best_area = area;
                    best_scale = entry.scale.scale_factor;
                    best_monitor_id = entry.scale.monitor_id;
                }
            }
        }

        best_scale
    }

    /// Determine which monitor owns the most area of the window.
    ///
    /// Returns `None` if no monitors overlap.
    pub fn owning_monitor(&self, window_rect: LogicalRect) -> Option<MonitorId> {
        let mut best_area: f32 = 0.0;
        let mut best_id: Option<MonitorId> = None;

        for entry in self.monitors.values() {
            if let Some(overlap) = window_rect.intersection(entry.bounds) {
                let area = overlap.area();
                // On tie, prefer the monitor with the lower ID for determinism.
                if area > best_area
                    || (area == best_area
                        && best_id.is_some_and(|id| entry.scale.monitor_id < id))
                {
                    best_area = area;
                    best_id = Some(entry.scale.monitor_id);
                }
            }
        }

        best_id
    }

    /// Notify that a monitor's scale factor changed.
    ///
    /// Updates the internal state and queues a [`ScaleEvent::MonitorScaleChanged`].
    pub fn on_monitor_change(&mut self, id: MonitorId, new_scale: f64) {
        if let Some(entry) = self.monitors.get_mut(&id) {
            let old_scale = entry.scale.scale_factor;
            if (old_scale - new_scale).abs() > 1e-9 {
                entry.scale.scale_factor = new_scale.clamp(0.5, 8.0);
                self.pending_events.push(ScaleEvent::MonitorScaleChanged {
                    monitor_id: id,
                    old_scale,
                    new_scale: entry.scale.scale_factor,
                });
            }
        }
    }

    /// Set the global/fallback scale factor.
    ///
    /// Queues a [`ScaleEvent::GlobalScaleChanged`] if the value changed.
    pub fn set_global_scale(&mut self, scale: f64) {
        let old = self.global_scale;
        let new = scale.clamp(0.5, 8.0);
        if (old - new).abs() > 1e-9 {
            self.global_scale = new;
            self.pending_events.push(ScaleEvent::GlobalScaleChanged {
                old_scale: old,
                new_scale: new,
            });
        }
    }

    /// The current global/fallback scale.
    #[inline]
    pub fn global_scale(&self) -> f64 {
        self.global_scale
    }

    /// Drain all pending scale events.
    pub fn drain_events(&mut self) -> Vec<ScaleEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// The number of tracked monitors.
    #[inline]
    pub fn monitor_count(&self) -> usize {
        self.monitors.len()
    }

    /// Iterator over all tracked monitors.
    pub fn monitors(&self) -> impl Iterator<Item = &MonitorScale> + '_ {
        self.monitors.values().map(|e| &e.scale)
    }

    /// Update the bounds for a monitor (e.g. after resolution change or rearrangement).
    pub fn set_monitor_bounds(&mut self, id: MonitorId, bounds: LogicalRect) {
        if let Some(entry) = self.monitors.get_mut(&id) {
            entry.bounds = bounds;
        }
    }
}

impl Default for ScaleManager {
    fn default() -> Self {
        Self::new()
    }
}
