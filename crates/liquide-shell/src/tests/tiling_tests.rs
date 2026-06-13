use crate::tiling::*;
use crate::workspace::WorkspaceId;
use liquide_compositor::geometry::Rect;
use liquide_tiling::{
    SnapTarget, SnapZones, TilingEngine as CanonicalEngine, TilingGaps, TilingLayout,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
//
// Single-sourcing (t52-e3/e4): window-layout *computation* and snap geometry
// are owned by the canonical `liquide_tiling` engine. These tests exercise that
// canonical surface directly. The shell-side `TilingEngine` (below) is retained
// only for the config / preset / window-rule / snap-preview / per-workspace
// render-state that has no canonical equivalent (see `.orchestration/logs/`
// `t52-e3.md`), and its own tests follow further down.

fn default_engine() -> TilingEngine {
    TilingEngine::new(TilingConfig::default())
}

fn screen() -> Rect {
    Rect::new(0.0, 0.0, 1920.0, 1080.0)
}

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.1
}

/// Build a canonical engine for `layout` with `n` windows added (default gaps
/// and master ratio), then compute its layout over the 1920x1080 screen and
/// return the rects in window order.
fn canonical_rects(layout: TilingLayout, n: usize) -> Vec<Rect> {
    let mut engine = CanonicalEngine::with_config(layout, TilingGaps::default(), 0.55);
    for i in 0..n {
        engine.add_window(i as u64);
    }
    engine
        .compute_layout(screen())
        .into_iter()
        .map(|(_, r)| r)
        .collect()
}

// ========== TilingConfig defaults ==========

#[test]
fn tiling_config_default_values() {
    let cfg = TilingConfig::default();
    assert!(cfg.enabled);
    assert_eq!(cfg.default_mode, TilingMode::Floating);
    assert_eq!(cfg.default_layout, TilingLayoutKind::SplitHorizontal);
    assert!(approx_eq(cfg.gap, 8.0));
    assert!(approx_eq(cfg.outer_gap, 8.0));
    assert!(approx_eq(cfg.snap_threshold, 32.0));
    assert!(approx_eq(cfg.master_ratio, 0.55));
    assert!(cfg.respect_min_size);
    assert!(cfg.tiling_indicator);
}

// ========== CustomGridConfig defaults ==========

#[test]
fn custom_grid_config_default_values() {
    let g = CustomGridConfig::default();
    assert_eq!(g.rows, 2);
    assert_eq!(g.columns, 2);
    assert_eq!(g.col_ratios, vec![0.5, 0.5]);
    assert_eq!(g.row_ratios, vec![0.5, 0.5]);
}

// ===========================================================================
// CANONICAL layout computation (single-sourced onto `liquide_tiling`)
// ===========================================================================
//
// These tests exercise `liquide_tiling::{TilingEngine, algorithms}` — the
// single source of truth for tiled-window geometry. The canonical engine uses a
// `TilingGaps { inner: 8, outer: 8, smart_gaps: true }` model: a single tiled
// window fills the full work area (smart-gaps collapses gaps for one window),
// and multi-window layouts inset by the 8px outer gap (usable = 8,8,1904,1064).

// ---- zero windows -------------------------------------------------------

#[test]
fn canonical_zero_windows_returns_empty() {
    let rects = canonical_rects(TilingLayout::Columns, 0);
    assert!(rects.is_empty());
}

// ---- Columns (master left / stack right; ≈ shell SplitHorizontal) -------

#[test]
fn columns_single_window_fills_work_area() {
    // smart_gaps collapses all gaps for a single window → full screen.
    let rects = canonical_rects(TilingLayout::Columns, 1);
    assert_eq!(rects.len(), 1);
    assert!(approx_eq(rects[0].x, 0.0));
    assert!(approx_eq(rects[0].y, 0.0));
    assert!(approx_eq(rects[0].width, 1920.0));
    assert!(approx_eq(rects[0].height, 1080.0));
}

#[test]
fn columns_two_windows_master_and_stack() {
    let rects = canonical_rects(TilingLayout::Columns, 2);
    assert_eq!(rects.len(), 2);
    // usable = (8,8,1904,1064); master_w = 1904*0.55 - 4 = 1043.2.
    assert!(approx_eq(rects[0].x, 8.0));
    assert!(approx_eq(rects[0].width, 1043.2));
    assert!(approx_eq(rects[0].height, 1064.0));
    // stack = usable_w - master_w - gap.
    let expected_stack_w = 1904.0 - 1043.2 - 8.0;
    assert!(approx_eq(rects[1].width, expected_stack_w));
    assert!(approx_eq(rects[1].height, 1064.0));
    // No horizontal overlap between master and stack.
    assert!(rects[0].x + rects[0].width <= rects[1].x + 0.1);
}

#[test]
fn columns_three_windows_stack_divides_height() {
    let rects = canonical_rects(TilingLayout::Columns, 3);
    assert_eq!(rects.len(), 3);
    // master (idx 0) spans full usable height; the two stack windows split it.
    let stack_h = (1064.0 - 8.0) / 2.0;
    assert!(approx_eq(rects[1].height, stack_h));
    assert!(approx_eq(rects[2].height, stack_h));
    // Second stack window starts after the first + gap.
    assert!(approx_eq(rects[2].y, rects[1].y + stack_h + 8.0));
}

// ---- Rows (master top / stack bottom; ≈ shell SplitVertical) ------------

#[test]
fn rows_single_window_fills_work_area() {
    let rects = canonical_rects(TilingLayout::Rows, 1);
    assert_eq!(rects.len(), 1);
    assert!(approx_eq(rects[0].width, 1920.0));
    assert!(approx_eq(rects[0].height, 1080.0));
}

#[test]
fn rows_two_windows_master_top_stack_bottom() {
    let rects = canonical_rects(TilingLayout::Rows, 2);
    assert_eq!(rects.len(), 2);
    // master_h = usable_h * 0.55 - gap/2 = 1064 * 0.55 - 4 = 581.2.
    assert!(approx_eq(rects[0].height, 581.2));
    assert!(approx_eq(rects[0].width, 1904.0));
    // Stack starts below master + gap (usable.y=8).
    assert!(approx_eq(rects[1].y, 8.0 + 581.2 + 8.0));
}

// ---- Grid (equal cells; covers the shell Quadrant / CustomGrid cases) ---

#[test]
fn grid_four_windows_fills_2x2() {
    // n=4 → cols = ceil(sqrt(4)) = 2, rows = 2.
    let rects = canonical_rects(TilingLayout::Grid, 4);
    assert_eq!(rects.len(), 4);
    let cell_w = (1904.0 - 8.0) / 2.0; // minus one inter-column gap
    let cell_h = (1064.0 - 8.0) / 2.0; // minus one inter-row gap
    // Top-left cell.
    assert!(approx_eq(rects[0].x, 8.0));
    assert!(approx_eq(rects[0].y, 8.0));
    assert!(approx_eq(rects[0].width, cell_w));
    assert!(approx_eq(rects[0].height, cell_h));
    // Top-right cell.
    assert!(approx_eq(rects[1].x, 8.0 + cell_w + 8.0));
    assert!(approx_eq(rects[1].y, 8.0));
}

#[test]
fn grid_two_windows_produces_two_rects() {
    let rects = canonical_rects(TilingLayout::Grid, 2);
    assert_eq!(rects.len(), 2);
    // No overlap.
    assert!(
        rects[0].x + rects[0].width <= rects[1].x + 0.1
            || rects[0].y + rects[0].height <= rects[1].y + 0.1
    );
}

#[test]
fn grid_produces_one_rect_per_window() {
    // Grid never caps: every window gets a cell (no fixed-slot ceiling).
    let rects = canonical_rects(TilingLayout::Grid, 6);
    assert_eq!(rects.len(), 6);
    for r in &rects {
        assert!(r.width > 0.0 && r.height > 0.0);
    }
}

#[test]
fn grid_nine_windows_fills_3x3() {
    // n=9 → cols = ceil(sqrt(9)) = 3, rows = 3. Equal-sized cells.
    let rects = canonical_rects(TilingLayout::Grid, 9);
    assert_eq!(rects.len(), 9);
    let cell_w = (1904.0 - 2.0 * 8.0) / 3.0; // 2 inter-column gaps
    let cell_h = (1064.0 - 2.0 * 8.0) / 3.0; // 2 inter-row gaps
    assert!(approx_eq(rects[0].x, 8.0));
    assert!(approx_eq(rects[0].y, 8.0));
    assert!(approx_eq(rects[0].width, cell_w));
    assert!(approx_eq(rects[0].height, cell_h));
}

#[test]
fn grid_cells_do_not_overlap() {
    let rects = canonical_rects(TilingLayout::Grid, 4);
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let (a, b) = (&rects[i], &rects[j]);
            let overlap_x = a.x < b.x + b.width && a.x + a.width > b.x;
            let overlap_y = a.y < b.y + b.height && a.y + a.height > b.y;
            assert!(!(overlap_x && overlap_y), "grid cells {i}/{j} overlap");
        }
    }
}

// ---- ThreeColumn (left stack | center master | right stack) -------------

#[test]
fn three_column_single_window_fills_work_area() {
    let rects = canonical_rects(TilingLayout::ThreeColumn, 1);
    assert_eq!(rects.len(), 1);
    assert!(approx_eq(rects[0].width, 1920.0));
}

#[test]
fn three_column_three_windows_center_left_right() {
    let rects = canonical_rects(TilingLayout::ThreeColumn, 3);
    assert_eq!(rects.len(), 3);
    // Center column (idx 0) = master_ratio of usable width.
    let center_w = 1904.0 * 0.55;
    let side_w = (1904.0 - center_w - 2.0 * 8.0) / 2.0;
    assert!(approx_eq(rects[0].width, center_w));
    // Index 1 → left stack, index 2 → right stack.
    assert!(approx_eq(rects[1].x, 8.0));
    assert!(approx_eq(rects[1].width, side_w));
    // Right column sits to the right of center.
    assert!(rects[2].x > rects[0].x);
}

// ---- Spiral (fibonacci; alternating split direction) --------------------

#[test]
fn spiral_single_window_fills_work_area() {
    let rects = canonical_rects(TilingLayout::Spiral, 1);
    assert_eq!(rects.len(), 1);
    assert!(approx_eq(rects[0].width, 1920.0));
    assert!(approx_eq(rects[0].height, 1080.0));
}

#[test]
fn spiral_two_windows_alternating_split() {
    let rects = canonical_rects(TilingLayout::Spiral, 2);
    assert_eq!(rects.len(), 2);
    // First split is horizontal: left = master_ratio * usable_w - gap/2.
    let master_w = 1904.0 * 0.55 - 4.0;
    assert!(approx_eq(rects[0].width, master_w));
    assert!(approx_eq(rects[0].height, 1064.0));
    let rest_w = 1904.0 - master_w - 8.0;
    assert!(approx_eq(rects[1].width, rest_w));
}

#[test]
fn spiral_three_windows_fibonacci() {
    let rects = canonical_rects(TilingLayout::Spiral, 3);
    assert_eq!(rects.len(), 3);
    // Second split is vertical (i=1 is odd): rect 1 is shorter than full.
    assert!(rects[1].height < 1064.0);
}

// ---- Monocle (all windows full area; ≈ shell Stacking) ------------------

#[test]
fn monocle_all_windows_same_full_rect() {
    let rects = canonical_rects(TilingLayout::Monocle, 5);
    assert_eq!(rects.len(), 5);
    // n>1 → smart_gaps inactive → usable inset by the 8px outer gap.
    for r in &rects {
        assert!(approx_eq(r.x, 8.0));
        assert!(approx_eq(r.y, 8.0));
        assert!(approx_eq(r.width, 1904.0));
        assert!(approx_eq(r.height, 1064.0));
    }
}

// ===========================================================================
// CANONICAL snap detection + preview geometry (single-sourced)
// ===========================================================================
//
// `liquide_tiling::SnapZones::detect_zone` / `zone_preview` are the single
// source for snap. Preview geometry differs from the retired shell helper: no
// outer-gap inset, and `Top`/`Center` map to the full screen (maximize).

fn detect(x: f32, y: f32) -> SnapTarget {
    // 32px default snap threshold (matches TilingConfig::default).
    SnapZones::detect_zone((x, y), screen(), 32.0)
}

#[test]
fn detect_snap_zone_top_left_corner() {
    assert_eq!(detect(5.0, 5.0), SnapTarget::TopLeft);
}

#[test]
fn detect_snap_zone_top_right_corner() {
    assert_eq!(detect(1915.0, 5.0), SnapTarget::TopRight);
}

#[test]
fn detect_snap_zone_bottom_left() {
    assert_eq!(detect(5.0, 1075.0), SnapTarget::BottomLeft);
}

#[test]
fn detect_snap_zone_bottom_right() {
    assert_eq!(detect(1915.0, 1075.0), SnapTarget::BottomRight);
}

#[test]
fn detect_snap_zone_left_edge() {
    assert_eq!(detect(10.0, 540.0), SnapTarget::Left);
}

#[test]
fn detect_snap_zone_right_edge() {
    assert_eq!(detect(1910.0, 540.0), SnapTarget::Right);
}

#[test]
fn detect_snap_zone_top_edge() {
    assert_eq!(detect(960.0, 10.0), SnapTarget::Top);
}

#[test]
fn detect_snap_zone_bottom_edge() {
    assert_eq!(detect(960.0, 1070.0), SnapTarget::Bottom);
}

#[test]
fn detect_snap_zone_center_is_none() {
    assert_eq!(detect(960.0, 540.0), SnapTarget::None);
}

// ---- preview rectangles -------------------------------------------------

#[test]
fn snap_preview_left_half() {
    let r = SnapZones::zone_preview(SnapTarget::Left, screen());
    assert!(approx_eq(r.x, 0.0));
    assert!(approx_eq(r.y, 0.0));
    assert!(approx_eq(r.width, 960.0));
    assert!(approx_eq(r.height, 1080.0));
}

#[test]
fn snap_preview_center_is_maximize() {
    // Canonical Center/Top = full-screen maximize (no outer-gap inset).
    let r = SnapZones::zone_preview(SnapTarget::Center, screen());
    assert!(approx_eq(r.x, 0.0));
    assert!(approx_eq(r.y, 0.0));
    assert!(approx_eq(r.width, 1920.0));
    assert!(approx_eq(r.height, 1080.0));
}

#[test]
fn snap_preview_top_right_quarter() {
    let r = SnapZones::zone_preview(SnapTarget::TopRight, screen());
    assert!(approx_eq(r.x, 960.0));
    assert!(approx_eq(r.y, 0.0));
    assert!(approx_eq(r.width, 960.0));
    assert!(approx_eq(r.height, 540.0));
}

#[test]
fn snap_bridge_zone_to_target_round_trip() {
    // The shell `SnapZone` ↔ canonical `SnapTarget` bridge (t52-e3) is the
    // single conversion surface: every active zone maps to its target and back.
    for zone in [
        SnapZone::Left,
        SnapZone::Right,
        SnapZone::Top,
        SnapZone::Bottom,
        SnapZone::TopLeft,
        SnapZone::TopRight,
        SnapZone::BottomLeft,
        SnapZone::BottomRight,
        SnapZone::Center,
    ] {
        let target: SnapTarget = zone.into();
        assert!(target.is_active());
        assert_eq!(SnapZone::from_target(target), Some(zone));
    }
    // The inactive canonical target maps back to "no zone".
    assert_eq!(SnapZone::from_target(SnapTarget::None), None);
}

// ========== Per-workspace config ==========

#[test]
fn workspace_layout_falls_back_to_default() {
    let engine = default_engine();
    assert_eq!(
        engine.workspace_layout(WorkspaceId(1)),
        TilingLayoutKind::SplitHorizontal
    );
}

#[test]
fn set_and_get_workspace_layout() {
    let mut engine = default_engine();
    engine.set_workspace_layout(WorkspaceId(1), TilingLayoutKind::Spiral);
    assert_eq!(
        engine.workspace_layout(WorkspaceId(1)),
        TilingLayoutKind::Spiral
    );
    // Other workspace still uses default
    assert_eq!(
        engine.workspace_layout(WorkspaceId(2)),
        TilingLayoutKind::SplitHorizontal
    );
}

#[test]
fn workspace_mode_falls_back_to_default() {
    let engine = default_engine();
    assert_eq!(engine.workspace_mode(WorkspaceId(1)), TilingMode::Floating);
}

#[test]
fn set_and_get_workspace_mode() {
    let mut engine = default_engine();
    engine.set_workspace_mode(WorkspaceId(1), TilingMode::Tiling);
    assert_eq!(engine.workspace_mode(WorkspaceId(1)), TilingMode::Tiling);
}

// ========== Window rules ==========

#[test]
fn add_and_match_window_rule() {
    let mut engine = default_engine();
    engine.add_window_rule(WindowRule {
        app_id_pattern: "firefox".into(),
        tile_behavior: WindowTileBehavior::Floating,
        default_zone: Some(SnapZone::Center),
    });
    let rule = engine.match_window_rule("org.mozilla.firefox");
    assert!(rule.is_some());
    let rule = rule.unwrap();
    assert_eq!(rule.tile_behavior, WindowTileBehavior::Floating);
    assert_eq!(rule.default_zone, Some(SnapZone::Center));
}

#[test]
fn match_window_rule_no_match() {
    let mut engine = default_engine();
    engine.add_window_rule(WindowRule {
        app_id_pattern: "firefox".into(),
        tile_behavior: WindowTileBehavior::Tiling,
        default_zone: None,
    });
    assert!(engine.match_window_rule("chrome").is_none());
}

// ========== Presets ==========

#[test]
fn save_and_load_preset() {
    let mut engine = default_engine();
    let preset = TilingPreset {
        name: "dev".into(),
        layout: TilingLayoutKind::ThreeColumn,
        window_positions: vec![
            Rect::new(0.0, 0.0, 960.0, 1080.0),
            Rect::new(960.0, 0.0, 960.0, 1080.0),
        ],
    };
    engine.save_preset(preset);
    let loaded = engine.load_preset("dev");
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().layout, TilingLayoutKind::ThreeColumn);
    assert_eq!(loaded.unwrap().window_positions.len(), 2);
}

#[test]
fn save_preset_overwrites_existing() {
    let mut engine = default_engine();
    engine.save_preset(TilingPreset {
        name: "a".into(),
        layout: TilingLayoutKind::Spiral,
        window_positions: vec![],
    });
    engine.save_preset(TilingPreset {
        name: "a".into(),
        layout: TilingLayoutKind::Stacking,
        window_positions: vec![Rect::new(0.0, 0.0, 100.0, 100.0)],
    });
    assert_eq!(engine.presets().len(), 1);
    assert_eq!(
        engine.load_preset("a").unwrap().layout,
        TilingLayoutKind::Stacking
    );
}

#[test]
fn load_preset_not_found() {
    let engine = default_engine();
    assert!(engine.load_preset("nonexistent").is_none());
}

#[test]
fn presets_returns_all() {
    let mut engine = default_engine();
    engine.save_preset(TilingPreset {
        name: "p1".into(),
        layout: TilingLayoutKind::Quadrant,
        window_positions: vec![],
    });
    engine.save_preset(TilingPreset {
        name: "p2".into(),
        layout: TilingLayoutKind::Spiral,
        window_positions: vec![],
    });
    assert_eq!(engine.presets().len(), 2);
}

// ========== Snap preview ==========

#[test]
fn snap_preview_initially_none() {
    let engine = default_engine();
    assert!(engine.snap_preview().is_none());
}

#[test]
fn set_and_get_snap_preview() {
    let mut engine = default_engine();
    let preview = SnapPreview {
        zone: SnapZone::Left,
        preview_rect: Rect::new(0.0, 0.0, 960.0, 1080.0),
        active: true,
    };
    engine.set_snap_preview(Some(preview));
    let p = engine.snap_preview().unwrap();
    assert_eq!(p.zone, SnapZone::Left);
    assert!(p.active);
}

#[test]
fn clear_snap_preview() {
    let mut engine = default_engine();
    engine.set_snap_preview(Some(SnapPreview {
        zone: SnapZone::Right,
        preview_rect: Rect::new(960.0, 0.0, 960.0, 1080.0),
        active: true,
    }));
    engine.set_snap_preview(None);
    assert!(engine.snap_preview().is_none());
}

// ========== Config and custom grid accessors ==========

#[test]
fn config_accessor_returns_reference() {
    let engine = default_engine();
    assert!(engine.config().enabled);
}

#[test]
fn set_custom_grid_and_read_back() {
    let mut engine = default_engine();
    let grid = CustomGridConfig {
        rows: 4,
        columns: 3,
        col_ratios: vec![0.2, 0.5, 0.3],
        row_ratios: vec![0.25, 0.25, 0.25, 0.25],
    };
    engine.set_custom_grid(grid);
    let g = engine.custom_grid();
    assert_eq!(g.rows, 4);
    assert_eq!(g.columns, 3);
}

// ========== Display impls ==========

#[test]
fn display_tiling_mode() {
    assert_eq!(format!("{}", TilingMode::Floating), "Floating");
    assert_eq!(format!("{}", TilingMode::Tiling), "Tiling");
    assert_eq!(format!("{}", TilingMode::Hybrid), "Hybrid");
}

#[test]
fn display_tiling_layout_kind() {
    assert_eq!(
        format!("{}", TilingLayoutKind::SplitHorizontal),
        "SplitHorizontal"
    );
    assert_eq!(format!("{}", TilingLayoutKind::Quadrant), "Quadrant");
    assert_eq!(format!("{}", TilingLayoutKind::CustomGrid), "CustomGrid");
}

#[test]
fn display_snap_zone() {
    assert_eq!(format!("{}", SnapZone::TopLeft), "TopLeft");
    assert_eq!(format!("{}", SnapZone::BottomRight), "BottomRight");
    assert_eq!(format!("{}", SnapZone::Center), "Center");
}

#[test]
fn display_tiling_engine() {
    let mut engine = default_engine();
    engine.add_window_rule(WindowRule {
        app_id_pattern: "x".into(),
        tile_behavior: WindowTileBehavior::Tiling,
        default_zone: None,
    });
    let s = format!("{engine}");
    assert!(s.contains("TilingEngine"));
    assert!(s.contains("rules=1"));
}
