use crate::layout::TilingLayout;
use crate::notification::TrayMenuItem;
use crate::seamless::{TrayIconInfo, TrayMenuEntry};
use crate::shell::Shell;
use crate::window::*;
use liquide_compositor::geometry::Rect;

#[test]
fn shell_create() {
    let shell = Shell::new(1920.0, 1080.0);
    assert_eq!(shell.window_count(), 0);
    assert_eq!(shell.screen_rect(), Rect::new(0.0, 0.0, 1920.0, 1080.0));
}

#[test]
fn shell_open_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    assert_eq!(shell.window_count(), 1);
    let w = shell.window(id).unwrap();
    assert_eq!(w.title, "Test");
}

#[test]
fn shell_close_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::ZERO);
    let closed = shell.close_window(id).unwrap();
    assert_eq!(closed.id, id);
    assert_eq!(shell.window_count(), 0);
}

#[test]
fn shell_close_not_found() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(shell.close_window(WindowId(999)).is_err());
}

#[test]
fn shell_move_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(0.0, 0.0, 100.0, 100.0));
    shell.move_window(id, 50.0, 75.0).unwrap();
    let w = shell.window(id).unwrap();
    assert_eq!(w.bounds.x, 50.0);
    assert_eq!(w.bounds.y, 75.0);
}

#[test]
fn shell_resize_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(0.0, 0.0, 100.0, 100.0));
    shell.resize_window(id, 500.0, 400.0).unwrap();
    let w = shell.window(id).unwrap();
    assert_eq!(w.bounds.width, 500.0);
    assert_eq!(w.bounds.height, 400.0);
}

#[test]
fn shell_minimize_restore() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.minimize(id).unwrap();
    assert_eq!(shell.window(id).unwrap().state, WindowState::Minimized);
    assert!(!shell.window(id).unwrap().visible);

    shell.restore(id).unwrap();
    assert_eq!(shell.window(id).unwrap().state, WindowState::Normal);
    assert!(shell.window(id).unwrap().visible);
    assert_eq!(shell.window(id).unwrap().bounds.width, 400.0);
}

#[test]
fn shell_maximize_restore() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let work = shell.work_area();
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.maximize(id).unwrap();
    assert_eq!(shell.window(id).unwrap().state, WindowState::Maximized);
    assert_eq!(shell.window(id).unwrap().bounds.width, work.width);
    assert_eq!(shell.window(id).unwrap().bounds.height, work.height);

    shell.restore(id).unwrap();
    assert_eq!(shell.window(id).unwrap().state, WindowState::Normal);
    assert_eq!(shell.window(id).unwrap().bounds.width, 400.0);
}

#[test]
fn shell_toggle_fullscreen() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));

    shell.toggle_fullscreen(id).unwrap();
    assert_eq!(shell.window(id).unwrap().state, WindowState::Fullscreen);
    assert_eq!(shell.window(id).unwrap().bounds.width, 1920.0);

    shell.toggle_fullscreen(id).unwrap();
    assert_eq!(shell.window(id).unwrap().state, WindowState::Normal);
    assert_eq!(shell.window(id).unwrap().bounds.width, 400.0);
}

#[test]
fn shell_focus() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::ZERO);
    shell.set_focus(id).unwrap();
    assert_eq!(shell.focus_manager().focused(), Some(id));
}

#[test]
fn shell_visible_windows_sorted() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::ZERO);
    let id2 = shell.open_window("B", Rect::ZERO);
    let id3 = shell.open_window("C", Rect::ZERO);
    shell.window_mut(id1).unwrap().z_order = 10;
    shell.window_mut(id2).unwrap().z_order = 5;
    shell.window_mut(id3).unwrap().z_order = 20;

    let visible = shell.visible_windows();
    assert_eq!(visible.len(), 3);
    assert_eq!(visible[0].id, id2); // z=5
    assert_eq!(visible[1].id, id1); // z=10
    assert_eq!(visible[2].id, id3); // z=20
}

#[test]
fn shell_window_count() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert_eq!(shell.window_count(), 0);
    shell.open_window("A", Rect::ZERO);
    shell.open_window("B", Rect::ZERO);
    assert_eq!(shell.window_count(), 2);
}

#[test]
fn shell_arrange_floating() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.arrange_windows();
    assert_eq!(shell.window(id).unwrap().bounds.x, 100.0);
}

#[test]
fn shell_arrange_tiling() {
    let mut shell = Shell::new(1000.0, 800.0);
    shell.set_layout(Box::new(TilingLayout::new(10.0, 4)));
    let id1 = shell.open_window("A", Rect::new(0.0, 0.0, 100.0, 100.0));
    let id2 = shell.open_window("B", Rect::new(0.0, 0.0, 100.0, 100.0));
    shell.arrange_windows();

    let w1 = shell.window(id1).unwrap();
    let w2 = shell.window(id2).unwrap();
    assert!(w1.bounds.width > 100.0);
    assert!(w2.bounds.width > 100.0);
}

#[test]
fn shell_resize_screen() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.resize_screen(2560.0, 1440.0);
    assert_eq!(shell.screen_rect(), Rect::new(0.0, 0.0, 2560.0, 1440.0));
}

fn child_by_attr(shell: &Shell, parent: liquide_dom::NodeId, attr: &str, value: &str) -> liquide_dom::NodeId {
    shell
        .desktop_dom
        .doc
        .children(parent)
        .iter()
        .copied()
    .find(|&child| shell.desktop_dom.doc.get_attribute(child, attr).as_deref() == Some(value))
        .expect("child with matching attribute")
}

#[test]
fn shell_sync_dom_formats_clock_from_status_bar_model() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.status_bar.set_clock_offset_minutes(60);
    shell.status_bar.set_clock_show_seconds(true);
    shell.status_bar
        .update_clock((13_u64 * 3600 + 5 * 60 + 9) * 1_000_000);

    shell.sync_dom();

    let center_slot = shell
        .desktop_dom
        .doc
        .get_element_by_id("statusbar-slot-center")
        .expect("center slot");
    let item = shell.desktop_dom.doc.children(center_slot)[0];
    let text = shell.desktop_dom.doc.children(item)[0];
    assert_eq!(
        shell.desktop_dom.doc.get(text).unwrap().text_content(),
        Some("14:05:09")
    );
}

#[test]
fn shell_sync_dom_hides_branding_when_app_menu_is_disabled() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.config.status_bar.show_app_menu = false;
    shell.status_bar = liquide_statusbar::ShellStatusBar::new(shell.config.status_bar.clone());

    shell.sync_dom();

    assert!(shell.desktop_dom.doc.get_element_by_id("logo").is_none());
}

#[test]
fn shell_sync_dom_renders_dock_focus_attention_and_badges() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.dock.set_badge("com.liquide.files", 5);
    shell.dock
        .set_needs_attention("com.liquide.terminal", true);

    let focused = shell.open_window("Browser", Rect::new(120.0, 160.0, 480.0, 320.0));
    shell.window_mut(focused).unwrap().app_id = "com.liquide.browser".into();
    shell.set_focus(focused).unwrap();

    shell.sync_dom();

    let dock = shell
        .desktop_dom
        .doc
        .get_element_by_id("shell-dock")
        .expect("dock node");
    let files = child_by_attr(&shell, dock, "data-app-id", "com.liquide.files");
    let browser = child_by_attr(&shell, dock, "data-app-id", "com.liquide.browser");
    let terminal = child_by_attr(&shell, dock, "data-app-id", "com.liquide.terminal");

    assert_eq!(
        shell.desktop_dom.doc.get_attribute(files, "data-badge").as_deref(),
        Some("5")
    );
    assert!(shell.desktop_dom.doc.get(browser).unwrap().has_class("focused"));
    assert!(
        shell
            .desktop_dom
            .doc
            .get(terminal)
            .unwrap()
            .has_class("needs-attention")
    );

    let badge = shell
        .desktop_dom
        .doc
        .children(files)
        .iter()
        .copied()
        .find(|&child| shell.desktop_dom.doc.get(child).unwrap().tag_name() == "dock-badge")
        .expect("dock badge");
    let badge_text = shell.desktop_dom.doc.children(badge)[0];
    assert_eq!(
        shell.desktop_dom.doc.get(badge_text).unwrap().text_content(),
        Some("5")
    );
}

#[test]
fn shell_sync_dom_renders_live_tray_items() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let notification_id = shell
        .notifications
        .add_tray_icon("Mail", "Unread mail", "mail-icon", 0);
    shell.notifications.update_tray_icon(
        notification_id,
        None,
        None,
        Some(Some("3")),
        1,
    );
    shell.notifications.set_tray_menu(
        notification_id,
        vec![TrayMenuItem::new("open", "Open Inbox")],
    );
    shell.seamless.add_tray_icon(TrayIconInfo {
        item_id: "remote-app".into(),
        app_id: "remote.app".into(),
        icon_data: vec![0x89, 0x50, 0x4E, 0x47],
        tooltip: "Remote tray icon".into(),
        menu_items: vec![TrayMenuEntry {
            id: "show".into(),
            label: "Show".into(),
            enabled: true,
            separator: false,
        }],
    });

    shell.sync_dom();

    let tray = shell
        .desktop_dom
        .doc
        .get_element_by_id("tray")
        .expect("tray node");
    let children = shell.desktop_dom.doc.children(tray);
    assert_eq!(children.len(), 2);

    let notification_item = child_by_attr(&shell, tray, "data-source", "notification");
    let seamless_item = child_by_attr(&shell, tray, "data-source", "seamless");

    assert_eq!(
        shell.desktop_dom.doc.get_attribute(notification_item, "data-label").as_deref(),
        Some("Mail")
    );
    assert_eq!(
        shell
            .desktop_dom
            .doc
            .get_attribute(notification_item, "data-has-menu")
            .as_deref(),
        Some("true")
    );
    let notification_badge = shell
        .desktop_dom
        .doc
        .children(notification_item)
        .iter()
        .copied()
        .find(|&child| shell.desktop_dom.doc.get(child).unwrap().tag_name() == "status-tray-badge")
        .expect("notification badge");
    let badge_text = shell.desktop_dom.doc.children(notification_badge)[0];
    assert_eq!(
        shell.desktop_dom.doc.get(badge_text).unwrap().text_content(),
        Some("3")
    );
    assert_eq!(
        shell.desktop_dom.doc.get_attribute(seamless_item, "data-label").as_deref(),
        Some("remote.app")
    );
    assert_eq!(
        shell
            .desktop_dom
            .doc
            .get_attribute(seamless_item, "data-has-icon-data")
            .as_deref(),
        Some("true")
    );
}
