//! Per-monitor DPI tracking.
//!
//! [`MonitorDpi`] maintains a registry of monitor IDs to DPI scale factors,
//! supporting multi-monitor setups where each display may have a different
//! pixel density.

use crate::scale::DpiScale;
use std::collections::HashMap;

/// Unique identifier for a monitor / display output.
pub type MonitorId = u32;

/// Per-monitor DPI registry.
///
/// Tracks the scale factor for each connected monitor. A primary monitor
/// is designated and used as the default when no specific monitor is requested.
#[derive(Debug, Clone)]
pub struct MonitorDpi {
    /// Map from monitor id to its current DPI scale.
    scales: HashMap<MonitorId, DpiScale>,
    /// The id of the primary monitor (if one has been registered).
    primary_id: Option<MonitorId>,
}

impl MonitorDpi {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            scales: HashMap::new(),
            primary_id: None,
        }
    }

    /// Create a registry with a single primary monitor at the given scale.
    pub fn with_primary(id: MonitorId, scale: DpiScale) -> Self {
        let mut scales = HashMap::new();
        scales.insert(id, scale);
        Self {
            scales,
            primary_id: Some(id),
        }
    }

    /// Register or update a monitor's scale factor.
    ///
    /// Returns the previous scale factor if this monitor was already tracked,
    /// or `None` if it's newly registered.
    pub fn set(&mut self, id: MonitorId, scale: DpiScale) -> Option<DpiScale> {
        let prev = self.scales.insert(id, scale);
        // If this is the first monitor, make it primary by default.
        if self.primary_id.is_none() {
            self.primary_id = Some(id);
        }
        prev
    }

    /// Remove a monitor from tracking (e.g., when disconnected).
    ///
    /// Returns the scale factor if the monitor was tracked.
    pub fn remove(&mut self, id: MonitorId) -> Option<DpiScale> {
        let removed = self.scales.remove(&id);
        if self.primary_id == Some(id) {
            // Pick any remaining monitor as the new primary.
            self.primary_id = self.scales.keys().next().copied();
        }
        removed
    }

    /// Designate a monitor as the primary display.
    ///
    /// Returns `false` if the monitor is not tracked.
    pub fn set_primary(&mut self, id: MonitorId) -> bool {
        if self.scales.contains_key(&id) {
            self.primary_id = Some(id);
            true
        } else {
            false
        }
    }

    /// Get the scale factor for the primary monitor.
    ///
    /// Returns `DpiScale::identity()` if no monitors are tracked.
    pub fn primary(&self) -> DpiScale {
        self.primary_id
            .and_then(|id| self.scales.get(&id).copied())
            .unwrap_or(DpiScale::identity())
    }

    /// Get the ID of the primary monitor, if any.
    pub fn primary_id(&self) -> Option<MonitorId> {
        self.primary_id
    }

    /// Get the scale factor for a specific monitor.
    ///
    /// Returns `None` if the monitor is not tracked.
    pub fn for_monitor(&self, id: MonitorId) -> Option<DpiScale> {
        self.scales.get(&id).copied()
    }

    /// Get the scale factor for a specific monitor, falling back to the
    /// primary monitor's scale, then to `DpiScale::identity()`.
    pub fn for_monitor_or_primary(&self, id: MonitorId) -> DpiScale {
        self.for_monitor(id).unwrap_or_else(|| self.primary())
    }

    /// The number of tracked monitors.
    pub fn count(&self) -> usize {
        self.scales.len()
    }

    /// Whether any monitors are tracked.
    pub fn is_empty(&self) -> bool {
        self.scales.is_empty()
    }

    /// Iterator over all (monitor_id, scale) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (MonitorId, DpiScale)> + '_ {
        self.scales.iter().map(|(&id, &scale)| (id, scale))
    }

    /// The maximum scale factor across all tracked monitors.
    ///
    /// Useful for allocating shared resources (e.g., texture atlases) that
    /// must be large enough for the highest-DPI display.
    pub fn max_scale(&self) -> DpiScale {
        self.scales
            .values()
            .copied()
            .max_by(|a, b| a.factor().total_cmp(&b.factor()))
            .unwrap_or(DpiScale::identity())
    }

    /// The minimum scale factor across all tracked monitors.
    pub fn min_scale(&self) -> DpiScale {
        self.scales
            .values()
            .copied()
            .min_by(|a, b| a.factor().total_cmp(&b.factor()))
            .unwrap_or(DpiScale::identity())
    }

    /// Whether all monitors share the same scale factor.
    pub fn is_uniform(&self) -> bool {
        if self.scales.len() <= 1 {
            return true;
        }
        let mut it = self.scales.values();
        let first = it.next().unwrap().factor();
        it.all(|s| (s.factor() - first).abs() < f32::EPSILON)
    }
}

impl Default for MonitorDpi {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for objects that need to respond to DPI changes.
///
/// Implement this on UI components, renderers, or layout engines that cache
/// pixel-dependent state and must invalidate/re-layout when the DPI changes.
pub trait DpiAware {
    /// Called when the DPI scale factor changes.
    ///
    /// `old_scale` is the previous scale factor and `new_scale` is the new one.
    /// Implementors should re-compute any cached pixel measurements, resize
    /// buffers, and request a re-layout/re-paint as needed.
    fn on_dpi_changed(&mut self, old_scale: DpiScale, new_scale: DpiScale);
}
