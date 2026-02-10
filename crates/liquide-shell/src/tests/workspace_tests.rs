use crate::window::WindowId;
use crate::workspace::*;

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
