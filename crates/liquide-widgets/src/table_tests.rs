//! `<lq-table>` real-pipeline gallery tests (no fake-green).
//!
//! Teeth: clicking a body row's LAID-OUT box selects THAT display row (per-row
//! hit from layout, not `index*row_height`); :checked restyles the selected row's
//! pixels; a sortable-header click reorders the rows (the cell content actually
//! moves in the DOM); keyboard nav + Shift-range; column widths come from CSS
//! grid.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::table::{SortDir, Table, CHANGED_ACTION, SORTED_ACTION};

const W: u32 = 460;
const H: u32 = 320;

fn as_table<'a>(g: &'a Gallery, id: &str) -> &'a Table {
    g.host.behavior(id).unwrap().as_any().downcast_ref::<Table>().unwrap()
}

/// A 3-column, 4-row table; the first column is numeric so the sort is observable.
fn sample() -> Table {
    Table::new()
        .column("Rank")
        .column("Name")
        .column("City")
        .row(["3".into(), "Cara".into(), "Oslo".into()])
        .row(["1".into(), "Alice".into(), "Paris".into()])
        .row(["4".into(), "Dan".into(), "Rome".into()])
        .row(["2".into(), "Bob".into(), "Lyon".into()])
}

fn row_box(g: &Gallery, id: &str, pos: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("row-{pos}")).expect("row box")
}
fn head_box(g: &Gallery, id: &str, c: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("head-{c}")).expect("head box")
}

fn click_row(g: &mut Gallery, id: &str, pos: usize) -> Vec<crate::host::WidgetAction> {
    let r = row_box(g, id, pos);
    g.left_click(r.x + r.width / 2.0, r.y + r.height / 2.0);
    g.process()
}

/// Header + rows render with the right counts.
#[test]
fn table_renders_header_and_rows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(sample()));
    g.relayout();
    assert_eq!(as_table(&g, "t").row_count(), 4);
    assert_eq!(as_table(&g, "t").column_count(), 3);
    // The first display row is insertion order (no sort yet): "3, Cara, Oslo".
    assert_eq!(as_table(&g, "t").cell(0, 1), Some("Cara"));
}

/// Clicking a body row's LAID-OUT box selects that display position.
#[test]
fn click_selects_row() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(sample()));
    g.relayout();
    let actions = click_row(&mut g, "t", 2);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, CHANGED_ACTION);
    assert_eq!(as_table(&g, "t").selected_positions(), vec![2]);
}

/// NO-FAKE-GREEN tooth: tall rows → a click in row 2's REAL box selects 2, where
/// a constant row-height assumption would mis-map.
#[test]
fn row_hit_comes_from_layout_not_constant() {
    let css = "lq-gallery { padding: 8px; } lq-table > lq-tbody > lq-tr > lq-td { padding-top: 16px; padding-bottom: 16px; }";
    let mut g = Gallery::new(W, H, css);
    g.mount("t", Box::new(sample()));
    g.relayout();

    let r0 = row_box(&g, "t", 0);
    let r2 = row_box(&g, "t", 2);
    assert!(r2.height > 36.0, "rows tall (got {})", r2.height);
    assert!(r2.y > r0.y + 2.0 * 36.0, "row 2 below a constant-height guess");

    g.left_click(r2.x + r2.width / 2.0, r2.y + r2.height / 2.0);
    let _ = g.process();
    assert_eq!(as_table(&g, "t").selected_positions(), vec![2]);
}

/// :checked restyles the selected row's rasterized pixels.
#[test]
fn selected_row_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(sample()));
    g.relayout();
    let r1 = row_box(&g, "t", 1);
    let (cx, cy) = ((r1.x + 12.0) as u32, (r1.y + r1.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);
    g.left_click(r1.x + r1.width / 2.0, r1.y + r1.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(before != after, "selected row restyles (before {before:?} after {after:?})");
}

/// Clicking a sortable header reorders the rows (ascending by the numeric column),
/// and the DOM cell content actually moves. A second click flips to descending.
#[test]
fn header_click_sorts_rows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(sample().sortable(true)));
    g.relayout();

    // Before sort, display row 0 is the insertion-order first ("3").
    assert_eq!(as_table(&g, "t").cell(0, 0), Some("3"));

    let h0 = head_box(&g, "t", 0);
    g.left_click(h0.x + h0.width / 2.0, h0.y + h0.height / 2.0);
    let actions = g.process();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, SORTED_ACTION);
    assert_eq!(actions[0].payload.as_deref(), Some("0:asc"));
    assert_eq!(as_table(&g, "t").sort(), Some((0, SortDir::Asc)));
    // Ascending by rank → "1, Alice, Paris" is now display row 0.
    assert_eq!(as_table(&g, "t").cell(0, 0), Some("1"));
    assert_eq!(as_table(&g, "t").cell(0, 1), Some("Alice"));
    assert_eq!(as_table(&g, "t").cell(3, 0), Some("4"));

    // Second click on the same header → descending. A bare repeat click on the
    // SAME node would coalesce into a DoubleClick (dispatcher debounce), so break
    // the same-node chain with an intervening click on empty gallery space.
    g.left_click(2.0, 2.0);
    let _ = g.process();
    let h0 = head_box(&g, "t", 0);
    g.left_click(h0.x + h0.width / 2.0, h0.y + h0.height / 2.0);
    let _ = g.process();
    assert_eq!(as_table(&g, "t").sort(), Some((0, SortDir::Desc)));
    assert_eq!(as_table(&g, "t").cell(0, 0), Some("4"));
}

/// A non-sortable table ignores header clicks (no reorder).
#[test]
fn non_sortable_header_ignored() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(sample())); // not sortable
    g.relayout();
    let h0 = head_box(&g, "t", 0);
    g.left_click(h0.x + h0.width / 2.0, h0.y + h0.height / 2.0);
    let _ = g.process();
    assert_eq!(as_table(&g, "t").sort(), None);
    assert_eq!(as_table(&g, "t").cell(0, 0), Some("3"), "order unchanged");
}

/// Keyboard nav + Shift-range over the rows.
#[test]
fn keyboard_navigates_and_ranges() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(sample()));
    g.relayout();
    g.host.set_focus(Some("t"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_table(&g, "t").selected_positions(), vec![1]);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_table(&g, "t").selected_positions(), vec![3]);

    // Shift+Up from the anchor at 3 grows the range upward.
    g.key(KeyInput::new(keys::HOME, 0)); // anchor 0
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::SHIFT));
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::SHIFT));
    assert_eq!(as_table(&g, "t").selected_positions(), vec![0, 1, 2]);
}

/// Per-column grid widths come from CSS (a fixed-px column is exactly that wide).
#[test]
fn column_widths_from_css_grid() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    let t = Table::new()
        .column_px("A", 80.0)
        .column("B")
        .row(["x".into(), "y".into()]);
    g.mount("t", Box::new(t));
    g.relayout();
    // The first header cell should be ~80px wide (fixed grid track).
    let h0 = head_box(&g, "t", 0);
    assert!(
        (h0.width - 80.0).abs() < 4.0,
        "fixed column width from CSS grid (got {})",
        h0.width
    );
}

/// Disabled table swallows clicks + keys.
#[test]
fn disabled_table_swallows_input() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(sample().sortable(true).disabled(true)));
    g.relayout();
    let r1 = row_box(&g, "t", 1);
    g.left_click(r1.x + r1.width / 2.0, r1.y + r1.height / 2.0);
    assert!(g.process().is_empty());
    g.host.set_focus(Some("t"), &mut g.doc, &mut g.dispatcher);
    assert!(g.key(KeyInput::new(keys::ARROW_DOWN, 0)).is_empty());
    assert!(as_table(&g, "t").selected_positions().is_empty());
}
