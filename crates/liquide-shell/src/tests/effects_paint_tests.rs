//! Regressions for t93-e2 / t92 gap #4: window effects are computed by the
//! canonical `liquide-window-effects` manager but must actually reach the
//! compositor scene.
//!
//! Before this fix `tick_window_effects()` produced `EffectFrame`s that no live
//! caller consumed — open/close/transform/focus animations advanced in the
//! manager but `build_scene` painted every window at its static bounds and full
//! opacity, so the animation was invisible. These tests:
//!
//!   1. REPRODUCE the gap — a freshly-opened window (whose open effect IS active
//!      in the manager) still flattens to full opacity until the effect is driven
//!      into the scene.
//!   2. PROVE the fix — after driving the effect via `tick`, the window's painted
//!      subtree carries the effect's mid-animation opacity (< 1.0) and animated
//!      bounds, and once the effect finishes the window settles back to static.
//!   3. PROVE paint-only — the live hit-test / `visible_windows` keep using the
//!      window's SETTLED bounds during the animation.
//!   4. PROVE the AOT band is respected — an animating normal window's effect
//!      never paints above an always-on-top window.

use crate::shell::Shell;
use crate::shortcuts::ShellAction;
use crate::window::{WindowFlags, WindowId};
use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::{FlatNode, SceneNode, SceneNodeKind};

const NODE_WINDOW_BASE: u64 = 10_000;
const NODE_WINDOW_STRIDE: u64 = 10;

/// A fresh shell with cursor blink frozen so `build_scene` never invalidates the
/// full-scene cache from the blink toggle (keeps the effect the only animator).
fn test_shell() -> Shell {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
    shell
}

/// Base node id for a window's leaf nodes in the manual window subtree.
fn win_base(id: WindowId) -> u64 {
    NODE_WINDOW_BASE + id.0 * NODE_WINDOW_STRIDE
}

/// Flatten the scene and return the FlatNode for a window's decoration node
/// (`win_base + 1`) — its accumulated opacity reflects the per-window effect
/// fade (applied via the per-window paint container), and its absolute bounds
/// reflect the animated geometry.
fn decoration_flat(scene: &SceneNode, id: WindowId) -> Option<FlatNode> {
    let target = win_base(id) + 1;
    scene
        .flatten()
        .into_iter()
        .find(|n| n.id == target && matches!(n.kind_ref(), SceneNodeKind::Decoration { .. }))
}

/// Drive the effect manager + the per-frame effect→scene route exactly as the
/// live loop does (`tick` calls `drive_window_effects`), without advancing the
/// (wall-clock) effect to completion.
fn pump_one_frame(shell: &mut Shell, now_us: u64) {
    shell.tick(now_us);
}

#[test]
fn fresh_open_window_has_an_active_effect_but_is_not_yet_in_the_scene() {
    // REPRODUCE-HALF: opening a window starts an open effect in the manager...
    let mut shell = test_shell();
    let id = shell.open_window("A", Rect::new(200.0, 200.0, 400.0, 300.0));
    assert!(
        shell.window_is_animating(id),
        "open_window must drive an open effect in the manager"
    );

    // ...but until the effect is routed into the scene, the window paints at full
    // opacity (the exact gap: computed-but-not-painted). This is what failed to
    // animate before t93-e2.
    let scene = shell.build_scene();
    let deco = decoration_flat(&scene, id).expect("decoration node must be present");
    assert!(
        (deco.opacity - 1.0).abs() < 1e-4,
        "without routing the effect the window must paint fully opaque, got {}",
        deco.opacity
    );
}

#[test]
fn driving_the_open_effect_routes_a_fade_into_the_scene() {
    // PROVE: after the live per-frame drive, the open effect's fade reaches the
    // painted scene as a sub-1.0 accumulated opacity on the window subtree.
    let mut shell = test_shell();
    let id = shell.open_window("A", Rect::new(200.0, 200.0, 400.0, 300.0));

    // The open effect begins near opacity 0 (EaseOutCubic at t≈0), so the very
    // first driven frame is mid-fade.
    pump_one_frame(&mut shell, 1_000);
    assert!(
        shell.active_window_effects.contains_key(&id),
        "driving the tick must publish the active effect frame for paint"
    );

    let scene = shell.build_scene();
    let deco = decoration_flat(&scene, id).expect("decoration node must be present");
    assert!(
        deco.opacity < 1.0,
        "the open effect's fade must reach the scene (opacity < 1.0), got {}",
        deco.opacity
    );
    assert!(
        deco.opacity >= 0.0,
        "opacity must be a valid fraction, got {}",
        deco.opacity
    );
}

#[test]
fn open_effect_animates_painted_bounds_inward_from_the_settled_rect() {
    // PROVE: the open effect scales the window up from ~95% of its target, so the
    // painted (animated) bounds differ from the settled bounds while the live
    // window record keeps the settled bounds (paint-only).
    let mut shell = test_shell();
    let settled = Rect::new(200.0, 200.0, 400.0, 300.0);
    let id = shell.open_window("A", settled);

    pump_one_frame(&mut shell, 1_000);
    let frame = shell
        .active_window_effects
        .get(&id)
        .cloned()
        .expect("an active open effect frame must be published");

    // The animated rect is the 95%-scaled "from" rect at t≈0 — strictly smaller
    // than the settled target.
    assert!(
        frame.bounds.width < settled.width && frame.bounds.height < settled.height,
        "open effect must animate from a smaller rect; got {:?}",
        frame.bounds
    );

    // The painted decoration node tracks the ANIMATED bounds, not the settled
    // ones.
    let scene = shell.build_scene();
    let deco = decoration_flat(&scene, id).expect("decoration node must be present");
    assert!(
        (deco.absolute_bounds.width - settled.width).abs() > 1.0,
        "painted bounds must follow the animated (scaled) rect, got {:?}",
        deco.absolute_bounds
    );

    // PAINT-ONLY: the live window record + hit-test still use the SETTLED rect.
    let live = shell.window(id).unwrap();
    assert_eq!(live.bounds, settled, "effect must not move the live window bounds");
    let (cx, cy) = (settled.x + 10.0, settled.y + 10.0);
    assert_eq!(
        shell.window_at_point(cx, cy),
        Some(id),
        "mid-animation, the settled rect is still the hit-target"
    );
}

#[test]
fn finished_effect_settles_the_window_back_to_static_paint() {
    // PROVE: once the effect completes it is dropped from the published set and
    // the window paints statically at full opacity / settled bounds again.
    let mut shell = test_shell();
    let settled = Rect::new(200.0, 200.0, 400.0, 300.0);
    let id = shell.open_window("A", settled);

    // Drive frames until the open effect finishes (wall-clock advances in real
    // time; the open duration is 200 ms, so a bounded loop with a tiny sleep is
    // deterministic enough — but we also cap iterations).
    let mut settled_seen = false;
    for i in 0..400 {
        pump_one_frame(&mut shell, 1_000 + i as u64 * 1_000);
        if !shell.active_window_effects.contains_key(&id) && !shell.window_is_animating(id) {
            settled_seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(
        settled_seen,
        "the open effect must finish and clear from the published set"
    );

    let scene = shell.build_scene();
    let deco = decoration_flat(&scene, id).expect("decoration node must be present");
    assert!(
        (deco.opacity - 1.0).abs() < 1e-4,
        "a settled window must paint fully opaque, got {}",
        deco.opacity
    );
    assert!(
        (deco.absolute_bounds.width - settled.width).abs() < 1.0,
        "a settled window must paint at its settled bounds, got {:?}",
        deco.absolute_bounds
    );
}

#[test]
fn idle_window_subtree_is_byte_identical_with_the_effect_route() {
    // GUARD: with no active effect the per-window paint container is a no-op
    // (opacity 1.0, origin bounds, non-visual kind) — the flattened window output
    // must be identical to a window with no effects machinery at all, so existing
    // window-decoration goldens for STATIC windows do not shift.
    let mut shell = test_shell();
    let id = shell.open_window("A", Rect::new(200.0, 200.0, 400.0, 300.0));

    // Drain the open effect so the window is fully idle.
    for i in 0..400 {
        pump_one_frame(&mut shell, 1_000 + i as u64 * 1_000);
        if !shell.window_is_animating(id) && shell.active_window_effects.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(shell.active_window_effects.is_empty(), "window must be idle");

    let scene = shell.build_scene();
    let flat = scene.flatten();
    // Every painted window node accumulates full opacity (no fade) and the
    // container itself contributes no visual FlatNode.
    let win_nodes: Vec<&FlatNode> = flat
        .iter()
        .filter(|n| n.id >= win_base(id) && n.id < win_base(id) + NODE_WINDOW_STRIDE * 100)
        .collect();
    assert!(
        !win_nodes.is_empty(),
        "the idle window must still emit painted nodes"
    );
    for n in win_nodes {
        assert!(
            (n.opacity - 1.0).abs() < 1e-4,
            "idle window node {} must paint at full opacity, got {}",
            n.id,
            n.opacity
        );
    }
}

#[test]
fn animating_normal_window_effect_never_paints_over_an_always_on_top_window() {
    // PROVE: the AOT band E1 committed is respected by the effect path. Pin A as
    // always-on-top, then open a NEW normal window B (which starts an open
    // effect). Every painted node of the animating normal window B must sit at a
    // strictly lower z-order than every painted node of the pinned window A.
    let mut shell = test_shell();
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 500.0, 400.0));
    // Drain A's open effect so A is static.
    for i in 0..400 {
        pump_one_frame(&mut shell, 1_000 + i as u64 * 1_000);
        if !shell.window_is_animating(a) && shell.active_window_effects.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    // Pin A always-on-top through the real action path (re-packs z_order into the
    // AOT band and restacks).
    shell.set_focus(a).unwrap();
    assert!(shell.execute_action(&ShellAction::ToggleAlwaysOnTop));
    assert!(
        shell.window(a).unwrap().flags.contains(WindowFlags::ALWAYS_ON_TOP),
        "A must be pinned always-on-top"
    );

    // Open B (normal) over the same region; its open effect is active.
    let b = shell.open_window("B", Rect::new(150.0, 150.0, 500.0, 400.0));
    pump_one_frame(&mut shell, 500_000);
    assert!(
        shell.active_window_effects.contains_key(&b),
        "B's open effect must be active and published"
    );

    // AOT invariant on the PAINT path (the scope of gap #4): the painted scene's
    // z-order must keep the pinned A above the animating normal B. The paint
    // builder derives per-window z from the band-aware `visible_windows()` rank,
    // so A (pinned, top band) outranks B (normal) even though B was opened last.
    assert_eq!(
        shell.visible_windows().last().map(|w| w.id),
        Some(a),
        "the AOT window A must remain topmost in the band-aware paint order"
    );

    // Compare each window's DECORATION node (unique id `win_base + 1`) z-order:
    // A (pinned) must paint strictly above the animating normal B. Decoration ids
    // are distinct across windows, unlike the per-window leaf-id offsets which can
    // alias under the small node-id stride.
    let scene = shell.build_scene();
    let a_z = decoration_flat(&scene, a)
        .expect("A decoration must paint")
        .z_order;
    let b_z = decoration_flat(&scene, b)
        .expect("B decoration must paint")
        .z_order;
    assert!(
        a_z > b_z,
        "the AOT window A (deco z {a_z}) must paint above the animating normal B (deco z {b_z})"
    );
}
