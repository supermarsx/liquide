//! Regressions for t51-e11: the canonical `liquide-window-tree` and
//! `liquide-window-effects` crates wired into the running shell.
//!
//! These assert real behavior driven through the new wiring:
//!   - window create inserts a node into the `WindowTree` (mapped via `tree_id`),
//!   - hit-test routes through the tree (correct topmost-at-point),
//!   - restack (raise/lower) reorders the tree, changing the topmost-at-point,
//!   - destroy removes the tree node,
//!   - an effect (open/transform/focus) is driven on its trigger.

use crate::shell::Shell;
use liquide_compositor::geometry::Rect;

#[test]
fn open_window_inserts_node_into_window_tree() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    // The window now carries a tree node id (it was mirrored into the tree).
    let tree_id = shell.window(id).unwrap().tree_id;
    assert!(
        tree_id.is_some(),
        "open_window should register the window in the WindowTree"
    );
}

#[test]
fn hit_test_routes_through_tree_for_point_inside_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    // A point inside the window resolves to it via the tree-routed hit-test.
    assert_eq!(shell.window_at_point(200.0, 200.0), Some(id));
    // A point outside every window misses.
    assert_eq!(shell.window_at_point(10.0, 10.0), None);
}

#[test]
fn hit_test_returns_topmost_window_at_overlapping_point() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(200.0, 200.0, 400.0, 300.0));
    // Both A and B contain (300, 300); B was created last (topmost).
    assert_eq!(
        shell.window_at_point(300.0, 300.0),
        Some(b),
        "topmost (last-created) window should win the hit-test"
    );
    // Raise A above B — now A is topmost at the overlap point.
    shell.raise_window(a).unwrap();
    assert_eq!(
        shell.window_at_point(300.0, 300.0),
        Some(a),
        "restack should reorder the tree so A becomes topmost-at-point"
    );
}

#[test]
fn lower_window_restacks_tree_so_other_window_wins_hit_test() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(200.0, 200.0, 400.0, 300.0));
    // B topmost initially.
    assert_eq!(shell.window_at_point(300.0, 300.0), Some(b));
    // Lower B to the bottom — A wins the overlap point.
    shell.lower_window(b).unwrap();
    assert_eq!(
        shell.window_at_point(300.0, 300.0),
        Some(a),
        "send-to-bottom should reorder the tree z-order"
    );
}

/// t62 MAJOR-4 regression: focusing a background window must sync the canonical
/// `WindowTree` z-order so that subsequent hit-tests route to the now-focused
/// window. Before the fix, `set_focus` updated only the focus manager, leaving
/// the tree's topmost entry pointing at the previously-raised window — so a
/// click on a background window focused it but input still went to the old
/// topmost window.
#[test]
fn set_focus_raises_window_in_tree_z_order() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(200.0, 200.0, 400.0, 300.0));
    // B was created last, so it is topmost at the overlap point.
    assert_eq!(shell.window_at_point(300.0, 300.0), Some(b));

    // Focus A (the background window). This must bring A to the top of the tree
    // z-order so the very same overlap point now hit-tests to A — otherwise
    // input would keep routing to B despite A being focused.
    shell.set_focus(a).unwrap();
    assert_eq!(
        shell.window_at_point(300.0, 300.0),
        Some(a),
        "focusing a background window must sync the tree z-order so it wins the hit-test"
    );

    // Symmetric: focusing B again restores B as the topmost-at-point.
    shell.set_focus(b).unwrap();
    assert_eq!(
        shell.window_at_point(300.0, 300.0),
        Some(b),
        "re-focusing B must raise it back to the top of the tree z-order"
    );
}

#[test]
fn minimized_window_is_skipped_by_tree_hit_test() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(100.0, 100.0, 400.0, 300.0));
    // B exactly overlaps A and is topmost.
    assert_eq!(shell.window_at_point(200.0, 200.0), Some(b));
    // Minimize B — the tree hit-test must skip it and report A.
    shell.minimize(b).unwrap();
    assert_eq!(
        shell.window_at_point(200.0, 200.0),
        Some(a),
        "minimized window should be invisible to the tree hit-test"
    );
}

#[test]
fn close_window_removes_node_from_tree() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(200.0, 200.0, 400.0, 300.0));
    assert_eq!(shell.window_at_point(300.0, 300.0), Some(b));
    shell.close_window(b).unwrap();
    // With B's node destroyed, the overlap point now resolves to A.
    assert_eq!(
        shell.window_at_point(300.0, 300.0),
        Some(a),
        "closing a window should remove its node from the tree"
    );
    // And the bare A point still hits A.
    assert_eq!(shell.window_at_point(150.0, 150.0), Some(a));
}

#[test]
fn opening_a_window_drives_an_open_effect() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    // The canonical EffectManager should now report an active animation
    // for the freshly opened window.
    assert!(
        shell.window_is_animating(id),
        "open_window should drive a canonical open effect"
    );
}

#[test]
fn maximize_drives_a_transform_effect() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    // Drain the open effect (advance until finished) so we isolate maximize.
    for _ in 0..64 {
        let frames = shell.tick_window_effects();
        if frames.iter().all(|f| f.finished) {
            break;
        }
    }
    // Maximizing moves the window to the work area: a transform effect fires.
    shell.maximize(id).unwrap();
    assert!(
        shell.window_is_animating(id),
        "maximize should drive a canonical transform effect"
    );
    // Ticking the effects yields at least one frame for the window.
    let frames = shell.tick_window_effects();
    assert!(
        frames.iter().any(|f| f.window_id == id.0),
        "effect tick should produce a frame for the transformed window"
    );
}

#[test]
fn focusing_a_window_drives_a_focus_effect() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    // Drain open effects.
    for _ in 0..64 {
        let frames = shell.tick_window_effects();
        if frames.iter().all(|f| f.finished) {
            break;
        }
    }
    shell.set_focus(a).unwrap();
    assert!(
        shell.window_is_animating(a),
        "set_focus should drive a canonical focus-highlight effect"
    );
}

#[test]
fn move_window_keeps_tree_hit_test_geometry_in_sync() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("A", Rect::new(100.0, 100.0, 200.0, 200.0));
    assert_eq!(shell.window_at_point(150.0, 150.0), Some(id));
    // Move the window far away; the old point must miss and the new one hit.
    shell.move_window(id, 800.0, 600.0).unwrap();
    assert_eq!(
        shell.window_at_point(150.0, 150.0),
        None,
        "after move, the old location should no longer hit the window"
    );
    assert_eq!(
        shell.window_at_point(850.0, 650.0),
        Some(id),
        "after move, the tree hit-test should track the new bounds"
    );
}
