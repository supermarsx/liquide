//! Scene-graph-based status bar used by the shell at runtime.
//!
//! This is the status bar implementation that the shell uses to produce
//! `SceneNode` output for the compositor, as opposed to the painter-based
//! `StatusBar` in [`crate::status_bar`].

use std::fmt;

use liquide_compositor::geometry::Rect;
use liquide_datetime::{ClockFormat, ClockSettings, DateTime};
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
            height: 34.0,
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
    /// UTC offset in minutes applied to the clock item (e.g. -300 for UTC-5).
    /// Defaults to 0 (UTC). The shell sets this from `liquide-datetime`.
    clock_offset_minutes: i32,
    /// Whether the clock uses 24-hour formatting (true) or 12-hour + AM/PM (false).
    clock_24h: bool,
    /// Whether the clock shows seconds.
    clock_show_seconds: bool,
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

        // Seed the clock with the system's current local UTC offset so the
        // clock renders in local time without the shell having to call
        // `set_clock_offset_minutes` immediately on construction. Falls back
        // to UTC (0) when the platform query fails.
        let clock_offset_minutes =
            liquide_datetime::PlatformTimeBridge::get_utc_offset_minutes().unwrap_or(0);

        Self {
            config,
            items,
            dirty: true,
            clock_offset_minutes,
            clock_24h: true,
            clock_show_seconds: false,
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

    /// UTC offset in minutes used to render the clock item in local time.
    #[must_use]
    pub fn clock_offset_minutes(&self) -> i32 {
        self.clock_offset_minutes
    }

    /// Set the UTC offset in minutes for the clock item (e.g. `-300` for EST).
    pub fn set_clock_offset_minutes(&mut self, offset: i32) {
        if self.clock_offset_minutes != offset {
            self.clock_offset_minutes = offset;
            self.dirty = true;
        }
    }

    /// Whether the clock is rendered in 24-hour format.
    #[must_use]
    pub fn clock_24h(&self) -> bool {
        self.clock_24h
    }

    /// Configure 24-hour vs 12-hour clock formatting.
    pub fn set_clock_24h(&mut self, enabled: bool) {
        if self.clock_24h != enabled {
            self.clock_24h = enabled;
            self.dirty = true;
        }
    }

    /// Whether the clock shows seconds.
    #[must_use]
    pub fn clock_show_seconds(&self) -> bool {
        self.clock_show_seconds
    }

    /// Toggle whether the clock shows seconds.
    pub fn set_clock_show_seconds(&mut self, show: bool) {
        if self.clock_show_seconds != show {
            self.clock_show_seconds = show;
            self.dirty = true;
        }
    }

    /// Format a wall-clock timestamp using the status bar's active clock settings.
    #[must_use]
    pub fn format_clock_timestamp(&self, timestamp_us: u64, format: &str) -> String {
        let total_secs = (timestamp_us / 1_000_000) as i64;
        let dt_utc = DateTime::from_unix_timestamp(total_secs);
        let dt_local = dt_utc.with_offset_minutes(self.clock_offset_minutes());
        let settings = ClockSettings {
            format: if self.clock_24h() {
                ClockFormat::H24
            } else if format.contains('%') {
                ClockFormat::Custom(format.to_string())
            } else {
                ClockFormat::H12
            },
            show_seconds: self.clock_show_seconds(),
            show_date: false,
            timezone: String::new(),
        };
        settings.format_time(&dt_local)
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
    /// The bar spans the full width of `screen` and sits at the screen's
    /// top edge with a height taken from the configuration.
    #[must_use]
    pub fn compute_bounds(&self, screen: Rect) -> Rect {
        Rect::new(screen.x, screen.y, screen.width, self.config.height)
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
