//! Application dock — a macOS-style bar showing pinned and running apps.
//!
//! Supports auto-hide, magnification on hover, badge counts, and per-monitor
//! positioning.

use std::fmt;

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNode, SceneNodeKind};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Theme colors
// ---------------------------------------------------------------------------

/// Colors needed by the dock for rendering. Populated by the shell from its
/// own theme before calling `build_scene`.
#[derive(Debug, Clone, Copy)]
pub struct DockThemeColors {
    pub glass_tint: Color,
    pub border: Color,
    pub item_active: Color,
    pub item_inactive: Color,
    pub hover_highlight: Color,
    /// Outline / glow used to mark an item that is requesting user attention
    /// (`DockItem::needs_attention = true`). Typically a warm accent such as
    /// orange or red. If the shell omits a value here the dock falls back to
    /// [`DockThemeColors::default_needs_attention()`].
    pub needs_attention: Color,
    /// Outline used for the currently focused app. Typically the accent color.
    pub focus_outline: Color,
}

impl DockThemeColors {
    /// A reasonable default attention color (warm orange) for shells that do
    /// not provide a theme override.
    pub const fn default_needs_attention() -> Color {
        Color {
            r: 0xFF,
            g: 0x8A,
            b: 0x00,
            a: 0xFF,
        }
    }

    /// A reasonable default focus outline (neutral accent blue).
    pub const fn default_focus_outline() -> Color {
        Color {
            r: 0x3B,
            g: 0x82,
            b: 0xF6,
            a: 0xFF,
        }
    }
}

// ---------------------------------------------------------------------------
// Node ID constants (mirrored from the shell's scene_builder)
// ---------------------------------------------------------------------------

const NODE_DOCK: u64 = 2_000;
const NODE_DOCK_ITEM_BASE: u64 = 2_100;

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
// Click behavior
// ---------------------------------------------------------------------------

/// Behavior when clicking a running app icon in the dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DockClickBehavior {
    /// Toggle minimize/restore.
    ToggleMinimize,
    /// Always launch new instance.
    AlwaysNew,
    /// Bring to front if minimized, minimize if already front.
    SmartToggle,
    /// Show all windows for that app.
    ShowAllWindows,
}

// ---------------------------------------------------------------------------
// Render config
// ---------------------------------------------------------------------------

/// Configurable rendering parameters for [`Dock::build_scene`].
///
/// These can be populated from CSS layout values (e.g. `DockLayout`) or
/// left at their defaults for standalone usage.
#[derive(Debug, Clone, Copy)]
pub struct DockRenderConfig {
    /// Blur radius for the glass backdrop.
    pub blur_radius: u32,
    /// Height of the accent border at the top edge of the dock.
    pub border_height: f32,
}

impl Default for DockRenderConfig {
    fn default() -> Self {
        Self {
            blur_radius: 20,
            border_height: 2.0,
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
    /// Behavior when clicking a running app icon.
    pub click_running_behavior: DockClickBehavior,
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
            click_running_behavior: DockClickBehavior::SmartToggle,
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
    /// The app is requesting user attention (e.g. a window flashed its
    /// taskbar icon on Windows or set `_NET_WM_STATE_DEMANDS_ATTENTION` on X).
    #[serde(default)]
    pub needs_attention: bool,
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
    /// `app_id` of the currently focused application (if any).
    focused_app: Option<String>,
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
            focused_app: None,
        }
    }

    /// Add a pinned application to the dock.
    pub fn add_pinned(
        &mut self,
        app_id: impl Into<String>,
        label: impl Into<String>,
        icon: impl Into<String>,
    ) -> u32 {
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
            needs_attention: false,
        });
        id
    }

    /// Remove a pinned item by its dock ID.
    pub fn remove_pinned(&mut self, id: u32) -> bool {
        let before = self.items.len();
        self.items
            .retain(|i| !(i.id == id && i.kind == DockItemKind::Pinned));
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
            needs_attention: false,
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
        let pad = 8.0_f32; // horizontal/vertical padding around items
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
        let pad = 8.0_f32;
        let bounds = self.compute_bounds(screen);
        let mut rects = Vec::new();
        for (i, _item) in self.items.iter().enumerate() {
            let rect = match self.config.position {
                DockPosition::Bottom | DockPosition::Top => Rect::new(
                    bounds.x + pad + i as f32 * icon,
                    bounds.y + (bounds.height - icon) / 2.0,
                    icon,
                    icon,
                ),
                DockPosition::Left | DockPosition::Right => Rect::new(
                    bounds.x + (bounds.width - icon) / 2.0,
                    bounds.y + pad + i as f32 * icon,
                    icon,
                    icon,
                ),
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

    /// `app_id` of the currently focused application, if any.
    #[must_use]
    pub fn focused_app(&self) -> Option<&str> {
        self.focused_app.as_deref()
    }

    /// Mark an app as focused (receives keyboard input).
    ///
    /// Called from the window-manager event stream (e.g. `WindowFocused` on
    /// Win32 / `xdg_toplevel.activated` on Wayland). The previously focused
    /// app is automatically un-focused.
    pub fn set_focused_app(&mut self, app_id: Option<&str>) {
        self.focused_app = app_id.map(str::to_string);
    }

    /// Set the `needs_attention` flag on an app's dock item.
    ///
    /// Called from the window-manager event stream when a window requests
    /// user attention (flashing taskbar icon on Win32, urgency hint on X).
    pub fn set_needs_attention(&mut self, app_id: &str, needs_attention: bool) {
        if let Some(item) = self.items.iter_mut().find(|i| i.app_id == app_id) {
            item.needs_attention = needs_attention;
        }
    }

    /// Notify the dock that an app's title or metadata changed.
    ///
    /// Currently this only clears attention on the focused app so the
    /// indicator dismisses when the user selects the attention-requesting
    /// window. Future versions may update the dock label.
    pub fn on_window_changed(&mut self, app_id: &str) {
        if self.focused_app.as_deref() == Some(app_id) {
            self.set_needs_attention(app_id, false);
        }
    }

    /// Build the scene graph for the dock.
    ///
    /// # Parameters
    /// - `screen`: full screen rect
    /// - `colors`: theme colors for the dock
    /// - `icon_resolver`: function to map icon name → numeric icon ID
    /// - `render_config`: optional rendering parameters (blur, border); defaults used if `None`
    pub fn build_scene(
        &self,
        screen: Rect,
        colors: &DockThemeColors,
        icon_resolver: &dyn Fn(&str) -> u32,
        render_config: Option<&DockRenderConfig>,
    ) -> SceneNode {
        let defaults = DockRenderConfig::default();
        let rc = render_config.unwrap_or(&defaults);

        let dock_bounds = self.compute_bounds(screen);
        let mut dock_node = SceneNode::new(
            NODE_DOCK,
            SceneNodeKind::Glass(GlassParams {
                blur_radius: rc.blur_radius,
                tint_color: colors.glass_tint,
                inner_glow: true,
                parallax: false,
            }),
            NodeProperties::new(dock_bounds).with_z_order(900),
        );

        // Item rects are in screen coords; convert to parent-relative
        // so that walk_inner's translation doesn't double-offset them.
        let item_rects = self.compute_item_rects(screen);

        // Accent border at the top edge of the dock (parent-relative).
        let border_rect = Rect::new(0.0, 0.0, dock_bounds.width, rc.border_height);
        dock_node.add_child(SceneNode::new(
            NODE_DOCK + 1,
            SceneNodeKind::Background {
                color: colors.border,
            },
            NodeProperties::new(border_rect).with_z_order(903),
        ));

        for (i, (_idx, item_rect)) in item_rects.iter().enumerate() {
            let item_id = NODE_DOCK_ITEM_BASE + i as u64 * 3;
            let color = if i < self.items.len() && self.items[i].running_window_count > 0 {
                colors.item_active
            } else {
                colors.item_inactive
            };
            let local_rect = Rect::new(
                item_rect.x - dock_bounds.x,
                item_rect.y - dock_bounds.y,
                item_rect.width,
                item_rect.height,
            );

            // Resolve the icon ID from the item's icon name.
            let iid = if i < self.items.len() {
                icon_resolver(&self.items[i].icon)
            } else {
                0
            };

            // Render the icon filling the item rect.
            dock_node.add_child(SceneNode::new(
                item_id,
                SceneNodeKind::Icon {
                    icon_id: iid,
                    color,
                },
                NodeProperties::new(local_rect).with_z_order(901),
            ));

            // Running indicator dot below the icon.
            if i < self.items.len()
                && self.items[i].running_window_count > 0
                && self.config.show_running_indicators
            {
                let dot_size = 4.0_f32;
                let dot_x = local_rect.x + (local_rect.width - dot_size) / 2.0;
                let dot_y = local_rect.y + local_rect.height - dot_size - 1.0;
                let dot_rect = Rect::new(dot_x, dot_y, dot_size, dot_size);
                dock_node.add_child(SceneNode::new(
                    item_id + 2,
                    SceneNodeKind::Background {
                        color: colors.item_active,
                    },
                    NodeProperties::new(dot_rect).with_z_order(902),
                ));
            }
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
                dock_node.add_child(SceneNode::new(
                    NODE_DOCK_ITEM_BASE + 500,
                    SceneNodeKind::Tint {
                        color: colors.hover_highlight,
                    },
                    NodeProperties::new(local_hover).with_z_order(902),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn default_dock() -> Dock {
        Dock::new(DockConfig::default())
    }

    // ── Item management ───────────────────────────────────────────

    #[test]
    fn test_dock_new_empty() {
        let dock = default_dock();
        assert_eq!(dock.item_count(), 0);
        assert!(dock.items().is_empty());
    }

    #[test]
    fn test_dock_add_pinned_item() {
        let mut dock = default_dock();
        let id = dock.add_pinned("files", "Files", "files-icon");
        assert!(id > 0);
        assert_eq!(dock.item_count(), 1);
        assert_eq!(dock.items()[0].app_id, "files");
        assert_eq!(dock.items()[0].kind, DockItemKind::Pinned);
        assert_eq!(dock.items()[0].pinned_position, Some(0));
    }

    #[test]
    fn test_dock_add_multiple_pinned() {
        let mut dock = default_dock();
        dock.add_pinned("files", "Files", "icon1");
        dock.add_pinned("browser", "Browser", "icon2");
        dock.add_pinned("terminal", "Terminal", "icon3");
        assert_eq!(dock.item_count(), 3);
        assert_eq!(dock.items()[0].pinned_position, Some(0));
        assert_eq!(dock.items()[1].pinned_position, Some(1));
        assert_eq!(dock.items()[2].pinned_position, Some(2));
    }

    #[test]
    fn test_dock_remove_pinned_item() {
        let mut dock = default_dock();
        let id1 = dock.add_pinned("files", "Files", "icon1");
        let _id2 = dock.add_pinned("browser", "Browser", "icon2");
        assert_eq!(dock.item_count(), 2);

        assert!(dock.remove_pinned(id1));
        assert_eq!(dock.item_count(), 1);
        assert_eq!(dock.items()[0].app_id, "browser");
        // Pinned position should be re-indexed
        assert_eq!(dock.items()[0].pinned_position, Some(0));
    }

    #[test]
    fn test_dock_remove_pinned_nonexistent_returns_false() {
        let mut dock = default_dock();
        assert!(!dock.remove_pinned(999));
    }

    #[test]
    fn test_dock_add_running_app() {
        let mut dock = default_dock();
        let id = dock.add_running("terminal");
        assert!(id > 0);
        assert_eq!(dock.item_count(), 1);
        assert_eq!(dock.items()[0].kind, DockItemKind::Running);
        assert_eq!(dock.items()[0].running_window_count, 1);
    }

    #[test]
    fn test_dock_add_running_increments_count() {
        let mut dock = default_dock();
        dock.add_running("terminal");
        dock.add_running("terminal");
        assert_eq!(dock.item_count(), 1); // same app, one entry
        assert_eq!(dock.items()[0].running_window_count, 2);
    }

    #[test]
    fn test_dock_add_running_increments_pinned_count() {
        let mut dock = default_dock();
        dock.add_pinned("files", "Files", "icon");
        dock.add_running("files");
        assert_eq!(dock.item_count(), 1); // still one item
        assert_eq!(dock.items()[0].running_window_count, 1);
        assert_eq!(dock.items()[0].kind, DockItemKind::Pinned);
    }

    #[test]
    fn test_dock_remove_running_decrements_count() {
        let mut dock = default_dock();
        dock.add_running("terminal");
        dock.add_running("terminal");
        dock.remove_running("terminal");
        assert_eq!(dock.item_count(), 1);
        assert_eq!(dock.items()[0].running_window_count, 1);
    }

    #[test]
    fn test_dock_remove_running_removes_when_zero() {
        let mut dock = default_dock();
        dock.add_running("terminal");
        dock.remove_running("terminal");
        assert_eq!(dock.item_count(), 0);
    }

    #[test]
    fn test_dock_remove_running_keeps_pinned() {
        let mut dock = default_dock();
        dock.add_pinned("files", "Files", "icon");
        dock.add_running("files");
        dock.remove_running("files");
        assert_eq!(dock.item_count(), 1); // pinned stays
        assert_eq!(dock.items()[0].running_window_count, 0);
    }

    #[test]
    fn test_dock_set_badge() {
        let mut dock = default_dock();
        dock.add_running("mail");
        dock.set_badge("mail", 5);
        assert_eq!(dock.items()[0].badge_count, 5);
    }

    #[test]
    fn test_dock_set_badge_nonexistent_noop() {
        let mut dock = default_dock();
        dock.set_badge("nonexistent", 3); // should not panic
    }

    // ── Reorder ───────────────────────────────────────────────────

    #[test]
    fn test_dock_reorder_pinned() {
        let mut dock = default_dock();
        dock.add_pinned("a", "A", "icon");
        dock.add_pinned("b", "B", "icon");
        dock.add_pinned("c", "C", "icon");

        dock.reorder_pinned(0, 2);
        // a should now be at position 2, c at position 0
        let a = dock.items().iter().find(|i| i.app_id == "a").unwrap();
        let c = dock.items().iter().find(|i| i.app_id == "c").unwrap();
        assert_eq!(a.pinned_position, Some(2));
        assert_eq!(c.pinned_position, Some(0));
    }

    #[test]
    fn test_dock_reorder_out_of_bounds_noop() {
        let mut dock = default_dock();
        dock.add_pinned("a", "A", "icon");
        dock.reorder_pinned(0, 5); // out of bounds, should not panic
        assert_eq!(dock.items()[0].pinned_position, Some(0));
    }

    // ── Hover ─────────────────────────────────────────────────────

    #[test]
    fn test_dock_hover() {
        let mut dock = default_dock();
        dock.add_running("a");
        dock.add_running("b");

        assert!(dock.hover_index().is_none());
        dock.on_hover(1);
        assert_eq!(dock.hover_index(), Some(1));
        dock.on_hover_leave();
        assert!(dock.hover_index().is_none());
    }

    #[test]
    fn test_dock_hover_out_of_bounds_ignored() {
        let mut dock = default_dock();
        dock.add_running("a");
        dock.on_hover(5); // out of bounds
        assert!(dock.hover_index().is_none());
    }

    // ── Auto-hide ─────────────────────────────────────────────────

    #[test]
    fn test_dock_auto_hide_disabled_is_visible() {
        let dock = Dock::new(DockConfig {
            auto_hide: false,
            ..Default::default()
        });
        assert!(dock.is_visible());
        assert_eq!(dock.auto_hide_state(), AutoHideState::Visible);
    }

    #[test]
    fn test_dock_auto_hide_enabled_starts_hidden() {
        let dock = Dock::new(DockConfig {
            auto_hide: true,
            ..Default::default()
        });
        assert!(!dock.is_visible());
        assert_eq!(dock.auto_hide_state(), AutoHideState::Hidden);
    }

    #[test]
    fn test_dock_auto_hide_state_transitions() {
        let mut dock = Dock::new(DockConfig {
            auto_hide: true,
            ..Default::default()
        });
        assert!(!dock.is_visible());

        dock.set_auto_hide_state(AutoHideState::Showing);
        assert!(dock.is_visible());

        dock.set_auto_hide_state(AutoHideState::Visible);
        assert!(dock.is_visible());

        dock.set_auto_hide_state(AutoHideState::Hiding);
        assert!(!dock.is_visible());

        dock.set_auto_hide_state(AutoHideState::Hidden);
        assert!(!dock.is_visible());
    }

    // ── Positioning & geometry ──────────────────────────────────────

    #[test]
    fn test_dock_position_display() {
        assert_eq!(DockPosition::Bottom.to_string(), "Bottom");
        assert_eq!(DockPosition::Left.to_string(), "Left");
        assert_eq!(DockPosition::Right.to_string(), "Right");
        assert_eq!(DockPosition::Top.to_string(), "Top");
    }

    #[test]
    fn test_dock_compute_bounds_bottom() {
        let config = DockConfig {
            position: DockPosition::Bottom,
            icon_size: 48,
            ..Default::default()
        };
        let mut dock = Dock::new(config);
        dock.add_running("a");
        dock.add_running("b");

        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let bounds = dock.compute_bounds(screen);

        // Bottom-anchored, centered horizontally
        assert!(bounds.y > 1000.0); // near bottom
        assert!(bounds.x > 0.0); // centered, not at 0
        assert!(bounds.width > 0.0);
        assert!(bounds.height > 0.0);
    }

    #[test]
    fn test_dock_compute_bounds_left() {
        let config = DockConfig {
            position: DockPosition::Left,
            icon_size: 48,
            ..Default::default()
        };
        let mut dock = Dock::new(config);
        dock.add_running("a");

        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let bounds = dock.compute_bounds(screen);

        assert_eq!(bounds.x, 0.0); // left edge
    }

    #[test]
    fn test_dock_compute_bounds_right() {
        let config = DockConfig {
            position: DockPosition::Right,
            icon_size: 48,
            ..Default::default()
        };
        let mut dock = Dock::new(config);
        dock.add_running("a");

        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let bounds = dock.compute_bounds(screen);

        assert!(bounds.x + bounds.width >= 1919.0); // right edge
    }

    #[test]
    fn test_dock_compute_item_rects_count() {
        let mut dock = default_dock();
        dock.add_running("a");
        dock.add_running("b");
        dock.add_running("c");

        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let rects = dock.compute_item_rects(screen);
        assert_eq!(rects.len(), 3);
    }

    // ── Magnification ───────────────────────────────────────────────

    #[test]
    fn test_magnified_size_disabled() {
        let config = DockConfig {
            magnification_enabled: false,
            icon_size: 48,
            ..Default::default()
        };
        let dock = Dock::new(config);
        assert_eq!(dock.magnified_size(0, 0.0), 48);
    }

    #[test]
    fn test_magnified_size_hovered_item() {
        let config = DockConfig {
            magnification_enabled: true,
            magnification_factor: 1.5,
            icon_size: 48,
            ..Default::default()
        };
        let dock = Dock::new(config);
        let size = dock.magnified_size(0, 0.0);
        // At distance 0, scale = 1 + (1.5 - 1) * 1.0 = 1.5, size = 72
        assert_eq!(size, 72);
    }

    #[test]
    fn test_magnified_size_decreases_with_distance() {
        let config = DockConfig {
            magnification_enabled: true,
            magnification_factor: 1.5,
            icon_size: 48,
            ..Default::default()
        };
        let dock = Dock::new(config);
        let at_0 = dock.magnified_size(0, 0.0);
        let at_1 = dock.magnified_size(0, 1.0);
        let at_3 = dock.magnified_size(0, 3.0);
        assert!(at_0 > at_1);
        assert!(at_1 > at_3);
    }

    // ── Config defaults ───────────────────────────────────────────

    #[test]
    fn test_dock_config_defaults() {
        let config = DockConfig::default();
        assert_eq!(config.position, DockPosition::Bottom);
        assert_eq!(config.icon_size, 48);
        assert!(!config.auto_hide);
        assert!(config.magnification_enabled);
        assert!(config.show_running_indicators);
        assert_eq!(config.monitor_mode, DockMonitorMode::PrimaryOnly);
        assert_eq!(
            config.click_running_behavior,
            DockClickBehavior::SmartToggle
        );
    }

    // ── Display traits ─────────────────────────────────────────────

    #[test]
    fn test_dock_display_format() {
        let mut dock = default_dock();
        dock.add_running("a");
        let s = format!("{}", dock);
        assert!(s.contains("1 items"));
        assert!(s.contains("Bottom"));
        assert!(s.contains("Visible"));
    }

    #[test]
    fn test_dock_item_kind_display() {
        assert_eq!(DockItemKind::Pinned.to_string(), "Pinned");
        assert_eq!(DockItemKind::Running.to_string(), "Running");
        assert_eq!(DockItemKind::Separator.to_string(), "Separator");
        assert_eq!(DockItemKind::Trash.to_string(), "Trash");
    }

    #[test]
    fn test_dock_monitor_mode_display() {
        assert_eq!(DockMonitorMode::PrimaryOnly.to_string(), "PrimaryOnly");
        assert_eq!(DockMonitorMode::AllScreens.to_string(), "AllScreens");
        assert_eq!(DockMonitorMode::FollowFocus.to_string(), "FollowFocus");
    }

    #[test]
    fn test_auto_hide_state_display() {
        assert_eq!(AutoHideState::Hidden.to_string(), "Hidden");
        assert_eq!(AutoHideState::Showing.to_string(), "Showing");
        assert_eq!(AutoHideState::Visible.to_string(), "Visible");
        assert_eq!(AutoHideState::Hiding.to_string(), "Hiding");
    }

    // ── Unique IDs ─────────────────────────────────────────────────

    #[test]
    fn test_dock_items_get_unique_ids() {
        let mut dock = default_dock();
        let id1 = dock.add_pinned("a", "A", "icon");
        let id2 = dock.add_pinned("b", "B", "icon");
        let id3 = dock.add_running("c");
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_dock_item_at_index() {
        let mut dock = default_dock();
        dock.add_running("x");
        assert!(dock.item_at_index(0).is_some());
        assert_eq!(dock.item_at_index(0).unwrap().app_id, "x");
        assert!(dock.item_at_index(1).is_none());
    }
}
