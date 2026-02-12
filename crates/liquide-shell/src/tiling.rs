//! Advanced tiling engine with 7 layout algorithms and snap zone detection.
//!
//! Provides split-horizontal, split-vertical, quadrant, three-column, spiral,
//! stacking, and custom-grid layouts, plus cursor-driven snap zones and
//! per-workspace configuration.

use std::collections::HashMap;
use std::fmt;

use liquide_compositor::geometry::Rect;
use serde::{Deserialize, Serialize};

use crate::workspace::WorkspaceId;

// ---------------------------------------------------------------------------
// Mode
// ---------------------------------------------------------------------------

/// Global tiling policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TilingMode {
    Floating,
    Tiling,
    Hybrid,
}

impl fmt::Display for TilingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Floating => write!(f, "Floating"),
            Self::Tiling => write!(f, "Tiling"),
            Self::Hybrid => write!(f, "Hybrid"),
        }
    }
}

// ---------------------------------------------------------------------------
// Layout kind
// ---------------------------------------------------------------------------

/// Predefined layout algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TilingLayoutKind {
    SplitHorizontal,
    SplitVertical,
    Quadrant,
    ThreeColumn,
    Spiral,
    Stacking,
    CustomGrid,
}

impl fmt::Display for TilingLayoutKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SplitHorizontal => write!(f, "SplitHorizontal"),
            Self::SplitVertical => write!(f, "SplitVertical"),
            Self::Quadrant => write!(f, "Quadrant"),
            Self::ThreeColumn => write!(f, "ThreeColumn"),
            Self::Spiral => write!(f, "Spiral"),
            Self::Stacking => write!(f, "Stacking"),
            Self::CustomGrid => write!(f, "CustomGrid"),
        }
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Persistent configuration for the tiling engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TilingConfig {
    /// Whether tiling is available.
    pub enabled: bool,
    /// Default mode for new workspaces.
    pub default_mode: TilingMode,
    /// Default layout kind.
    pub default_layout: TilingLayoutKind,
    /// Inner gap between tiled windows (px).
    pub gap: f32,
    /// Outer gap between windows and screen edge (px).
    pub outer_gap: f32,
    /// Snap detection threshold in pixels.
    pub snap_threshold: f32,
    /// Ratio of screen given to the master pane in split layouts (0..1).
    pub master_ratio: f32,
    /// Honour minimum window sizes when tiling.
    pub respect_min_size: bool,
    /// Show a visual indicator during drag-snap.
    pub tiling_indicator: bool,
}

impl Default for TilingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_mode: TilingMode::Floating,
            default_layout: TilingLayoutKind::SplitHorizontal,
            gap: 8.0,
            outer_gap: 8.0,
            snap_threshold: 32.0,
            master_ratio: 0.55,
            respect_min_size: true,
            tiling_indicator: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Custom grid
// ---------------------------------------------------------------------------

/// Configuration for the custom grid layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomGridConfig {
    /// Number of rows.
    pub rows: u32,
    /// Number of columns.
    pub columns: u32,
    /// Relative column widths (must sum to 1.0).
    pub col_ratios: Vec<f32>,
    /// Relative row heights (must sum to 1.0).
    pub row_ratios: Vec<f32>,
}

impl Default for CustomGridConfig {
    fn default() -> Self {
        Self {
            rows: 2,
            columns: 2,
            col_ratios: vec![0.5, 0.5],
            row_ratios: vec![0.5, 0.5],
        }
    }
}

// ---------------------------------------------------------------------------
// Snap zones
// ---------------------------------------------------------------------------

/// Named snap zone on the screen perimeter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SnapZone {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

impl fmt::Display for SnapZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
            Self::Top => write!(f, "Top"),
            Self::Bottom => write!(f, "Bottom"),
            Self::TopLeft => write!(f, "TopLeft"),
            Self::TopRight => write!(f, "TopRight"),
            Self::BottomLeft => write!(f, "BottomLeft"),
            Self::BottomRight => write!(f, "BottomRight"),
            Self::Center => write!(f, "Center"),
        }
    }
}

/// Visual preview shown while the user drags a window near a snap zone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapPreview {
    pub zone: SnapZone,
    pub preview_rect: Rect,
    pub active: bool,
}

// ---------------------------------------------------------------------------
// Window rules
// ---------------------------------------------------------------------------

/// Per-app override for tiling behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WindowTileBehavior {
    Tiling,
    Floating,
    ForceFloating,
}

/// Rule that matches windows by app ID pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowRule {
    /// Substring or glob to match against `Window::app_id`.
    pub app_id_pattern: String,
    /// Tiling behaviour to apply.
    pub tile_behavior: WindowTileBehavior,
    /// Default snap zone when first tiled.
    pub default_zone: Option<SnapZone>,
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

/// Named layout preset (stored arrangement).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TilingPreset {
    pub name: String,
    pub layout: TilingLayoutKind,
    pub window_positions: Vec<Rect>,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// The tiling engine runtime.
pub struct TilingEngine {
    config: TilingConfig,
    custom_grid: CustomGridConfig,
    per_workspace_layouts: HashMap<WorkspaceId, TilingLayoutKind>,
    per_workspace_modes: HashMap<WorkspaceId, TilingMode>,
    window_rules: Vec<WindowRule>,
    presets: Vec<TilingPreset>,
    current_snap_preview: Option<SnapPreview>,
}

impl TilingEngine {
    /// Create a new tiling engine.
    #[must_use]
    pub fn new(config: TilingConfig) -> Self {
        Self {
            config,
            custom_grid: CustomGridConfig::default(),
            per_workspace_layouts: HashMap::new(),
            per_workspace_modes: HashMap::new(),
            window_rules: Vec::new(),
            presets: Vec::new(),
            current_snap_preview: None,
        }
    }

    // =======================================================================
    // Layout dispatch
    // =======================================================================

    /// Arrange `window_count` windows using the given layout within `screen`.
    ///
    /// Returns one [`Rect`] per window.
    #[must_use]
    pub fn arrange(
        &self,
        kind: TilingLayoutKind,
        window_count: usize,
        screen: Rect,
    ) -> Vec<Rect> {
        if window_count == 0 {
            return Vec::new();
        }
        match kind {
            TilingLayoutKind::SplitHorizontal => self.arrange_split_h(window_count, screen),
            TilingLayoutKind::SplitVertical => self.arrange_split_v(window_count, screen),
            TilingLayoutKind::Quadrant => self.arrange_quadrant(window_count, screen),
            TilingLayoutKind::ThreeColumn => self.arrange_three_column(window_count, screen),
            TilingLayoutKind::Spiral => self.arrange_spiral(window_count, screen),
            TilingLayoutKind::Stacking => self.arrange_stacking(window_count, screen),
            TilingLayoutKind::CustomGrid => self.arrange_custom_grid(window_count, screen),
        }
    }

    // =======================================================================
    // Individual layouts
    // =======================================================================

    /// Master-stack horizontal split.
    ///
    /// The first window takes `master_ratio` of the width; remaining windows
    /// share the rest vertically.
    #[must_use]
    pub fn arrange_split_h(&self, n: usize, screen: Rect) -> Vec<Rect> {
        let g = self.config.gap;
        let og = self.config.outer_gap;
        let usable = Rect::new(
            screen.x + og,
            screen.y + og,
            screen.width - 2.0 * og,
            screen.height - 2.0 * og,
        );

        if n == 1 {
            return vec![usable];
        }

        let master_w = usable.width * self.config.master_ratio - g / 2.0;
        let stack_w = usable.width - master_w - g;
        let stack_count = (n - 1) as f32;
        let stack_h = (usable.height - g * (stack_count - 1.0).max(0.0)) / stack_count;

        let mut rects = vec![Rect::new(usable.x, usable.y, master_w, usable.height)];
        let sx = usable.x + master_w + g;
        for i in 0..(n - 1) {
            let y = usable.y + i as f32 * (stack_h + g);
            rects.push(Rect::new(sx, y, stack_w, stack_h));
        }
        rects
    }

    /// Master-stack vertical split.
    #[must_use]
    pub fn arrange_split_v(&self, n: usize, screen: Rect) -> Vec<Rect> {
        let g = self.config.gap;
        let og = self.config.outer_gap;
        let usable = Rect::new(
            screen.x + og,
            screen.y + og,
            screen.width - 2.0 * og,
            screen.height - 2.0 * og,
        );

        if n == 1 {
            return vec![usable];
        }

        let master_h = usable.height * self.config.master_ratio - g / 2.0;
        let stack_h = usable.height - master_h - g;
        let stack_count = (n - 1) as f32;
        let stack_w = (usable.width - g * (stack_count - 1.0).max(0.0)) / stack_count;

        let mut rects = vec![Rect::new(usable.x, usable.y, usable.width, master_h)];
        let sy = usable.y + master_h + g;
        for i in 0..(n - 1) {
            let x = usable.x + i as f32 * (stack_w + g);
            rects.push(Rect::new(x, sy, stack_w, stack_h));
        }
        rects
    }

    /// 2x2 quadrant layout — up to 4 windows.
    #[must_use]
    pub fn arrange_quadrant(&self, n: usize, screen: Rect) -> Vec<Rect> {
        let g = self.config.gap;
        let og = self.config.outer_gap;
        let usable = Rect::new(
            screen.x + og,
            screen.y + og,
            screen.width - 2.0 * og,
            screen.height - 2.0 * og,
        );

        let half_w = (usable.width - g) / 2.0;
        let half_h = (usable.height - g) / 2.0;

        let slots = [
            Rect::new(usable.x, usable.y, half_w, half_h),
            Rect::new(usable.x + half_w + g, usable.y, half_w, half_h),
            Rect::new(usable.x, usable.y + half_h + g, half_w, half_h),
            Rect::new(usable.x + half_w + g, usable.y + half_h + g, half_w, half_h),
        ];

        slots.iter().take(n.min(4)).copied().collect()
    }

    /// Three-column layout: narrow–wide–narrow.
    #[must_use]
    pub fn arrange_three_column(&self, n: usize, screen: Rect) -> Vec<Rect> {
        let g = self.config.gap;
        let og = self.config.outer_gap;
        let usable = Rect::new(
            screen.x + og,
            screen.y + og,
            screen.width - 2.0 * og,
            screen.height - 2.0 * og,
        );

        if n == 1 {
            return vec![usable];
        }

        let side_w = usable.width * 0.2;
        let center_w = usable.width - 2.0 * side_w - 2.0 * g;

        let mut rects = Vec::new();
        // First window → center (master).
        rects.push(Rect::new(usable.x + side_w + g, usable.y, center_w, usable.height));

        // Remaining windows alternate left / right columns.
        let mut left_items = Vec::new();
        let mut right_items = Vec::new();
        for i in 1..n {
            if i % 2 == 1 {
                left_items.push(i);
            } else {
                right_items.push(i);
            }
        }

        let layout_column = |items: &[usize], x: f32, w: f32| -> Vec<(usize, Rect)> {
            if items.is_empty() {
                return Vec::new();
            }
            let count = items.len() as f32;
            let h = (usable.height - g * (count - 1.0).max(0.0)) / count;
            items
                .iter()
                .enumerate()
                .map(|(j, &idx)| {
                    let y = usable.y + j as f32 * (h + g);
                    (idx, Rect::new(x, y, w, h))
                })
                .collect()
        };

        let left_rects = layout_column(&left_items, usable.x, side_w);
        let right_rects = layout_column(&right_items, usable.x + side_w + g + center_w + g, side_w);

        // Merge into a flat Vec at the right indices.
        let total = n;
        let mut result = vec![Rect::new(0.0, 0.0, 0.0, 0.0); total];
        result[0] = rects[0];
        for (idx, r) in left_rects {
            result[idx] = r;
        }
        for (idx, r) in right_rects {
            result[idx] = r;
        }
        result
    }

    /// Fibonacci / spiral layout.
    #[must_use]
    pub fn arrange_spiral(&self, n: usize, screen: Rect) -> Vec<Rect> {
        let g = self.config.gap;
        let og = self.config.outer_gap;
        let mut area = Rect::new(
            screen.x + og,
            screen.y + og,
            screen.width - 2.0 * og,
            screen.height - 2.0 * og,
        );

        let mut rects = Vec::with_capacity(n);
        for i in 0..n {
            if i == n - 1 {
                rects.push(area);
                break;
            }
            // Alternate splitting direction.
            if i % 2 == 0 {
                // Split horizontally (left / right).
                let w = area.width * self.config.master_ratio - g / 2.0;
                rects.push(Rect::new(area.x, area.y, w, area.height));
                area = Rect::new(area.x + w + g, area.y, area.width - w - g, area.height);
            } else {
                // Split vertically (top / bottom).
                let h = area.height * self.config.master_ratio - g / 2.0;
                rects.push(Rect::new(area.x, area.y, area.width, h));
                area = Rect::new(area.x, area.y + h + g, area.width, area.height - h - g);
            }
        }
        rects
    }

    /// Stacking (monocle) layout — all windows occupy the full area.
    #[must_use]
    pub fn arrange_stacking(&self, n: usize, screen: Rect) -> Vec<Rect> {
        let og = self.config.outer_gap;
        let usable = Rect::new(
            screen.x + og,
            screen.y + og,
            screen.width - 2.0 * og,
            screen.height - 2.0 * og,
        );
        vec![usable; n]
    }

    /// Custom grid layout using [`CustomGridConfig`].
    #[must_use]
    pub fn arrange_custom_grid(&self, n: usize, screen: Rect) -> Vec<Rect> {
        let g = self.config.gap;
        let og = self.config.outer_gap;
        let usable = Rect::new(
            screen.x + og,
            screen.y + og,
            screen.width - 2.0 * og,
            screen.height - 2.0 * og,
        );

        let rows = self.custom_grid.rows as usize;
        let cols = self.custom_grid.columns as usize;
        let total_slots = rows * cols;

        let col_ratios = &self.custom_grid.col_ratios;
        let row_ratios = &self.custom_grid.row_ratios;

        let total_col_gap = g * (cols as f32 - 1.0).max(0.0);
        let total_row_gap = g * (rows as f32 - 1.0).max(0.0);
        let avail_w = usable.width - total_col_gap;
        let avail_h = usable.height - total_row_gap;

        let mut rects = Vec::new();
        let mut y_offset = usable.y;
        for r in 0..rows {
            let rh = avail_h * row_ratios.get(r).copied().unwrap_or(1.0 / rows as f32);
            let mut x_offset = usable.x;
            for c in 0..cols {
                let cw = avail_w * col_ratios.get(c).copied().unwrap_or(1.0 / cols as f32);
                if rects.len() < n.min(total_slots) {
                    rects.push(Rect::new(x_offset, y_offset, cw, rh));
                }
                x_offset += cw + g;
            }
            y_offset += rh + g;
        }
        rects
    }

    // =======================================================================
    // Snap zones
    // =======================================================================

    /// Detect which snap zone a cursor position falls into, if any.
    #[must_use]
    pub fn detect_snap_zone(&self, cursor_x: f32, cursor_y: f32, screen: Rect) -> Option<SnapZone> {
        let t = self.config.snap_threshold;
        let near_left = cursor_x - screen.x < t;
        let near_right = (screen.x + screen.width) - cursor_x < t;
        let near_top = cursor_y - screen.y < t;
        let near_bottom = (screen.y + screen.height) - cursor_y < t;

        match (near_left, near_right, near_top, near_bottom) {
            (true, _, true, _) => Some(SnapZone::TopLeft),
            (true, _, _, true) => Some(SnapZone::BottomLeft),
            (_, true, true, _) => Some(SnapZone::TopRight),
            (_, true, _, true) => Some(SnapZone::BottomRight),
            (true, _, _, _) => Some(SnapZone::Left),
            (_, true, _, _) => Some(SnapZone::Right),
            (_, _, true, _) => Some(SnapZone::Top),
            (_, _, _, true) => Some(SnapZone::Bottom),
            _ => None,
        }
    }

    /// Compute the rectangle that a window would occupy when snapped to `zone`.
    #[must_use]
    pub fn snap_zone_rect(&self, zone: SnapZone, screen: Rect) -> Rect {
        let og = self.config.outer_gap;
        let usable = Rect::new(
            screen.x + og,
            screen.y + og,
            screen.width - 2.0 * og,
            screen.height - 2.0 * og,
        );
        let hw = usable.width / 2.0;
        let hh = usable.height / 2.0;

        match zone {
            SnapZone::Left => Rect::new(usable.x, usable.y, hw, usable.height),
            SnapZone::Right => Rect::new(usable.x + hw, usable.y, hw, usable.height),
            SnapZone::Top => Rect::new(usable.x, usable.y, usable.width, hh),
            SnapZone::Bottom => Rect::new(usable.x, usable.y + hh, usable.width, hh),
            SnapZone::TopLeft => Rect::new(usable.x, usable.y, hw, hh),
            SnapZone::TopRight => Rect::new(usable.x + hw, usable.y, hw, hh),
            SnapZone::BottomLeft => Rect::new(usable.x, usable.y + hh, hw, hh),
            SnapZone::BottomRight => Rect::new(usable.x + hw, usable.y + hh, hw, hh),
            SnapZone::Center => usable,
        }
    }

    // =======================================================================
    // Per-workspace config
    // =======================================================================

    /// Set the layout kind for a specific workspace.
    pub fn set_workspace_layout(&mut self, ws: WorkspaceId, layout: TilingLayoutKind) {
        self.per_workspace_layouts.insert(ws, layout);
    }

    /// Set the tiling mode for a specific workspace.
    pub fn set_workspace_mode(&mut self, ws: WorkspaceId, mode: TilingMode) {
        self.per_workspace_modes.insert(ws, mode);
    }

    /// Get the effective layout for a workspace (falls back to default).
    #[must_use]
    pub fn workspace_layout(&self, ws: WorkspaceId) -> TilingLayoutKind {
        self.per_workspace_layouts
            .get(&ws)
            .copied()
            .unwrap_or(self.config.default_layout)
    }

    /// Get the effective mode for a workspace.
    #[must_use]
    pub fn workspace_mode(&self, ws: WorkspaceId) -> TilingMode {
        self.per_workspace_modes
            .get(&ws)
            .copied()
            .unwrap_or(self.config.default_mode)
    }

    // =======================================================================
    // Window rules
    // =======================================================================

    /// Register a window rule.
    pub fn add_window_rule(&mut self, rule: WindowRule) {
        self.window_rules.push(rule);
    }

    /// Match a window's `app_id` against registered rules.
    #[must_use]
    pub fn match_window_rule(&self, app_id: &str) -> Option<&WindowRule> {
        self.window_rules
            .iter()
            .find(|r| app_id.contains(&r.app_id_pattern))
    }

    // =======================================================================
    // Presets
    // =======================================================================

    /// Save a named preset.
    pub fn save_preset(&mut self, preset: TilingPreset) {
        if let Some(existing) = self.presets.iter_mut().find(|p| p.name == preset.name) {
            *existing = preset;
        } else {
            self.presets.push(preset);
        }
    }

    /// Retrieve a preset by name.
    #[must_use]
    pub fn load_preset(&self, name: &str) -> Option<&TilingPreset> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// All registered presets.
    #[must_use]
    pub fn presets(&self) -> &[TilingPreset] {
        &self.presets
    }

    // =======================================================================
    // Snap preview
    // =======================================================================

    /// Set or clear the current snap preview.
    pub fn set_snap_preview(&mut self, preview: Option<SnapPreview>) {
        self.current_snap_preview = preview;
    }

    /// Get the current snap preview.
    #[must_use]
    pub fn snap_preview(&self) -> Option<&SnapPreview> {
        self.current_snap_preview.as_ref()
    }

    /// The active configuration.
    #[must_use]
    pub fn config(&self) -> &TilingConfig {
        &self.config
    }

    /// Set the custom grid configuration.
    pub fn set_custom_grid(&mut self, grid: CustomGridConfig) {
        self.custom_grid = grid;
    }

    /// Get the custom grid configuration.
    #[must_use]
    pub fn custom_grid(&self) -> &CustomGridConfig {
        &self.custom_grid
    }
}

impl fmt::Display for TilingEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TilingEngine(mode={}, layout={}, rules={})",
            self.config.default_mode,
            self.config.default_layout,
            self.window_rules.len(),
        )
    }
}
