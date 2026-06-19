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

/// True if ANY pixel in the laid-out box of `part` differs between two FBs.
fn part_region_differs(
    g: &Gallery,
    id: &str,
    part: &str,
    a: &liquide_compositor::framebuffer::FrameBuffer,
    b: &liquide_compositor::framebuffer::FrameBuffer,
) -> bool {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let r = q.box_of_part(root, part).expect("part box");
    let x0 = r.x.max(0.0) as u32;
    let y0 = r.y.max(0.0) as u32;
    let x1 = ((r.x + r.width).min(W as f32)) as u32;
    let y1 = ((r.y + r.height).min(H as f32)) as u32;
    for y in y0..y1 {
        for x in x0..x1 {
            if Gallery::pixel(a, x, y) != Gallery::pixel(b, x, y) {
                return true;
            }
        }
    }
    false
}

/// Normal render: the header cells and every body row paint opaque pixels.
#[test]
fn normal_render_paints_header_and_rows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(sample()));
    g.relayout();
    let fb = g.rasterize();
    let h0 = head_box(&g, "t", 0);
    let hpx = Gallery::pixel(&fb, (h0.x + h0.width / 2.0) as u32, (h0.y + h0.height / 2.0) as u32);
    assert!(hpx.a > 0, "header must paint (alpha {})", hpx.a);
    for pos in 0..4 {
        let r = row_box(&g, "t", pos);
        let px = Gallery::pixel(&fb, (r.x + 12.0) as u32, (r.y + r.height / 2.0) as u32);
        assert!(px.a > 0, "row {pos} must paint (alpha {})", px.a);
    }
}

/// :hover restyles ONLY the hovered body row (the hover fill #3f3f46), not its
/// neighbours.
#[test]
fn hover_restyles_only_hovered_row() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(sample()));
    g.relayout();
    let before = g.rasterize();

    let r1 = row_box(&g, "t", 1);
    g.pointer_move(r1.x + r1.width / 2.0, r1.y + r1.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = g.rasterize();

    assert!(
        part_region_differs(&g, "t", "row-1", &before, &after),
        "hovered row 1 must restyle"
    );
    assert!(
        !part_region_differs(&g, "t", "row-3", &before, &after),
        "non-hovered row 3 must be unchanged"
    );
}

/// :checked selection fill MOVES with the selection across rows.
#[test]
fn selection_fill_moves_across_rows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(sample()));
    g.relayout();
    let plain = g.rasterize();

    let _ = click_row(&mut g, "t", 2);
    assert_eq!(as_table(&g, "t").selected_positions(), vec![2]);
    g.relayout();
    let sel2 = g.rasterize();
    assert!(
        part_region_differs(&g, "t", "row-2", &plain, &sel2),
        "row 2 must gain the selection fill"
    );

    let _ = click_row(&mut g, "t", 0);
    assert_eq!(as_table(&g, "t").selected_positions(), vec![0]);
    g.relayout();
    let sel0 = g.rasterize();
    assert!(
        part_region_differs(&g, "t", "row-0", &sel2, &sel0),
        "row 0 must gain the fill"
    );
    assert!(
        part_region_differs(&g, "t", "row-2", &sel2, &sel0),
        "row 2 must lose the fill"
    );
}

/// Zebra striping: even-positioned body rows carry a different background fill
/// than odd ones (CSS `:nth-child(even)`), so an even row's pixels differ from an
/// odd row's at the same in-row offset on a plain (unselected) render.
#[test]
fn zebra_stripes_even_rows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(sample()));
    g.relayout();
    let fb = g.rasterize();

    // Sample the far-right gutter of each row (past the text) so the delta is the
    // row background, not glyph ink.
    let sample_px = |pos: usize| -> liquide_compositor::pixel::Color {
        let r = row_box(&g, "t", pos);
        Gallery::pixel(&fb, (r.x + r.width - 4.0) as u32, (r.y + r.height / 2.0) as u32)
    };
    // Row 1 is :nth-child(even) (1-based 2nd) → tinted; row 0 is transparent.
    assert!(
        sample_px(0) != sample_px(1),
        "even-row zebra tint must differ from the odd row background"
    );
}

/// The sort indicator marker (::after glyph) appears on the sorted header after a
/// sort and is absent before — a structural pixel delta in the header cell.
#[test]
fn sort_indicator_marker_appears_on_sort() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(sample().sortable(true)));
    g.relayout();
    let before = g.rasterize();
    assert_eq!(as_table(&g, "t").sort(), None, "no sort glyph yet");

    let h0 = head_box(&g, "t", 0);
    g.left_click(h0.x + h0.width / 2.0, h0.y + h0.height / 2.0);
    let _ = g.process();
    assert_eq!(as_table(&g, "t").sort(), Some((0, SortDir::Asc)));
    g.relayout();
    let after = g.rasterize();
    assert!(
        part_region_differs(&g, "t", "head-0", &before, &after),
        "the sorted header must gain a sort-indicator marker (::after glyph)"
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
