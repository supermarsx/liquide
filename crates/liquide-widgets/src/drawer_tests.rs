//! `<lq-drawer>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::drawer::{Drawer, Edge, CLOSE_ACTION};
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 600;
const H: u32 = 400;

fn as_dr<'a>(g: &'a Gallery, id: &str) -> &'a Drawer {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Drawer>()
        .unwrap()
}

fn mount_open(g: &mut Gallery, id: &str, edge: Edge) {
    g.mount(
        id,
        Box::new(Drawer::new(edge).title("Settings").content("Drawer body").open(true)),
    );
    g.relayout();
    g.host.set_focus(Some(id), &mut g.doc, &mut g.dispatcher);
}

/// A closed drawer paints neither scrim nor panel; an open one paints both.
#[test]
fn closed_paints_nothing_open_paints_scrim_and_panel() {
    let mut g = Gallery::new(W, H, "");
    g.mount("dr", Box::new(Drawer::new(Edge::Right).content("x").open(false)));
    g.relayout();
    let root = g.host.root_of("dr").unwrap();
    {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "scrim").is_none(), "closed: no scrim");
        assert!(q.box_of_part(root, "panel").is_none(), "closed: no panel");
    }
    g.mount("dr", Box::new(Drawer::new(Edge::Right).content("x").open(true)));
    g.relayout();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.box_of_part(root, "scrim").is_some(), "open: scrim exists");
    assert!(q.box_of_part(root, "panel").is_some(), "open: panel exists");
}

/// The right-edge panel is laid out at the right side of the surface (geometry
/// from layout, driven by the edge class — not a constant).
#[test]
fn right_edge_panel_pins_to_the_right() {
    let mut g = Gallery::new(W, H, "");
    mount_open(&mut g, "dr", Edge::Right);
    let root = g.host.root_of("dr").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let panel = q.box_of_part(root, "panel").unwrap();
    let scrim = q.box_of_part(root, "scrim").unwrap();
    // The panel's right edge meets the surface's right edge; its left is well
    // past the surface midpoint.
    assert!(
        (panel.right() - scrim.right()).abs() < 2.0,
        "right drawer panel hugs the right edge (panel.right={}, surface.right={})",
        panel.right(),
        scrim.right()
    );
    assert!(panel.x > scrim.width / 2.0, "panel is on the right half");
}

/// A left-edge panel pins to the left (different geometry → not hardcoded).
#[test]
fn left_edge_panel_pins_to_the_left() {
    let mut g = Gallery::new(W, H, "");
    mount_open(&mut g, "dr", Edge::Left);
    let root = g.host.root_of("dr").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let panel = q.box_of_part(root, "panel").unwrap();
    let scrim = q.box_of_part(root, "scrim").unwrap();
    assert!(
        (panel.x - scrim.x).abs() < 2.0,
        "left drawer panel hugs the left edge (panel.x={}, surface.x={})",
        panel.x,
        scrim.x
    );
    assert!(panel.right() < scrim.width / 2.0 + scrim.x, "panel is on the left half");
}

/// Clicking the scrim (outside the panel) closes + emits close.
#[test]
fn scrim_click_closes() {
    let mut g = Gallery::new(W, H, "");
    mount_open(&mut g, "dr", Edge::Right);
    let root = g.host.root_of("dr").unwrap();
    let (scrim, panel) = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (q.box_of_part(root, "scrim").unwrap(), q.box_of_part(root, "panel").unwrap())
    };
    // A point in the scrim but NOT in the panel: the left third (panel is right).
    let px = scrim.x + 20.0;
    let py = scrim.y + scrim.height / 2.0;
    assert!(!panel.contains(liquide_layout::geometry::Point::new(px, py)), "point is outside panel");
    g.left_click(px, py);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CLOSE_ACTION);
    g.relayout();
    assert!(!as_dr(&g, "dr").is_open());
}

/// Clicking INSIDE the panel does NOT close (swallowed).
#[test]
fn panel_click_does_not_close() {
    let mut g = Gallery::new(W, H, "");
    mount_open(&mut g, "dr", Edge::Right);
    let root = g.host.root_of("dr").unwrap();
    let panel = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "panel").unwrap()
    };
    g.left_click(panel.x + panel.width / 2.0, panel.y + panel.height / 2.0);
    let a = g.process();
    assert!(a.is_empty(), "a click inside the panel emits nothing");
    g.relayout();
    assert!(as_dr(&g, "dr").is_open(), "the drawer stays open");
}

/// Esc closes an open drawer.
#[test]
fn escape_closes() {
    let mut g = Gallery::new(W, H, "");
    mount_open(&mut g, "dr", Edge::Bottom);
    assert!(as_dr(&g, "dr").is_open());
    let a = g.key(KeyInput::new(keys::ESCAPE, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CLOSE_ACTION);
    g.relayout();
    assert!(!as_dr(&g, "dr").is_open());
}

/// Opening restyles pixels (the scrim dims the surface).
#[test]
fn open_changes_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("dr", Box::new(Drawer::new(Edge::Right).content("x").open(false)));
    g.relayout();
    let before = Gallery::pixel(&g.rasterize(), 30, H / 2);
    g.mount("dr", Box::new(Drawer::new(Edge::Right).content("x").open(true)));
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), 30, H / 2);
    assert!(before != after, "the scrim must dim the surface when open");
}
