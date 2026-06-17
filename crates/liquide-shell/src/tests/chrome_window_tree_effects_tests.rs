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
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

/// A real left-button press at `(x, y)`, the way the live input path delivers it.
fn left_press(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Button {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            x,
            y,
        },
    }
}

/// A real pointer move to `(x, y)`.
fn mouse_move(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Move { x, y },
    }
}

/// A real right-button press at `(x, y)`.
fn right_press(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Button {
            button: MouseButton::Right,
            state: ButtonState::Pressed,
            x,
            y,
        },
    }
}

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

// ---------------------------------------------------------------------------
// Live hit-test router unification (t93-e3 / t92 gap #3)
//
// The live click/right-click paths must route window picking through the SINGLE
// canonical resolver (`window_at_point`, the WindowTree hit-test) rather than a
// second, flat z-ordered scan that could diverge from it. These tests
// REPRODUCE a state where the old flat `z_order` scan and the canonical tree
// disagreed, then prove a real mouse-down picks the window the tree (and the
// user, via focus) sees as topmost — i.e. exactly one hit-test path remains.
// ---------------------------------------------------------------------------

/// REPRODUCE THE DIVERGENCE between the canonical tree and the retired flat
/// `z_order` scan, then prove the LIVE press resolves through the tree.
///
/// Setup: open A then B, then `raise_window(b)` so `z_order` is A=0, B=1. Now
/// `set_focus(a)` — this brings A to the top of the canonical TREE
/// (`bring_to_top`) but does NOT change `z_order`. The two former sources of
/// truth now genuinely disagree at the overlap point:
///   - canonical tree (`window_at_point`)            → A (tree-topmost), and
///   - the retired flat scan over `visible_windows()` → B (B still has the
///     higher `z_order`, so `.rev()` ranks it first).
/// The unified live router uses the tree, so a real left press must focus A.
/// With the old flat scan still live, the press would have focused B — the
/// exact two-sources-of-truth divergence this gap retires.
#[test]
fn live_left_press_matches_tree_router_not_flat_z_scan() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(200.0, 200.0, 400.0, 300.0));

    // Give the windows distinct z_order: A=0, B=1.
    shell.raise_window(b).unwrap();
    assert!(shell.window(a).unwrap().z_order < shell.window(b).unwrap().z_order);

    // Focus the background window A: tree-topmost becomes A, z_order untouched.
    shell.set_focus(a).unwrap();

    // The two former paths now DIVERGE at the overlap point.
    let pt = liquide_compositor::geometry::Point::new(300.0, 300.0);
    let tree_pick = shell.window_at_point(300.0, 300.0);
    let flat_pick = {
        // Faithful mirror of the RETIRED flat scan: topmost by z_order first.
        let mut v: Vec<_> = shell.visible_windows();
        v.sort_by_key(|w| w.z_order);
        v.into_iter().rev().find(|w| w.bounds.contains(pt)).map(|w| w.id)
    };
    assert_eq!(tree_pick, Some(a), "canonical tree router picks freshly-focused A");
    assert_eq!(
        flat_pick,
        Some(b),
        "the retired flat z_order scan diverges and picks B — two sources of truth"
    );
    assert_ne!(
        tree_pick, flat_pick,
        "the divergence must be real for this test to have teeth"
    );

    // Drive a REAL left press at the overlap. The unified live router must focus
    // the tree's pick (A), proving the live path no longer uses the flat scan.
    shell.handle_platform_event(&left_press(300.0, 300.0));
    assert_eq!(
        shell.focus.focused(),
        tree_pick,
        "a live left press must focus exactly the window the canonical tree router picks"
    );
    assert_eq!(
        shell.focus.focused(),
        Some(a),
        "the unified router focuses A (tree pick), NOT B (the retired flat-scan pick)"
    );
}

/// AOT-over-normal composition on the LIVE click path (E1 + E3 compose). A
/// click over the overlap of an always-on-top window and a normal one must
/// focus the AOT window, because the unified router resolves through the tree,
/// which E1's `restack_tree_band_order` keeps band-correct. Reproduces the gap:
/// a normal window raised last would, under a naive scan, steal the click.
#[test]
fn live_left_press_over_aot_window_focuses_the_aot_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(200.0, 200.0, 400.0, 300.0));

    // Pin A always-on-top through the real action path, then raise the normal B.
    toggle_always_on_top(&mut shell, a);
    shell.raise_window(b).unwrap();

    // The canonical router places AOT A on top at the overlap.
    assert_eq!(shell.window_at_point(300.0, 300.0), Some(a));

    // A real left press at the overlap must focus the AOT window A, not the
    // freshly-raised normal B.
    shell.handle_platform_event(&left_press(300.0, 300.0));
    assert_eq!(
        shell.focus.focused(),
        Some(a),
        "a click over an AOT window covering a normal one must focus the AOT window"
    );
}

/// The off-edge resize-ring fallback survives unification: a press just OUTSIDE
/// a window's exact bounds (within `resize_tolerance`) still picks that window
/// so the resize affordance works. The canonical tree (exact bounds) misses
/// such a point, so this exercises the fallback half of `pick_window_at`.
#[test]
fn live_press_in_resize_ring_just_outside_bounds_still_picks_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // A resizable, decorated window away from screen edges.
    let id = shell.open_window("A", Rect::new(400.0, 400.0, 300.0, 200.0));
    if let Ok(w) = shell.window_mut(id) {
        w.flags.set(WindowFlags::DECORATED);
        w.flags.set(WindowFlags::RESIZABLE);
    }
    let rt = shell.decoration_style.resize_tolerance;
    assert!(rt > 0.0, "resize tolerance must be positive for this test");

    // A point just LEFT of the exact bounds — inside the resize ring, outside
    // the tree's exact-bounds hit-test.
    let x = 400.0 - rt / 2.0;
    let y = 500.0;
    assert_eq!(
        shell.window_at_point(x, y),
        None,
        "the exact-bounds tree hit-test misses a point in the off-edge resize ring"
    );
    assert_eq!(
        shell.pick_window_at(x, y),
        Some(id),
        "the unified picker's resize-ring fallback still picks the window off-edge"
    );

    // And a real press there focuses the window (so a resize grab can start).
    shell.handle_platform_event(&left_press(x, y));
    assert_eq!(
        shell.focus.focused(),
        Some(id),
        "a press in the resize ring focuses the window for resize"
    );
}

/// Invariant guard for the common case: with no overlap and no focus/z skew,
/// the unified picker and a plain band-ordered flat scan agree. This pins the
/// requirement that the two former paths produce identical results for the
/// ordinary case (so unification is behavior-preserving there).
#[test]
fn unified_picker_agrees_with_flat_scan_for_non_overlapping_windows() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 200.0, 150.0));
    let b = shell.open_window("B", Rect::new(700.0, 100.0, 200.0, 150.0));

    for (x, y, expect) in [
        (150.0_f32, 150.0_f32, Some(a)),
        (750.0_f32, 150.0_f32, Some(b)),
        (500.0_f32, 800.0_f32, None), // empty desktop
    ] {
        let unified = shell.pick_window_at(x, y);
        let flat = shell
            .visible_windows()
            .into_iter()
            .rev()
            .find(|w| w.bounds.contains(liquide_compositor::geometry::Point::new(x, y)))
            .map(|w| w.id);
        assert_eq!(unified, flat, "unified vs flat disagree at ({x},{y})");
        assert_eq!(unified, expect, "wrong window at ({x},{y})");
    }
}

/// Hover button-highlight routes through the canonical router too: a window
/// occluded at the cursor must not get a title-bar-button hover. Reproduces the
/// old flat-scan bug where a lower window whose title bar sat under the cursor
/// could be highlighted even though a higher window covered that exact point.
#[test]
fn hover_button_highlight_uses_tree_router_not_occluded_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // A: decorated; its title bar occupies y in [300, 336), buttons on the right.
    let a = shell.open_window("A", Rect::new(300.0, 300.0, 400.0, 100.0));
    // B: topmost, covers A's right-side title-bar button region with its body.
    let b = shell.open_window("B", Rect::new(620.0, 300.0, 300.0, 200.0));
    assert!(shell.window(a).unwrap().flags.contains(WindowFlags::DECORATED));

    // A point over A's title-bar button strip that B now covers (B is topmost).
    let (hx, hy) = (690.0_f32, 316.0_f32);
    assert_eq!(
        shell.window_at_point(hx, hy),
        Some(b),
        "canonical router places B on top at the hover point"
    );

    shell.handle_platform_event(&mouse_move(hx, hy));
    // The occluded window A must NOT receive a button hover.
    assert!(
        shell.hovered_button.map(|(wid, _)| wid) != Some(a),
        "an occluded window must not get a title-bar-button hover ({:?})",
        shell.hovered_button
    );
}

/// Right-click window picking also routes through the canonical router: a
/// right-press on a window's title bar opens the app menu for the window the
/// tree picks (the topmost), not a different window a flat scan might choose.
#[test]
fn live_right_press_on_titlebar_uses_tree_router_for_app_menu() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(200.0, 100.0, 400.0, 300.0));
    if let Ok(w) = shell.window_mut(a) {
        w.flags.set(WindowFlags::DECORATED);
    }
    if let Ok(w) = shell.window_mut(b) {
        w.flags.set(WindowFlags::DECORATED);
    }

    // Both title bars are at y in [100, 100+tbh). At x=300 both A and B overlap;
    // B is topmost (created last). The tree picks B.
    assert_eq!(shell.window_at_point(300.0, 110.0), Some(b));
    shell.handle_platform_event(&right_press(300.0, 110.0));
    assert_eq!(
        shell.app_menu_open.as_deref(),
        Some(format!("window-{}", b.0).as_str()),
        "right-click on the overlapping title bar opens the app menu for the tree-topmost window"
    );
}
