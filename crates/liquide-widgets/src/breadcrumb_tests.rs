//! `<lq-breadcrumb>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::breadcrumb::{Breadcrumb, NAVIGATE_ACTION};
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 400;
const H: u32 = 80;

fn as_bc<'a>(g: &'a Gallery, id: &str) -> &'a Breadcrumb {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Breadcrumb>()
        .unwrap()
}

fn crumbs() -> Vec<String> {
    vec!["Home".into(), "Projects".into(), "liquide".into()]
}

/// Clicking a non-last crumb's laid-out box emits Navigate(index).
#[test]
fn click_crumb_navigates() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("bc", Box::new(Breadcrumb::new(crumbs())));
    g.relayout();

    let root = g.host.root_of("bc").unwrap();
    let c1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "crumb-1").expect("crumb-1 box")
    };
    g.left_click(c1.x + 4.0, c1.y + c1.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, NAVIGATE_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("1"));
}

/// The last (current) crumb is not clickable.
#[test]
fn current_crumb_not_clickable() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("bc", Box::new(Breadcrumb::new(crumbs())));
    g.relayout();
    let root = g.host.root_of("bc").unwrap();
    let last = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "crumb-2").expect("crumb-2 box")
    };
    g.left_click(last.x + 4.0, last.y + last.height / 2.0);
    assert!(g.process().is_empty(), "the current crumb must not navigate");
    assert_eq!(as_bc(&g, "bc").current_index(), Some(2));
}

/// NO-FAKE-GREEN tooth: per-crumb hit reads each crumb's REAL laid-out box. The
/// crumbs are different widths (different labels), so a fixed-width guess would
/// mis-target. A click in crumb-0's true box navigates to 0, not 1.
#[test]
fn crumb_hit_comes_from_layout_not_constant() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount(
        "bc",
        Box::new(Breadcrumb::new(vec![
            "A".into(),
            "LongerMiddle".into(),
            "End".into(),
        ])),
    );
    g.relayout();
    let root = g.host.root_of("bc").unwrap();
    let c0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "crumb-0").expect("crumb-0 box")
    };
    g.left_click(c0.x + 2.0, c0.y + c0.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("0"), "click in crumb-0's REAL box -> 0");
}

/// Keyboard: Right moves the cursor across links, Enter navigates.
#[test]
fn keyboard_navigates() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("bc", Box::new(Breadcrumb::new(crumbs())));
    g.relayout();
    g.host.set_focus(Some("bc"), &mut g.doc, &mut g.dispatcher);
    assert_eq!(as_bc(&g, "bc").cursor(), 0);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_bc(&g, "bc").cursor(), 1, "cursor moves to the last link");
    // Cannot move past the last link (index 1; index 2 is current).
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_bc(&g, "bc").cursor(), 1, "cursor clamps at the last link");

    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(a[0].payload.as_deref(), Some("1"));
}

/// A single-crumb breadcrumb has nothing clickable.
#[test]
fn single_crumb_is_inert() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("bc", Box::new(Breadcrumb::new(vec!["Only".into()])));
    g.relayout();
    let root = g.host.root_of("bc").unwrap();
    let c0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "crumb-0").unwrap()
    };
    g.left_click(c0.x + 4.0, c0.y + c0.height / 2.0);
    assert!(g.process().is_empty());
}

// ── added: per-state styling proofs — DEFERRED (no provable pixel signal) ──
//
// Breadcrumb visual states are NOT pixel-provable through the gallery in the
// current engine, so no pixel-delta tests are added here (a fake-green test that
// could not fail when the style is removed would be worse than none). The gaps,
// reported to the coordinator:
//
//   * link / current / :hover / :focus crumb styling is expressed ONLY as text
//     `color:` on `<lq-crumb>` (no background). A crumb box rasterizes to a fully
//     transparent (0,0,0,0) region — the gallery glyph rasterizer does not ink the
//     crumb text — so a colour-rule change produces no pixel delta. (The sibling
//     `label_tests` link colour-state tests fail the same way in this build.)
//   * the separator `lq-crumb.link::after { content: "›" }` generates NO layout
//     box: a link crumb and an equal-label current crumb measure identical widths
//     (44.48 == 44.48), so the `::after` neither reserves space nor paints — CSS
//     generated content is not realised by this layout/paint path.
//
// The interaction + keyboard + hit-from-layout coverage above remains the
// breadcrumb's behavioural proof; the colour/separator styling needs either a
// painted glyph path or a non-text affordance before it can be pixel-tested.
