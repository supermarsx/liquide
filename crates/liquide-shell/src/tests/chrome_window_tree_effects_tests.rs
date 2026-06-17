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
use crate::shortcuts::ShellAction;
use crate::window::{WindowFlags, WindowId};
use liquide_compositor::geometry::Rect;

/// Pin / unpin `id` as always-on-top through the *real* action path
/// (`ToggleAlwaysOnTop` operates on the focused window), so the band-aware
/// restack runs exactly as it does live. Returns with `id` left focused.
fn toggle_always_on_top(shell: &mut Shell, id: WindowId) {
    shell.set_focus(id).unwrap();
    assert!(shell.execute_action(&ShellAction::ToggleAlwaysOnTop));
}

/// Convenience: is `id`'s window currently flagged always-on-top?
fn is_aot(shell: &Shell, id: WindowId) -> bool {
    shell
        .window(id)
        .unwrap()
        .flags
        .contains(WindowFlags::ALWAYS_ON_TOP)
}

/// The id of the topmost (last-painted) window in the live stacking order.
fn topmost(shell: &Shell) -> Option<WindowId> {
    shell.visible_windows().last().map(|w| w.id)
}

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

// ---------------------------------------------------------------------------
// Always-on-top band ordering (t93-e1 / t92 gap #2)
//
// Always-on-top windows form a top band that sorts strictly above the normal
// band. raise/lower/focus respect the band: a normal window raised to the top
// of its band still sits below every AOT window, and an AOT window lowered
// stays within the AOT band (above all normals). Within each band, relative
// order is preserved. Asserted on BOTH live consumers: `visible_windows`
// (paint order) and `window_at_point` (the tree-routed hit-test).
// ---------------------------------------------------------------------------

/// The exact gap t92 described: pin A as always-on-top, then raise a *later*
/// normal window B. AOT must keep A on top — over the overlap point AND in the
/// paint order. This FAILS without band-aware ordering (raising B bumps it to
/// the global max z and over A in both the flat sort and the tree).
#[test]
fn always_on_top_window_stays_above_a_later_raised_normal_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(200.0, 200.0, 400.0, 300.0));

    // Pin A as always-on-top (A becomes the top band).
    toggle_always_on_top(&mut shell, a);
    assert!(is_aot(&shell, a));
    assert_eq!(
        topmost(&shell),
        Some(a),
        "pinning A always-on-top must lift it to the top of the paint order"
    );
    assert_eq!(
        shell.window_at_point(300.0, 300.0),
        Some(a),
        "pinned A must win the overlap hit-test"
    );

    // Raise the normal window B. It may top the *normal* band, but it must NOT
    // climb above the always-on-top A.
    shell.raise_window(b).unwrap();
    assert_eq!(
        topmost(&shell),
        Some(a),
        "raising a normal window must not lift it above an always-on-top window (paint)"
    );
    assert_eq!(
        shell.window_at_point(300.0, 300.0),
        Some(a),
        "raising a normal window must not let it win the hit-test over an AOT window"
    );
    // Sanity: A's z_order is strictly above B's (band is monotonic in z_order).
    assert!(
        shell.window(a).unwrap().z_order > shell.window(b).unwrap().z_order,
        "AOT band must occupy the higher z_order ordinals"
    );
}

/// Focusing a normal window must not lift it above an always-on-top window on
/// the live tree-routed hit-test. (set_focus mirrors a bring_to_top into the
/// tree; the band must be re-asserted afterward.)
#[test]
fn focusing_a_normal_window_keeps_always_on_top_above_it() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(200.0, 200.0, 400.0, 300.0));

    toggle_always_on_top(&mut shell, a); // A pinned, top band
    // Focus the normal background window B.
    shell.set_focus(b).unwrap();
    assert_eq!(
        shell.window_at_point(300.0, 300.0),
        Some(a),
        "focusing a normal window must not steal the hit-test from an AOT window"
    );
    assert_eq!(
        topmost(&shell),
        Some(a),
        "focusing a normal window must not lift it above the AOT band in paint order"
    );
}

/// An always-on-top window *lowered* stays inside the AOT band — i.e. still
/// above every normal window — even though it is at the back of its own band.
#[test]
fn lowered_always_on_top_window_stays_above_normals() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let _b = shell.open_window("B", Rect::new(200.0, 200.0, 400.0, 300.0));

    toggle_always_on_top(&mut shell, a); // A pinned
    // Lower A. There is only one AOT window, so A is both top and bottom of its
    // band — and the band floor is still above all normals.
    shell.lower_window(a).unwrap();
    assert_eq!(
        topmost(&shell),
        Some(a),
        "an always-on-top window lowered within its band must stay above normals (paint)"
    );
    assert_eq!(
        shell.window_at_point(300.0, 300.0),
        Some(a),
        "an always-on-top window lowered within its band must stay above normals (hit-test)"
    );
}

/// Within the always-on-top band, relative order is preserved and raise/lower
/// reorder only inside the band. Two pinned windows + one normal: the normals
/// never appear above either AOT window, and raising one AOT window over the
/// other keeps both above the normal.
#[test]
fn relative_order_preserved_within_each_band() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 600.0, 500.0));
    let b = shell.open_window("B", Rect::new(120.0, 120.0, 600.0, 500.0));
    let c = shell.open_window("C", Rect::new(140.0, 140.0, 600.0, 500.0));

    // Pin A and B (both into the AOT band); C stays normal.
    toggle_always_on_top(&mut shell, a);
    toggle_always_on_top(&mut shell, b);
    assert!(is_aot(&shell, a) && is_aot(&shell, b) && !is_aot(&shell, c));

    // Both AOT windows must outrank the normal C in z_order.
    let zc = shell.window(c).unwrap().z_order;
    assert!(
        shell.window(a).unwrap().z_order > zc && shell.window(b).unwrap().z_order > zc,
        "both AOT windows must sit above the normal window"
    );

    // Raise C as high as a normal window can go: it must still be the bottom of
    // the global stack, below both AOT windows.
    shell.raise_window(c).unwrap();
    let order: Vec<WindowId> = shell.visible_windows().iter().map(|w| w.id).collect();
    let pos = |id: WindowId| order.iter().position(|&x| x == id).unwrap();
    assert!(
        pos(c) < pos(a) && pos(c) < pos(b),
        "a raised normal window stays below the entire AOT band: {order:?}"
    );

    // Reorder *within* the AOT band: raise A above B. Both remain above C, and
    // A is now the topmost.
    shell.raise_window(a).unwrap();
    assert_eq!(
        topmost(&shell),
        Some(a),
        "raising A within the AOT band makes it topmost overall"
    );
    let order: Vec<WindowId> = shell.visible_windows().iter().map(|w| w.id).collect();
    let pos = |id: WindowId| order.iter().position(|&x| x == id).unwrap();
    assert!(
        pos(b) > pos(c),
        "B (still AOT) stays above the normal C after the within-band reorder: {order:?}"
    );
    assert!(
        pos(a) > pos(b),
        "A is above B within the AOT band after the reorder: {order:?}"
    );
}

/// Un-pinning an always-on-top window drops it back into the single normal
/// band, where it once again competes with the other normals on equal footing
/// (no longer force-pinned on top): a normal window can now be raised over it.
#[test]
fn unpinning_always_on_top_returns_window_to_normal_band() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(200.0, 200.0, 400.0, 300.0));

    toggle_always_on_top(&mut shell, a);
    assert_eq!(topmost(&shell), Some(a));

    // Un-pin A: it rejoins the normal band. While pinned, A was forced on top no
    // matter what; now that it is normal again, raising B *can* lift B over A —
    // which it could NOT while A was always-on-top.
    toggle_always_on_top(&mut shell, a);
    assert!(!is_aot(&shell, a));
    shell.raise_window(b).unwrap();
    assert_eq!(
        shell.window_at_point(300.0, 300.0),
        Some(b),
        "after un-pinning A, a normal window raised above it now wins the overlap"
    );
    assert_eq!(
        topmost(&shell),
        Some(b),
        "after un-pinning A, normal stacking resumes and a raised normal can top it"
    );
}

/// Overlay/modal stacking is independent of the window AOT band: the AOT band
/// only re-packs *window* z ordinals (small sequential integers). It must never
/// push a window into the high overlay z-bases (overview 50k / tooltip 60k /
/// lock 80k in scene.rs), so modal/overlay layers keep compositing above every
/// window — pinned or not. Even after pinning and a within-band raise, the
/// maximum window ordinal stays far below those bases, and the scene builds.
#[test]
fn always_on_top_band_does_not_disturb_overlay_stacking() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(200.0, 200.0, 400.0, 300.0));
    toggle_always_on_top(&mut shell, a);
    shell.raise_window(b).unwrap();

    let max_window_z = shell
        .visible_windows()
        .iter()
        .map(|w| w.z_order)
        .max()
        .unwrap_or(0);
    // The lowest overlay z-base is the overview band (50_000) — window ordinals
    // must stay far below it so overlays always win.
    assert!(
        (max_window_z as f32) < 50_000.0,
        "window z ordinals ({max_window_z}) must stay well below the overlay z-bases"
    );

    // The scene still assembles cleanly with the pinned window present.
    let scene = shell.build_scene();
    assert!(
        !scene.children.is_empty(),
        "scene with a pinned window must still build"
    );
}
