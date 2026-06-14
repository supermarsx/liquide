//! Regressions for t51-e12: the canonical `liquide-workspaces` manager wired
//! into the running shell, the t49-e5-F01 fix (workspaces are no longer
//! cosmetic), and the t51-e10 `lock_session()` integration into the
//! `LockSession` action arm.
//!
//! These assert REAL behavior driven through the new wiring:
//!   - `visible_windows()` filters by active-workspace membership,
//!   - a window on workspace B is hidden while A is active and reappears on
//!     switching to B,
//!   - switching is driven through the canonical `WorkspaceManager`
//!     (`chrome_workspaces` becomes populated/authoritative),
//!   - moving a window to another workspace repatriates its visibility,
//!   - the `LockSession` action arm drives the canonical lockscreen.

use crate::shell::Shell;
use crate::shortcuts::ShellAction;
use liquide_compositor::geometry::Rect;

/// Open a window and return its id (it lands on the active workspace).
fn open(shell: &mut Shell, title: &str) -> crate::window::WindowId {
    shell.open_window(title, Rect::new(0.0, 0.0, 400.0, 300.0))
}

#[test]
fn f01_window_on_other_workspace_is_not_visible_while_first_is_active() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Two workspaces.
    shell.execute_action(&ShellAction::WorkspaceAdd);

    // Window A opened on workspace 0 (the active one).
    let a = open(&mut shell, "A");
    assert!(
        shell.visible_windows().iter().any(|w| w.id == a),
        "A should be visible on its own (active) workspace"
    );

    // Switch to workspace 1 and open B there.
    assert!(
        shell.execute_action(&ShellAction::SwitchToWorkspace(1)),
        "SwitchToWorkspace returns true (action handled)"
    );
    let b = open(&mut shell, "B");

    // While workspace 1 is active: B visible, A NOT visible (real switch, not
    // cosmetic). This is the F01 fix.
    let vis: Vec<_> = shell.visible_windows().iter().map(|w| w.id).collect();
    assert!(vis.contains(&b), "B is on the active workspace -> visible");
    assert!(
        !vis.contains(&a),
        "A is on workspace 0 -> must NOT render while workspace 1 is active (F01)"
    );
}

#[test]
fn f01_window_reappears_after_switching_back_to_its_workspace() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.execute_action(&ShellAction::WorkspaceAdd);

    let a = open(&mut shell, "A"); // workspace 0
    shell.execute_action(&ShellAction::SwitchToWorkspace(1));
    let b = open(&mut shell, "B"); // workspace 1

    // Back to workspace 0: A visible again, B hidden.
    assert!(shell.execute_action(&ShellAction::SwitchToWorkspace(0)));
    let vis: Vec<_> = shell.visible_windows().iter().map(|w| w.id).collect();
    assert!(
        vis.contains(&a),
        "A reappears on switching back to workspace 0"
    );
    assert!(
        !vis.contains(&b),
        "B (workspace 1) is hidden while workspace 0 is active"
    );
}

#[test]
fn switching_is_driven_through_the_canonical_workspace_manager() {
    // t52-e5: the canonical `liquide-workspaces` manager is now embedded inside
    // `self.workspaces` (single-sourced) — there is no separate
    // `chrome_workspaces` field. The canonical engine is reachable read-only via
    // `WorkspaceManager::canonical()`.
    let mut shell = Shell::new(1920.0, 1080.0);
    assert_eq!(
        shell.workspace_manager().canonical().workspace_count(),
        1,
        "single workspace before any add"
    );

    shell.execute_action(&ShellAction::WorkspaceAdd);
    assert!(shell.execute_action(&ShellAction::WorkspaceNext));

    // The canonical manager tracks the workspace count and active workspace.
    assert!(
        shell.workspace_manager().canonical().workspace_count() >= 2,
        "canonical manager mirrors the workspace count"
    );
    // Active workspace moved off index 0.
    assert_ne!(
        shell.workspace_manager().active().id.0,
        0,
        "WorkspaceNext advanced the active workspace"
    );
}

#[test]
fn workspace_next_then_prev_returns_to_origin_and_restores_visibility() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.execute_action(&ShellAction::WorkspaceAdd);
    let a = open(&mut shell, "A"); // workspace 0

    shell.execute_action(&ShellAction::WorkspaceNext); // -> ws 1, A hidden
    assert!(
        !shell.visible_windows().iter().any(|w| w.id == a),
        "A hidden on workspace 1"
    );

    shell.execute_action(&ShellAction::WorkspacePrev); // -> ws 0, A shown
    assert_eq!(shell.workspace_manager().active().id.0, 0);
    assert!(
        shell.visible_windows().iter().any(|w| w.id == a),
        "A visible again after returning to workspace 0"
    );
}

#[test]
fn moving_focused_window_to_another_workspace_repatriates_visibility() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.execute_action(&ShellAction::WorkspaceAdd);

    let a = open(&mut shell, "A"); // workspace 0, focused on open path
    shell.set_focus(a).unwrap();
    assert!(shell.visible_windows().iter().any(|w| w.id == a));

    // Move it to workspace 1; it leaves the active (workspace 0) rendered set.
    assert!(shell.execute_action(&ShellAction::MoveWindowToWorkspace(1)));
    assert!(
        !shell.visible_windows().iter().any(|w| w.id == a),
        "moved window stops rendering on the (still-active) workspace 0"
    );
    // Canonical manager records the move.
    assert_eq!(
        shell.workspace_manager().find_window(a),
        Some(crate::workspace::WorkspaceId(1)),
        "internal manager records the window on workspace 1"
    );

    // Follow it to workspace 1: now it renders there.
    shell.execute_action(&ShellAction::SwitchToWorkspace(1));
    assert!(
        shell.visible_windows().iter().any(|w| w.id == a),
        "the moved window renders on its new workspace"
    );
}

#[test]
fn switching_workspaces_does_not_leak_inactive_windows_into_input() {
    // Defense-in-depth: input/hit-test iterate `visible_windows()`, so a window
    // on an inactive workspace must not appear in that set (and thus cannot be
    // hit-tested or receive input).
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.execute_action(&ShellAction::WorkspaceAdd);
    let a = open(&mut shell, "A"); // ws 0

    shell.execute_action(&ShellAction::SwitchToWorkspace(1));
    let hit_targets: Vec<_> = shell
        .visible_windows()
        .into_iter()
        .rev()
        .map(|w| w.id)
        .collect();
    assert!(
        !hit_targets.contains(&a),
        "a window on an inactive workspace must not be an input/hit-test target"
    );
}

/// t62 CRITICAL-2 regression: `arrange_windows` must apply the active layout
/// only to windows that are members of the active workspace. With a non-trivial
/// (tiling) layout policy installed, a window on an inactive workspace must keep
/// its original bounds — otherwise the layout engine relocates inactive-
/// workspace windows and they flicker into view on switch.
#[test]
fn arrange_windows_ignores_inactive_workspace_windows() {
    use crate::layout::TilingLayout;
    use liquide_compositor::geometry::Rect as GRect;

    let mut shell = Shell::new(1920.0, 1080.0);
    shell.set_layout(Box::new(TilingLayout::new(8.0, 2)));
    shell.execute_action(&ShellAction::WorkspaceAdd);

    // A on workspace 0 (active).
    let a = shell.open_window("A", GRect::new(5.0, 5.0, 123.0, 77.0));

    // B on workspace 1.
    assert!(shell.execute_action(&ShellAction::SwitchToWorkspace(1)));
    let _b = shell.open_window("B", GRect::new(0.0, 0.0, 100.0, 100.0));

    // Force A's `visible` flag back on so the membership filter is the sole
    // guard under test (the switch path otherwise sets `visible=false`, masking
    // the bug). t62 CRITICAL-2 is specifically about the membership filter.
    shell.window_mut(a).unwrap().visible = true;
    let a_before = shell.window(a).unwrap().bounds;

    // Arrange while workspace 1 is active — A (workspace 0) must be untouched.
    shell.arrange_windows();
    assert_eq!(
        shell.window(a).unwrap().bounds,
        a_before,
        "arrange_windows must not lay out windows belonging to inactive workspaces"
    );
}

/// t62 CRITICAL-1 regression: closing a window that lives on an *inactive*
/// workspace must remove it from its **owning** workspace, not the active one.
/// Otherwise a stale membership entry dangles and the (now-destroyed) window id
/// resurfaces when switching back to that workspace.
#[test]
fn close_window_on_inactive_workspace_removes_from_owning_workspace() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.execute_action(&ShellAction::WorkspaceAdd);

    // A on workspace 0 (active).
    let a = open(&mut shell, "A");

    // B on workspace 1.
    assert!(shell.execute_action(&ShellAction::SwitchToWorkspace(1)));
    let b = open(&mut shell, "B");

    // Back to workspace 0 (B's workspace is now inactive) and close B from here.
    assert!(shell.execute_action(&ShellAction::SwitchToWorkspace(0)));
    shell.close_window(b).expect("closing B succeeds");

    // B must no longer be a member of any workspace — in particular it must not
    // dangle on workspace 1.
    assert_eq!(
        shell.workspace_manager().find_window(b),
        None,
        "closed window must be removed from its owning (inactive) workspace, not left dangling"
    );

    // Switching back to workspace 1 must not resurrect B as visible.
    assert!(shell.execute_action(&ShellAction::SwitchToWorkspace(1)));
    assert!(
        !shell.visible_windows().iter().any(|w| w.id == b),
        "a window closed while its workspace was inactive must not reappear on switch"
    );

    // A is unaffected and still owned by workspace 0.
    assert_eq!(
        shell.workspace_manager().find_window(a),
        Some(crate::workspace::WorkspaceId(0)),
    );
}

#[test]
fn lock_session_action_drives_canonical_lockscreen() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(!shell.is_session_locked(), "session starts unlocked");

    // The production LockSession arm now folds in t51-e10's lock_session().
    let handled = shell.execute_action(&ShellAction::LockSession);
    assert!(handled, "LockSession action is handled");
    assert!(
        shell.is_session_locked(),
        "LockSession arm drove the canonical lockscreen into the locked state"
    );
}

// ── t52-e5: 0-vs-1-based id identity re-proof (single-sourcing F01) ──────────
//
// The workspace manager was collapsed onto the canonical `liquide-workspaces`
// engine, which uses 1-based ids internally; the shell facade stays 0-based.
// These assert the flip introduced no off-by-one: workspace 1 vs workspace 2
// (0-based ids 0 vs 1) membership/visibility is exactly right, and the facade
// ids/index math used by every caller (scene node id, tick.rs, batch.rs) line
// up with the 0-based contract the rest of the shell relies on.

#[test]
fn e5_facade_ids_are_zero_based_after_canonical_collapse() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // The single default workspace is the 0-based first workspace.
    assert_eq!(
        shell.workspace_manager().active().id.0,
        0,
        "first/default workspace has 0-based facade id 0 (canonical 1-based hidden)"
    );
    assert_eq!(shell.workspace_manager().workspace_count(), 1);

    // Add a second workspace — still 0-based facade (ids 0 and 1, never 1 and 2).
    shell.execute_action(&ShellAction::WorkspaceAdd);
    assert_eq!(shell.workspace_manager().workspace_count(), 2);
    let ids: Vec<u32> = (0..2)
        .map(|i| {
            shell.execute_action(&ShellAction::SwitchToWorkspace(i as u32));
            shell.workspace_manager().active().id.0
        })
        .collect();
    assert_eq!(
        ids,
        vec![0, 1],
        "workspace facade ids are 0-based and dense (no off-by-one from the 1-based canonical flip)"
    );
}

#[test]
fn e5_f01_membership_correct_for_workspace_one_vs_two() {
    // Re-prove F01 precisely across the 0-vs-1 boundary: a window opened on the
    // FIRST workspace (facade id 0) must be hidden when the SECOND (facade id 1)
    // is active, and vice-versa — and the *correct* window each time (no swap).
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.execute_action(&ShellAction::WorkspaceAdd);

    // On workspace 0: open A.
    assert_eq!(shell.workspace_manager().active().id.0, 0);
    let a = open(&mut shell, "A");
    // Switch to workspace 1: open B.
    assert!(shell.execute_action(&ShellAction::SwitchToWorkspace(1)));
    assert_eq!(shell.workspace_manager().active().id.0, 1);
    let b = open(&mut shell, "B");

    // Membership is recorded against the correct (0-based) workspaces.
    assert_eq!(
        shell.workspace_manager().find_window(a),
        Some(crate::workspace::WorkspaceId(0)),
        "A belongs to workspace 0"
    );
    assert_eq!(
        shell.workspace_manager().find_window(b),
        Some(crate::workspace::WorkspaceId(1)),
        "B belongs to workspace 1"
    );

    // On workspace 1: only B renders.
    let vis1: Vec<_> = shell.visible_windows().iter().map(|w| w.id).collect();
    assert!(vis1.contains(&b) && !vis1.contains(&a));

    // Back to workspace 0: only A renders (correct window, not B).
    assert!(shell.execute_action(&ShellAction::SwitchToWorkspace(0)));
    assert_eq!(shell.workspace_manager().active().id.0, 0);
    let vis0: Vec<_> = shell.visible_windows().iter().map(|w| w.id).collect();
    assert!(
        vis0.contains(&a) && !vis0.contains(&b),
        "workspace 0 shows A (its member), not B — membership not off-by-one-swapped"
    );
}

#[test]
fn e5_switch_to_already_active_workspace_is_a_noop() {
    // Guards the adapter's "actually changed" detection: switching to the
    // already-active workspace must not toggle visibility (a stale off-by-one
    // here would hide/show the wrong members).
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = open(&mut shell, "A"); // workspace 0, active
    assert!(shell.visible_windows().iter().any(|w| w.id == a));
    // Switch to workspace 0 while already on it.
    shell.execute_action(&ShellAction::SwitchToWorkspace(0));
    assert!(
        shell.visible_windows().iter().any(|w| w.id == a),
        "A stays visible — switching to the active workspace is a no-op"
    );
}
