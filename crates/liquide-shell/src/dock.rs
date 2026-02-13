//! Application dock — a macOS-style bar showing pinned and running apps.
//!
//! Supports auto-hide, magnification on hover, badge counts, and per-monitor
//! positioning.

use std::fmt;

use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::SceneNode;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Position & monitor mode
// ---------------------------------------------------------------------------

/// Edge of the screen where the dock is anchored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DockPosition {
    Bottom,
    Left,
    Right,
    Top,
}

impl fmt::Display for DockPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bottom => write!(f, "Bottom"),
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
            Self::Top => write!(f, "Top"),
        }
    }
}

/// Which monitors should display the dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DockMonitorMode {
    PrimaryOnly,
    AllScreens,
    FollowFocus,
}

impl fmt::Display for DockMonitorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrimaryOnly => write!(f, "PrimaryOnly"),
            Self::AllScreens => write!(f, "AllScreens"),
            Self::FollowFocus => write!(f, "FollowFocus"),
        }
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Persistent configuration for the dock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockConfig {
    /// Which edge the dock sits on.
    pub position: DockPosition,
    /// Base icon size in logical pixels.
    pub icon_size: u32,
    /// Magnification scale factor when hovering (e.g. 1.5).
    pub magnification_factor: f32,
    /// Whether magnification is enabled.
    pub magnification_enabled: bool,
    /// Whether auto-hide is enabled.
    pub auto_hide: bool,
    /// Delay in ms before the dock hides after the cursor leaves.
    pub auto_hide_delay_ms: u64,
    /// Show running-app indicator dots.
    pub show_running_indicators: bool,
    /// Monitor display mode.
    pub monitor_mode: DockMonitorMode,
    /// Maximum recent items to keep.
    pub max_recent_items: usize,
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            position: DockPosition::Bottom,
            icon_size: 48,
            magnification_factor: 1.5,
            magnification_enabled: true,
            auto_hide: false,
            auto_hide_delay_ms: 500,
            show_running_indicators: true,
            monitor_mode: DockMonitorMode::PrimaryOnly,
            max_recent_items: 10,
        }
    }
}

// ---------------------------------------------------------------------------
// Item kind & item
// ---------------------------------------------------------------------------

/// What kind of entry a dock item represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DockItemKind {
    Pinned,
    Running,
    Separator,
    Trash,
}

impl fmt::Display for DockItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pinned => write!(f, "Pinned"),
            Self::Running => write!(f, "Running"),
            Self::Separator => write!(f, "Separator"),
            Self::Trash => write!(f, "Trash"),
        }
    }
}

/// A single entry in the dock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockItem {
    /// Unique ID within the dock.
    pub id: u32,
    /// Entry type.
    pub kind: DockItemKind,
    /// Application identifier (empty for separators/trash).
    pub app_id: String,
    /// Display label.
    pub label: String,
    /// Icon resource path or name.
    pub icon: String,
    /// Unread badge count (0 = no badge).
    pub badge_count: u32,
    /// Number of running windows for this app.
    pub running_window_count: u32,
    /// Position among pinned items (0-indexed, `None` if not pinned).
    pub pinned_position: Option<usize>,
}

// ---------------------------------------------------------------------------
// Auto-hide state
// ---------------------------------------------------------------------------

/// Phase of the auto-hide animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutoHideState {
    Hidden,
    Showing,
    Visible,
    Hiding,
}

impl fmt::Display for AutoHideState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hidden => write!(f, "Hidden"),
            Self::Showing => write!(f, "Showing"),
            Self::Visible => write!(f, "Visible"),
            Self::Hiding => write!(f, "Hiding"),
        }
    }
}

// ---------------------------------------------------------------------------
// Dock
// ---------------------------------------------------------------------------

/// Runtime state for the application dock.
pub struct Dock {
    config: DockConfig,
    items: Vec<DockItem>,
    visible: bool,
    hover_index: Option<usize>,
    auto_hide_state: AutoHideState,
    next_id: u32,
}

impl Dock {
    /// Create a new dock from the given configuration.
    #[must_use]
    pub fn new(config: DockConfig) -> Self {
        let visible = !config.auto_hide;
        Self {
            config,
            items: Vec::new(),
            visible,
            hover_index: None,
            auto_hide_state: if visible {
                AutoHideState::Visible
            } else {
                AutoHideState::Hidden
            },
            next_id: 1,
        }
    }

    /// Add a pinned application to the dock.
    pub fn add_pinned(&mut self, app_id: impl Into<String>, label: impl Into<String>, icon: impl Into<String>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let pinned_pos = self
            .items
            .iter()
            .filter(|i| i.kind == DockItemKind::Pinned)
            .count();
        self.items.push(DockItem {
            id,
            kind: DockItemKind::Pinned,
            app_id: app_id.into(),
            label: label.into(),
            icon: icon.into(),
            badge_count: 0,
            running_window_count: 0,
            pinned_position: Some(pinned_pos),
        });
        id
    }

    /// Remove a pinned item by its dock ID.
    pub fn remove_pinned(&mut self, id: u32) -> bool {
        let before = self.items.len();
        self.items.retain(|i| !(i.id == id && i.kind == DockItemKind::Pinned));
        let removed = self.items.len() < before;
        if removed {
            self.reindex_pinned();
        }
        removed
    }

    /// Add or update a running-app entry.
    ///
    /// If the app is already pinned, increments its `running_window_count`.
    /// Otherwise creates a new Running entry.
    pub fn add_running(&mut self, app_id: &str) -> u32 {
        // Check if already present (pinned or running).
        if let Some(item) = self.items.iter_mut().find(|i| i.app_id == app_id) {
            item.running_window_count += 1;
            return item.id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(DockItem {
            id,
            kind: DockItemKind::Running,
            app_id: app_id.to_string(),
            label: app_id.to_string(),
            icon: String::new(),
            badge_count: 0,
            running_window_count: 1,
            pinned_position: None,
        });
        id
    }

    /// Decrement the running window count for an app.
    ///
    /// If the count reaches zero and the item is not pinned, it is removed.
    pub fn remove_running(&mut self, app_id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.app_id == app_id) {
            item.running_window_count = item.running_window_count.saturating_sub(1);
            if item.running_window_count == 0 && item.kind == DockItemKind::Running {
                let target_id = item.id;
                self.items.retain(|i| i.id != target_id);
            }
        }
    }

    /// Set the badge count for an app.
    pub fn set_badge(&mut self, app_id: &str, count: u32) {
        if let Some(item) = self.items.iter_mut().find(|i| i.app_id == app_id) {
            item.badge_count = count;
        }
    }

    /// Reorder pinned items by swapping two positions.
    pub fn reorder_pinned(&mut self, from_pos: usize, to_pos: usize) {
        let pinned_ids: Vec<u32> = self
            .items
            .iter()
            .filter(|i| i.kind == DockItemKind::Pinned)
            .map(|i| i.id)
            .collect();
        if from_pos >= pinned_ids.len() || to_pos >= pinned_ids.len() {
            return;
        }
        let from_id = pinned_ids[from_pos];
        let to_id = pinned_ids[to_pos];
        // Swap pinned_position values.
        if let Some(a) = self.items.iter_mut().find(|i| i.id == from_id) {
            a.pinned_position = Some(to_pos);
        }
        if let Some(b) = self.items.iter_mut().find(|i| i.id == to_id) {
            b.pinned_position = Some(from_pos);
        }
    }

    /// All items in display order.
    #[must_use]
    pub fn items(&self) -> &[DockItem] {
        &self.items
    }

    /// Look up an item by index.
    #[must_use]
    pub fn item_at_index(&self, index: usize) -> Option<&DockItem> {
        self.items.get(index)
    }

    /// Number of items.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Compute the screen-space bounding rectangle for the dock.
    #[must_use]
    pub fn compute_bounds(&self, screen: Rect) -> Rect {
        let icon = self.config.icon_size as f32;
        let count = self.items.len().max(1) as f32;
        let pad = 12.0_f32; // horizontal/vertical padding around items
        match self.config.position {
            DockPosition::Bottom => {
                let w = count * icon + pad * 2.0;
                let x = screen.x + (screen.width - w) / 2.0;
                Rect::new(x, screen.y + screen.height - icon - pad, w, icon + pad)
            }
            DockPosition::Top => {
                let w = count * icon + pad * 2.0;
                let x = screen.x + (screen.width - w) / 2.0;
                Rect::new(x, screen.y, w, icon + pad)
            }
            DockPosition::Left => {
                let h = count * icon + pad * 2.0;
                let y = screen.y + (screen.height - h) / 2.0;
                Rect::new(screen.x, y, icon + pad, h)
            }
            DockPosition::Right => {
                let h = count * icon + pad * 2.0;
                let y = screen.y + (screen.height - h) / 2.0;
                Rect::new(screen.x + screen.width - icon - pad, y, icon + pad, h)
            }
        }
    }

    /// Compute per-item bounding rectangles.
    #[must_use]
    pub fn compute_item_rects(&self, screen: Rect) -> Vec<(usize, Rect)> {
        let icon = self.config.icon_size as f32;
        let pad = 12.0_f32;
        let bounds = self.compute_bounds(screen);
        let mut rects = Vec::new();
        for (i, _item) in self.items.iter().enumerate() {
            let rect = match self.config.position {
                DockPosition::Bottom | DockPosition::Top => {
                    Rect::new(bounds.x + pad + i as f32 * icon, bounds.y + (bounds.height - icon) / 2.0, icon, icon)
                }
                DockPosition::Left | DockPosition::Right => {
                    Rect::new(bounds.x + (bounds.width - icon) / 2.0, bounds.y + pad + i as f32 * icon, icon, icon)
                }
            };
            rects.push((i, rect));
        }
        rects
    }

    /// Compute the magnified icon size for a given item based on hover distance.
    ///
    /// `hover_distance` is 0.0 for the hovered item, increasing for neighbours.
    /// Returns the base `icon_size` if magnification is disabled.
    #[must_use]
    pub fn magnified_size(&self, _item_index: usize, hover_distance: f32) -> u32 {
        if !self.config.magnification_enabled {
            return self.config.icon_size;
        }
        let factor = self.config.magnification_factor;
        let base = self.config.icon_size as f32;
        // Gaussian-ish falloff over 3 icon widths.
        let sigma = 2.0;
        let scale = 1.0 + (factor - 1.0) * (-hover_distance.powi(2) / (2.0 * sigma * sigma)).exp();
        (base * scale) as u32
    }

    /// Transition the auto-hide state.
    pub fn set_auto_hide_state(&mut self, state: AutoHideState) {
        self.auto_hide_state = state;
        self.visible = matches!(state, AutoHideState::Visible | AutoHideState::Showing);
    }

    /// Handle a hover event on a specific item index.
    pub fn on_hover(&mut self, index: usize) {
        if index < self.items.len() {
            self.hover_index = Some(index);
        }
    }

    /// Handle the cursor leaving the dock area.
    pub fn on_hover_leave(&mut self) {
        self.hover_index = None;
    }

    /// Currently hovered item index.
    #[must_use]
    pub fn hover_index(&self) -> Option<usize> {
        self.hover_index
    }

    /// Whether the dock is currently visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Current auto-hide state.
    #[must_use]
    pub fn auto_hide_state(&self) -> AutoHideState {
        self.auto_hide_state
    }

    /// The active configuration.
    #[must_use]
    pub fn config(&self) -> &DockConfig {
        &self.config
    }

    /// Build the scene graph for the dock.
    pub fn build_scene(&self, screen: Rect, theme: &crate::theme::ShellTheme) -> SceneNode {
        use crate::scene_builder::*;
        use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNode, SceneNodeKind};

        let dock_bounds = self.compute_bounds(screen);
        let mut dock_node = SceneNode::new(
            NODE_DOCK,
            SceneNodeKind::Glass(GlassParams {
                blur_radius: 20,
                tint_color: theme.dock_glass_tint,
                inner_glow: true,
                parallax: false,
            }),
            NodeProperties::new(dock_bounds).with_z_order(900),
        );

        // Item rects are in screen coords; convert to parent-relative
        // so that walk_inner's translation doesn't double-offset them.
        let item_rects = self.compute_item_rects(screen);

        // 2px accent border at the top edge of the dock (parent-relative).
        let border_rect = Rect::new(0.0, 0.0, dock_bounds.width, 2.0);
        dock_node.add_child(solid_rect(NODE_DOCK + 1, theme.dock_border, border_rect, 903));

        for (i, (_idx, item_rect)) in item_rects.iter().enumerate() {
            let item_id = NODE_DOCK_ITEM_BASE + i as u64;
            let color = if i < self.items.len() && self.items[i].running_window_count > 0 {
                theme.dock_item_active
            } else {
                theme.dock_item_inactive
            };
            let local_rect = Rect::new(
                item_rect.x - dock_bounds.x,
                item_rect.y - dock_bounds.y,
                item_rect.width,
                item_rect.height,
            );
            dock_node.add_child(solid_rect(item_id, color, local_rect, 901));
        }

        if let Some(hover_idx) = self.hover_index {
            if hover_idx < item_rects.len() {
                let (_, hover_rect) = &item_rects[hover_idx];
                let local_hover = Rect::new(
                    hover_rect.x - dock_bounds.x,
                    hover_rect.y - dock_bounds.y,
                    hover_rect.width,
                    hover_rect.height,
                );
                dock_node.add_child(tint_overlay(
                    NODE_DOCK_ITEM_BASE + 500,
                    theme.dock_hover_highlight,
                    local_hover,
                    902,
                ));
            }
        }

        dock_node
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    fn reindex_pinned(&mut self) {
        let mut pos = 0usize;
        for item in &mut self.items {
            if item.kind == DockItemKind::Pinned {
                item.pinned_position = Some(pos);
                pos += 1;
            }
        }
    }
}

impl fmt::Display for Dock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Dock({} items, {}, {})",
            self.items.len(),
            self.config.position,
            self.auto_hide_state,
        )
    }
}
