//! Tests for the tiling engine.

#[cfg(test)]
mod tests {
    use liquide_compositor::geometry::Rect;

    use crate::algorithms;
    use crate::engine::TilingEngine;
    use crate::gaps::TilingGaps;
    use crate::layout::*;
    use crate::navigate;
    use crate::rules::{RuleEngine, TileAction, TileRule};
    use crate::snap::{SnapTarget, SnapZones};

    fn screen() -> Rect {
        Rect::new(0.0, 0.0, 1920.0, 1080.0)
    }

    fn small_screen() -> Rect {
        Rect::new(0.0, 0.0, 1000.0, 800.0)
    }

    // =======================================================================
    // TilingLayout
    // =======================================================================

    #[test]
    fn layout_cycle_order() {
        let layout = TilingLayout::Columns;
        let next = layout.next_in_cycle();
        assert_eq!(next, TilingLayout::Rows);

        let after_monocle = TilingLayout::Monocle.next_in_cycle();
        assert_eq!(after_monocle, TilingLayout::Columns);
    }

    #[test]
    fn layout_is_tiling() {
        assert!(TilingLayout::Columns.is_tiling());
        assert!(TilingLayout::Monocle.is_tiling());
        assert!(!TilingLayout::Float.is_tiling());
    }

    #[test]
    fn float_next_cycles_to_columns() {
        let layout = TilingLayout::Float;
        let next = layout.next_in_cycle();
        assert_eq!(next, TilingLayout::Columns);
    }

    // =======================================================================
    // NormalizedRect
    // =======================================================================

    #[test]
    fn normalized_rect_clamp() {
        let nr = NormalizedRect::new(-0.1, 1.5, 0.5, 2.0);
        let c = nr.clamped();
        assert_eq!(c.x, 0.0);
        assert_eq!(c.y, 1.0);
        assert_eq!(c.w, 0.5);
        assert_eq!(c.h, 1.0);
    }

    // =======================================================================
    // TilingGaps
    // =======================================================================

    #[test]
    fn gaps_default() {
        let g = TilingGaps::default();
        assert_eq!(g.inner, 8.0);
        assert_eq!(g.outer, 8.0);
        assert!(g.smart_gaps);
    }

    #[test]
    fn gaps_smart_single_window() {
        let g = TilingGaps::default();
        let eff = g.effective(1);
        assert_eq!(eff.inner, 0.0);
        assert_eq!(eff.outer, 0.0);
    }

    #[test]
    fn gaps_smart_multiple_windows() {
        let g = TilingGaps::default();
        let eff = g.effective(3);
        assert_eq!(eff.inner, 8.0);
        assert_eq!(eff.outer, 8.0);
    }

    #[test]
    fn gaps_usable_area() {
        let g = TilingGaps {
            inner: 8.0,
            outer: 10.0,
            smart_gaps: false,
        };
        let usable = g.usable_area(screen());
        assert_eq!(usable.x, 10.0);
        assert_eq!(usable.y, 10.0);
        assert_eq!(usable.width, 1900.0);
        assert_eq!(usable.height, 1060.0);
    }

    // =======================================================================
    // Algorithms — Columns
    // =======================================================================

    #[test]
    fn columns_single_window() {
        let gaps = TilingGaps::default();
        let rects = algorithms::compute_layout(&TilingLayout::Columns, 1, screen(), 0.55, 1, &gaps);
        assert_eq!(rects.len(), 1);
        // Smart gaps: single window gets full screen.
        assert_eq!(rects[0], screen());
    }

    #[test]
    fn columns_two_windows() {
        let gaps = TilingGaps {
            inner: 10.0,
            outer: 0.0,
            smart_gaps: false,
        };
        let rects =
            algorithms::compute_layout(&TilingLayout::Columns, 2, small_screen(), 0.5, 1, &gaps);
        assert_eq!(rects.len(), 2);
        // Master takes 50% minus half gap, stack takes the rest.
        let master_w = 1000.0 * 0.5 - 10.0 / 2.0;
        assert!((rects[0].width - master_w).abs() < 0.01);
        assert!(rects[1].x > rects[0].x);
    }

    #[test]
    fn columns_multi_master() {
        let gaps = TilingGaps {
            inner: 0.0,
            outer: 0.0,
            smart_gaps: false,
        };
        let rects =
            algorithms::compute_layout(&TilingLayout::Columns, 4, small_screen(), 0.5, 2, &gaps);
        assert_eq!(rects.len(), 4);
        // First 2 are masters (same x), last 2 are stack (same x).
        assert_eq!(rects[0].x, rects[1].x);
        assert_eq!(rects[2].x, rects[3].x);
        assert!(rects[2].x > rects[0].x);
    }

    // =======================================================================
    // Algorithms — Rows
    // =======================================================================

    #[test]
    fn rows_two_windows() {
        let gaps = TilingGaps {
            inner: 10.0,
            outer: 0.0,
            smart_gaps: false,
        };
        let rects =
            algorithms::compute_layout(&TilingLayout::Rows, 2, small_screen(), 0.5, 1, &gaps);
        assert_eq!(rects.len(), 2);
        assert!(rects[1].y > rects[0].y);
    }

    #[test]
    fn rows_multi_master() {
        let gaps = TilingGaps {
            inner: 0.0,
            outer: 0.0,
            smart_gaps: false,
        };
        let rects =
            algorithms::compute_layout(&TilingLayout::Rows, 3, small_screen(), 0.6, 2, &gaps);
        assert_eq!(rects.len(), 3);
        // First 2 masters share the top row.
        assert_eq!(rects[0].y, rects[1].y);
        // Stack is below.
        assert!(rects[2].y > rects[0].y);
    }

    // =======================================================================
    // Algorithms — Grid
    // =======================================================================

    #[test]
    fn grid_four_windows() {
        let gaps = TilingGaps {
            inner: 0.0,
            outer: 0.0,
            smart_gaps: false,
        };
        let rects =
            algorithms::compute_layout(&TilingLayout::Grid, 4, small_screen(), 0.55, 1, &gaps);
        assert_eq!(rects.len(), 4);
        // 2x2 grid: each cell is 500x400.
        for r in &rects {
            assert!((r.width - 500.0).abs() < 0.01);
            assert!((r.height - 400.0).abs() < 0.01);
        }
    }

    #[test]
    fn grid_three_windows() {
        let gaps = TilingGaps {
            inner: 0.0,
            outer: 0.0,
            smart_gaps: false,
        };
        let rects =
            algorithms::compute_layout(&TilingLayout::Grid, 3, small_screen(), 0.55, 1, &gaps);
        assert_eq!(rects.len(), 3);
        // ceil(sqrt(3)) = 2 cols, 2 rows. First row: 2 windows, second: 1 window (full width).
        assert!((rects[0].width - 500.0).abs() < 0.01);
        assert!((rects[2].width - 1000.0).abs() < 0.01);
    }

    #[test]
    fn grid_single_window() {
        let gaps = TilingGaps::default();
        let rects =
            algorithms::compute_layout(&TilingLayout::Grid, 1, small_screen(), 0.55, 1, &gaps);
        assert_eq!(rects.len(), 1);
    }

    // =======================================================================
    // Algorithms — ThreeColumn
    // =======================================================================

    #[test]
    fn three_column_single() {
        let gaps = TilingGaps::default();
        let rects = algorithms::compute_layout(
            &TilingLayout::ThreeColumn,
            1,
            small_screen(),
            0.5,
            1,
            &gaps,
        );
        assert_eq!(rects.len(), 1);
    }

    #[test]
    fn three_column_five_windows() {
        let gaps = TilingGaps {
            inner: 0.0,
            outer: 0.0,
            smart_gaps: false,
        };
        let rects = algorithms::compute_layout(
            &TilingLayout::ThreeColumn,
            5,
            small_screen(),
            0.5,
            1,
            &gaps,
        );
        assert_eq!(rects.len(), 5);
        // Master (index 0) is in center.
        let center_x = rects[0].x;
        // Left-stack windows should be left of center.
        assert!(rects[1].x < center_x);
        assert!(rects[3].x < center_x);
        // Right-stack windows should be right of center.
        assert!(rects[2].x > center_x);
        assert!(rects[4].x > center_x);
    }

    // =======================================================================
    // Algorithms — Spiral
    // =======================================================================

    #[test]
    fn spiral_four_windows() {
        let gaps = TilingGaps {
            inner: 0.0,
            outer: 0.0,
            smart_gaps: false,
        };
        let rects =
            algorithms::compute_layout(&TilingLayout::Spiral, 4, small_screen(), 0.5, 1, &gaps);
        assert_eq!(rects.len(), 4);
        // Each subsequent window should be smaller.
        assert!(rects[0].area() >= rects[1].area());
        assert!(rects[1].area() >= rects[2].area());
    }

    #[test]
    fn spiral_alternates_direction() {
        let gaps = TilingGaps {
            inner: 0.0,
            outer: 0.0,
            smart_gaps: false,
        };
        let rects =
            algorithms::compute_layout(&TilingLayout::Spiral, 3, small_screen(), 0.5, 1, &gaps);
        // First split is horizontal: rect[0] is on the left.
        assert!(rects[0].x < rects[1].x);
        // Second split is vertical: rect[1] is above rect[2].
        assert!(rects[1].y < rects[2].y);
    }

    // =======================================================================
    // Algorithms — Monocle
    // =======================================================================

    #[test]
    fn monocle_all_same_size() {
        let gaps = TilingGaps {
            inner: 10.0,
            outer: 20.0,
            smart_gaps: false,
        };
        let rects =
            algorithms::compute_layout(&TilingLayout::Monocle, 3, small_screen(), 0.55, 1, &gaps);
        assert_eq!(rects.len(), 3);
        for r in &rects {
            assert_eq!(r.x, 20.0);
            assert_eq!(r.y, 20.0);
            assert_eq!(r.width, 960.0);
            assert_eq!(r.height, 760.0);
        }
    }

    // =======================================================================
    // Algorithms — Custom
    // =======================================================================

    #[test]
    fn custom_zones_basic() {
        let zones = vec![
            TileZone::new(NormalizedRect::new(0.0, 0.0, 0.5, 1.0))
                .with_name("left")
                .with_max_windows(1),
            TileZone::new(NormalizedRect::new(0.5, 0.0, 0.5, 1.0)).with_name("right"),
        ];
        let gaps = TilingGaps {
            inner: 0.0,
            outer: 0.0,
            smart_gaps: false,
        };
        let rects = algorithms::compute_layout(
            &TilingLayout::Custom(zones),
            3,
            small_screen(),
            0.55,
            1,
            &gaps,
        );
        assert_eq!(rects.len(), 3);
        // First window in left zone.
        assert_eq!(rects[0].x, 0.0);
        assert_eq!(rects[0].width, 500.0);
        // Remaining 2 in right zone, stacked vertically.
        assert_eq!(rects[1].x, 500.0);
        assert_eq!(rects[2].x, 500.0);
        assert!(rects[1].y < rects[2].y);
    }

    // =======================================================================
    // Algorithms — Float
    // =======================================================================

    #[test]
    fn float_returns_centered_rects() {
        let gaps = TilingGaps::default();
        let rects = algorithms::compute_layout(&TilingLayout::Float, 2, screen(), 0.55, 1, &gaps);
        assert_eq!(rects.len(), 2);
        // Each rect should be smaller than the screen.
        for r in &rects {
            assert!(r.width < screen().width);
            assert!(r.height < screen().height);
        }
    }

    // =======================================================================
    // Algorithms — zero windows
    // =======================================================================

    #[test]
    fn zero_windows_returns_empty() {
        let gaps = TilingGaps::default();
        for layout in &[
            TilingLayout::Columns,
            TilingLayout::Rows,
            TilingLayout::Grid,
            TilingLayout::ThreeColumn,
            TilingLayout::Spiral,
            TilingLayout::Monocle,
            TilingLayout::Float,
        ] {
            let rects = algorithms::compute_layout(layout, 0, screen(), 0.55, 1, &gaps);
            assert!(rects.is_empty(), "Expected empty for {:?}", layout);
        }
    }

    // =======================================================================
    // Engine — basic operations
    // =======================================================================

    #[test]
    fn engine_add_remove_windows() {
        let mut engine = TilingEngine::new();
        engine.add_window(1);
        engine.add_window(2);
        engine.add_window(3);
        assert_eq!(engine.window_count(), 3);

        engine.remove_window(2);
        assert_eq!(engine.window_count(), 2);
        assert_eq!(engine.windows(), &[1, 3]);
    }

    #[test]
    fn engine_add_duplicate_ignored() {
        let mut engine = TilingEngine::new();
        engine.add_window(1);
        engine.add_window(1);
        assert_eq!(engine.window_count(), 1);
    }

    #[test]
    fn engine_swap_windows() {
        let mut engine = TilingEngine::new();
        engine.add_window(1);
        engine.add_window(2);
        engine.add_window(3);
        engine.swap_windows(1, 3);
        assert_eq!(engine.windows(), &[3, 2, 1]);
    }

    #[test]
    fn engine_promote_to_master() {
        let mut engine = TilingEngine::new();
        engine.add_window(1);
        engine.add_window(2);
        engine.add_window(3);
        engine.promote_to_master(3);
        assert_eq!(engine.windows()[0], 3);
        assert_eq!(engine.focused_window(), Some(3));
    }

    #[test]
    fn engine_rotate_forward() {
        let mut engine = TilingEngine::new();
        engine.add_window(1);
        engine.add_window(2);
        engine.add_window(3);
        engine.rotate_windows(RotateDir::Forward);
        assert_eq!(engine.windows(), &[3, 1, 2]);
    }

    #[test]
    fn engine_rotate_backward() {
        let mut engine = TilingEngine::new();
        engine.add_window(1);
        engine.add_window(2);
        engine.add_window(3);
        engine.rotate_windows(RotateDir::Backward);
        assert_eq!(engine.windows(), &[2, 3, 1]);
    }

    // =======================================================================
    // Engine — layout adjustment
    // =======================================================================

    #[test]
    fn engine_master_ratio_clamped() {
        let mut engine = TilingEngine::new();
        engine.increase_master_ratio(1.0);
        assert!(engine.master_ratio() <= 0.9);
        engine.decrease_master_ratio(2.0);
        assert!(engine.master_ratio() >= 0.1);
    }

    #[test]
    fn engine_master_count_bounds() {
        let mut engine = TilingEngine::new();
        assert_eq!(engine.master_count(), 1);
        engine.increment_master_count();
        assert_eq!(engine.master_count(), 2);
        engine.increment_master_count();
        assert_eq!(engine.master_count(), 3);
        engine.increment_master_count();
        assert_eq!(engine.master_count(), 3); // Capped at 3.
        engine.decrement_master_count();
        engine.decrement_master_count();
        assert_eq!(engine.master_count(), 1);
        engine.decrement_master_count();
        assert_eq!(engine.master_count(), 1); // Min is 1.
    }

    #[test]
    fn engine_cycle_layout() {
        let mut engine = TilingEngine::new();
        assert_eq!(*engine.layout(), TilingLayout::Columns);
        engine.cycle_layout();
        assert_eq!(*engine.layout(), TilingLayout::Rows);
        engine.cycle_layout();
        assert_eq!(*engine.layout(), TilingLayout::Grid);
    }

    #[test]
    fn engine_set_layout() {
        let mut engine = TilingEngine::new();
        engine.set_layout(TilingLayout::Spiral);
        assert_eq!(*engine.layout(), TilingLayout::Spiral);
    }

    // =======================================================================
    // Engine — navigation
    // =======================================================================

    #[test]
    fn engine_focus_next_prev() {
        let mut engine = TilingEngine::new();
        engine.add_window(10);
        engine.add_window(20);
        engine.add_window(30);
        engine.set_focused(10);

        assert_eq!(engine.focus_next(), Some(20));
        assert_eq!(engine.focus_next(), Some(30));
        assert_eq!(engine.focus_next(), Some(10)); // Wraps.

        assert_eq!(engine.focus_prev(), Some(30));
    }

    #[test]
    fn engine_focus_master() {
        let mut engine = TilingEngine::new();
        engine.add_window(10);
        engine.add_window(20);
        engine.set_focused(20);
        assert_eq!(engine.focus_master(), Some(10));
    }

    #[test]
    fn engine_focus_direction() {
        let mut engine = TilingEngine::new();
        engine.add_window(1);
        engine.add_window(2);
        // Compute layout so cached_positions exist.
        let gaps = TilingGaps {
            inner: 0.0,
            outer: 0.0,
            smart_gaps: false,
        };
        engine.set_gaps(gaps);
        let _ = engine.compute_layout(small_screen());
        engine.set_focused(1);

        // In Columns layout: window 1 is on left (master), window 2 on right.
        let result = engine.focus_direction(Direction::Right);
        assert_eq!(result, Some(2));
    }

    #[test]
    fn engine_swap_direction() {
        let mut engine = TilingEngine::new();
        engine.add_window(1);
        engine.add_window(2);
        let gaps = TilingGaps {
            inner: 0.0,
            outer: 0.0,
            smart_gaps: false,
        };
        engine.set_gaps(gaps);
        let _ = engine.compute_layout(small_screen());

        engine.swap_direction(1, Direction::Right);
        assert_eq!(engine.windows(), &[2, 1]);
    }

    #[test]
    fn engine_focus_empty() {
        let mut engine = TilingEngine::new();
        assert_eq!(engine.focus_next(), None);
        assert_eq!(engine.focus_prev(), None);
        assert_eq!(engine.focus_master(), None);
    }

    // =======================================================================
    // Engine — compute_layout integration
    // =======================================================================

    #[test]
    fn engine_compute_layout_returns_pairs() {
        let mut engine = TilingEngine::new();
        engine.add_window(100);
        engine.add_window(200);
        let pairs = engine.compute_layout(screen());
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0, 100);
        assert_eq!(pairs[1].0, 200);
    }

    #[test]
    fn engine_remove_adjusts_focus() {
        let mut engine = TilingEngine::new();
        engine.add_window(1);
        engine.add_window(2);
        engine.add_window(3);
        engine.set_focused(3); // index 2
        engine.remove_window(2); // index 1 removed; focused index stays at 1 (was 2)
        assert_eq!(engine.focused_window(), Some(3));
    }

    // =======================================================================
    // Snap zones
    // =======================================================================

    #[test]
    fn snap_detect_left() {
        let target = SnapZones::detect_zone((5.0, 500.0), screen(), 32.0);
        assert_eq!(target, SnapTarget::Left);
    }

    #[test]
    fn snap_detect_top_right() {
        let target = SnapZones::detect_zone((1910.0, 5.0), screen(), 32.0);
        assert_eq!(target, SnapTarget::TopRight);
    }

    #[test]
    fn snap_detect_none() {
        let target = SnapZones::detect_zone((960.0, 540.0), screen(), 32.0);
        assert_eq!(target, SnapTarget::None);
    }

    #[test]
    fn snap_detect_bottom_left() {
        let target = SnapZones::detect_zone((5.0, 1075.0), screen(), 32.0);
        assert_eq!(target, SnapTarget::BottomLeft);
    }

    #[test]
    fn snap_preview_left() {
        let rect = SnapZones::zone_preview(SnapTarget::Left, screen());
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.width, 960.0);
        assert_eq!(rect.height, 1080.0);
    }

    #[test]
    fn snap_preview_top_left_quarter() {
        let rect = SnapZones::zone_preview(SnapTarget::TopLeft, screen());
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.0);
        assert_eq!(rect.width, 960.0);
        assert_eq!(rect.height, 540.0);
    }

    #[test]
    fn snap_preview_center_maximize() {
        let rect = SnapZones::zone_preview(SnapTarget::Center, screen());
        assert_eq!(rect, screen());
    }

    #[test]
    fn snap_preview_none_is_zero() {
        let rect = SnapZones::zone_preview(SnapTarget::None, screen());
        assert_eq!(rect, Rect::ZERO);
    }

    // =======================================================================
    // Rules
    // =======================================================================

    #[test]
    fn rule_engine_default_floats_dialogs() {
        let engine = RuleEngine::new();
        let action = engine.evaluate(Some("GTK Dialog"), None);
        assert_eq!(action, TileAction::Float);
    }

    #[test]
    fn rule_engine_tiles_normal_windows() {
        let engine = RuleEngine::new();
        let action = engine.evaluate(Some("normal"), Some("firefox"));
        assert_eq!(action, TileAction::Tile);
    }

    #[test]
    fn rule_engine_custom_rule() {
        let mut engine = RuleEngine::empty();
        engine.add_rule(TileRule::by_app_id("spotify", TileAction::Workspace(2)));
        let action = engine.evaluate(None, Some("com.spotify.client"));
        assert_eq!(action, TileAction::Workspace(2));
    }

    #[test]
    fn rule_engine_priority_rule() {
        let mut engine = RuleEngine::new();
        engine.add_priority_rule(TileRule::by_class("dialog", TileAction::Tile));
        // Priority rule overrides the default "dialog -> Float".
        let action = engine.evaluate(Some("dialog"), None);
        assert_eq!(action, TileAction::Tile);
    }

    #[test]
    fn rule_case_insensitive() {
        let rule = TileRule::by_class("Dialog", TileAction::Float);
        assert!(rule.matches(Some("GTK DIALOG"), None));
        assert!(rule.matches(Some("dialog box"), None));
    }

    #[test]
    fn rule_no_match_returns_none() {
        let rule = TileRule::by_app_id("vscode", TileAction::Master);
        assert!(!rule.matches(None, Some("firefox")));
    }

    // =======================================================================
    // Navigate helpers
    // =======================================================================

    #[test]
    fn navigate_next_prev_index() {
        assert_eq!(navigate::next_index(2, 5), 3);
        assert_eq!(navigate::next_index(4, 5), 0);
        assert_eq!(navigate::prev_index(0, 5), 4);
        assert_eq!(navigate::prev_index(3, 5), 2);
    }

    #[test]
    fn navigate_find_in_direction_basic() {
        let positions = vec![
            Rect::new(0.0, 0.0, 500.0, 800.0),
            Rect::new(500.0, 0.0, 500.0, 400.0),
            Rect::new(500.0, 400.0, 500.0, 400.0),
        ];
        // From index 0, looking right should find index 1 or 2 (closest).
        let idx = navigate::find_index_in_direction(Direction::Right, 0, &positions);
        assert!(idx == Some(1) || idx == Some(2));

        // From index 1, looking down should find index 2.
        let idx = navigate::find_index_in_direction(Direction::Down, 1, &positions);
        assert_eq!(idx, Some(2));

        // From index 2, looking up should find index 1.
        let idx = navigate::find_index_in_direction(Direction::Up, 2, &positions);
        assert_eq!(idx, Some(1));
    }

    // =======================================================================
    // TileZone builder
    // =======================================================================

    #[test]
    fn tile_zone_builder() {
        let zone = TileZone::new(NormalizedRect::FULL)
            .with_name("main")
            .with_max_windows(2);
        assert_eq!(zone.name, Some("main".to_string()));
        assert_eq!(zone.max_windows, Some(2));
    }

    // =======================================================================
    // Engine — with_config constructor
    // =======================================================================

    #[test]
    fn engine_with_config() {
        let engine = TilingEngine::with_config(TilingLayout::Grid, TilingGaps::uniform(4.0), 0.7);
        assert_eq!(*engine.layout(), TilingLayout::Grid);
        assert_eq!(engine.gaps().inner, 4.0);
        assert_eq!(engine.master_ratio(), 0.7);
    }

    #[test]
    fn engine_with_config_clamps_ratio() {
        let engine = TilingEngine::with_config(TilingLayout::Columns, TilingGaps::default(), 1.5);
        assert!(engine.master_ratio() <= 0.9);
    }

    // =======================================================================
    // SnapTarget::is_active
    // =======================================================================

    #[test]
    fn snap_target_is_active() {
        assert!(!SnapTarget::None.is_active());
        assert!(SnapTarget::Left.is_active());
        assert!(SnapTarget::TopRight.is_active());
        assert!(SnapTarget::Center.is_active());
    }
}
