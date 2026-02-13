use crate::seamless::*;
use crate::window::{WindowId, WindowState};
use liquide_compositor::geometry::Rect;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_manager() -> SeamlessManager {
    SeamlessManager::new(SeamlessConfig::default())
}

fn make_window(id: u64, app_id: &str, title: &str) -> SeamlessWindow {
    SeamlessWindow {
        window_id: WindowId(id),
        app_id: app_id.into(),
        title: title.into(),
        icon: None,
        geometry: Rect::new(100.0, 100.0, 800.0, 600.0),
        state: WindowState::Normal,
        z_order: 0,
        parent_id: None,
        window_type: SeamlessWindowType::Normal,
    }
}

fn make_tray_icon(item_id: &str, app_id: &str) -> TrayIconInfo {
    TrayIconInfo {
        item_id: item_id.into(),
        app_id: app_id.into(),
        icon_data: vec![0x89, 0x50, 0x4E, 0x47], // PNG header stub
        tooltip: format!("{app_id} tray icon"),
        menu_items: vec![TrayMenuEntry {
            id: "quit".into(),
            label: "Quit".into(),
            enabled: true,
            separator: false,
        }],
    }
}

// ========== SeamlessConfig defaults ==========

#[test]
fn seamless_config_default_values() {
    let cfg = SeamlessConfig::default();
    assert!(!cfg.enabled);
    assert_eq!(cfg.default_mode, SeamlessMode::Desktop);
    assert!(cfg.exclude_apps.is_empty());
    assert!(!cfg.shell_as_window);
    assert!(cfg.forward_notifications);
    assert!(cfg.forward_tray_icons);
    assert!(cfg.dnd_enabled);
}

// ========== SeamlessManager creation ==========

#[test]
fn manager_new_starts_empty() {
    let mgr = default_manager();
    assert_eq!(mgr.window_count(), 0);
    assert_eq!(mgr.tray_icon_count(), 0);
    assert!(mgr.z_order().is_empty());
    assert_eq!(mgr.mode(), SeamlessMode::Desktop);
}

// ========== Window CRUD ==========

#[test]
fn create_window_adds_to_map() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "term", "Terminal"));
    assert_eq!(mgr.window_count(), 1);
    let win = mgr.window(WindowId(1)).unwrap();
    assert_eq!(win.app_id, "term");
    assert_eq!(win.title, "Terminal");
}

#[test]
fn create_window_adds_to_z_order() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.create_window(make_window(2, "b", "B"));
    assert_eq!(mgr.z_order(), &[WindowId(1), WindowId(2)]);
}

#[test]
fn create_window_does_not_duplicate_in_z_order() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.create_window(make_window(1, "a", "A v2"));
    // z_order should still contain just one entry for WindowId(1)
    assert_eq!(
        mgr.z_order().iter().filter(|&&id| id == WindowId(1)).count(),
        1
    );
}

#[test]
fn destroy_window_removes_from_map_and_z_order() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.create_window(make_window(2, "b", "B"));
    let removed = mgr.destroy_window(WindowId(1));
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().app_id, "a");
    assert_eq!(mgr.window_count(), 1);
    assert!(!mgr.z_order().contains(&WindowId(1)));
}

#[test]
fn destroy_window_nonexistent_returns_none() {
    let mut mgr = default_manager();
    assert!(mgr.destroy_window(WindowId(999)).is_none());
}

// ========== Window updates ==========

#[test]
fn update_geometry_changes_window_rect() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.update_geometry(WindowId(1), Rect::new(200.0, 300.0, 1024.0, 768.0));
    let win = mgr.window(WindowId(1)).unwrap();
    assert!((win.geometry.x - 200.0).abs() < 0.1);
    assert!((win.geometry.width - 1024.0).abs() < 0.1);
}

#[test]
fn update_state_changes_window_state() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.update_state(WindowId(1), WindowState::Maximized);
    assert_eq!(mgr.window(WindowId(1)).unwrap().state, WindowState::Maximized);
}

#[test]
fn update_title_changes_window_title() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.update_title(WindowId(1), "New Title".into());
    assert_eq!(mgr.window(WindowId(1)).unwrap().title, "New Title");
}

#[test]
fn update_icon_sets_icon_data() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.update_icon(WindowId(1), vec![1, 2, 3]);
    assert_eq!(mgr.window(WindowId(1)).unwrap().icon, Some(vec![1, 2, 3]));
}

// ========== Z-order management ==========

#[test]
fn set_z_order_replaces_list() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.create_window(make_window(2, "b", "B"));
    mgr.set_z_order(vec![WindowId(2), WindowId(1)]);
    assert_eq!(mgr.z_order(), &[WindowId(2), WindowId(1)]);
}

// ========== window_mut ==========

#[test]
fn window_mut_allows_modification() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    if let Some(win) = mgr.window_mut(WindowId(1)) {
        win.title = "Modified".into();
    }
    assert_eq!(mgr.window(WindowId(1)).unwrap().title, "Modified");
}

// ========== all_windows ==========

#[test]
fn all_windows_returns_full_map() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.create_window(make_window(2, "b", "B"));
    assert_eq!(mgr.all_windows().len(), 2);
    assert!(mgr.all_windows().contains_key(&WindowId(1)));
}

// ========== Mode management ==========

#[test]
fn default_mode_is_desktop() {
    let mgr = default_manager();
    assert_eq!(mgr.mode(), SeamlessMode::Desktop);
    assert!(!mgr.is_seamless());
}

#[test]
fn set_mode_to_seamless() {
    let mut mgr = default_manager();
    mgr.set_mode(SeamlessMode::Seamless);
    assert_eq!(mgr.mode(), SeamlessMode::Seamless);
    assert!(mgr.is_seamless());
}

#[test]
fn set_mode_back_to_desktop() {
    let mut mgr = default_manager();
    mgr.set_mode(SeamlessMode::Seamless);
    mgr.set_mode(SeamlessMode::Desktop);
    assert!(!mgr.is_seamless());
}

// ========== App exclusion ==========

#[test]
fn is_excluded_returns_false_by_default() {
    let mgr = default_manager();
    assert!(!mgr.is_excluded("some.app"));
}

#[test]
fn is_excluded_with_configured_exclusions() {
    let cfg = SeamlessConfig {
        exclude_apps: vec!["panel".into(), "taskbar".into()],
        ..SeamlessConfig::default()
    };
    let mgr = SeamlessManager::new(cfg);
    assert!(mgr.is_excluded("panel"));
    assert!(mgr.is_excluded("taskbar"));
    assert!(!mgr.is_excluded("firefox"));
}

// ========== Tray icon management ==========

#[test]
fn add_tray_icon_inserts_into_map() {
    let mut mgr = default_manager();
    mgr.add_tray_icon(make_tray_icon("icon1", "app1"));
    assert_eq!(mgr.tray_icon_count(), 1);
    let icon = mgr.tray_icon("icon1").unwrap();
    assert_eq!(icon.app_id, "app1");
    assert_eq!(icon.menu_items.len(), 1);
}

#[test]
fn remove_tray_icon_returns_removed() {
    let mut mgr = default_manager();
    mgr.add_tray_icon(make_tray_icon("icon1", "app1"));
    let removed = mgr.remove_tray_icon("icon1");
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().item_id, "icon1");
    assert_eq!(mgr.tray_icon_count(), 0);
}

#[test]
fn remove_tray_icon_nonexistent_returns_none() {
    let mut mgr = default_manager();
    assert!(mgr.remove_tray_icon("nonexistent").is_none());
}

#[test]
fn tray_icons_returns_full_map() {
    let mut mgr = default_manager();
    mgr.add_tray_icon(make_tray_icon("i1", "a1"));
    mgr.add_tray_icon(make_tray_icon("i2", "a2"));
    assert_eq!(mgr.tray_icons().len(), 2);
}

// ========== apply_message — WindowCreate ==========

#[test]
fn apply_message_window_create() {
    let mut mgr = default_manager();
    mgr.apply_message(SeamlessMessage::WindowCreate {
        window: make_window(10, "app", "Window"),
    });
    assert_eq!(mgr.window_count(), 1);
    assert!(mgr.window(WindowId(10)).is_some());
}

// ========== apply_message — WindowDestroy ==========

#[test]
fn apply_message_window_destroy() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(10, "app", "Window"));
    mgr.apply_message(SeamlessMessage::WindowDestroy {
        window_id: WindowId(10),
    });
    assert_eq!(mgr.window_count(), 0);
}

// ========== apply_message — WindowGeometry ==========

#[test]
fn apply_message_window_geometry() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(10, "app", "Window"));
    mgr.apply_message(SeamlessMessage::WindowGeometry {
        window_id: WindowId(10),
        geometry: Rect::new(50.0, 60.0, 640.0, 480.0),
    });
    let win = mgr.window(WindowId(10)).unwrap();
    assert!((win.geometry.x - 50.0).abs() < 0.1);
    assert!((win.geometry.height - 480.0).abs() < 0.1);
}

// ========== apply_message — WindowState ==========

#[test]
fn apply_message_window_state() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(10, "app", "Window"));
    mgr.apply_message(SeamlessMessage::WindowState {
        window_id: WindowId(10),
        state: WindowState::Minimized,
    });
    assert_eq!(
        mgr.window(WindowId(10)).unwrap().state,
        WindowState::Minimized
    );
}

// ========== apply_message — WindowTitle ==========

#[test]
fn apply_message_window_title() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(10, "app", "Old Title"));
    mgr.apply_message(SeamlessMessage::WindowTitle {
        window_id: WindowId(10),
        title: "New Title".into(),
    });
    assert_eq!(mgr.window(WindowId(10)).unwrap().title, "New Title");
}

// ========== apply_message — WindowZOrder ==========

#[test]
fn apply_message_window_z_order() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.create_window(make_window(2, "b", "B"));
    mgr.apply_message(SeamlessMessage::WindowZOrder {
        window_ids: vec![WindowId(2), WindowId(1)],
    });
    assert_eq!(mgr.z_order(), &[WindowId(2), WindowId(1)]);
}

// ========== apply_message — TrayIconCreate ==========

#[test]
fn apply_message_tray_icon_create() {
    let mut mgr = default_manager();
    mgr.apply_message(SeamlessMessage::TrayIconCreate {
        info: make_tray_icon("t1", "a1"),
    });
    assert_eq!(mgr.tray_icon_count(), 1);
    assert!(mgr.tray_icon("t1").is_some());
}

// ========== apply_message — TrayIconDestroy ==========

#[test]
fn apply_message_tray_icon_destroy() {
    let mut mgr = default_manager();
    mgr.add_tray_icon(make_tray_icon("t1", "a1"));
    mgr.apply_message(SeamlessMessage::TrayIconDestroy {
        item_id: "t1".into(),
    });
    assert_eq!(mgr.tray_icon_count(), 0);
}

// ========== apply_message — TrayIconUpdate ==========

#[test]
fn apply_message_tray_icon_update_tooltip() {
    let mut mgr = default_manager();
    mgr.add_tray_icon(make_tray_icon("t1", "a1"));
    mgr.apply_message(SeamlessMessage::TrayIconUpdate {
        item_id: "t1".into(),
        icon_data: None,
        tooltip: Some("Updated tooltip".into()),
    });
    assert_eq!(mgr.tray_icon("t1").unwrap().tooltip, "Updated tooltip");
}

#[test]
fn apply_message_tray_icon_update_icon_data() {
    let mut mgr = default_manager();
    mgr.add_tray_icon(make_tray_icon("t1", "a1"));
    mgr.apply_message(SeamlessMessage::TrayIconUpdate {
        item_id: "t1".into(),
        icon_data: Some(vec![0xDE, 0xAD]),
        tooltip: None,
    });
    assert_eq!(mgr.tray_icon("t1").unwrap().icon_data, vec![0xDE, 0xAD]);
}

// ========== apply_message — ClientWindowMove ==========

#[test]
fn apply_message_client_window_move() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(10, "app", "Window"));
    mgr.apply_message(SeamlessMessage::ClientWindowMove {
        window_id: WindowId(10),
        x: 300.0,
        y: 400.0,
    });
    let win = mgr.window(WindowId(10)).unwrap();
    assert!((win.geometry.x - 300.0).abs() < 0.1);
    assert!((win.geometry.y - 400.0).abs() < 0.1);
    // Width and height unchanged
    assert!((win.geometry.width - 800.0).abs() < 0.1);
}

// ========== apply_message — ClientWindowResize ==========

#[test]
fn apply_message_client_window_resize() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(10, "app", "Window"));
    mgr.apply_message(SeamlessMessage::ClientWindowResize {
        window_id: WindowId(10),
        width: 1024.0,
        height: 768.0,
    });
    let win = mgr.window(WindowId(10)).unwrap();
    assert!((win.geometry.width - 1024.0).abs() < 0.1);
    assert!((win.geometry.height - 768.0).abs() < 0.1);
    // Position unchanged
    assert!((win.geometry.x - 100.0).abs() < 0.1);
}

// ========== apply_message — ClientWindowClose ==========

#[test]
fn apply_message_client_window_close() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(10, "app", "Window"));
    mgr.apply_message(SeamlessMessage::ClientWindowClose {
        window_id: WindowId(10),
    });
    assert_eq!(mgr.window_count(), 0);
}

// ========== apply_message — transient messages (no state change) ==========

#[test]
fn apply_message_window_focus_is_noop() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    let count_before = mgr.window_count();
    mgr.apply_message(SeamlessMessage::WindowFocus {
        window_id: WindowId(1),
    });
    assert_eq!(mgr.window_count(), count_before);
}

#[test]
fn apply_message_dnd_events_are_noop() {
    let mut mgr = default_manager();
    mgr.apply_message(SeamlessMessage::DndOffer {
        source_window_id: WindowId(1),
        mime_types: vec!["text/plain".into()],
    });
    mgr.apply_message(SeamlessMessage::DndMotion { x: 10.0, y: 20.0 });
    mgr.apply_message(SeamlessMessage::DndFinished { accepted: true });
    mgr.apply_message(SeamlessMessage::DndCancel);
    // No state change
    assert_eq!(mgr.window_count(), 0);
}

#[test]
fn apply_message_client_focus_is_noop() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.apply_message(SeamlessMessage::ClientWindowFocus {
        window_id: WindowId(1),
    });
    assert_eq!(mgr.window_count(), 1);
}

#[test]
fn apply_message_client_tray_action_is_noop() {
    let mut mgr = default_manager();
    mgr.add_tray_icon(make_tray_icon("t1", "a1"));
    mgr.apply_message(SeamlessMessage::ClientTrayAction {
        item_id: "t1".into(),
        action_id: "quit".into(),
    });
    // Tray icon still exists
    assert_eq!(mgr.tray_icon_count(), 1);
}

#[test]
fn apply_message_client_dnd_drop_is_noop() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.apply_message(SeamlessMessage::ClientDndDrop {
        target_window_id: WindowId(1),
        mime_type: "text/plain".into(),
        data: vec![72, 101, 108, 108, 111],
    });
    assert_eq!(mgr.window_count(), 1);
}

// ========== apply_message — ClientWindowState ==========

#[test]
fn apply_message_client_window_state() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(10, "app", "Window"));
    mgr.apply_message(SeamlessMessage::ClientWindowState {
        window_id: WindowId(10),
        state: WindowState::Fullscreen,
    });
    assert_eq!(
        mgr.window(WindowId(10)).unwrap().state,
        WindowState::Fullscreen,
    );
}

// ========== Display impls ==========

#[test]
fn display_seamless_mode() {
    assert_eq!(format!("{}", SeamlessMode::Desktop), "Desktop");
    assert_eq!(format!("{}", SeamlessMode::Seamless), "Seamless");
}

#[test]
fn display_seamless_window_type() {
    assert_eq!(format!("{}", SeamlessWindowType::Normal), "Normal");
    assert_eq!(format!("{}", SeamlessWindowType::Dialog), "Dialog");
    assert_eq!(format!("{}", SeamlessWindowType::Popup), "Popup");
    assert_eq!(format!("{}", SeamlessWindowType::Tooltip), "Tooltip");
    assert_eq!(format!("{}", SeamlessWindowType::Overlay), "Overlay");
}

#[test]
fn display_seamless_manager() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.add_tray_icon(make_tray_icon("t1", "a1"));
    let s = format!("{mgr}");
    assert!(s.contains("SeamlessManager"));
    assert!(s.contains("windows=1"));
    assert!(s.contains("tray_icons=1"));
}

// ========== Config accessor ==========

#[test]
fn config_accessor_returns_reference() {
    let mgr = default_manager();
    assert!(!mgr.config().enabled);
    assert!(mgr.config().forward_notifications);
}

// ========== SeamlessWindow fields ==========

#[test]
fn seamless_window_with_parent_and_type() {
    let win = SeamlessWindow {
        window_id: WindowId(42),
        app_id: "dialog_app".into(),
        title: "Save As".into(),
        icon: Some(vec![1, 2, 3]),
        geometry: Rect::new(200.0, 150.0, 400.0, 300.0),
        state: WindowState::Normal,
        z_order: 5,
        parent_id: Some(WindowId(10)),
        window_type: SeamlessWindowType::Dialog,
    };
    assert_eq!(win.window_type, SeamlessWindowType::Dialog);
    assert_eq!(win.parent_id, Some(WindowId(10)));
    assert_eq!(win.z_order, 5);
    assert!(win.icon.is_some());
}

// ========== WindowIcon apply_message ==========

#[test]
fn apply_message_window_icon() {
    let mut mgr = default_manager();
    mgr.create_window(make_window(1, "a", "A"));
    mgr.apply_message(SeamlessMessage::WindowIcon {
        window_id: WindowId(1),
        icon_data: vec![0xAB, 0xCD],
    });
    assert_eq!(
        mgr.window(WindowId(1)).unwrap().icon,
        Some(vec![0xAB, 0xCD])
    );
}
