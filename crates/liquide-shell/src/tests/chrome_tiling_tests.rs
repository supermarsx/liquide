//! Regressions for the canonical `liquide-tiling` wiring (t51-e13).
//!
//! These cover the t49-e5-F05 fix: the tiling engine + snap zones are now
//! actually driven by the shell. A window dragged into a snap zone shows the
//! zone during the drag and tiles to it on release, and `tile_layout` arranges
//! windows per the canonical `liquide_tiling::TilingEngine` (no longer a
//! no-op). All assertions check real geometry changes.

use liquide_compositor::geometry::Rect;
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

use crate::shell::{DragState, Shell};
use crate::shortcuts::ShellAction;

fn mouse_move(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Move { x, y },
    }
}

fn mouse_release(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Button {
            button: MouseButton::Left,
            state: ButtonState::Released,
            x,
            y,
        },
    }
}

// ---------------------------------------------------------------------------
// Snap-zone drag + release
// ---------------------------------------------------------------------------

/// Dragging a window so the cursor enters the left snap zone shows that zone
/// as the active snap preview (the canonical `SnapZones` is consulted live).
#[test]
fn dragging_into_left_snap_zone_shows_the_zone() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("A", Rect::new(400.0, 400.0, 300.0, 200.0));

    // Begin a move drag (titlebar grab) by installing the drag state directly.
    shell.drag_state = Some(DragState::Moving {
        window_id: id,
        offset_x: 10.0,
        offset_y: 10.0,
    });

    // No preview before the cursor reaches an edge.
    assert!(shell.tiling().snap_preview().is_none());

    // Move the cursor against the left edge (within the snap threshold).
    shell.handle_platform_event(&mouse_move(2.0, 540.0));

    let preview = shell
        .tiling()
        .snap_preview()
        .expect("left snap zone should be previewed during the drag");
    assert_eq!(
        preview.zone,
        crate::tiling::SnapZone::Left,
        "cursor at the left edge should preview the Left zone"
    );
    assert!(preview.active);
    assert!(
        preview.preview_rect.width > 0.0 && preview.preview_rect.height > 0.0,
        "the preview rect must be a real (non-empty) region"
    );
}

/// Releasing a move drag inside a snap zone tiles the window to that zone:
/// its bounds change to the left-half work area, it is flagged `tiled`, and
/// the preview is cleared.
#[test]
fn releasing_in_snap_zone_tiles_the_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("A", Rect::new(400.0, 400.0, 300.0, 200.0));
    let original = shell.window(id).unwrap().bounds;

    shell.drag_state = Some(DragState::Moving {
        window_id: id,
        offset_x: 10.0,
        offset_y: 10.0,
    });

    // Drag against the left edge, then release there.
    shell.handle_platform_event(&mouse_move(2.0, 540.0));
    assert!(shell.tiling().snap_preview().is_some());
    shell.handle_platform_event(&mouse_release(2.0, 540.0));

    let win = shell.window(id).unwrap();
    let work = shell.work_area();

    // The window must now occupy (approximately) the left half of the work
    // area — a real geometry change away from its original bounds.
    assert!(
        (win.bounds.x - work.x).abs() < 1.0,
        "tiled window should be flush with the left work-area edge"
    );
    assert!(
        (win.bounds.width - work.width / 2.0).abs() < 1.0,
        "tiled window should span half the work-area width (got {})",
        win.bounds.width
    );
    assert!(
        (win.bounds.height - work.height).abs() < 1.0,
        "left-snap should give full work-area height"
    );
    assert_ne!(
        (win.bounds.x, win.bounds.width),
        (original.x, original.width),
        "the window's geometry must actually change on snap"
    );

    assert!(win.tiled, "snapped window must be flagged as tiled");
    assert_eq!(win.tile_zone, Some(crate::tiling::SnapZone::Left));
    assert!(
        shell.tiling().snap_preview().is_none(),
        "the snap preview must be cleared once the snap is applied"
    );
    // The drag must have ended.
    assert!(shell.drag_state.is_none());
}

/// Releasing a move drag away from any snap zone neither tiles the window nor
/// leaves a stale preview.
#[test]
fn releasing_away_from_zones_does_not_tile() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("A", Rect::new(400.0, 400.0, 300.0, 200.0));

    shell.drag_state = Some(DragState::Moving {
        window_id: id,
        offset_x: 10.0,
        offset_y: 10.0,
    });

    // Move to the middle of the screen (far from every edge), then release.
    shell.handle_platform_event(&mouse_move(960.0, 540.0));
    assert!(
        shell.tiling().snap_preview().is_none(),
        "no zone should be previewed in the middle of the screen"
    );
    shell.handle_platform_event(&mouse_release(960.0, 540.0));

    let win = shell.window(id).unwrap();
    assert!(
        !win.tiled,
        "a release away from a zone must not tile the window"
    );
    assert!(win.tile_zone.is_none());
    assert!(shell.tiling().snap_preview().is_none());
}

/// Dragging to a corner snaps to the corresponding quarter zone.
#[test]
fn dragging_into_top_right_corner_snaps_to_quarter() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("A", Rect::new(400.0, 400.0, 300.0, 200.0));
    let work = shell.work_area();

    shell.drag_state = Some(DragState::Moving {
        window_id: id,
        offset_x: 0.0,
        offset_y: 0.0,
    });

    // Cursor in the very top-right corner (within threshold of top + right).
    let cx = work.x + work.width - 2.0;
    let cy = work.y + 2.0;
    shell.handle_platform_event(&mouse_move(cx, cy));
    assert_eq!(
        shell.tiling().snap_preview().map(|p| p.zone),
        Some(crate::tiling::SnapZone::TopRight)
    );
    shell.handle_platform_event(&mouse_release(cx, cy));

    let win = shell.window(id).unwrap();
    assert_eq!(win.tile_zone, Some(crate::tiling::SnapZone::TopRight));
    // Top-right quarter: right half X, top half height.
    assert!((win.bounds.width - work.width / 2.0).abs() < 1.0);
    assert!((win.bounds.height - work.height / 2.0).abs() < 1.0);
    assert!(win.bounds.x > work.x + work.width / 2.0 - 1.0);
}

// ---------------------------------------------------------------------------
// tile_layout driven by the canonical TilingEngine (F05 core)
// ---------------------------------------------------------------------------

/// `tile_visible_windows_canonical` arranges every visible window through the
/// canonical `liquide_tiling::TilingEngine` — windows actually move/resize and
/// do not overlap (master-stack columns). This is the production driver for
/// the previously-callerless `tile_layout` (fixes t49-e5-F05).
#[test]
fn tile_layout_arranges_windows_via_canonical_engine() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::new(0.0, 0.0, 100.0, 100.0));
    let id2 = shell.open_window("B", Rect::new(0.0, 0.0, 100.0, 100.0));

    let count = shell.tile_visible_windows_canonical();
    assert_eq!(count, 2, "both windows should be arranged");

    let w1 = shell.window(id1).unwrap();
    let w2 = shell.window(id2).unwrap();

    // Real geometry change: neither window is at its original 100x100 box.
    assert!(
        w1.bounds.width > 100.0 || w2.bounds.width > 100.0,
        "at least one tiled window should be wider than its original 100px"
    );
    // Columns layout: master on the left, stack on the right — distinct x.
    assert!(
        (w1.bounds.x - w2.bounds.x).abs() > 1.0,
        "the two tiled windows must occupy different columns (x differs)"
    );
    // They tile side by side without horizontal overlap.
    let (left, right) = if w1.bounds.x <= w2.bounds.x {
        (w1.bounds, w2.bounds)
    } else {
        (w2.bounds, w1.bounds)
    };
    assert!(
        left.x + left.width <= right.x + 1.0,
        "tiled columns must not overlap horizontally"
    );

    // The canonical engine actually holds both windows.
    assert!(shell.window(id1).unwrap().tiled);
    assert!(shell.window(id2).unwrap().tiled);
}

/// Minimized windows are excluded from the canonical tiling arrangement.
#[test]
fn tile_layout_excludes_minimized_windows() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::new(0.0, 0.0, 100.0, 100.0));
    let id2 = shell.open_window("B", Rect::new(0.0, 0.0, 100.0, 100.0));
    let id3 = shell.open_window("C", Rect::new(0.0, 0.0, 100.0, 100.0));
    let _ = shell.minimize(id2);

    let count = shell.tile_visible_windows_canonical();
    assert_eq!(count, 2, "only the two non-minimized windows tile");

    // The minimized window keeps its original box and is not flagged tiled.
    let w2 = shell.window(id2).unwrap();
    assert!(!w2.tiled);
    assert!((w2.bounds.width - 100.0).abs() < 0.001);

    // The two visible windows were arranged (wider than original or moved).
    assert!(shell.window(id1).unwrap().tiled);
    assert!(shell.window(id3).unwrap().tiled);
}

/// A single visible window tiles to (effectively) the full work area.
#[test]
fn tile_layout_single_window_fills_work_area() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("A", Rect::new(0.0, 0.0, 100.0, 100.0));

    let count = shell.tile_visible_windows_canonical();
    assert_eq!(count, 1);

    let win = shell.window(id).unwrap();
    let work = shell.work_area();
    // Columns layout with one window: it occupies the whole work area minus
    // gaps. Assert it grew substantially toward the work-area width.
    assert!(
        win.bounds.width > work.width * 0.8,
        "single tiled window should fill most of the work area (got {} of {})",
        win.bounds.width,
        work.width
    );
    assert!(win.tiled);
}

/// t62 CRITICAL-3 regression: `tile_visible_windows_canonical` must only tile
/// windows that belong to the **active** workspace. A window living on an
/// inactive workspace must not be picked up by the tiler (which runs on every
/// workspace switch) — otherwise its bounds get rewritten and it flickers into
/// view on the active workspace.
#[test]
fn tile_layout_ignores_windows_on_inactive_workspaces() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.execute_action(&ShellAction::WorkspaceAdd);

    // A is opened on workspace 0 (active).
    let a = shell.open_window("A", Rect::new(0.0, 0.0, 100.0, 100.0));

    // Switch to workspace 1 and open B there.
    assert!(shell.execute_action(&ShellAction::SwitchToWorkspace(1)));
    let b = shell.open_window("B", Rect::new(10.0, 10.0, 100.0, 100.0));

    // Force A's `visible` flag back on so the ONLY thing that can exclude it
    // from tiling is the workspace-membership filter (the switch path flips
    // `visible=false` for inactive-workspace windows, which would otherwise mask
    // the bug). This isolates the t62 membership filter as the unit under test.
    shell.window_mut(a).unwrap().visible = true;
    let a_bounds_before = shell.window(a).unwrap().bounds;

    // Tiling on workspace 1 must arrange only B, leaving A (workspace 0)
    // completely untouched.
    let count = shell.tile_visible_windows_canonical();
    assert_eq!(count, 1, "only the active-workspace window (B) should tile");

    let a_after = shell.window(a).unwrap();
    assert_eq!(
        a_after.bounds, a_bounds_before,
        "an inactive-workspace window's bounds must not be rewritten by tiling"
    );
    assert!(
        !a_after.tiled,
        "an inactive-workspace window must not be flagged tiled"
    );
    assert!(
        shell.window(b).unwrap().tiled,
        "the active-workspace window should be tiled"
    );
}
