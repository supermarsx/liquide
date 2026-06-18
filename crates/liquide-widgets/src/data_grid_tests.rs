//! `<lq-data-grid>` real-pipeline gallery tests.
//!
//! Each test drives the REAL pipeline (style->layout->paint) + real event
//! dispatcher via the S0 [`Gallery`]. The virtualization, cell hit-test, column
//! sort/resize, and scroll windowing all resolve from the LAID-OUT boxes — a
//! constant-based grid cannot pass (the anti-constant tooth).
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::data_grid::{
    clamp_range, DataGrid, RESIZED_ACTION, SCROLLED_ACTION, SELECTED_ACTION, SORTED_ACTION,
};
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 560;
const H: u32 = 420;

fn as_grid<'a>(g: &'a Gallery, id: &str) -> &'a DataGrid {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<DataGrid>()
        .unwrap()
}

/// A grid over `n` rows: col0 = the row's index (so sorting is observable), col1
/// a label. Columns 120px each. Viewport CSS height 220px (≈ 8 rows at 28px).
fn big_grid(n: usize) -> DataGrid {
    DataGrid::new()
        .column("Id", 120.0)
        .column("Name", 200.0)
        .row_height(28.0)
        .rows_from(n, |i| vec![format!("{i}"), format!("Row {i}")])
}

/// Unit: the windowing range clamps to the row count.
#[test]
fn clamp_range_is_bounded() {
    assert_eq!(clamp_range(0, 10, 5), (0, 5));
    assert_eq!(clamp_range(3, 4, 100), (3, 7));
    assert_eq!(clamp_range(200, 4, 100), (100, 100));
}

/// VIRTUALIZATION: a grid over 1000 rows renders only a windowful of rows, not
/// all 1000 — proven by counting the laid-out `row-<i>` boxes that exist.
#[test]
fn only_visible_rows_are_materialized() {
    let mut g = Gallery::new(W, H, "");
    g.mount("dg", Box::new(big_grid(1000)));
    g.relayout();
    // After one relayout the viewport height is observed; refresh by processing a
    // zero-scroll wheel so the window recomputes, then relayout again.
    let root = g.host.root_of("dg").unwrap();

    // Count how many row boxes exist in the laid-out tree.
    let count_rows = |g: &Gallery, root| {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (0..1000)
            .filter(|i| q.box_of_part(root, &format!("row-{i}")).is_some())
            .count()
    };
    let n = count_rows(&g, root);
    assert!(
        n > 0 && n < 60,
        "windowed render: expected a small visible window, got {n} rows materialized"
    );
    assert_eq!(as_grid(&g, "dg").row_count(), 1000, "but the model holds all rows");
}

/// VIRTUALIZATION + GEOMETRY: scrolling down materializes LATER rows (a row that
/// did not exist before the scroll exists after). The visible window follows the
/// laid-out viewport height, not a constant.
#[test]
fn scrolling_windows_to_later_rows() {
    let mut g = Gallery::new(W, H, "");
    g.mount("dg", Box::new(big_grid(1000)));
    g.relayout();
    let root = g.host.root_of("dg").unwrap();

    let row_500_before = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "row-500").is_some()
    };
    assert!(!row_500_before, "row 500 is far below the viewport initially");

    // Scroll far down (row 500 * 28px = 14000px). Wheel over the viewport.
    let vp = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "viewport").expect("viewport box")
    };
    g.scroll(vp.x + 20.0, vp.y + 20.0, 0.0, 14000.0);
    let acts = g.process();
    assert!(
        acts.iter().any(|a| a.name == SCROLLED_ACTION),
        "wheel emits a Scrolled action"
    );
    g.relayout();
    let row_500_after = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "row-500").is_some()
    };
    assert!(
        row_500_after,
        "after scrolling to row 500's band, that row is now materialized"
    );
    assert!(as_grid(&g, "dg").scroll_y() > 1000.0, "scroll offset advanced");
}

/// CELL HIT FROM LAYOUT: clicking inside a specific cell's REAL box selects that
/// cell (row data index + column). Anti-constant: cells have an unusual size, so
/// a `row*h`/`col*w` guess would mis-target.
#[test]
fn cell_click_selects_from_layout() {
    let mut g = Gallery::new(
        W,
        H,
        // Wide, tall cells so a fixed pitch guess would miss.
        "lq-grid-row { } lq-data-grid { width: 520px; }",
    );
    g.mount("dg", Box::new(big_grid(50).row_height(40.0)));
    g.relayout();
    g.relayout(); // second pass: viewport height + row height observed
    let root = g.host.root_of("dg").unwrap();

    // Find a materialized cell (row 2, col 1) and click its center.
    let cell = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "cell-2-1").expect("cell-2-1 box")
    };
    g.left_click(cell.x + cell.width / 2.0, cell.y + cell.height / 2.0);
    let acts = g.process();
    let sel = acts.iter().find(|a| a.name == SELECTED_ACTION).expect("selected action");
    assert_eq!(sel.payload.as_deref(), Some("2,1"), "click in cell-2-1's REAL box -> data row 2, col 1");
    assert_eq!(as_grid(&g, "dg").selected(), Some((2, 1)));
}

/// SORT: clicking a header re-sorts ascending; the top display row becomes the
/// smallest id.
#[test]
fn header_click_sorts_ascending() {
    let mut g = Gallery::new(W, H, "");
    // Insert rows out of order so an ascending sort is observable.
    g.mount(
        "dg",
        Box::new(
            DataGrid::new()
                .column("Id", 120.0)
                .column("Name", 200.0)
                .row(vec!["7".into(), "G".into()])
                .row(vec!["3".into(), "C".into()])
                .row(vec!["9".into(), "I".into()])
                .row(vec!["1".into(), "A".into()]),
        ),
    );
    g.relayout();
    let root = g.host.root_of("dg").unwrap();
    let head0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "head-0").expect("head-0 box")
    };
    g.left_click(head0.x + 16.0, head0.y + head0.height / 2.0);
    let a = g.process();
    let s = a.iter().find(|a| a.name == SORTED_ACTION).expect("sorted action");
    assert_eq!(s.payload.as_deref(), Some("0:asc"));
    g.relayout();
    // Ascending by numeric id: display position 0 = "1", last = "9".
    assert_eq!(as_grid(&g, "dg").cell(0, 0), Some("1"));
    assert_eq!(as_grid(&g, "dg").cell(3, 0), Some("9"));
}

/// SORT TOGGLE: a header click on a grid already sorted ascending toggles it to
/// descending (top display row becomes the largest id). Two separate galleries
/// avoid the dispatcher coalescing two rapid same-node clicks into a double-click.
#[test]
fn header_reclick_toggles_descending() {
    let base = || {
        DataGrid::new()
            .column("Id", 120.0)
            .row(vec!["7".into()])
            .row(vec!["3".into()])
            .row(vec!["9".into()])
            .row(vec!["1".into()])
    };

    // Gallery 1: one click -> ascending.
    let mut g0 = Gallery::new(W, H, "");
    g0.mount("p", Box::new(base()));
    g0.relayout();
    let r = g0.host.root_of("p").unwrap();
    let h = {
        let q = LayoutQuery::new(g0.hit_test_engine(), g0.doc());
        q.box_of_part(r, "head-0").unwrap()
    };
    g0.left_click(h.x + 16.0, h.y + h.height / 2.0);
    let _ = g0.process();
    let presorted = as_grid(&g0, "p").clone();
    assert_eq!(
        presorted.sort().map(|(_, d)| d),
        Some(crate::data_grid::SortDir::Asc),
        "first click sorts ascending"
    );

    // Gallery 2: mount the already-ascending grid; one more click -> descending.
    let mut g2 = Gallery::new(W, H, "");
    g2.mount("dg2", Box::new(presorted));
    g2.relayout();
    let root2 = g2.host.root_of("dg2").unwrap();
    let head = {
        let q = LayoutQuery::new(g2.hit_test_engine(), g2.doc());
        q.box_of_part(root2, "head-0").expect("head-0 box")
    };
    g2.left_click(head.x + 16.0, head.y + head.height / 2.0);
    let a = g2.process();
    let s = a.iter().find(|a| a.name == SORTED_ACTION).expect("sorted action");
    assert_eq!(s.payload.as_deref(), Some("0:desc"), "re-sort toggles to descending");
    g2.relayout();
    assert_eq!(as_grid(&g2, "dg2").cell(0, 0), Some("9"), "desc: largest id on top");
}

/// RESIZE: dragging a column separator from its LAID-OUT box widens the column;
/// the resized width is read back from state + emitted.
#[test]
fn dragging_separator_resizes_column() {
    let mut g = Gallery::new(W, H, "");
    g.mount("dg", Box::new(big_grid(10)));
    g.relayout();
    let root = g.host.root_of("dg").unwrap();

    let before = as_grid(&g, "dg").column_width(0).unwrap();
    let sep = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "sep-0").expect("sep-0 box")
    };
    // Press on the separator, drag right by 40px, release.
    g.mouse_down(sep.x + sep.width / 2.0, sep.y + sep.height / 2.0);
    let _ = g.process();
    g.relayout();
    g.pointer_move(sep.x + sep.width / 2.0 + 40.0, sep.y + sep.height / 2.0);
    let acts = g.process();
    assert!(
        acts.iter().any(|a| a.name == RESIZED_ACTION),
        "a separator drag emits a Resized action"
    );
    g.mouse_up(sep.x + 40.0, sep.y);
    let _ = g.process();

    let after = as_grid(&g, "dg").column_width(0).unwrap();
    assert!(
        (after - before - 40.0).abs() < 2.0,
        "column 0 widened by ~40px (before {before}, after {after})"
    );
}

/// KEYBOARD: PageDown scrolls a viewport-ish page; Home returns to the top.
#[test]
fn keyboard_paging_scrolls() {
    let mut g = Gallery::new(W, H, "");
    g.mount("dg", Box::new(big_grid(500)));
    g.relayout();
    g.relayout();
    g.host.set_focus(Some("dg"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::PAGE_DOWN, 0));
    g.relayout();
    let after_page = as_grid(&g, "dg").scroll_y();
    assert!(after_page > 100.0, "PageDown advanced the scroll (got {after_page})");

    g.key(KeyInput::new(keys::HOME, 0));
    g.relayout();
    assert_eq!(as_grid(&g, "dg").scroll_y(), 0.0, "Home returns to the top");
}

/// ANTI-CONSTANT (windowing follows the laid-out viewport): with a TALLER
/// viewport more rows are materialized than with the default — the window count
/// tracks the CSS height, not a constant.
#[test]
fn taller_viewport_materializes_more_rows() {
    let count_window = |css: &str| -> usize {
        let mut g = Gallery::new(W, H, css);
        g.mount("dg", Box::new(big_grid(1000)));
        g.relayout();
        // Seed the viewport-height cache with a benign (0-delta) wheel so the
        // window recomputes against the REAL laid-out viewport height, then
        // relayout so the new window materializes.
        let root = g.host.root_of("dg").unwrap();
        let vp = {
            let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
            q.box_of_part(root, "viewport").expect("viewport box")
        };
        g.scroll(vp.x + 10.0, vp.y + 10.0, 0.0, 1.0);
        let _ = g.process();
        g.relayout();
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (0..1000)
            .filter(|i| q.box_of_part(root, &format!("row-{i}")).is_some())
            .count()
    };
    let short = count_window("lq-grid-viewport { height: 120px; }");
    let tall = count_window("lq-grid-viewport { height: 360px; }");
    assert!(
        tall > short,
        "a taller viewport must window in MORE rows (short {short}, tall {tall})"
    );
}

/// PIXELS: a selected cell restyles the rasterized pixels (the selection fill).
#[test]
fn selected_cell_restyles_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("dg", Box::new(big_grid(20).row_height(34.0)));
    g.relayout();
    g.relayout();
    let root = g.host.root_of("dg").unwrap();
    let cell = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "cell-1-0").expect("cell-1-0 box")
    };
    let (sx, sy) = ((cell.x + cell.width / 2.0) as u32, (cell.y + cell.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);
    g.left_click(cell.x + cell.width / 2.0, cell.y + cell.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "selecting a cell must restyle its pixels");
}
