use crate::window::*;
use liquide_compositor::geometry::Rect;

#[test]
fn window_create() {
    let w = Window::new(WindowId(1), "Test", Rect::new(0.0, 0.0, 800.0, 600.0));
    assert_eq!(w.id, WindowId(1));
    assert_eq!(w.title, "Test");
    assert_eq!(w.bounds.width, 800.0);
}

#[test]
fn window_state_default_normal() {
    let w = Window::new(WindowId(1), "Test", Rect::ZERO);
    assert_eq!(w.state, WindowState::Normal);
}

#[test]
fn window_flags_default() {
    let flags = WindowFlags::default();
    assert!(flags.contains(WindowFlags::DECORATED));
    assert!(flags.contains(WindowFlags::RESIZABLE));
    assert!(flags.contains(WindowFlags::FOCUSABLE));
    assert!(!flags.contains(WindowFlags::ALWAYS_ON_TOP));
    assert!(!flags.contains(WindowFlags::SKIP_TASKBAR));
}

#[test]
fn window_is_decorated() {
    let w = Window::new(WindowId(1), "Test", Rect::ZERO);
    assert!(w.is_decorated());
}

#[test]
fn window_is_resizable() {
    let w = Window::new(WindowId(1), "Test", Rect::ZERO);
    assert!(w.is_resizable());
}

#[test]
fn window_is_focusable() {
    let w = Window::new(WindowId(1), "Test", Rect::ZERO);
    assert!(w.is_focusable());
}

#[test]
fn window_id_equality() {
    assert_eq!(WindowId(42), WindowId(42));
    assert_ne!(WindowId(1), WindowId(2));
}

#[test]
fn window_opacity_default() {
    let w = Window::new(WindowId(1), "Test", Rect::ZERO);
    assert_eq!(w.opacity, 1.0);
}

#[test]
fn window_effective_bounds_normal() {
    let bounds = Rect::new(10.0, 20.0, 300.0, 200.0);
    let w = Window::new(WindowId(1), "Test", bounds);
    assert_eq!(w.effective_bounds(), bounds);
}

#[test]
fn window_parent_none() {
    let w = Window::new(WindowId(1), "Test", Rect::ZERO);
    assert_eq!(w.parent, None);
}
