use crate::tiling::*;
use crate::workspace::WorkspaceId;
use liquide_compositor::geometry::Rect;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn default_engine() -> TilingEngine {
    TilingEngine::new(TilingConfig::default())
}

fn screen() -> Rect {
    Rect::new(0.0, 0.0, 1920.0, 1080.0)
}

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.1
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

// ========== arrange — zero windows ==========

#[test]
fn arrange_zero_windows_returns_empty() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::SplitHorizontal, 0, screen());
    assert!(rects.is_empty());
}

// ========== SplitHorizontal ==========

#[test]
fn split_h_single_window_fills_usable() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::SplitHorizontal, 1, screen());
    assert_eq!(rects.len(), 1);
    // usable = (8, 8, 1904, 1064)
    assert!(approx_eq(rects[0].x, 8.0));
    assert!(approx_eq(rects[0].y, 8.0));
    assert!(approx_eq(rects[0].width, 1904.0));
    assert!(approx_eq(rects[0].height, 1064.0));
}

#[test]
fn split_h_two_windows_master_and_stack() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::SplitHorizontal, 2, screen());
    assert_eq!(rects.len(), 2);
    // Master width = usable_w * 0.55 - gap/2 = 1904 * 0.55 - 4 = 1043.2
    assert!(approx_eq(rects[0].width, 1043.2));
    // Stack = usable_w - master_w - gap
    let expected_stack_w = 1904.0 - 1043.2 - 8.0;
    assert!(approx_eq(rects[1].width, expected_stack_w));
    assert!(approx_eq(rects[1].height, 1064.0));
}

#[test]
fn split_h_three_windows_stack_divides_height() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::SplitHorizontal, 3, screen());
    assert_eq!(rects.len(), 3);
    // Two stack windows share the height with a gap between them
    let stack_h = (1064.0 - 8.0) / 2.0;
    assert!(approx_eq(rects[1].height, stack_h));
    assert!(approx_eq(rects[2].height, stack_h));
    // Second stack window starts after first + gap
    assert!(approx_eq(rects[2].y, rects[1].y + stack_h + 8.0));
}

// ========== SplitVertical ==========

#[test]
fn split_v_single_window_fills_usable() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::SplitVertical, 1, screen());
    assert_eq!(rects.len(), 1);
    assert!(approx_eq(rects[0].width, 1904.0));
    assert!(approx_eq(rects[0].height, 1064.0));
}

#[test]
fn split_v_two_windows_master_top_stack_bottom() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::SplitVertical, 2, screen());
    assert_eq!(rects.len(), 2);
    // Master height = usable_h * 0.55 - gap/2 = 1064 * 0.55 - 4 = 581.2
    assert!(approx_eq(rects[0].height, 581.2));
    assert!(approx_eq(rects[0].width, 1904.0));
    // Stack starts below master + gap
    assert!(approx_eq(rects[1].y, 8.0 + 581.2 + 8.0));
}

// ========== Quadrant ==========

#[test]
fn quadrant_four_windows_fills_quadrants() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::Quadrant, 4, screen());
    assert_eq!(rects.len(), 4);
    let half_w = (1904.0 - 8.0) / 2.0;
    let half_h = (1064.0 - 8.0) / 2.0;
    // Top-left
    assert!(approx_eq(rects[0].x, 8.0));
    assert!(approx_eq(rects[0].y, 8.0));
    assert!(approx_eq(rects[0].width, half_w));
    assert!(approx_eq(rects[0].height, half_h));
    // Top-right
    assert!(approx_eq(rects[1].x, 8.0 + half_w + 8.0));
    assert!(approx_eq(rects[1].y, 8.0));
}

#[test]
fn quadrant_two_windows_only_fills_two() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::Quadrant, 2, screen());
    assert_eq!(rects.len(), 2);
}

#[test]
fn quadrant_caps_at_four() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::Quadrant, 6, screen());
    assert_eq!(rects.len(), 4);
}

// ========== ThreeColumn ==========

#[test]
fn three_column_single_window_fills_usable() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::ThreeColumn, 1, screen());
    assert_eq!(rects.len(), 1);
    assert!(approx_eq(rects[0].width, 1904.0));
}

#[test]
fn three_column_three_windows_center_left_right() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::ThreeColumn, 3, screen());
    assert_eq!(rects.len(), 3);
    // Center column takes 60% of usable width (1 - 2*0.2)
    let side_w = 1904.0 * 0.2;
    let center_w = 1904.0 - 2.0 * side_w - 2.0 * 8.0;
    assert!(approx_eq(rects[0].width, center_w));
    // Second window goes to left column
    assert!(approx_eq(rects[1].x, 8.0));
    assert!(approx_eq(rects[1].width, side_w));
    // Third window goes to right column
    assert!(approx_eq(rects[2].x, 8.0 + side_w + 8.0 + center_w + 8.0));
}

// ========== Spiral ==========

#[test]
fn spiral_single_window_fills_usable() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::Spiral, 1, screen());
    assert_eq!(rects.len(), 1);
    assert!(approx_eq(rects[0].width, 1904.0));
    assert!(approx_eq(rects[0].height, 1064.0));
}

#[test]
fn spiral_two_windows_alternating_split() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::Spiral, 2, screen());
    assert_eq!(rects.len(), 2);
    // First split is horizontal: left = master_ratio * usable_w - gap/2
    let master_w = 1904.0 * 0.55 - 4.0;
    assert!(approx_eq(rects[0].width, master_w));
    assert!(approx_eq(rects[0].height, 1064.0));
    // Second window takes the rest
    let rest_w = 1904.0 - master_w - 8.0;
    assert!(approx_eq(rects[1].width, rest_w));
}

#[test]
fn spiral_three_windows_fibonacci() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::Spiral, 3, screen());
    assert_eq!(rects.len(), 3);
    // Third window is the last, so it occupies the remaining area
    // Second split is vertical (i=1 is odd): top portion
    assert!(rects[1].height < 1064.0);
}

// ========== Stacking ==========

#[test]
fn stacking_all_windows_same_rect() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::Stacking, 5, screen());
    assert_eq!(rects.len(), 5);
    for r in &rects {
        assert!(approx_eq(r.x, 8.0));
        assert!(approx_eq(r.y, 8.0));
        assert!(approx_eq(r.width, 1904.0));
        assert!(approx_eq(r.height, 1064.0));
    }
}

// ========== CustomGrid ==========

#[test]
fn custom_grid_default_2x2_four_windows() {
    let engine = default_engine();
    let rects = engine.arrange(TilingLayoutKind::CustomGrid, 4, screen());
    assert_eq!(rects.len(), 4);
    // All cells should have equal size
    let avail_w = 1904.0 - 8.0; // minus one gap between 2 cols
    let avail_h = 1064.0 - 8.0; // minus one gap between 2 rows
    let cell_w = avail_w * 0.5;
    let cell_h = avail_h * 0.5;
    assert!(approx_eq(rects[0].width, cell_w));
    assert!(approx_eq(rects[0].height, cell_h));
}

#[test]
fn custom_grid_caps_at_total_slots() {
    let engine = default_engine();
    // Default 2x2 grid = 4 slots, requesting 10 windows
    let rects = engine.arrange(TilingLayoutKind::CustomGrid, 10, screen());
    assert_eq!(rects.len(), 4);
}

#[test]
fn custom_grid_3x3_config() {
    let mut engine = default_engine();
    engine.set_custom_grid(CustomGridConfig {
        rows: 3,
        columns: 3,
        col_ratios: vec![0.25, 0.5, 0.25],
        row_ratios: vec![0.33, 0.34, 0.33],
    });
    let rects = engine.arrange(TilingLayoutKind::CustomGrid, 9, screen());
    assert_eq!(rects.len(), 9);
    // First cell is top-left with 25% width, 33% height
    let avail_w = 1904.0 - 2.0 * 8.0; // 2 gaps for 3 cols
    let avail_h = 1064.0 - 2.0 * 8.0; // 2 gaps for 3 rows
    assert!(approx_eq(rects[0].width, avail_w * 0.25));
    assert!(approx_eq(rects[0].height, avail_h * 0.33));
}

// ========== Snap zone detection ==========

#[test]
fn detect_snap_zone_top_left_corner() {
    let engine = default_engine();
    let zone = engine.detect_snap_zone(5.0, 5.0, screen());
    assert_eq!(zone, Some(SnapZone::TopLeft));
}

#[test]
fn detect_snap_zone_top_right_corner() {
    let engine = default_engine();
    let zone = engine.detect_snap_zone(1915.0, 5.0, screen());
    assert_eq!(zone, Some(SnapZone::TopRight));
}

#[test]
fn detect_snap_zone_bottom_left() {
    let engine = default_engine();
    let zone = engine.detect_snap_zone(5.0, 1075.0, screen());
    assert_eq!(zone, Some(SnapZone::BottomLeft));
}

#[test]
fn detect_snap_zone_bottom_right() {
    let engine = default_engine();
    let zone = engine.detect_snap_zone(1915.0, 1075.0, screen());
    assert_eq!(zone, Some(SnapZone::BottomRight));
}

#[test]
fn detect_snap_zone_left_edge() {
    let engine = default_engine();
    let zone = engine.detect_snap_zone(10.0, 540.0, screen());
    assert_eq!(zone, Some(SnapZone::Left));
}

#[test]
fn detect_snap_zone_right_edge() {
    let engine = default_engine();
    let zone = engine.detect_snap_zone(1910.0, 540.0, screen());
    assert_eq!(zone, Some(SnapZone::Right));
}

#[test]
fn detect_snap_zone_top_edge() {
    let engine = default_engine();
    let zone = engine.detect_snap_zone(960.0, 10.0, screen());
    assert_eq!(zone, Some(SnapZone::Top));
}

#[test]
fn detect_snap_zone_bottom_edge() {
    let engine = default_engine();
    let zone = engine.detect_snap_zone(960.0, 1070.0, screen());
    assert_eq!(zone, Some(SnapZone::Bottom));
}

#[test]
fn detect_snap_zone_center_is_none() {
    let engine = default_engine();
    let zone = engine.detect_snap_zone(960.0, 540.0, screen());
    assert_eq!(zone, None);
}

// ========== Snap zone rectangles ==========

#[test]
fn snap_zone_rect_left_half() {
    let engine = default_engine();
    let r = engine.snap_zone_rect(SnapZone::Left, screen());
    assert!(approx_eq(r.x, 8.0));
    assert!(approx_eq(r.y, 8.0));
    assert!(approx_eq(r.width, 952.0));
    assert!(approx_eq(r.height, 1064.0));
}

#[test]
fn snap_zone_rect_center_is_full_usable() {
    let engine = default_engine();
    let r = engine.snap_zone_rect(SnapZone::Center, screen());
    assert!(approx_eq(r.x, 8.0));
    assert!(approx_eq(r.y, 8.0));
    assert!(approx_eq(r.width, 1904.0));
    assert!(approx_eq(r.height, 1064.0));
}

#[test]
fn snap_zone_rect_top_right_quarter() {
    let engine = default_engine();
    let r = engine.snap_zone_rect(SnapZone::TopRight, screen());
    assert!(approx_eq(r.x, 8.0 + 952.0));
    assert!(approx_eq(r.y, 8.0));
    assert!(approx_eq(r.width, 952.0));
    assert!(approx_eq(r.height, 532.0));
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
