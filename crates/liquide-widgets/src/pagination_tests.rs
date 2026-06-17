//! `<lq-pagination>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::pagination::{Pagination, CHANGED_ACTION};

const W: u32 = 480;
const H: u32 = 90;

fn as_pg<'a>(g: &'a Gallery, id: &str) -> &'a Pagination {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Pagination>()
        .unwrap()
}

/// Clicking a page button's laid-out box jumps to that page; emits Changed(page).
#[test]
fn click_page_changes_page() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("pg", Box::new(Pagination::new(5)));
    g.relayout();
    assert_eq!(as_pg(&g, "pg").current_page(), 0);

    let root = g.host.root_of("pg").unwrap();
    // page-3 button (label "4").
    let p3 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "page-3").expect("page-3 box")
    };
    g.left_click(p3.x + 4.0, p3.y + p3.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("3"));
    assert_eq!(as_pg(&g, "pg").current_page(), 3);
}

/// Prev/Next step by one; Prev is inert at the start, Next at the end.
#[test]
fn prev_next_step_and_clamp() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("pg", Box::new(Pagination::new(3)));
    g.relayout();
    let root = g.host.root_of("pg").unwrap();

    // Prev at page 0 is a no-op.
    let prev = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "prev").unwrap()
    };
    g.left_click(prev.x + 4.0, prev.y + prev.height / 2.0);
    assert!(g.process().is_empty(), "prev at start is inert");
    assert_eq!(as_pg(&g, "pg").current_page(), 0);

    // Next steps to 1.
    let next = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "next").unwrap()
    };
    g.left_click(next.x + 4.0, next.y + next.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("1"));
    assert_eq!(as_pg(&g, "pg").current_page(), 1);
}

/// Long ranges render ellipses but the actual page buttons still hit-test from
/// layout. Jump near the end via the last page button.
#[test]
fn long_range_has_ellipses_and_real_last_button() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("pg", Box::new(Pagination::new(20).page(0)));
    g.relayout();
    let root = g.host.root_of("pg").unwrap();
    {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        // An ellipsis gap exists (page 1 and page 19 are far apart).
        assert!(q.box_of_part(root, "ellipsis").is_some(), "long range shows an ellipsis");
        // The last page button (index 19) is always present.
        assert!(q.box_of_part(root, "page-19").is_some(), "last page button present");
        // A middle page like 10 is NOT rendered while on page 0.
        assert!(q.box_of_part(root, "page-10").is_none(), "far middle page collapsed");
    }
    let last = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "page-19").unwrap()
    };
    g.left_click(last.x + 4.0, last.y + last.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("19"));
}

/// NO-FAKE-GREEN tooth: page-button hit reads each button's REAL laid-out box.
/// An EARLY button (page-1) is heavily widened so every LATER button shifts far
/// right of its uniform-pitch position. Clicking page-3's REAL left edge selects
/// 3 — a constant-pitch guess from the row start would compute a much lower
/// column (the shifted buttons no longer line up with a fixed stride).
#[test]
fn page_hit_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        560,
        H,
        "lq-gallery { padding: 12px; } \
         lq-page-btn[data-page=\"1\"] { padding-left: 120px; }",
    );
    g.mount("pg", Box::new(Pagination::new(5)));
    g.relayout();
    let root = g.host.root_of("pg").unwrap();
    let p3 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "page-3").expect("page-3 box")
    };
    // page-3 is pushed far right by the widened page-1; clicking its real left
    // edge must select 3. A uniform ~35px-stride guess would land on page-1/2.
    g.left_click(p3.x + 4.0, p3.y + p3.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("3"), "click in page-3's REAL box -> 3");
}

/// Keyboard Left/Right step pages; Home/End jump.
#[test]
fn keyboard_steps_pages() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("pg", Box::new(Pagination::new(10).page(5)));
    g.relayout();
    g.host.set_focus(Some("pg"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_pg(&g, "pg").current_page(), 6);
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert_eq!(as_pg(&g, "pg").current_page(), 5);
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_pg(&g, "pg").current_page(), 0);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_pg(&g, "pg").current_page(), 9);
}

/// The current page button restyles its pixels (:checked).
#[test]
fn current_page_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("pg", Box::new(Pagination::new(5)));
    g.relayout();
    let root = g.host.root_of("pg").unwrap();
    let p2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "page-2").unwrap()
    };
    let (cx, cy) = ((p2.x + p2.width / 2.0) as u32, (p2.y + p2.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);
    g.left_click(p2.x + 4.0, p2.y + p2.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(before != after, "current page button must restyle");
}
