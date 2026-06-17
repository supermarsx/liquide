//! `<lq-accordion>` real-pipeline gallery tests.
#![cfg(test)]

use crate::accordion::{Accordion, TOGGLED_ACTION};
use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 360;
const H: u32 = 400;

fn as_acc<'a>(g: &'a Gallery, id: &str) -> &'a Accordion {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Accordion>()
        .unwrap()
}

fn sections() -> Vec<(String, String)> {
    vec![
        ("General".into(), "General settings body text.".into()),
        ("Network".into(), "Network settings body text.".into()),
        ("Privacy".into(), "Privacy settings body text.".into()),
    ]
}

/// Clicking a header toggles that section open; emits Toggled(index). The body
/// panel only exists when expanded.
#[test]
fn header_click_toggles_section() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("ac", Box::new(Accordion::new(sections())));
    g.relayout();
    assert!(!as_acc(&g, "ac").is_expanded(0));

    let root = g.host.root_of("ac").unwrap();
    // No body box before expansion.
    {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "body").is_none(), "collapsed: no body");
    }
    let h0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "header-0").expect("header-0 box")
    };
    g.left_click(h0.x + 8.0, h0.y + h0.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, TOGGLED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("0"));
    assert!(as_acc(&g, "ac").is_expanded(0));
    g.relayout();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.box_of_part(root, "body").is_some(), "expanded: body appears");
}

/// NO-FAKE-GREEN tooth: header hit reads each header's REAL laid-out box. Once
/// section 0 expands (its body pushes section 1's header DOWN), a click on
/// section-1's header still targets section 1 — a constant header-pitch would
/// land in the wrong section after the panel grows.
#[test]
fn header_hit_tracks_layout_after_expansion() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-section-body { padding: 40px; }");
    g.mount("ac", Box::new(Accordion::new(sections())));
    g.relayout();
    let root = g.host.root_of("ac").unwrap();

    // Expand section 0 (grows a tall body, shifting header-1 down).
    let h0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "header-0").unwrap()
    };
    g.left_click(h0.x + 8.0, h0.y + h0.height / 2.0);
    let _ = g.process();
    g.relayout();

    // header-1 is now further down. Read its REAL box and click it.
    let h1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "header-1").expect("header-1 box")
    };
    g.left_click(h1.x + 8.0, h1.y + h1.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("1"), "click in shifted header-1 toggles section 1");
    assert!(as_acc(&g, "ac").is_expanded(1));
}

/// Single-open mode: opening a second section closes the first.
#[test]
fn single_open_closes_others() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount(
        "ac",
        Box::new(Accordion::new(sections()).single_open(true).expand(0)),
    );
    g.relayout();
    assert!(as_acc(&g, "ac").is_expanded(0));
    let root = g.host.root_of("ac").unwrap();
    let h1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "header-1").unwrap()
    };
    g.left_click(h1.x + 8.0, h1.y + h1.height / 2.0);
    let _ = g.process();
    assert!(as_acc(&g, "ac").is_expanded(1));
    assert!(!as_acc(&g, "ac").is_expanded(0), "single-open closed section 0");
}

/// Multi-open mode: both can be open at once.
#[test]
fn multi_open_keeps_both() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("ac", Box::new(Accordion::new(sections()).expand(0)));
    g.relayout();
    let root = g.host.root_of("ac").unwrap();
    let h2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "header-2").unwrap()
    };
    g.left_click(h2.x + 8.0, h2.y + h2.height / 2.0);
    let _ = g.process();
    assert_eq!(as_acc(&g, "ac").expanded_indices(), vec![0, 2]);
}

/// Keyboard: Down moves the cursor, Enter toggles the cursor section.
#[test]
fn keyboard_toggles() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("ac", Box::new(Accordion::new(sections())));
    g.relayout();
    g.host.set_focus(Some("ac"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_acc(&g, "ac").cursor(), 1);
    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(a[0].payload.as_deref(), Some("1"));
    assert!(as_acc(&g, "ac").is_expanded(1));
}

/// Expanding restyles the header pixels (:expanded).
#[test]
fn expansion_restyles_header_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("ac", Box::new(Accordion::new(sections())));
    g.relayout();
    let root = g.host.root_of("ac").unwrap();
    let h0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "header-0").unwrap()
    };
    let (cx, cy) = ((h0.x + h0.width - 20.0) as u32, (h0.y + h0.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    g.left_click(h0.x + 8.0, h0.y + h0.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(before != after, "expanded header must restyle");
}
