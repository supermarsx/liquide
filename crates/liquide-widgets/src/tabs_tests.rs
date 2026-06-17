//! `<lq-tabs>` real-pipeline gallery tests (no fake-green).
//!
//! Teeth: clicking a tab's LAID-OUT box switches the active panel (the panel
//! content actually changes in the DOM); the active tab carries :checked which
//! restyles its pixels; arrow keys move the active tab; a click in tab-1's real
//! box selects tab 1 (geometry per-tab from layout, not an index over a constant
//! tab width).
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::tabs::{Tabs, CHANGED_ACTION};

const W: u32 = 420;
const H: u32 = 240;

fn as_tabs<'a>(g: &'a Gallery, id: &str) -> &'a Tabs {
    g.host.behavior(id).unwrap().as_any().downcast_ref::<Tabs>().unwrap()
}

fn three_tabs() -> Tabs {
    Tabs::new()
        .tab("One", "first panel content")
        .tab("Two", "second panel content")
        .tab("Three", "third panel content")
}

fn tab_box(g: &Gallery, id: &str, i: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("tab-{i}")).expect("tab box")
}

/// The panel text node under the panel part (proves which panel is shown).
fn panel_text(g: &Gallery, id: &str) -> String {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let panel = q.find_part(root, "panel").expect("panel");
    // The panel's single text child.
    let mut out = String::new();
    fn collect(doc: &liquide_dom::Document, node: liquide_dom::NodeId, out: &mut String) {
        if let Some(t) = doc.get(node).and_then(|n| n.text_content()) {
            out.push_str(t);
        }
        for &c in doc.children(node) {
            collect(doc, c, out);
        }
    }
    collect(g.doc(), panel, &mut out);
    out
}

/// Initially the first tab is active and its panel is shown.
#[test]
fn first_tab_active_initially() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs()));
    g.relayout();
    assert_eq!(as_tabs(&g, "t").selected_index(), 0);
    assert!(panel_text(&g, "t").contains("first"), "first panel shown");
}

/// Clicking the second tab's LAID-OUT box switches the active panel.
#[test]
fn clicking_tab_switches_panel() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs()));
    g.relayout();

    let t1 = tab_box(&g, "t", 1);
    g.left_click(t1.x + t1.width / 2.0, t1.y + t1.height / 2.0);
    let actions = g.process();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, CHANGED_ACTION);
    assert_eq!(actions[0].payload.as_deref(), Some("1"));
    assert_eq!(as_tabs(&g, "t").selected_index(), 1);

    g.relayout();
    assert!(panel_text(&g, "t").contains("second"), "second panel now shown");
    assert!(!panel_text(&g, "t").contains("first"), "first panel hidden");
}

/// NO-FAKE-GREEN tooth: with unequal tab widths, a click in tab 2's REAL box
/// selects 2 — proving per-tab hit-test from layout, not an index over a constant
/// width. The first tab is widened so a constant-width index would mis-map.
#[test]
fn tab_hit_test_comes_from_layout_not_constant() {
    // Make tab-0 very wide; a constant tab width would put the click into the
    // wrong tab.
    let css = "lq-gallery { padding: 8px; }
               lq-tabs > lq-tablist > lq-tab:first-child { padding-left: 80px; padding-right: 80px; }";
    let mut g = Gallery::new(W, H, css);
    g.mount("t", Box::new(three_tabs()));
    g.relayout();

    let t0 = tab_box(&g, "t", 0);
    let t2 = tab_box(&g, "t", 2);
    // Precondition: tab-0 is wider than a "default" tab so geometry is unequal.
    assert!(t0.width > 100.0, "tab-0 widened (got {})", t0.width);

    // Click inside tab-2's REAL box.
    g.left_click(t2.x + t2.width / 2.0, t2.y + t2.height / 2.0);
    let _ = g.process();
    assert_eq!(
        as_tabs(&g, "t").selected_index(),
        2,
        "click in tab-2's laid-out box must select 2 (geometry from layout)"
    );
}

/// The active tab's :checked restyles its rasterized pixels.
#[test]
fn active_tab_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs()));
    g.relayout();

    let t1 = tab_box(&g, "t", 1);
    let (cx, cy) = ((t1.x + t1.width / 2.0) as u32, (t1.y + t1.height - 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    g.left_click(t1.x + t1.width / 2.0, t1.y + t1.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(before != after, "active tab must restyle (before {before:?} after {after:?})");
}

/// Arrow keys move the active tab (wrapping); Home/End jump.
#[test]
fn keyboard_moves_active_tab() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs()));
    g.relayout();
    g.host.set_focus(Some("t"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_tabs(&g, "t").selected_index(), 1);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_tabs(&g, "t").selected_index(), 2);
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0)); // wrap
    assert_eq!(as_tabs(&g, "t").selected_index(), 0);
    g.key(KeyInput::new(keys::ARROW_LEFT, 0)); // wrap back
    assert_eq!(as_tabs(&g, "t").selected_index(), 2);
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_tabs(&g, "t").selected_index(), 0);
}

/// Disabled tabs ignore clicks and keys.
#[test]
fn disabled_tabs_ignore_input() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs().disabled(true)));
    g.relayout();
    let t1 = tab_box(&g, "t", 1);
    g.left_click(t1.x + t1.width / 2.0, t1.y + t1.height / 2.0);
    let _ = g.process();
    g.host.set_focus(Some("t"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_tabs(&g, "t").selected_index(), 0, "disabled tabs hold selection");
}
