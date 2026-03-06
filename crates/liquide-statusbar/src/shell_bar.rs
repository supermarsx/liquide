//! Scene-graph-based status bar used by the shell at runtime.
//!
//! This is the status bar implementation that the shell uses to produce
//! `SceneNode` output for the compositor, as opposed to the painter-based
//! `StatusBar` in [`crate::status_bar`].

use std::fmt;

use liquide_compositor::geometry::Rect;
use serde::{Deserialize, Serialize};

use crate::items::{StatusBarItem, StatusBarItemKind};
use crate::slot::StatusBarSlot;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Persistent configuration for the shell's scene-graph status bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellBarConfig {
    /// Whether the status bar is shown at all.
    pub enabled: bool,
    /// Height in logical pixels.
    pub height: f32,
    /// Show the clock widget.
    pub show_clock: bool,
    /// `strftime`-style format string for the clock.
    pub clock_format: String,
    /// Show the system-tray area.
    pub show_tray: bool,
    /// Show the notification indicator.
    pub show_notifications: bool,
    /// Show the connection quality badge.
    pub show_connection_quality: bool,
    /// macOS-style app menu on the left.
    pub show_app_menu: bool,
    /// Auto-hide when a window is maximized.
    pub auto_hide_on_maximize: bool,
    /// Hover reveal distance from top edge (pixels).
    pub auto_hide_reveal_distance: f32,
}

impl Default for ShellBarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            height: 28.0,
            show_clock: true,
            clock_format: "%H:%M".into(),
            show_tray: true,
            show_notifications: true,
            show_connection_quality: true,
            show_app_menu: true,
            auto_hide_on_maximize: true,
            auto_hide_reveal_distance: 5.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

/// Runtime state for the shell's scene-graph status bar.
///
/// Holds the active configuration, the set of items, and a dirty flag that
/// signals the compositor to re-render the bar.
pub struct ShellStatusBar {
    config: ShellBarConfig,
    items: Vec<StatusBarItem>,
    dirty: bool,
}

impl ShellStatusBar {
    /// Create a new status bar, pre-populating default items according to
    /// the supplied configuration.
    #[must_use]
    pub fn new(config: ShellBarConfig) -> Self {
        let mut items = Vec::new();

        if config.show_clock {
            items.push(StatusBarItem {
                id: "clock".into(),
                kind: StatusBarItemKind::Clock {
                    format: config.clock_format.clone(),
                },
                slot: StatusBarSlot::Center,
                visible: true,
                cached: false,
                last_update_us: 0,
            });
        }

        if config.show_notifications {
            items.push(StatusBarItem {
                id: "notifications".into(),
                kind: StatusBarItemKind::NotificationIndicator {
                    unread_count: 0,
                    dnd_active: false,
                },
                slot: StatusBarSlot::Right,
                visible: true,
                cached: false,
                last_update_us: 0,
            });
        }

        if config.show_connection_quality {
            items.push(StatusBarItem {
                id: "connection".into(),
                kind: StatusBarItemKind::ConnectionQuality {
                    quality_percent: 100,
                    latency_ms: 0,
                },
                slot: StatusBarSlot::Right,
                visible: true,
                cached: false,
                last_update_us: 0,
            });
        }

        if config.show_tray {
            items.push(StatusBarItem {
                id: "tray".into(),
                kind: StatusBarItemKind::TrayArea,
                slot: StatusBarSlot::Right,
                visible: true,
                cached: false,
                last_update_us: 0,
            });
        }

        // Session / power button always present on the far right.
        items.push(StatusBarItem {
            id: "session".into(),
            kind: StatusBarItemKind::SessionButton,
            slot: StatusBarSlot::Right,
            visible: true,
            cached: false,
            last_update_us: 0,
        });

        Self {
            config,
            items,
            dirty: true,
        }
    }

    /// Append a custom item and mark the bar dirty.
    pub fn add_item(&mut self, item: StatusBarItem) {
        self.items.push(item);
        self.dirty = true;
    }

    /// Remove the item with the given `id`.
    ///
    /// Returns `true` if an item was found and removed.
    pub fn remove_item(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        let removed = self.items.len() < before;
        if removed {
            self.dirty = true;
        }
        removed
    }

    /// Replace the kind (payload) of an existing item and mark dirty.
    pub fn update_item_kind(&mut self, id: &str, kind: StatusBarItemKind) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.kind = kind;
            item.cached = false;
            item.last_update_us = item.last_update_us.wrapping_add(1);
            self.dirty = true;
        }
    }

    /// Refresh the clock timestamp without changing its format string.
    pub fn update_clock(&mut self, timestamp_us: u64) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == "clock") {
            item.last_update_us = timestamp_us;
            item.cached = false;
            self.dirty = true;
        }
    }

    /// Update the unread notification count.
    pub fn update_notification_count(&mut self, count: u32) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == "notifications") {
            if let StatusBarItemKind::NotificationIndicator {
                ref mut unread_count,
                ..
            } = item.kind
            {
                *unread_count = count;
                item.cached = false;
                self.dirty = true;
            }
        }
    }

    /// Update the connection quality readout.
    pub fn update_connection_quality(&mut self, percent: u8, latency: u32) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == "connection") {
            if let StatusBarItemKind::ConnectionQuality {
                ref mut quality_percent,
                ref mut latency_ms,
            } = item.kind
            {
                *quality_percent = percent;
                *latency_ms = latency;
                item.cached = false;
                self.dirty = true;
            }
        }
    }

    /// Toggle do-not-disturb mode on the notification indicator.
    pub fn set_dnd(&mut self, active: bool) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == "notifications") {
            if let StatusBarItemKind::NotificationIndicator {
                ref mut dnd_active, ..
            } = item.kind
            {
                *dnd_active = active;
                item.cached = false;
                self.dirty = true;
            }
        }
    }

    /// Whether the bar has pending visual changes.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag and mark every item as cached.
    pub fn mark_clean(&mut self) {
        self.dirty = false;
        for item in &mut self.items {
            item.cached = true;
        }
    }

    /// All items in insertion order.
    #[must_use]
    pub fn items(&self) -> &[StatusBarItem] {
        &self.items
    }

    /// Items that belong to the given slot.
    #[must_use]
    pub fn items_in_slot(&self, slot: StatusBarSlot) -> Vec<&StatusBarItem> {
        self.items.iter().filter(|i| i.slot == slot).collect()
    }

    /// Total number of items.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Look up an item by its unique id.
    #[must_use]
    pub fn find_item(&self, id: &str) -> Option<&StatusBarItem> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Compute the screen-space bounds for the status bar.
    ///
    /// The bar spans the full width of `screen` and sits at the top edge
    /// (`y = 0`) with a height taken from the configuration.
    #[must_use]
    pub fn compute_bounds(&self, screen: Rect) -> Rect {
        Rect::new(screen.x, 0.0, screen.width, self.config.height)
    }

    /// The active configuration.
    #[must_use]
    pub fn config(&self) -> &ShellBarConfig {
        &self.config
    }

    /// Whether the status bar is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

impl fmt::Display for ShellStatusBar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StatusBar({} items, {})",
            self.items.len(),
            if self.dirty { "dirty" } else { "clean" },
        )
    }
}
