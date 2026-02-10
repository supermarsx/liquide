use liquide_compositor::geometry::Rect;
use crate::window::*;
use crate::layout::TilingLayout;
use crate::shell::Shell;

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
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.maximize(id).unwrap();
    assert_eq!(shell.window(id).unwrap().state, WindowState::Maximized);
    assert_eq!(shell.window(id).unwrap().bounds.width, 1920.0);

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
