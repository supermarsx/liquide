use crate::window::WindowId;
use crate::workspace::*;

// ── t52-e6: WorkspaceId single-source proofs ─────────────────────────────
//
// `WorkspaceId` is now single-sourced onto `liquide_workspaces` (Wave N3). The
// shell `Workspace` / `WorkspaceManager` remain documented 0-based facade
// adapters, but the id newtype is THE canonical one. These tests pin that
// guarantee and re-prove the facade contracts the migration must not weaken.

/// The shell-facade `WorkspaceId` re-export IS the canonical type (not a
/// shell-local clone) — a value of one is accepted where the other is expected.
#[test]
fn e6_workspace_id_is_single_sourced_onto_canonical() {
    let shell_id: WorkspaceId = WorkspaceId(0);
    // Compiles only if `crate::workspace::WorkspaceId` ≡ `liquide_workspaces::WorkspaceId`.
    let canonical: liquide_workspaces::WorkspaceId = shell_id;
    assert_eq!(canonical.0, 0);
    assert_eq!(canonical.raw(), 0);
    // Canonical `Display` is the single source of the `Workspace(N)` format.
    assert_eq!(format!("{shell_id}"), "Workspace(0)");
}

/// The 0-based facade contract survives single-sourcing: the default workspace
/// is `WorkspaceId(0)` and freshly created ones are dense 0-based ids.
#[test]
fn e6_facade_ids_stay_zero_based_after_single_source() {
    let mut mgr = WorkspaceManager::new();
    assert_eq!(mgr.active().id, WorkspaceId(0));
    let second = mgr.create_workspace("Second");
    assert_eq!(second, WorkspaceId(1));
    let third = mgr.create_workspace("Third");
    assert_eq!(third, WorkspaceId(2));
}

/// F01 (membership-filtered visibility) re-proof at the facade level after the
/// id type collapsed: a window's owning workspace is reported by its 0-based
/// facade id, and membership does not bleed across workspaces.
#[test]
fn e6_f01_membership_owner_is_the_zero_based_facade_id() {
    let mut mgr = WorkspaceManager::new();
    let ws1 = mgr.create_workspace("Second");
    let a = WindowId(10);
    let b = WindowId(20);
    // A on ws0 (active), B on ws1.
    mgr.active_mut().add_window(a);
    assert!(mgr.switch_to(ws1).is_ok());
    mgr.active_mut().add_window(b);

    assert_eq!(mgr.find_window(a), Some(WorkspaceId(0)));
    assert_eq!(mgr.find_window(b), Some(WorkspaceId(1)));
    // No cross-membership bleed.
    assert!(!mgr.active().contains(a));
    assert!(mgr.active().contains(b));
}

#[test]
fn workspace_create() {
    let ws = Workspace::new(WorkspaceId(0), "Test");
    assert_eq!(ws.name, "Test");
    assert!(ws.is_empty());
}

#[test]
fn workspace_add_window() {
    let mut ws = Workspace::new(WorkspaceId(0), "Test");
    ws.add_window(WindowId(1));
    assert_eq!(ws.window_count(), 1);
    assert!(ws.contains(WindowId(1)));
}

#[test]
fn workspace_remove_window() {
    let mut ws = Workspace::new(WorkspaceId(0), "Test");
    ws.add_window(WindowId(1));
    assert!(ws.remove_window(WindowId(1)));
    assert!(!ws.contains(WindowId(1)));
    assert!(!ws.remove_window(WindowId(99)));
}

#[test]
fn workspace_contains() {
    let mut ws = Workspace::new(WorkspaceId(0), "Test");
    assert!(!ws.contains(WindowId(1)));
    ws.add_window(WindowId(1));
    assert!(ws.contains(WindowId(1)));
}

#[test]
fn workspace_is_empty() {
    let ws = Workspace::new(WorkspaceId(0), "Test");
    assert!(ws.is_empty());
}

#[test]
fn ws_manager_create_workspace() {
    let mut mgr = WorkspaceManager::new();
    assert_eq!(mgr.workspace_count(), 1);
    let id = mgr.create_workspace("Second");
    assert_eq!(mgr.workspace_count(), 2);
    assert_eq!(id, WorkspaceId(1));
}

#[test]
fn ws_manager_switch() {
    let mut mgr = WorkspaceManager::new();
    let id2 = mgr.create_workspace("Second");
    assert!(mgr.switch_to(id2).is_ok());
    assert_eq!(mgr.active().id, id2);
}

#[test]
fn ws_manager_move_window() {
    let mut mgr = WorkspaceManager::new();
    let ws2 = mgr.create_workspace("Second");
    let win = WindowId(42);
    mgr.active_mut().add_window(win);
    assert!(mgr.move_window(win, WorkspaceId(0), ws2).is_ok());
    assert_eq!(mgr.find_window(win), Some(ws2));
}

#[test]
fn ws_manager_find_window() {
    let mut mgr = WorkspaceManager::new();
    let win = WindowId(1);
    mgr.active_mut().add_window(win);
    assert_eq!(mgr.find_window(win), Some(WorkspaceId(0)));
    assert_eq!(mgr.find_window(WindowId(99)), None);
}

#[test]
fn ws_manager_remove_workspace() {
    let mut mgr = WorkspaceManager::new();
    let ws2 = mgr.create_workspace("Second");
    assert!(mgr.remove_workspace(ws2).is_ok());
    assert_eq!(mgr.workspace_count(), 1);
    assert!(mgr.remove_workspace(WorkspaceId(0)).is_err());
}
