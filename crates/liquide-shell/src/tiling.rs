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

// `SnapZone` is the single shell-side snap type. It is the serializable,
// always-active counterpart of the canonical `liquide_tiling::SnapTarget`
// (which carries an extra inactive `None` variant and is NOT `serde`-derived —
// `Window::tile_zone: Option<SnapZone>` and `WindowRule`/`SnapPreview` are
// persisted, so the shell type must stay `Serialize`/`Deserialize`). Rather
// than a lossy type-alias (t52-e3 verified the serde + `None`-variant gap), the
// two are unified at the conversion edge via these `From` impls — the canonical
// snap geometry/detection (`SnapZones`) is consumed through this single bridge.

impl From<SnapZone> for liquide_tiling::SnapTarget {
    fn from(zone: SnapZone) -> Self {
        match zone {
            SnapZone::Left => Self::Left,
            SnapZone::Right => Self::Right,
            SnapZone::Top => Self::Top,
            SnapZone::Bottom => Self::Bottom,
            SnapZone::TopLeft => Self::TopLeft,
            SnapZone::TopRight => Self::TopRight,
            SnapZone::BottomLeft => Self::BottomLeft,
            SnapZone::BottomRight => Self::BottomRight,
            SnapZone::Center => Self::Center,
        }
    }
}

impl SnapZone {
    /// Map a canonical [`liquide_tiling::SnapTarget`] to a shell [`SnapZone`].
    ///
    /// The inactive `SnapTarget::None` maps to `None`; every active target maps
    /// to its shell zone. (An `impl From<SnapTarget> for Option<SnapZone>` would
    /// violate the orphan rule — both types are foreign — so this reverse
    /// direction is an inherent associated fn.)
    #[must_use]
    pub fn from_target(target: liquide_tiling::SnapTarget) -> Option<Self> {
        use liquide_tiling::SnapTarget;
        Some(match target {
            SnapTarget::None => return None,
            SnapTarget::Left => SnapZone::Left,
            SnapTarget::Right => SnapZone::Right,
            SnapTarget::Top => SnapZone::Top,
            SnapTarget::Bottom => SnapZone::Bottom,
            SnapTarget::TopLeft => SnapZone::TopLeft,
            SnapTarget::TopRight => SnapZone::TopRight,
            SnapTarget::BottomLeft => SnapZone::BottomLeft,
            SnapTarget::BottomRight => SnapZone::BottomRight,
            SnapTarget::Center => SnapZone::Center,
        })
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

/// Slimmer-named handle for the shell-side tiling state.
///
/// The shell retains `TilingEngine` for the config/preset/rule/render-state and
/// per-workspace layout-kind map that have **no canonical equivalent** (these
/// stay shell-side by design — `liquide_tiling` is the layout/snap *policy*
/// engine, not a config store). `TilingState` is the forward-looking name for
/// that slimmer role; the struct keeps the `TilingEngine` name during migration
/// so the `lib.rs` re-export (t52-e4) and `shell/mod.rs` field (t52-e5) keep
/// compiling. Callers may use either name.
pub type TilingState = TilingEngine;

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

// ---------------------------------------------------------------------------
// Canonical `liquide-tiling` drive (t51-e13)
// ---------------------------------------------------------------------------
//
// Wires the canonical `liquide_tiling::TilingEngine` / `SnapZones` (held in
// `Shell::chrome_tiling`) into the running shell so tiling is actually driven
// (fixes t49-e5-F05: the engine + snap zones previously had zero production
// callers and `tile_layout` had no caller). Layout *computation* and snap
// geometry are now SINGLE-SOURCED onto these canonical paths; the shell-side
// `TilingEngine` above is retained only for its config/preset/window-rule/
// snap-preview render-state (consumed by the scene cache via
// `Window::tiled`/`tile_zone`) and the per-workspace layout-kind/mode maps that
// other shell modules still read.
//
// t52-e3/e4 single-sourcing status (COMPLETE):
//   * SNAP/COMPUTE TYPE unified at the edge — the shell `SnapZone` is bridged to
//     the canonical `liquide_tiling::SnapTarget` via `From` impls above (a lossy
//     type-alias was rejected: `SnapTarget` is not `serde`-derived and carries
//     an extra `None` variant, but `Window::tile_zone`/`WindowRule`/`SnapPreview`
//     are serialized — Rule-4 thin-enum + From-impl outcome). The canonical
//     `SnapZones` geometry/detection is consumed through that single bridge.
//   * The shell compute methods (`arrange`/`arrange_*`, `detect_snap_zone`,
//     `snap_zone_rect`) have been DELETED (t52-e4). The production caller
//     `shell/batch.rs::tile_visible_windows` now delegates to
//     `tile_visible_windows_canonical` below, and the layout/snap geometry
//     tests (`tests/tiling_tests.rs` + the external
//     `liquide-session/tests/e2e_alignment_tiling.rs`) were migrated to assert
//     against the canonical `liquide_tiling` surface (canonical geometry
//     differs: no outer-gap inset on a single window via smart-gaps, and
//     `Top`/`Center` map to full-screen in `SnapZones::zone_preview`).
//     See `.orchestration/logs/t52-e3.md` and `t52-e4.md`.

use crate::shell::Shell;
use crate::shell::batch::WindowBatch;
use crate::window::{WindowId, WindowState};

impl Shell {
    /// Lazily construct the canonical `liquide_tiling::TilingEngine` held in
    /// `chrome_tiling`, returning a mutable reference.
    fn canonical_tiling(&mut self) -> &mut liquide_tiling::TilingEngine {
        self.chrome_tiling
            .get_or_insert_with(liquide_tiling::TilingEngine::new)
    }

    /// Consult the canonical snap zones for the current drag cursor position and
    /// record the resulting preview (used by the scene render-state). Returns
    /// the detected zone, if any. Called from `events.rs` during a move drag.
    ///
    /// The detection uses the work area (screen minus statusbar/dock) and the
    /// shell-internal tiling config's snap threshold, so the snap region matches
    /// the tiled bounds that `apply_snap_on_release` will assign.
    pub(crate) fn update_snap_preview_for_drag(&mut self, x: f32, y: f32) -> Option<SnapZone> {
        let work = self.work_area();
        let threshold = self.tiling().config().snap_threshold;
        let target = liquide_tiling::SnapZones::detect_zone((x, y), work, threshold);
        let zone: Option<SnapZone> = SnapZone::from_target(target);
        let preview = zone.map(|z| SnapPreview {
            zone: z,
            preview_rect: liquide_tiling::SnapZones::zone_preview(target, work),
            active: true,
        });
        self.tiling_mut().set_snap_preview(preview);
        zone
    }

    /// Clear any active snap preview (called when a drag ends without snapping).
    pub(crate) fn clear_snap_preview(&mut self) {
        if self.tiling().snap_preview().is_some() {
            self.tiling_mut().set_snap_preview(None);
        }
    }

    /// On drag release, if a snap zone is currently active, tile the dragged
    /// window into that zone via the canonical `liquide_tiling::SnapZones`
    /// geometry. Sets the window's `tiled`/`tile_zone` render-state, applies the
    /// new bounds through the canonical batch path, and clears the preview.
    /// Returns `true` if the window was snapped.
    pub(crate) fn apply_snap_on_release(&mut self, window_id: WindowId) -> bool {
        let zone = match self.tiling().snap_preview().map(|p| p.zone) {
            Some(z) => z,
            None => return false,
        };
        let work = self.work_area();
        let target: liquide_tiling::SnapTarget = zone.into();
        let rect = liquide_tiling::SnapZones::zone_preview(target, work);

        // Apply the snapped bounds through the canonical batch entry point.
        let mut batch = WindowBatch::with_capacity(1);
        batch.tile_layout(&[(window_id, rect)]);
        self.apply_batch(batch);

        if let Some(window) = self.windows.get_mut(&window_id) {
            window.tiled = true;
            window.tile_zone = Some(zone);
            if window.state == WindowState::Maximized {
                window.state = WindowState::Normal;
            }
        }
        self.tiling_mut().set_snap_preview(None);
        true
    }

    /// Tile all visible (non-minimized) windows on the active workspace using
    /// the canonical `liquide_tiling::TilingEngine` and apply the computed
    /// arrangement through the canonical batch path. This is the production
    /// driver for `tile_layout` (fixes t49-e5-F05). Returns the number of
    /// windows arranged.
    pub fn tile_visible_windows_canonical(&mut self) -> usize {
        let work = self.work_area();

        // Visible, non-minimized windows in deterministic order.
        let mut visible_ids: Vec<WindowId> = self
            .windows
            .values()
            .filter(|w| w.visible && w.state != WindowState::Minimized)
            .map(|w| w.id)
            .collect();
        visible_ids.sort_by_key(|id| id.0);
        if visible_ids.is_empty() {
            // Nothing to tile; drop any stale engine windows.
            let stale: Vec<u64> = self.canonical_tiling().windows().to_vec();
            for wid in stale {
                self.canonical_tiling().remove_window(wid);
            }
            return 0;
        }

        // Bring the canonical engine's window set into lockstep with the shell's
        // visible set (add new, remove gone) while preserving tiling order for
        // windows that persist.
        {
            let engine = self.canonical_tiling();
            let stale: Vec<u64> = engine
                .windows()
                .iter()
                .copied()
                .filter(|w| !visible_ids.iter().any(|id| id.0 == *w))
                .collect();
            for wid in stale {
                engine.remove_window(wid);
            }
            for id in &visible_ids {
                engine.add_window(id.0);
            }
        }

        // Compute the layout and map canonical ids back to shell window ids.
        let layout: Vec<(WindowId, liquide_compositor::geometry::Rect)> = self
            .canonical_tiling()
            .compute_layout(work)
            .into_iter()
            .map(|(wid, rect)| (WindowId(wid), rect))
            .collect();
        let count = layout.len();

        let mut batch = WindowBatch::with_capacity(count);
        batch.tile_layout(&layout);
        self.apply_batch(batch);

        // Mark the arranged windows as tiled (render-state for the scene cache).
        for (id, _) in &layout {
            if let Some(window) = self.windows.get_mut(id) {
                window.tiled = true;
                window.tile_zone = None;
                if window.state == WindowState::Maximized {
                    window.state = WindowState::Normal;
                }
            }
        }
        count
    }
}
