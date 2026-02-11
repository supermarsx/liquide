//! Tests for the character grid.

use crate::grid::{CellAttrs, Grid};

#[test]
fn test_grid_new() {
    let g = Grid::new(24, 80);
    assert_eq!(g.rows(), 24);
    assert_eq!(g.cols(), 80);
    assert_eq!(g.cursor(), (0, 0));
}

#[test]
fn test_put_char() {
    let mut g = Grid::new(24, 80);
    g.put_char('A');
    assert_eq!(g.cursor(), (0, 1));
    assert_eq!(g.cell(0, 0).unwrap().ch, 'A');
}

#[test]
fn test_put_char_wraps() {
    let mut g = Grid::new(3, 3);
    g.put_char('a');
    g.put_char('b');
    g.put_char('c'); // wraps to next line
    assert_eq!(g.cursor(), (1, 0));
}

#[test]
fn test_cursor_movement() {
    let mut g = Grid::new(24, 80);
    g.set_cursor(5, 10);
    assert_eq!(g.cursor(), (5, 10));
    g.cursor_up(2);
    assert_eq!(g.cursor(), (3, 10));
    g.cursor_down(10);
    assert_eq!(g.cursor(), (13, 10));
    g.cursor_forward(5);
    assert_eq!(g.cursor(), (13, 15));
    g.cursor_back(3);
    assert_eq!(g.cursor(), (13, 12));
}

#[test]
fn test_cursor_clamp() {
    let mut g = Grid::new(24, 80);
    g.cursor_up(100);
    assert_eq!(g.cursor(), (0, 0));
    g.cursor_back(100);
    assert_eq!(g.cursor(), (0, 0));
    g.set_cursor(100, 100);
    assert_eq!(g.cursor(), (23, 79));
}

#[test]
fn test_carriage_return() {
    let mut g = Grid::new(24, 80);
    g.set_cursor(5, 40);
    g.carriage_return();
    assert_eq!(g.cursor(), (5, 0));
}

#[test]
fn test_line_feed() {
    let mut g = Grid::new(5, 5);
    g.set_cursor(2, 0);
    g.line_feed();
    assert_eq!(g.cursor(), (3, 0));
}

#[test]
fn test_erase_line() {
    let mut g = Grid::new(5, 10);
    for ch in "ABCDEFGHIJ".chars() {
        g.put_char(ch);
    }
    g.set_cursor(0, 5);
    g.erase_line_to_end();
    assert_eq!(g.row_text(0), "ABCDE");
}

#[test]
fn test_erase_line_all() {
    let mut g = Grid::new(5, 10);
    for ch in "hello".chars() {
        g.put_char(ch);
    }
    g.set_cursor(0, 0);
    g.erase_line_all();
    assert_eq!(g.row_text(0), "");
}

#[test]
fn test_erase_display_all() {
    let mut g = Grid::new(5, 5);
    g.put_char('X');
    g.erase_display_all();
    assert_eq!(g.row_text(0), "");
    assert_eq!(g.cell(0, 0).unwrap().ch, ' ');
}

#[test]
fn test_scroll_up() {
    let mut g = Grid::new(3, 5);
    g.set_cursor(0, 0);
    g.put_char('A');
    g.set_cursor(1, 0);
    g.put_char('B');
    g.set_cursor(2, 0);
    g.put_char('C');
    let scrolled = g.scroll_up(1);
    assert_eq!(scrolled.len(), 1);
    assert_eq!(scrolled[0][0].ch, 'A');
    assert_eq!(g.row_text(0), "B");
    assert_eq!(g.row_text(1), "C");
}

#[test]
fn test_scroll_down() {
    let mut g = Grid::new(3, 5);
    g.set_cursor(0, 0);
    g.put_char('A');
    g.set_cursor(1, 0);
    g.put_char('B');
    g.scroll_down(1);
    assert_eq!(g.row_text(0), "");
    assert_eq!(g.row_text(1), "A");
}

#[test]
fn test_resize() {
    let mut g = Grid::new(5, 5);
    g.put_char('X');
    g.resize(10, 10);
    assert_eq!(g.rows(), 10);
    assert_eq!(g.cols(), 10);
    assert_eq!(g.cell(0, 0).unwrap().ch, 'X');
}

#[test]
fn test_resize_smaller() {
    let mut g = Grid::new(10, 10);
    g.set_cursor(9, 9);
    g.resize(5, 5);
    assert_eq!(g.rows(), 5);
    assert_eq!(g.cursor(), (4, 4));
}

#[test]
fn test_row_text() {
    let mut g = Grid::new(5, 10);
    g.set_cursor(0, 0);
    for ch in "hello".chars() {
        g.put_char(ch);
    }
    assert_eq!(g.row_text(0), "hello");
}

#[test]
fn test_attrs() {
    let mut g = Grid::new(5, 5);
    let attrs = CellAttrs { bold: true, ..CellAttrs::default() };
    g.set_attrs(attrs);
    g.put_char('X');
    assert!(g.cell(0, 0).unwrap().attrs.bold);
    g.reset_attrs();
    g.put_char('Y');
    assert!(!g.cell(0, 1).unwrap().attrs.bold);
}

#[test]
fn test_set_scroll_region() {
    let mut g = Grid::new(10, 10);
    g.set_scroll_region(3, 8);
    // Setting region should not change cursor.
    assert_eq!(g.cursor(), (0, 0));
}
