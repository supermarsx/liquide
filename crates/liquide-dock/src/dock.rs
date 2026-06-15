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
// Auto-hide mode
// ---------------------------------------------------------------------------

/// How the dock hides itself when not in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutoHideMode {
    /// Dock is always shown; never hides.
    Off,
    /// Dock stays visible until a window overlaps its rectangle, then hides;
    /// it reveals again when the cursor reaches the dock's screen edge.
    OnOverlap,
    /// Dock is always hidden and only reveals when the cursor reaches the
    /// dock's screen edge (classic "auto-hide").
    AlwaysHidden,
}

impl Default for AutoHideMode {
    fn default() -> Self {
        Self::Off
    }
}

impl fmt::Display for AutoHideMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "Off"),
            Self::OnOverlap => write!(f, "OnOverlap"),
            Self::AlwaysHidden => write!(f, "AlwaysHidden"),
        }
    }
}

// ---------------------------------------------------------------------------
// Item alignment
// ---------------------------------------------------------------------------

/// How items are distributed along the dock's main axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DockAlignment {
    /// Items are packed together and centered on the screen edge (default,
    /// macOS-style).
    Centered,
    /// Items are spread to fill the whole edge with equal gaps between them
    /// (Windows-taskbar-style "justified").
    Justified,
}

impl Default for DockAlignment {
    fn default() -> Self {
        Self::Centered
    }
}

impl fmt::Display for DockAlignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Centered => write!(f, "Centered"),
            Self::Justified => write!(f, "Justified"),
        }
    }
}

// ---------------------------------------------------------------------------
// Pinned app (serializable description)
// ---------------------------------------------------------------------------

/// A persistent description of a pinned application, suitable for saving in
/// [`DockConfig`]. This is the serializable counterpart of a pinned
/// [`DockItem`]; the runtime [`Dock`] materializes these into live items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedApp {
    /// Application identifier (matches `DockItem::app_id`).
    pub app_id: String,
    /// Display label.
    pub label: String,
    /// Icon resource path or name.
    pub icon: String,
}

impl PinnedApp {
    /// Construct a new pinned app description.
    pub fn new(
        app_id: impl Into<String>,
        label: impl Into<String>,
        icon: impl Into<String>,
    ) -> Self {
        Self {
            app_id: app_id.into(),
            label: label.into(),
            icon: icon.into(),
        }
    }
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
    /// Thickness of the dock perpendicular to its edge (height for top/bottom,
    /// width for left/right), in logical pixels. When `None` the dock derives a
    /// thickness from `icon_size + 2 * padding` (the historical behavior).
    #[serde(default)]
    pub thickness: Option<u32>,
    /// Padding (in logical pixels) between the dock's edge/border and the items
    /// along the cross axis, and at the two ends of the main axis.
    #[serde(default = "default_padding")]
    pub padding: f32,
    /// Spacing (in logical pixels) between adjacent items along the main axis.
    #[serde(default = "default_spacing")]
    pub spacing: f32,
    /// Magnification scale factor when hovering (e.g. 1.5).
    pub magnification_factor: f32,
    /// Whether magnification is enabled.
    pub magnification_enabled: bool,
    /// Whether auto-hide is enabled.
    ///
    /// Retained for backward compatibility. The canonical control is
    /// [`DockConfig::auto_hide_mode`]; `auto_hide == true` is equivalent to
    /// [`AutoHideMode::AlwaysHidden`]. [`Dock::new`] reconciles the two so a
    /// config that only sets `auto_hide` still behaves as expected.
    pub auto_hide: bool,
    /// Auto-hide behavior (off / on-overlap / always-hidden). Takes precedence
    /// over `auto_hide` when it is non-default; see [`Dock::new`].
    #[serde(default)]
    pub auto_hide_mode: AutoHideMode,
    /// Delay in ms before the dock hides after the cursor leaves.
    pub auto_hide_delay_ms: u64,
    /// Width (in logical pixels) of the hot zone along the dock's edge that
    /// triggers a reveal when auto-hide is active.
    #[serde(default = "default_reveal_zone")]
    pub reveal_zone: f32,
    /// Show running-app indicator dots.
    pub show_running_indicators: bool,
    /// Show text labels next to/under each item.
    #[serde(default)]
    pub show_labels: bool,
    /// How items are aligned along the main axis.
    #[serde(default)]
    pub alignment: DockAlignment,
    /// Ordered list of pinned applications to materialize at startup. Empty by
    /// default; the shell may supply its own pinned set programmatically.
    #[serde(default)]
    pub pinned_apps: Vec<PinnedApp>,
    /// Monitor display mode.
    pub monitor_mode: DockMonitorMode,
    /// Maximum recent items to keep.
    pub max_recent_items: usize,
    /// Behavior when clicking a running app icon.
    pub click_running_behavior: DockClickBehavior,
}

fn default_padding() -> f32 {
    8.0
}

fn default_spacing() -> f32 {
    0.0
}

fn default_reveal_zone() -> f32 {
    2.0
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            position: DockPosition::Bottom,
            icon_size: 48,
            thickness: None,
            padding: default_padding(),
            spacing: default_spacing(),
            magnification_factor: 1.5,
            magnification_enabled: true,
            auto_hide: false,
            auto_hide_mode: AutoHideMode::Off,
            auto_hide_delay_ms: 500,
            reveal_zone: default_reveal_zone(),
            show_running_indicators: true,
            show_labels: false,
            alignment: DockAlignment::Centered,
            pinned_apps: Vec::new(),
            monitor_mode: DockMonitorMode::PrimaryOnly,
            max_recent_items: 10,
            click_running_behavior: DockClickBehavior::SmartToggle,
        }
    }
}

impl DockConfig {
    /// Resolve the effective auto-hide mode, reconciling the legacy `auto_hide`
    /// boolean with the richer [`AutoHideMode`].
    ///
    /// If `auto_hide_mode` is non-default it wins. Otherwise a `true`
    /// `auto_hide` maps to [`AutoHideMode::AlwaysHidden`] and `false` to
    /// [`AutoHideMode::Off`].
    #[must_use]
    pub fn effective_auto_hide_mode(&self) -> AutoHideMode {
        if self.auto_hide_mode != AutoHideMode::Off {
            self.auto_hide_mode
        } else if self.auto_hide {
            AutoHideMode::AlwaysHidden
        } else {
            AutoHideMode::Off
        }
    }

    /// The dock thickness (cross-axis size) in logical pixels, derived from
    /// `thickness` if set or from `icon_size + 2 * padding` otherwise.
    #[must_use]
    pub fn effective_thickness(&self) -> f32 {
        match self.thickness {
            Some(t) => t as f32,
            None => self.icon_size as f32 + self.padding,
        }
    }

    /// Whether the dock lays out along the vertical axis (left/right edges).
    #[must_use]
    pub fn is_vertical(&self) -> bool {
        matches!(self.position, DockPosition::Left | DockPosition::Right)
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
    /// Whether a window currently overlaps the dock rect (drives
    /// [`AutoHideMode::OnOverlap`]). Updated by [`Dock::set_occluded`].
    occluded: bool,
    /// Whether the cursor is currently within the dock's reveal hot-zone or
    /// over the revealed dock (drives the reveal state machine).
    cursor_revealing: bool,
}

impl Dock {
    /// Create a new dock from the given configuration.
    ///
    /// Any [`PinnedApp`]s listed in `config.pinned_apps` are materialized into
    /// live pinned items in order. The dock starts visible unless the effective
    /// auto-hide mode is [`AutoHideMode::AlwaysHidden`].
    #[must_use]
    pub fn new(config: DockConfig) -> Self {
        // Visible at startup unless we always start hidden.
        let visible = config.effective_auto_hide_mode() != AutoHideMode::AlwaysHidden;
        let pinned = config.pinned_apps.clone();
        let mut dock = Self {
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
            occluded: false,
            cursor_revealing: false,
        };
        for app in pinned {
            dock.add_pinned(app.app_id, app.label, app.icon);
        }
        dock
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

    /// Move a pinned item from one pinned position to another, shifting the
    /// items in between (true list reorder, unlike the swap-based
    /// [`Dock::reorder_pinned`]).
    ///
    /// This reorders the underlying item vector so that `items()` (and hence
    /// the rendered/DOM order) reflects the new arrangement, then re-indexes
    /// `pinned_position`. Out-of-range positions are a no-op.
    pub fn move_pinned(&mut self, from_pos: usize, to_pos: usize) -> bool {
        // Indices into `self.items` of pinned entries, in current order.
        let pinned_idx: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.kind == DockItemKind::Pinned)
            .map(|(idx, _)| idx)
            .collect();
        if from_pos >= pinned_idx.len() || to_pos >= pinned_idx.len() || from_pos == to_pos {
            return false;
        }
        let src = pinned_idx[from_pos];
        // Remove then reinsert at the target slot's underlying index.
        let item = self.items.remove(src);
        // After removal, recompute the destination underlying index.
        let pinned_idx_after: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.kind == DockItemKind::Pinned)
            .map(|(idx, _)| idx)
            .collect();
        let dst = if to_pos >= pinned_idx_after.len() {
            // Insert after the last pinned item.
            pinned_idx_after.last().map_or(0, |&i| i + 1)
        } else {
            pinned_idx_after[to_pos]
        };
        self.items.insert(dst, item);
        self.reindex_pinned();
        true
    }

    /// Materialize a new ordered pinned set from [`PinnedApp`] descriptions,
    /// removing any currently-pinned items first. Running entries are left
    /// untouched. Useful when applying a freshly loaded/edited config.
    pub fn apply_pinned_apps(&mut self, apps: &[PinnedApp]) {
        self.items.retain(|i| i.kind != DockItemKind::Pinned);
        for app in apps {
            self.add_pinned(app.app_id.clone(), app.label.clone(), app.icon.clone());
        }
    }

    /// Snapshot the current pinned items as serializable [`PinnedApp`]s in
    /// display order — suitable for persisting back into [`DockConfig`].
    #[must_use]
    pub fn pinned_apps(&self) -> Vec<PinnedApp> {
        self.items
            .iter()
            .filter(|i| i.kind == DockItemKind::Pinned)
            .map(|i| PinnedApp::new(i.app_id.clone(), i.label.clone(), i.icon.clone()))
            .collect()
    }

    /// All items in display order.
    #[must_use]
    pub fn items(&self) -> &[DockItem] {
        &self.items
    }

    /// Pinned items only, in display order.
    #[must_use]
    pub fn pinned_items(&self) -> Vec<&DockItem> {
        self.items
            .iter()
            .filter(|i| i.kind == DockItemKind::Pinned)
            .collect()
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
    ///
    /// Honors [`DockConfig`]'s `position`, `icon_size`, `thickness`, `padding`,
    /// `spacing` and `alignment`. For [`DockAlignment::Justified`] the dock
    /// spans the entire screen edge; for [`DockAlignment::Centered`] it shrinks
    /// to fit its items and is centered on the edge.
    #[must_use]
    pub fn compute_bounds(&self, screen: Rect) -> Rect {
        let thickness = self.config.effective_thickness();
        let main_len = self.main_axis_length();
        match self.config.position {
            DockPosition::Bottom => {
                let (x, w) = self.main_axis_origin(screen.x, screen.width, main_len);
                Rect::new(x, screen.y + screen.height - thickness, w, thickness)
            }
            DockPosition::Top => {
                let (x, w) = self.main_axis_origin(screen.x, screen.width, main_len);
                Rect::new(x, screen.y, w, thickness)
            }
            DockPosition::Left => {
                let (y, h) = self.main_axis_origin(screen.y, screen.height, main_len);
                Rect::new(screen.x, y, thickness, h)
            }
            DockPosition::Right => {
                let (y, h) = self.main_axis_origin(screen.y, screen.height, main_len);
                Rect::new(screen.x + screen.width - thickness, y, thickness, h)
            }
        }
    }

    /// Compute per-item bounding rectangles, position- and alignment-aware.
    ///
    /// Items are laid out vertically for [`DockPosition::Left`]/`Right` and
    /// horizontally for `Top`/`Bottom`. With [`DockAlignment::Centered`] items
    /// are packed at the start of the content area (padding + spacing); with
    /// [`DockAlignment::Justified`] they are spread evenly across the dock's
    /// full main-axis extent.
    #[must_use]
    pub fn compute_item_rects(&self, screen: Rect) -> Vec<(usize, Rect)> {
        let icon = self.config.icon_size as f32;
        let pad = self.config.padding;
        let spacing = self.config.spacing;
        let bounds = self.compute_bounds(screen);
        let vertical = self.config.is_vertical();
        let count = self.items.len();

        // Cross-axis position (centered within the dock thickness).
        let cross = |bounds_cross_origin: f32, bounds_cross_size: f32| {
            bounds_cross_origin + (bounds_cross_size - icon) / 2.0
        };

        // Determine per-item step along the main axis and the starting offset.
        let (step, start) = if matches!(self.config.alignment, DockAlignment::Justified)
            && count > 0
        {
            // Spread items across the available content length (bounds minus
            // end padding on both sides), distributing leftover space evenly.
            let main_extent = if vertical { bounds.height } else { bounds.width };
            let content = (main_extent - pad * 2.0).max(icon * count as f32);
            let step = if count > 1 {
                (content - icon) / (count as f32 - 1.0)
            } else {
                0.0
            };
            (step, pad)
        } else {
            // Packed at the start with fixed spacing.
            (icon + spacing, pad)
        };

        let mut rects = Vec::with_capacity(count);
        for i in 0..count {
            let main = start + i as f32 * step;
            let rect = if vertical {
                Rect::new(cross(bounds.x, bounds.width), bounds.y + main, icon, icon)
            } else {
                Rect::new(bounds.x + main, cross(bounds.y, bounds.height), icon, icon)
            };
            rects.push((i, rect));
        }
        rects
    }

    /// Main-axis length the dock content occupies (used for `Centered`).
    fn main_axis_length(&self) -> f32 {
        let icon = self.config.icon_size as f32;
        let pad = self.config.padding;
        let spacing = self.config.spacing;
        let count = self.items.len().max(1) as f32;
        // n icons + (n-1) gaps + padding at both ends.
        count * icon + (count - 1.0).max(0.0) * spacing + pad * 2.0
    }

    /// Compute the dock's main-axis origin and length on its edge.
    ///
    /// Returns `(origin, length)` along the main axis, honoring alignment:
    /// justified docks fill `screen_size`; centered docks use `content_len`
    /// centered within the screen.
    fn main_axis_origin(
        &self,
        screen_origin: f32,
        screen_size: f32,
        content_len: f32,
    ) -> (f32, f32) {
        match self.config.alignment {
            DockAlignment::Justified => (screen_origin, screen_size),
            DockAlignment::Centered => {
                let len = content_len.min(screen_size);
                (screen_origin + (screen_size - len) / 2.0, len)
            }
        }
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

    // ── Reveal state machine ─────────────────────────────────────────

    /// Report whether a window currently overlaps the dock's rectangle.
    ///
    /// Drives [`AutoHideMode::OnOverlap`]: when something occludes the dock it
    /// hides (unless the cursor is revealing it); when nothing overlaps it
    /// shows again. No-op for other modes.
    pub fn set_occluded(&mut self, occluded: bool) {
        if self.occluded != occluded {
            self.occluded = occluded;
            self.update_auto_hide();
        }
    }

    /// Whether a window is currently considered to overlap the dock.
    #[must_use]
    pub fn is_occluded(&self) -> bool {
        self.occluded
    }

    /// Report that the cursor entered (`true`) or left (`false`) the dock's
    /// reveal hot-zone (or the revealed dock itself).
    ///
    /// While revealing, the dock is shown regardless of mode/occlusion; once
    /// the cursor leaves, the dock returns to the state dictated by its mode.
    pub fn set_cursor_revealing(&mut self, revealing: bool) {
        if self.cursor_revealing != revealing {
            self.cursor_revealing = revealing;
            self.update_auto_hide();
        }
    }

    /// Whether the cursor is currently revealing the dock.
    #[must_use]
    pub fn is_cursor_revealing(&self) -> bool {
        self.cursor_revealing
    }

    /// Determine whether a cursor position falls within the dock's reveal
    /// hot-zone for the current screen, given the active auto-hide mode.
    ///
    /// The hot-zone is a `reveal_zone`-thick strip along the dock's anchored
    /// edge **plus** the dock's own rectangle when it is currently visible (so
    /// moving onto the revealed dock keeps it open). Returns `false` when
    /// auto-hide is off.
    #[must_use]
    pub fn cursor_in_reveal_zone(&self, screen: Rect, cursor: (f32, f32)) -> bool {
        if self.config.effective_auto_hide_mode() == AutoHideMode::Off {
            return false;
        }
        let (cx, cy) = cursor;
        let zone = self.config.reveal_zone.max(1.0);
        let edge_hit = match self.config.position {
            DockPosition::Bottom => cy >= screen.y + screen.height - zone,
            DockPosition::Top => cy <= screen.y + zone,
            DockPosition::Left => cx <= screen.x + zone,
            DockPosition::Right => cx >= screen.x + screen.width - zone,
        };
        if edge_hit {
            return true;
        }
        // Keep open while the cursor is over the (visible) dock body.
        if self.visible {
            let b = self.compute_bounds(screen);
            return cx >= b.x && cx <= b.x + b.width && cy >= b.y && cy <= b.y + b.height;
        }
        false
    }

    /// Process a cursor-position sample and update the reveal state.
    ///
    /// Convenience wrapper combining [`Dock::cursor_in_reveal_zone`] with
    /// [`Dock::set_cursor_revealing`]; returns the resulting visibility so the
    /// caller can decide whether to redraw.
    pub fn on_cursor_moved(&mut self, screen: Rect, cursor: (f32, f32)) -> bool {
        let revealing = self.cursor_in_reveal_zone(screen, cursor);
        self.set_cursor_revealing(revealing);
        self.visible
    }

    /// Recompute the auto-hide state from the current mode, occlusion and
    /// reveal inputs, transitioning the [`AutoHideState`] accordingly.
    ///
    /// Returns `true` if the visibility changed.
    pub fn update_auto_hide(&mut self) -> bool {
        let was_visible = self.visible;
        let desired_visible = match self.config.effective_auto_hide_mode() {
            AutoHideMode::Off => true,
            AutoHideMode::AlwaysHidden => self.cursor_revealing,
            AutoHideMode::OnOverlap => self.cursor_revealing || !self.occluded,
        };
        let state = if desired_visible {
            AutoHideState::Visible
        } else {
            AutoHideState::Hidden
        };
        self.set_auto_hide_state(state);
        self.visible != was_visible
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

    /// Replace the active configuration at runtime.
    ///
    /// This re-evaluates auto-hide: switching to [`AutoHideMode::Off`] forces
    /// the dock visible, while switching to [`AutoHideMode::AlwaysHidden`]
    /// hides it (unless the cursor is currently revealing it). The pinned-app
    /// list in the new config is **not** re-materialized — existing items are
    /// preserved so live running state and ordering survive a settings change.
    /// Use [`Dock::apply_pinned_apps`] explicitly to rebuild the pinned set.
    pub fn set_config(&mut self, config: DockConfig) {
        self.config = config;
        // Re-evaluate visibility against the new mode.
        match self.config.effective_auto_hide_mode() {
            AutoHideMode::Off => {
                self.set_auto_hide_state(AutoHideState::Visible);
            }
            AutoHideMode::OnOverlap | AutoHideMode::AlwaysHidden => {
                self.update_auto_hide();
            }
        }
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

    // ── Position-aware item layout (each edge) ──────────────────────

    const SCREEN: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };

    fn dock_with(position: DockPosition, n: usize) -> Dock {
        let mut dock = Dock::new(DockConfig {
            position,
            icon_size: 48,
            ..Default::default()
        });
        for i in 0..n {
            dock.add_running(&format!("app{i}"));
        }
        dock
    }

    #[test]
    fn test_item_rects_bottom_are_horizontal_row() {
        let dock = dock_with(DockPosition::Bottom, 3);
        let rects = dock.compute_item_rects(SCREEN);
        assert_eq!(rects.len(), 3);
        // x increases left→right, y constant near the bottom.
        assert!(rects[0].1.x < rects[1].1.x);
        assert!(rects[1].1.x < rects[2].1.x);
        assert!((rects[0].1.y - rects[2].1.y).abs() < f32::EPSILON);
        assert!(rects[0].1.y > 1000.0);
        // Each item is icon_size square.
        assert_eq!(rects[0].1.width, 48.0);
        assert_eq!(rects[0].1.height, 48.0);
    }

    #[test]
    fn test_item_rects_top_are_horizontal_row_at_top() {
        let dock = dock_with(DockPosition::Top, 2);
        let rects = dock.compute_item_rects(SCREEN);
        assert!(rects[0].1.x < rects[1].1.x);
        // Near the top of the screen.
        assert!(rects[0].1.y < 50.0);
    }

    #[test]
    fn test_item_rects_left_are_vertical_column() {
        let dock = dock_with(DockPosition::Left, 3);
        let rects = dock.compute_item_rects(SCREEN);
        // y increases top→bottom, x constant near the left edge.
        assert!(rects[0].1.y < rects[1].1.y);
        assert!(rects[1].1.y < rects[2].1.y);
        assert!((rects[0].1.x - rects[2].1.x).abs() < f32::EPSILON);
        assert!(rects[0].1.x < 50.0);
    }

    #[test]
    fn test_item_rects_right_are_vertical_column_at_right() {
        let dock = dock_with(DockPosition::Right, 2);
        let rects = dock.compute_item_rects(SCREEN);
        assert!(rects[0].1.y < rects[1].1.y);
        // Near the right edge.
        assert!(rects[0].1.x > 1800.0);
    }

    // ── Size affects rects ──────────────────────────────────────────

    #[test]
    fn test_icon_size_affects_item_rects() {
        let small = dock_with(DockPosition::Bottom, 2);
        let mut big = Dock::new(DockConfig {
            position: DockPosition::Bottom,
            icon_size: 96,
            ..Default::default()
        });
        big.add_running("app0");
        big.add_running("app1");

        let sr = small.compute_item_rects(SCREEN);
        let br = big.compute_item_rects(SCREEN);
        assert_eq!(sr[0].1.width, 48.0);
        assert_eq!(br[0].1.width, 96.0);
        // Larger icons => larger inter-item step.
        let small_step = sr[1].1.x - sr[0].1.x;
        let big_step = br[1].1.x - br[0].1.x;
        assert!(big_step > small_step);
    }

    #[test]
    fn test_thickness_affects_bounds() {
        let mut dock = Dock::new(DockConfig {
            position: DockPosition::Bottom,
            thickness: Some(120),
            ..Default::default()
        });
        dock.add_running("a");
        let b = dock.compute_bounds(SCREEN);
        assert_eq!(b.height, 120.0);
        // Anchored to the bottom edge.
        assert!((b.y + b.height - 1080.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_spacing_widens_layout() {
        let mut tight = Dock::new(DockConfig {
            position: DockPosition::Bottom,
            spacing: 0.0,
            ..Default::default()
        });
        let mut loose = Dock::new(DockConfig {
            position: DockPosition::Bottom,
            spacing: 20.0,
            ..Default::default()
        });
        for d in [&mut tight, &mut loose] {
            d.add_running("a");
            d.add_running("b");
        }
        let ts = tight.compute_item_rects(SCREEN);
        let ls = loose.compute_item_rects(SCREEN);
        assert!((ls[1].1.x - ls[0].1.x) > (ts[1].1.x - ts[0].1.x));
    }

    // ── Alignment ───────────────────────────────────────────────────

    #[test]
    fn test_justified_spans_full_edge() {
        let mut dock = Dock::new(DockConfig {
            position: DockPosition::Bottom,
            alignment: DockAlignment::Justified,
            ..Default::default()
        });
        dock.add_running("a");
        dock.add_running("b");
        let b = dock.compute_bounds(SCREEN);
        assert_eq!(b.x, 0.0);
        assert_eq!(b.width, 1920.0);
        let rects = dock.compute_item_rects(SCREEN);
        // First item near the left, last item near the right.
        assert!(rects[0].1.x < 100.0);
        assert!(rects[1].1.x > 1700.0);
    }

    #[test]
    fn test_centered_is_narrower_than_screen() {
        let mut dock = Dock::new(DockConfig {
            position: DockPosition::Bottom,
            alignment: DockAlignment::Centered,
            ..Default::default()
        });
        dock.add_running("a");
        let b = dock.compute_bounds(SCREEN);
        assert!(b.width < 1920.0);
        assert!(b.x > 0.0);
    }

    // ── Auto-hide modes & reveal state machine ──────────────────────

    #[test]
    fn test_effective_mode_from_bool() {
        let off = DockConfig::default();
        assert_eq!(off.effective_auto_hide_mode(), AutoHideMode::Off);
        let legacy = DockConfig {
            auto_hide: true,
            ..Default::default()
        };
        assert_eq!(
            legacy.effective_auto_hide_mode(),
            AutoHideMode::AlwaysHidden
        );
        let explicit = DockConfig {
            auto_hide_mode: AutoHideMode::OnOverlap,
            ..Default::default()
        };
        assert_eq!(explicit.effective_auto_hide_mode(), AutoHideMode::OnOverlap);
    }

    #[test]
    fn test_always_hidden_reveal_toggles() {
        let mut dock = Dock::new(DockConfig {
            auto_hide_mode: AutoHideMode::AlwaysHidden,
            ..Default::default()
        });
        assert!(!dock.is_visible());
        // Cursor reaches the bottom edge → reveal.
        let changed = dock.on_cursor_moved(SCREEN, (960.0, 1079.5));
        assert!(changed);
        assert!(dock.is_visible());
        // Cursor moves away → hide again.
        dock.on_cursor_moved(SCREEN, (960.0, 200.0));
        assert!(!dock.is_visible());
    }

    #[test]
    fn test_on_overlap_hides_only_when_occluded() {
        let mut dock = Dock::new(DockConfig {
            auto_hide_mode: AutoHideMode::OnOverlap,
            ..Default::default()
        });
        dock.add_running("a");
        assert!(dock.is_visible()); // nothing overlapping yet
        dock.set_occluded(true);
        assert!(!dock.is_visible());
        // Cursor reveals even while occluded.
        dock.set_cursor_revealing(true);
        assert!(dock.is_visible());
        dock.set_cursor_revealing(false);
        assert!(!dock.is_visible());
        // Window moves away → visible again.
        dock.set_occluded(false);
        assert!(dock.is_visible());
    }

    #[test]
    fn test_reveal_zone_edge_detection_per_position() {
        let bottom = Dock::new(DockConfig {
            position: DockPosition::Bottom,
            auto_hide_mode: AutoHideMode::AlwaysHidden,
            ..Default::default()
        });
        assert!(bottom.cursor_in_reveal_zone(SCREEN, (5.0, 1079.0)));
        assert!(!bottom.cursor_in_reveal_zone(SCREEN, (5.0, 5.0)));

        let left = Dock::new(DockConfig {
            position: DockPosition::Left,
            auto_hide_mode: AutoHideMode::AlwaysHidden,
            ..Default::default()
        });
        assert!(left.cursor_in_reveal_zone(SCREEN, (0.5, 500.0)));
        assert!(!left.cursor_in_reveal_zone(SCREEN, (500.0, 500.0)));
    }

    #[test]
    fn test_reveal_zone_off_when_mode_off() {
        let dock = Dock::new(DockConfig::default());
        assert!(!dock.cursor_in_reveal_zone(SCREEN, (960.0, 1079.5)));
    }

    #[test]
    fn test_set_config_to_off_forces_visible() {
        let mut dock = Dock::new(DockConfig {
            auto_hide_mode: AutoHideMode::AlwaysHidden,
            ..Default::default()
        });
        assert!(!dock.is_visible());
        dock.set_config(DockConfig::default()); // mode Off
        assert!(dock.is_visible());
    }

    // ── Pinning: add / remove / reorder ─────────────────────────────

    #[test]
    fn test_move_pinned_reorders_underlying_vec() {
        let mut dock = default_dock();
        dock.add_pinned("a", "A", "i");
        dock.add_pinned("b", "B", "i");
        dock.add_pinned("c", "C", "i");
        // Move "a" (pos 0) to pos 2 → order becomes b, c, a.
        assert!(dock.move_pinned(0, 2));
        let order: Vec<&str> = dock.items().iter().map(|i| i.app_id.as_str()).collect();
        assert_eq!(order, vec!["b", "c", "a"]);
        // pinned_position re-indexed to match display order.
        assert_eq!(dock.items()[0].pinned_position, Some(0));
        assert_eq!(dock.items()[2].pinned_position, Some(2));
    }

    #[test]
    fn test_move_pinned_noop_out_of_range() {
        let mut dock = default_dock();
        dock.add_pinned("a", "A", "i");
        assert!(!dock.move_pinned(0, 5));
        assert!(!dock.move_pinned(0, 0));
    }

    #[test]
    fn test_pinned_apps_snapshot_roundtrip() {
        let mut dock = default_dock();
        dock.add_pinned("a", "A", "ia");
        dock.add_pinned("b", "B", "ib");
        dock.add_running("running-only");
        let snap = dock.pinned_apps();
        assert_eq!(snap.len(), 2); // running excluded
        assert_eq!(snap[0].app_id, "a");
        assert_eq!(snap[1].icon, "ib");

        // Apply into a fresh dock reproduces the pinned set.
        let mut other = default_dock();
        other.apply_pinned_apps(&snap);
        assert_eq!(other.pinned_items().len(), 2);
        assert_eq!(other.pinned_items()[0].app_id, "a");
    }

    #[test]
    fn test_config_pinned_apps_materialized_on_new() {
        let config = DockConfig {
            pinned_apps: vec![
                PinnedApp::new("files", "Files", "folder"),
                PinnedApp::new("term", "Terminal", "terminal"),
            ],
            ..Default::default()
        };
        let dock = Dock::new(config);
        assert_eq!(dock.pinned_items().len(), 2);
        assert_eq!(dock.items()[0].app_id, "files");
    }

    #[test]
    fn test_apply_pinned_apps_replaces_existing() {
        let mut dock = default_dock();
        dock.add_pinned("old", "Old", "i");
        dock.apply_pinned_apps(&[PinnedApp::new("new", "New", "i")]);
        assert_eq!(dock.pinned_items().len(), 1);
        assert_eq!(dock.pinned_items()[0].app_id, "new");
    }

    // ── Serde roundtrip of the extended config ──────────────────────

    #[test]
    fn test_config_serde_roundtrip_with_new_fields() {
        let config = DockConfig {
            position: DockPosition::Left,
            thickness: Some(72),
            padding: 12.0,
            spacing: 6.0,
            auto_hide_mode: AutoHideMode::OnOverlap,
            show_labels: true,
            alignment: DockAlignment::Justified,
            pinned_apps: vec![PinnedApp::new("a", "A", "i")],
            ..Default::default()
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let back: DockConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.position, DockPosition::Left);
        assert_eq!(back.thickness, Some(72));
        assert_eq!(back.alignment, DockAlignment::Justified);
        assert_eq!(back.auto_hide_mode, AutoHideMode::OnOverlap);
        assert!(back.show_labels);
        assert_eq!(back.pinned_apps.len(), 1);
    }

    #[test]
    fn test_config_deserialize_legacy_without_new_fields() {
        // A config saved before the new fields existed must still load,
        // falling back to defaults for the missing keys.
        let legacy = r#"{
            "position": "Bottom",
            "icon_size": 48,
            "magnification_factor": 1.5,
            "magnification_enabled": true,
            "auto_hide": false,
            "auto_hide_delay_ms": 500,
            "show_running_indicators": true,
            "monitor_mode": "PrimaryOnly",
            "max_recent_items": 10,
            "click_running_behavior": "SmartToggle"
        }"#;
        let config: DockConfig = serde_json::from_str(legacy).expect("legacy deserialize");
        assert_eq!(config.padding, 8.0);
        assert_eq!(config.spacing, 0.0);
        assert_eq!(config.alignment, DockAlignment::Centered);
        assert_eq!(config.auto_hide_mode, AutoHideMode::Off);
        assert!(config.pinned_apps.is_empty());
        assert!(!config.show_labels);
    }
}
