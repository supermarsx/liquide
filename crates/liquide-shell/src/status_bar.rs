//! Status bar system for the shell surface.
//!
//! The status bar occupies a thin strip at the top of the screen and hosts
//! slots for the clock, notification indicator, connection quality badge,
//! system tray, and arbitrary plugin-provided widgets.

use std::fmt;

use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::SceneNode;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Slot
// ---------------------------------------------------------------------------

/// Horizontal slot within the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatusBarSlot {
    Left,
    Center,
    Right,
}

impl fmt::Display for StatusBarSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Left => write!(f, "Left"),
            Self::Center => write!(f, "Center"),
            Self::Right => write!(f, "Right"),
        }
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Persistent configuration for the status bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusBarConfig {
    /// Whether the status bar is shown at all.
    pub enabled: bool,
    /// Height in logical pixels.
    pub height: u32,
    /// Show the clock widget.
    pub show_clock: bool,
    /// `strftime`-style format string for the clock.
    pub clock_format: String,
    /// Show the system-tray area.
    pub show_tray: bool,
    /// Show the notification indicator.
    pub show_notification_indicator: bool,
    /// Show the connection quality badge.
    pub show_connection_quality: bool,
    /// macOS-style app menu on the left.
    pub show_app_menu: bool,
    /// Auto-hide when a window is maximized.
    pub auto_hide_on_maximize: bool,
    /// Hover reveal distance from top edge (pixels).
    pub auto_hide_reveal_distance: f32,
}

impl Default for StatusBarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            height: 28,
            show_clock: true,
            clock_format: "%H:%M".into(),
            show_tray: true,
            show_notification_indicator: true,
            show_connection_quality: true,
            show_app_menu: true,
            auto_hide_on_maximize: true,
            auto_hide_reveal_distance: 5.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Item kind
// ---------------------------------------------------------------------------

/// The payload carried by a status bar widget.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StatusBarItemKind {
    /// A clock displaying the current time.
    Clock { format: String },
    /// Unread-notification badge and do-not-disturb state.
    NotificationIndicator { unread_count: u32, dnd_active: bool },
    /// Network round-trip quality indicator.
    ConnectionQuality { quality_percent: u8, latency_ms: u32 },
    /// System tray area for background services.
    TrayArea,
    /// Plugin-provided custom content.
    Custom { plugin_id: String, content: String },
    /// Session / power button (shutdown, lock, etc.).
    SessionButton,
}

// ---------------------------------------------------------------------------
// Item
// ---------------------------------------------------------------------------

/// A single widget placed inside the status bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusBarItem {
    /// Unique identifier (e.g. `"clock"`, `"notifications"`).
    pub id: String,
    /// Payload / variant data.
    pub kind: StatusBarItemKind,
    /// Which horizontal slot this item occupies.
    pub slot: StatusBarSlot,
    /// Whether the item should be rendered.
    pub visible: bool,
    /// Whether the last rendered frame is still valid.
    pub cached: bool,
    /// Monotonic timestamp of the last content update (microseconds).
    pub last_update_us: u64,
}

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

/// Runtime state for the status bar.
///
/// Holds the active configuration, the set of items, and a dirty flag that
/// signals the compositor to re-render the bar.
pub struct ShellStatusBar {
    config: StatusBarConfig,
    items: Vec<StatusBarItem>,
    dirty: bool,
}

impl ShellStatusBar {
    /// Create a new status bar, pre-populating default items according to
    /// the supplied configuration.
    #[must_use]
    pub fn new(config: StatusBarConfig) -> Self {
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

        if config.show_notification_indicator {
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
        Rect::new(screen.x, 0.0, screen.width, self.config.height as f32)
    }

    /// The active configuration.
    #[must_use]
    pub fn config(&self) -> &StatusBarConfig {
        &self.config
    }

    /// Whether the status bar is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Build the scene graph for the status bar.
    pub fn build_scene(&self, screen: Rect, theme: &crate::theme::ShellTheme) -> SceneNode {
        use crate::scene_builder::{
            icon_node, solid_rect, text_node,
            NODE_STATUS_BAR, NODE_STATUS_BAR_ITEM_BASE,
        };
        use liquide_compositor::geometry::Rect as GRect;
        use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNode, SceneNodeKind};

        if !self.config.enabled {
            return SceneNode::new(
                NODE_STATUS_BAR,
                SceneNodeKind::Overlay,
                NodeProperties::new(GRect::ZERO).with_visible(false),
            );
        }

        let bar_bounds = self.compute_bounds(screen);
        let mut bar_node = SceneNode::new(
            NODE_STATUS_BAR,
            SceneNodeKind::Glass(GlassParams {
                blur_radius: 15,
                tint_color: theme.status_bar_glass_tint,
                inner_glow: false,
                parallax: false,
            }),
            NodeProperties::new(bar_bounds).with_z_order(950),
        );

        // Use parent-relative coordinates for child items so that
        // walk_inner's translation from bar_bounds doesn't double-offset them.
        let padding = 8.0_f32;
        let item_height = bar_bounds.height - 4.0;
        let item_y = 2.0; // relative to bar top
        let mut left_x = padding;
        let center_x = bar_bounds.width / 2.0;
        let mut right_x = bar_bounds.width - padding;

        // 2px accent border at the bottom edge of the status bar.
        let border_rect =
            GRect::new(0.0, bar_bounds.height - 2.0, bar_bounds.width, 2.0);
        bar_node.add_child(solid_rect(
            NODE_STATUS_BAR + 1,
            theme.status_bar_border,
            border_rect,
            952,
        ));

        for (i, item) in self.items.iter().enumerate() {
            if !item.visible {
                continue;
            }
            let item_id = NODE_STATUS_BAR_ITEM_BASE + i as u64;
            let item_width = match &item.kind {
                StatusBarItemKind::Clock { .. } => 120.0_f32,
                StatusBarItemKind::NotificationIndicator { .. } => 30.0,
                StatusBarItemKind::ConnectionQuality { .. } => 40.0,
                StatusBarItemKind::TrayArea => 80.0,
                StatusBarItemKind::Custom { .. } => 60.0,
                StatusBarItemKind::SessionButton => 28.0,
            };

            let ix = match item.slot {
                StatusBarSlot::Left => {
                    let x = left_x;
                    left_x += item_width + padding;
                    x
                }
                StatusBarSlot::Center => center_x - item_width / 2.0,
                StatusBarSlot::Right => {
                    right_x -= item_width;
                    let x = right_x;
                    right_x -= padding;
                    x
                }
            };

            let item_bounds = GRect::new(ix, item_y, item_width, item_height);
            let color = match &item.kind {
                StatusBarItemKind::Clock { .. } => theme.status_bar_text,
                StatusBarItemKind::NotificationIndicator { unread_count, .. } => {
                    if *unread_count > 0 {
                        theme.status_bar_notification_active
                    } else {
                        theme.status_bar_notification_inactive
                    }
                }
                StatusBarItemKind::ConnectionQuality { quality_percent, .. } => {
                    if *quality_percent > 70 {
                        theme.status_bar_connected
                    } else {
                        theme.status_bar_degraded
                    }
                }
                StatusBarItemKind::TrayArea => theme.status_bar_tray,
                StatusBarItemKind::Custom { .. } => theme.status_bar_notification_inactive,
                StatusBarItemKind::SessionButton => theme.status_bar_text,
            };

            match &item.kind {
                StatusBarItemKind::Clock { .. } => {
                    // Format time from the stored UNIX-epoch timestamp.
                    let total_secs = item.last_update_us / 1_000_000;
                    let hours = (total_secs / 3600) % 24;
                    let minutes = (total_secs / 60) % 60;
                    let time_str = format!("{hours:02}:{minutes:02}");
                    bar_node.add_child(text_node(
                        item_id, time_str, color, item_bounds, 951, 1,
                    ));
                }
                StatusBarItemKind::NotificationIndicator { unread_count, .. } => {
                    // Bell icon (icon_id 14 = Notification).
                    bar_node.add_child(icon_node(item_id, 14, color, item_bounds, 951));
                    // Badge count overlay if unread > 0.
                    if *unread_count > 0 {
                        let badge_text = format!("{unread_count}");
                        let badge_w = badge_text.len() as f32 * 8.0 + 4.0;
                        let badge_rect = GRect::new(
                            ix + item_width - badge_w,
                            item_y,
                            badge_w,
                            12.0,
                        );
                        bar_node.add_child(text_node(
                            item_id + 1,
                            badge_text,
                            theme.status_bar_notification_active,
                            badge_rect,
                            952,
                            1,
                        ));
                    }
                }
                StatusBarItemKind::ConnectionQuality { .. } => {
                    // WiFi icon (icon_id 12 = Wifi).
                    bar_node.add_child(icon_node(item_id, 12, color, item_bounds, 951));
                }
                StatusBarItemKind::TrayArea => {
                    // Battery icon (icon_id 13 = Battery).
                    bar_node.add_child(icon_node(item_id, 13, color, item_bounds, 951));
                }
                StatusBarItemKind::Custom { content, .. } => {
                    bar_node.add_child(text_node(
                        item_id,
                        content.clone(),
                        color,
                        item_bounds,
                        951,
                        1,
                    ));
                }
                StatusBarItemKind::SessionButton => {
                    // Power icon (icon_id 16 = power/shutdown).
                    bar_node.add_child(icon_node(item_id, 16, color, item_bounds, 951));
                }
            }
        }

        bar_node
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
