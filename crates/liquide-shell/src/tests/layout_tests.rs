use liquide_compositor::geometry::Rect;
use crate::window::*;
use crate::layout::*;

fn make_window(id: u64) -> Window {
    Window::new(WindowId(id), format!("Win{id}"), Rect::new(0.0, 0.0, 200.0, 150.0))
}

#[test]
fn floating_noop() {
    let layout = FloatingLayout;
    let mut wins = vec![make_window(1)];
    let orig_bounds = wins[0].bounds;
    layout.arrange(&mut wins, Rect::new(0.0, 0.0, 1920.0, 1080.0));
    assert_eq!(wins[0].bounds, orig_bounds);
}

#[test]
fn tiling_single_window() {
    let layout = TilingLayout::new(10.0, 4);
    let screen = Rect::new(0.0, 0.0, 1000.0, 800.0);
    let mut wins = vec![make_window(1)];
    layout.arrange(&mut wins, screen);
    assert!(wins[0].bounds.width > 900.0);
    assert!(wins[0].bounds.height > 700.0);
}

#[test]
fn tiling_two_windows() {
    let layout = TilingLayout::new(10.0, 4);
    let screen = Rect::new(0.0, 0.0, 1000.0, 800.0);
    let mut wins = vec![make_window(1), make_window(2)];
    layout.arrange(&mut wins, screen);
    assert!(wins[0].bounds.x < wins[1].bounds.x);
    assert!((wins[0].bounds.y - wins[1].bounds.y).abs() < 0.001);
}

#[test]
fn tiling_respects_gap() {
    let layout = TilingLayout::new(20.0, 4);
    let screen = Rect::new(0.0, 0.0, 1000.0, 800.0);
    let mut wins = vec![make_window(1), make_window(2)];
    layout.arrange(&mut wins, screen);
    let gap = wins[1].bounds.x - (wins[0].bounds.x + wins[0].bounds.width);
    assert!((gap - 20.0).abs() < 0.01, "gap should be ~20, got {gap}");
}

#[test]
fn tiling_max_columns() {
    let layout = TilingLayout::new(0.0, 2);
    let screen = Rect::new(0.0, 0.0, 1000.0, 800.0);
    let mut wins = vec![make_window(1), make_window(2), make_window(3)];
    layout.arrange(&mut wins, screen);
    assert!((wins[0].bounds.y - wins[1].bounds.y).abs() < 0.001);
    assert!(wins[2].bounds.y > wins[0].bounds.y);
}

#[test]
fn stacked_cascade() {
    let layout = StackedLayout::new();
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let mut wins = vec![make_window(1), make_window(2), make_window(3)];
    layout.arrange(&mut wins, screen);
    assert!(wins[1].bounds.x > wins[0].bounds.x);
    assert!(wins[1].bounds.y > wins[0].bounds.y);
    assert!(wins[2].bounds.x > wins[1].bounds.x);
}

#[test]
fn layout_name() {
    let tiling = TilingLayout::new(10.0, 4);
    assert_eq!(tiling.name(), "tiling");
}

#[test]
fn tiling_three_windows_wraps() {
    let layout = TilingLayout::new(0.0, 2);
    let screen = Rect::new(0.0, 0.0, 1000.0, 1000.0);
    let mut wins = vec![make_window(1), make_window(2), make_window(3), make_window(4)];
    layout.arrange(&mut wins, screen);
    assert!((wins[0].bounds.y - wins[1].bounds.y).abs() < 0.001);
    assert!((wins[2].bounds.y - wins[3].bounds.y).abs() < 0.001);
    assert!(wins[2].bounds.y > wins[0].bounds.y);
}

#[test]
fn floating_name() {
    assert_eq!(FloatingLayout.name(), "floating");
}

#[test]
fn stacked_name() {
    assert_eq!(StackedLayout::new().name(), "stacked");
}
