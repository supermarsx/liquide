//! Tests for layout types.

use crate::geometry::Rect;
use crate::layout::{
    BoxLayout, GridLayout, LayoutAlign, LayoutConstraints, LayoutDirection, Margin, Padding,
    StackLayout,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_constraints(pw: f32, ph: f32) -> LayoutConstraints {
    LayoutConstraints::with_preferred(pw, ph)
}

fn available_100x100() -> Rect {
    Rect::new(0.0, 0.0, 100.0, 100.0)
}

// ---------------------------------------------------------------------------
// BoxLayout - Horizontal
// ---------------------------------------------------------------------------

#[test]
fn test_box_horizontal_basic() {
    let layout = BoxLayout::new(LayoutDirection::Horizontal);
    let constraints = vec![make_constraints(30.0, 20.0), make_constraints(40.0, 20.0)];
    let rects = layout.layout(&constraints, available_100x100());

    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].x, 0.0);
    assert_eq!(rects[0].width, 30.0);
    assert_eq!(rects[1].x, 30.0);
    assert_eq!(rects[1].width, 40.0);
}

#[test]
fn test_box_horizontal_with_gap() {
    let mut layout = BoxLayout::new(LayoutDirection::Horizontal);
    layout.gap = 10.0;
    let constraints = vec![make_constraints(20.0, 20.0), make_constraints(20.0, 20.0)];
    let rects = layout.layout(&constraints, available_100x100());

    assert_eq!(rects[0].x, 0.0);
    assert_eq!(rects[0].width, 20.0);
    assert_eq!(rects[1].x, 30.0); // 20 + 10 gap
    assert_eq!(rects[1].width, 20.0);
}

#[test]
fn test_box_horizontal_with_padding() {
    let mut layout = BoxLayout::new(LayoutDirection::Horizontal);
    layout.padding = Padding::all(10.0);
    let constraints = vec![make_constraints(20.0, 20.0)];
    let rects = layout.layout(&constraints, available_100x100());

    assert_eq!(rects[0].x, 10.0); // padding left
    assert_eq!(rects[0].y, 10.0); // padding top
    assert_eq!(rects[0].width, 20.0);
}

#[test]
fn test_box_horizontal_empty_children() {
    let layout = BoxLayout::new(LayoutDirection::Horizontal);
    let rects = layout.layout(&[], available_100x100());
    assert!(rects.is_empty());
}

// ---------------------------------------------------------------------------
// BoxLayout - Vertical
// ---------------------------------------------------------------------------

#[test]
fn test_box_vertical_basic() {
    let layout = BoxLayout::new(LayoutDirection::Vertical);
    let constraints = vec![make_constraints(50.0, 30.0), make_constraints(50.0, 20.0)];
    let rects = layout.layout(&constraints, available_100x100());

    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].y, 0.0);
    assert_eq!(rects[0].height, 30.0);
    assert_eq!(rects[1].y, 30.0);
    assert_eq!(rects[1].height, 20.0);
}

#[test]
fn test_box_vertical_with_gap() {
    let mut layout = BoxLayout::new(LayoutDirection::Vertical);
    layout.gap = 5.0;
    let constraints = vec![
        make_constraints(50.0, 20.0),
        make_constraints(50.0, 20.0),
        make_constraints(50.0, 20.0),
    ];
    let rects = layout.layout(&constraints, available_100x100());

    assert_eq!(rects[0].y, 0.0);
    assert_eq!(rects[1].y, 25.0); // 20 + 5
    assert_eq!(rects[2].y, 50.0); // 25 + 20 + 5
}

#[test]
fn test_box_vertical_stretch() {
    let mut layout = BoxLayout::new(LayoutDirection::Vertical);
    layout.align = LayoutAlign::Stretch;
    let constraints = vec![make_constraints(30.0, 20.0)];
    let rects = layout.layout(&constraints, available_100x100());

    assert_eq!(rects[0].width, 100.0); // stretched to full width
}

// ---------------------------------------------------------------------------
// StackLayout
// ---------------------------------------------------------------------------

#[test]
fn test_stack_layout_all_same_rect() {
    let stack = StackLayout::new();
    let rects = stack.layout(3, available_100x100());

    assert_eq!(rects.len(), 3);
    for r in &rects {
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 0.0);
        assert_eq!(r.width, 100.0);
        assert_eq!(r.height, 100.0);
    }
}

#[test]
fn test_stack_layout_with_padding() {
    let mut stack = StackLayout::new();
    stack.padding = Padding::all(10.0);
    let rects = stack.layout(2, available_100x100());

    assert_eq!(rects.len(), 2);
    for r in &rects {
        assert_eq!(r.x, 10.0);
        assert_eq!(r.y, 10.0);
        assert_eq!(r.width, 80.0);
        assert_eq!(r.height, 80.0);
    }
}

#[test]
fn test_stack_layout_zero_children() {
    let stack = StackLayout::new();
    let rects = stack.layout(0, available_100x100());
    assert!(rects.is_empty());
}

// ---------------------------------------------------------------------------
// GridLayout
// ---------------------------------------------------------------------------

#[test]
fn test_grid_layout_2x2() {
    let grid = GridLayout::new(2, 2);
    let rects = grid.layout(4, available_100x100());

    assert_eq!(rects.len(), 4);
    // Each cell is 50x50
    assert_eq!(rects[0], Rect::new(0.0, 0.0, 50.0, 50.0));
    assert_eq!(rects[1], Rect::new(50.0, 0.0, 50.0, 50.0));
    assert_eq!(rects[2], Rect::new(0.0, 50.0, 50.0, 50.0));
    assert_eq!(rects[3], Rect::new(50.0, 50.0, 50.0, 50.0));
}

#[test]
fn test_grid_layout_with_gap() {
    let mut grid = GridLayout::new(2, 2);
    grid.gap = 10.0;
    let rects = grid.layout(4, available_100x100());

    // cell_width = (100 - 10) / 2 = 45.0
    // cell_height = (100 - 10) / 2 = 45.0
    assert_eq!(rects[0], Rect::new(0.0, 0.0, 45.0, 45.0));
    assert_eq!(rects[1], Rect::new(55.0, 0.0, 45.0, 45.0));
    assert_eq!(rects[2], Rect::new(0.0, 55.0, 45.0, 45.0));
    assert_eq!(rects[3], Rect::new(55.0, 55.0, 45.0, 45.0));
}

#[test]
fn test_grid_layout_fewer_children_than_cells() {
    let grid = GridLayout::new(3, 3);
    let rects = grid.layout(2, Rect::new(0.0, 0.0, 300.0, 300.0));

    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].width, 100.0);
    assert_eq!(rects[0].height, 100.0);
}

#[test]
fn test_grid_layout_more_children_than_cells() {
    let grid = GridLayout::new(2, 1);
    let rects = grid.layout(5, available_100x100());

    // Only 2 cells available (2 cols x 1 row), rest are clipped.
    assert_eq!(rects.len(), 2);
}

#[test]
fn test_grid_layout_zero_children() {
    let grid = GridLayout::new(2, 2);
    let rects = grid.layout(0, available_100x100());
    assert!(rects.is_empty());
}

// ---------------------------------------------------------------------------
// Padding and Margin helpers
// ---------------------------------------------------------------------------

#[test]
fn test_padding_all() {
    let p = Padding::all(10.0);
    assert_eq!(p.horizontal(), 20.0);
    assert_eq!(p.vertical(), 20.0);
}

#[test]
fn test_padding_symmetric() {
    let p = Padding::symmetric(5.0, 10.0);
    assert_eq!(p.top, 5.0);
    assert_eq!(p.bottom, 5.0);
    assert_eq!(p.left, 10.0);
    assert_eq!(p.right, 10.0);
}

#[test]
fn test_margin_all() {
    let m = Margin::all(8.0);
    assert_eq!(m.horizontal(), 16.0);
    assert_eq!(m.vertical(), 16.0);
}

#[test]
fn test_margin_symmetric() {
    let m = Margin::symmetric(4.0, 6.0);
    assert_eq!(m.top, 4.0);
    assert_eq!(m.bottom, 4.0);
    assert_eq!(m.left, 6.0);
    assert_eq!(m.right, 6.0);
}

#[test]
fn test_layout_constraints_with_preferred() {
    let c = LayoutConstraints::with_preferred(50.0, 30.0);
    assert_eq!(c.preferred_width, 50.0);
    assert_eq!(c.preferred_height, 30.0);
    assert_eq!(c.min_width, 0.0);
    assert_eq!(c.min_height, 0.0);
}

#[test]
fn test_box_layout_default() {
    let layout = BoxLayout::default();
    assert_eq!(layout.direction, LayoutDirection::Vertical);
    assert_eq!(layout.align, LayoutAlign::Start);
    assert_eq!(layout.gap, 0.0);
    assert!(!layout.wrap);
}
