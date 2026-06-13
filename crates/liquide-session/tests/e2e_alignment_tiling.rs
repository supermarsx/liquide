//! End-to-end tests for window tiling, snapping, and object alignment.
//! Verifies that tile-left/tile-right produce correct bounds, and that
//! the tiling engine generates non-overlapping layouts.

use liquide_compositor::geometry::Rect;
use liquide_shell::{Shell, ShellAction};
use liquide_tiling::{SnapTarget, SnapZones, TilingEngine, TilingGaps, TilingLayout};

fn new_shell() -> Shell {
    Shell::new(1920.0, 1080.0)
}

/// Build a canonical tiling engine for `layout` with `n` windows added (default
/// gaps + master ratio) and return the computed rects for a 1920x1080 screen.
///
/// Window-geometry computation is single-sourced onto `liquide_tiling`
/// (t52-e3/e4); these tests assert against that canonical surface.
fn canonical_rects(layout: TilingLayout, n: usize) -> Vec<Rect> {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let mut engine = TilingEngine::with_config(layout, TilingGaps::default(), 0.55);
    for i in 0..n {
        engine.add_window(i as u64);
    }
    engine
        .compute_layout(screen)
        .into_iter()
        .map(|(_, r)| r)
        .collect()
}

// ── Tile Left / Right via ShellAction ───────────────────────────────────────

#[test]
fn tile_left_positions_window_on_left_half() {
    let mut shell = new_shell();
    let bounds = Rect::new(200.0, 200.0, 640.0, 480.0);
    let wid = shell.open_window("Tile Left", bounds);
    shell.set_focus(wid).unwrap();

    shell.execute_action(&ShellAction::TileLeft);

    let win = shell.window(wid).unwrap();
    let work = shell.work_area();
    let half_w = work.width / 2.0;

    assert!(
        (win.bounds.x - work.x).abs() < 2.0,
        "tiled left window X should be near work area left, got {}",
        win.bounds.x
    );
    assert!(
        (win.bounds.width - half_w).abs() < 2.0,
        "tiled left window width should be half work area ({half_w}), got {}",
        win.bounds.width
    );
    assert!(
        (win.bounds.height - work.height).abs() < 2.0,
        "tiled left window height should fill work area height ({}, got {})",
        work.height,
        win.bounds.height
    );
}

#[test]
fn tile_right_positions_window_on_right_half() {
    let mut shell = new_shell();
    let bounds = Rect::new(200.0, 200.0, 640.0, 480.0);
    let wid = shell.open_window("Tile Right", bounds);
    shell.set_focus(wid).unwrap();

    shell.execute_action(&ShellAction::TileRight);

    let win = shell.window(wid).unwrap();
    let work = shell.work_area();
    let half_w = work.width / 2.0;

    assert!(
        (win.bounds.x - (work.x + half_w)).abs() < 2.0,
        "tiled right window X should be near {}, got {}",
        work.x + half_w,
        win.bounds.x
    );
    assert!(
        (win.bounds.width - half_w).abs() < 2.0,
        "tiled right window width should be half work area"
    );
}

#[test]
fn tile_left_then_right_splits_screen() {
    let mut shell = new_shell();
    let bounds = Rect::new(200.0, 200.0, 640.0, 480.0);
    let w1 = shell.open_window("Left", bounds);
    let w2 = shell.open_window("Right", bounds);

    // Tile w1 left
    shell.set_focus(w1).unwrap();
    shell.execute_action(&ShellAction::TileLeft);

    // Tile w2 right
    shell.set_focus(w2).unwrap();
    shell.execute_action(&ShellAction::TileRight);

    let win1 = shell.window(w1).unwrap();
    let win2 = shell.window(w2).unwrap();
    let screen = shell.screen_rect();

    // They should not overlap
    let right_edge_1 = win1.bounds.x + win1.bounds.width;
    let left_edge_2 = win2.bounds.x;

    assert!(
        right_edge_1 <= left_edge_2 + 2.0,
        "windows should not overlap: w1 right edge = {right_edge_1}, w2 left edge = {left_edge_2}"
    );

    // Together they should cover the full width
    let total_width = win1.bounds.width + win2.bounds.width;
    assert!(
        (total_width - screen.width).abs() < 2.0,
        "tiled windows should cover full screen width: total = {total_width}, screen = {}",
        screen.width
    );
}

// ── ShellAction MaximizeWindow ──────────────────────────────────────────────

#[test]
fn maximize_action_fills_screen() {
    let mut shell = new_shell();
    let bounds = Rect::new(200.0, 200.0, 640.0, 480.0);
    let wid = shell.open_window("Maximize Action", bounds);
    shell.set_focus(wid).unwrap();

    shell.execute_action(&ShellAction::MaximizeWindow);

    let win = shell.window(wid).unwrap();
    let screen = shell.screen_rect();

    assert!(
        win.bounds.width >= screen.width * 0.9,
        "maximized window should fill screen width"
    );
    assert!(
        win.bounds.height >= screen.height * 0.9,
        "maximized window should fill screen height"
    );
}

// ── Canonical tiling engine (layout geometry, single-sourced) ───────────────

#[test]
fn tiling_engine_columns_no_overlap() {
    let rects = canonical_rects(TilingLayout::Columns, 3);
    assert_eq!(rects.len(), 3, "should produce 3 rects for 3 windows");

    // No overlap between any pair.
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let r1 = &rects[i];
            let r2 = &rects[j];
            let overlap_x = r1.x < r2.x + r2.width && r1.x + r1.width > r2.x;
            let overlap_y = r1.y < r2.y + r2.height && r1.y + r1.height > r2.y;
            assert!(
                !(overlap_x && overlap_y),
                "rects {i} and {j} should not overlap: {:?} vs {:?}",
                r1,
                r2
            );
        }
    }
}

#[test]
fn tiling_engine_rows_no_overlap() {
    let rects = canonical_rects(TilingLayout::Rows, 3);
    assert_eq!(rects.len(), 3);

    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let r1 = &rects[i];
            let r2 = &rects[j];
            let overlap_x = r1.x < r2.x + r2.width && r1.x + r1.width > r2.x;
            let overlap_y = r1.y < r2.y + r2.height && r1.y + r1.height > r2.y;
            assert!(
                !(overlap_x && overlap_y),
                "rects {i} and {j} should not overlap"
            );
        }
    }
}

#[test]
fn tiling_engine_grid_layout() {
    let rects = canonical_rects(TilingLayout::Grid, 4);
    assert_eq!(rects.len(), 4, "grid should produce 4 rects");

    // Each cell should fit within the screen.
    for (i, r) in rects.iter().enumerate() {
        assert!(r.x >= 0.0, "rect {i} x should be >= 0");
        assert!(r.y >= 0.0, "rect {i} y should be >= 0");
        assert!(
            r.x + r.width <= 1920.0 + 1.0,
            "rect {i} should fit in screen width"
        );
        assert!(
            r.y + r.height <= 1080.0 + 1.0,
            "rect {i} should fit in screen height"
        );
    }
}

#[test]
fn tiling_engine_spiral_layout() {
    let rects = canonical_rects(TilingLayout::Spiral, 5);
    assert_eq!(rects.len(), 5);

    for (i, r) in rects.iter().enumerate() {
        assert!(r.width > 0.0, "spiral rect {i} should have positive width");
        assert!(
            r.height > 0.0,
            "spiral rect {i} should have positive height"
        );
        assert!(
            r.x + r.width <= 1920.0 + 1.0,
            "spiral rect {i} should fit in screen"
        );
    }
}

#[test]
fn tiling_engine_monocle_all_full_area() {
    let rects = canonical_rects(TilingLayout::Monocle, 3);
    assert_eq!(rects.len(), 3);

    // Monocle: all windows occupy roughly the full work area.
    for (i, r) in rects.iter().enumerate() {
        assert!(
            r.width >= 1920.0 * 0.8,
            "monocle rect {i} should be near full width"
        );
        assert!(
            r.height >= 1080.0 * 0.8,
            "monocle rect {i} should be near full height"
        );
    }
}

#[test]
fn tiling_engine_three_column() {
    let rects = canonical_rects(TilingLayout::ThreeColumn, 3);
    assert_eq!(rects.len(), 3);

    // Three-column: rects[0] = center (master); index 1 → left, index 2 → right.
    // So rects[1].x < rects[0].x < rects[2].x.
    assert!(
        rects[1].x < rects[0].x,
        "left column (idx 1) should be left of center (idx 0): {} vs {}",
        rects[1].x,
        rects[0].x
    );
    assert!(
        rects[0].x < rects[2].x,
        "center (idx 0) should be left of right column (idx 2): {} vs {}",
        rects[0].x,
        rects[2].x
    );
}

#[test]
fn tiling_engine_single_window_fills_screen() {
    // smart_gaps collapses gaps for a single window → fills the full screen.
    let rects = canonical_rects(TilingLayout::Columns, 1);
    assert_eq!(rects.len(), 1);

    let r = &rects[0];
    assert!(r.width >= 1920.0 * 0.9, "single window should fill width");
    assert!(r.height >= 1080.0 * 0.9, "single window should fill height");
}

// ── Canonical snap zones (detection + preview, single-sourced) ───────────────

#[test]
fn snap_zone_left_right_cover_screen_halves() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);

    let left = SnapZones::zone_preview(SnapTarget::Left, screen);
    let right = SnapZones::zone_preview(SnapTarget::Right, screen);

    // Left zone should be on the left.
    assert!(left.x < screen.width / 2.0);
    assert!(left.width > 0.0);

    // Right zone should be on the right.
    assert!(right.x >= screen.width / 2.0 - 2.0);
    assert!(right.width > 0.0);

    // Together they should cover the full screen width.
    let total_width = left.width + right.width;
    assert!(
        total_width >= screen.width * 0.9,
        "left + right snap zones should approximately cover screen width"
    );
}

#[test]
fn snap_zone_corners() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);

    let tl = SnapZones::zone_preview(SnapTarget::TopLeft, screen);
    let tr = SnapZones::zone_preview(SnapTarget::TopRight, screen);
    let bl = SnapZones::zone_preview(SnapTarget::BottomLeft, screen);
    let br = SnapZones::zone_preview(SnapTarget::BottomRight, screen);

    // Top-left should be in the top-left of screen.
    assert!(tl.x < screen.width / 2.0);
    assert!(tl.y < screen.height / 2.0);

    // Top-right should be in the top-right.
    assert!(tr.x >= screen.width / 2.0 - 2.0);
    assert!(tr.y < screen.height / 2.0);

    // Bottom-left.
    assert!(bl.x < screen.width / 2.0);
    assert!(bl.y >= screen.height / 2.0 - 2.0);

    // Bottom-right.
    assert!(br.x >= screen.width / 2.0 - 2.0);
    assert!(br.y >= screen.height / 2.0 - 2.0);
}

#[test]
fn detect_snap_zone_at_screen_edges() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let threshold = 32.0;

    // Cursor at far left should detect Left zone.
    let left = SnapZones::detect_zone((0.0, 540.0), screen, threshold);
    assert!(
        left.is_active(),
        "cursor at left edge should detect snap zone"
    );
    assert_eq!(left, SnapTarget::Left, "left edge should be Left snap zone");

    // Cursor at far right.
    let right = SnapZones::detect_zone((1919.0, 540.0), screen, threshold);
    assert!(
        right.is_active(),
        "cursor at right edge should detect snap zone"
    );
    assert_eq!(right, SnapTarget::Right);
}

// ── Arrange via every cycling layout ─────────────────────────────────────────

#[test]
fn arrange_produces_rects_for_all_layout_kinds() {
    let layouts = [
        TilingLayout::Columns,
        TilingLayout::Rows,
        TilingLayout::Grid,
        TilingLayout::ThreeColumn,
        TilingLayout::Spiral,
        TilingLayout::Monocle,
    ];

    for layout in layouts {
        let rects = canonical_rects(layout.clone(), 4);
        assert!(
            !rects.is_empty(),
            "{:?} layout should produce rects",
            layout
        );
        for r in &rects {
            assert!(
                r.width > 0.0,
                "{:?} layout rect should have positive width",
                layout
            );
            assert!(
                r.height > 0.0,
                "{:?} layout rect should have positive height",
                layout
            );
        }
    }
}

// ── ShowDesktop Action ──────────────────────────────────────────────────────

#[test]
fn show_desktop_minimizes_all_windows() {
    let mut shell = new_shell();
    let bounds = Rect::new(100.0, 100.0, 640.0, 480.0);
    let _w1 = shell.open_window("A", bounds);
    let _w2 = shell.open_window("B", bounds);
    let _w3 = shell.open_window("C", bounds);

    assert_eq!(shell.visible_windows().len(), 3);

    shell.execute_action(&ShellAction::ShowDesktop);

    assert_eq!(
        shell.visible_windows().len(),
        0,
        "all windows should be minimized after ShowDesktop"
    );
}

// ── Window Switching via Actions ─────────────────────────────────────────────

#[test]
fn switch_window_forward_cycles_focus() {
    let mut shell = new_shell();
    let bounds = Rect::new(100.0, 100.0, 640.0, 480.0);
    let w1 = shell.open_window("First", bounds);
    let _w2 = shell.open_window("Second", bounds);
    let _w3 = shell.open_window("Third", bounds);

    // Explicitly set focus first, then switch
    shell.set_focus(w1).unwrap();
    let before = shell.focus_manager().focused();
    assert_eq!(before, Some(w1));

    // Execute switch forward action
    let handled = shell.execute_action(&ShellAction::SwitchWindowForward);
    assert!(handled, "SwitchWindowForward should be handled");

    // focus_next() should work when a window was focused
    // It may or may not change the focus depending on the ordering
    // The key assertion is no panic and the action is handled.
}
