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

// ── Added: deep visual-STATE / styling coverage (no fake-green) ──────────────

/// A top-edge panel pins to the TOP and spans the full width (distinct geometry
/// from left/right — proves the edge class drives layout, not a constant).
#[test]
fn top_edge_panel_pins_to_the_top() {
    let mut g = Gallery::new(W, H, "");
    mount_open(&mut g, "dr", Edge::Top);
    let root = g.host.root_of("dr").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let panel = q.box_of_part(root, "panel").unwrap();
    let scrim = q.box_of_part(root, "scrim").unwrap();
    assert!((panel.y - scrim.y).abs() < 2.0, "top panel hugs the top (panel.y={}, scrim.y={})", panel.y, scrim.y);
    assert!((panel.width - scrim.width).abs() < 2.0, "top panel spans the full width");
    // A partial-height sheet: it does NOT cover the full surface (bottom is well
    // above the surface bottom), confirming it is pinned to the top edge.
    assert!(panel.bottom() < scrim.bottom() - 1.0, "top panel does not reach the bottom edge (panel.bottom={}, scrim.bottom={})", panel.bottom(), scrim.bottom());
}

/// A bottom-edge panel pins to the BOTTOM and spans the full width.
#[test]
fn bottom_edge_panel_pins_to_the_bottom() {
    let mut g = Gallery::new(W, H, "");
    mount_open(&mut g, "dr", Edge::Bottom);
    let root = g.host.root_of("dr").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let panel = q.box_of_part(root, "panel").unwrap();
    let scrim = q.box_of_part(root, "scrim").unwrap();
    assert!(
        (panel.bottom() - scrim.bottom()).abs() < 2.0,
        "bottom panel hugs the bottom (panel.bottom={}, scrim.bottom={})",
        panel.bottom(),
        scrim.bottom()
    );
    assert!((panel.width - scrim.width).abs() < 2.0, "bottom panel spans the full width");
    // Pinned to the bottom edge: its top is well below the surface top.
    assert!(panel.y > scrim.y + 1.0, "bottom panel does not start at the top edge (panel.y={}, scrim.y={})", panel.y, scrim.y);
}

/// The panel paints a solid surface that is visibly DIFFERENT from the dimmed
/// scrim beside it — the panel bg vs the translucent scrim are distinct pixels.
#[test]
fn panel_surface_differs_from_scrim() {
    let mut g = Gallery::new(W, H, "");
    mount_open(&mut g, "dr", Edge::Right);
    let root = g.host.root_of("dr").unwrap();
    let (scrim, panel) = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (q.box_of_part(root, "scrim").unwrap(), q.box_of_part(root, "panel").unwrap())
    };
    let fb = g.rasterize();
    // A point inside the panel vs a scrim-only point on the left third.
    let panel_px = Gallery::pixel(&fb, (panel.x + panel.width / 2.0) as u32, (panel.y + panel.height / 2.0) as u32);
    let scrim_px = Gallery::pixel(&fb, (scrim.x + 20.0) as u32, (scrim.y + scrim.height / 2.0) as u32);
    assert!(
        panel_px != scrim_px,
        "the panel surface must paint differently from the dimmed scrim (panel {panel_px:?} scrim {scrim_px:?})"
    );
}

/// A titled drawer renders a header band with a bottom border whose styling makes
/// the header row paint differently from the plain body below it.
#[test]
fn header_band_paints_distinct_from_body() {
    let mut g = Gallery::new(W, H, "");
    g.mount(
        "dr",
        Box::new(Drawer::new(Edge::Right).title("Settings").content("Drawer body").open(true)),
    );
    g.relayout();
    let root = g.host.root_of("dr").unwrap();
    let (header, body) = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (
            q.box_of_part(root, "header").expect("header box"),
            q.box_of_part(root, "body").expect("body box"),
        )
    };
    // The header sits above the body within the panel.
    assert!(header.bottom() <= body.y + 1.0, "header is above the body");
    let fb = g.rasterize();
    // Sample the header's bottom border line vs the body interior.
    let border = Gallery::pixel(&fb, (header.x + header.width / 2.0) as u32, (header.bottom() - 1.0) as u32);
    let body_px = Gallery::pixel(&fb, (body.x + body.width / 2.0) as u32, (body.y + body.height / 2.0) as u32);
    assert!(
        border != body_px,
        "the header's bordered band must paint distinctly from the body (border {border:?} body {body_px:?})"
    );
}

/// A non-dismissable drawer swallows the scrim click (stays open, emits nothing) —
/// the `dismissable(false)` flag really gates the close path through the real box.
#[test]
fn non_dismissable_scrim_click_is_inert() {
    let mut g = Gallery::new(W, H, "");
    g.mount(
        "dr",
        Box::new(Drawer::new(Edge::Right).content("x").dismissable(false).open(true)),
    );
    g.relayout();
    g.host.set_focus(Some("dr"), &mut g.doc, &mut g.dispatcher);
    let root = g.host.root_of("dr").unwrap();
    let (scrim, panel) = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (q.box_of_part(root, "scrim").unwrap(), q.box_of_part(root, "panel").unwrap())
    };
    let px = scrim.x + 20.0;
    let py = scrim.y + scrim.height / 2.0;
    assert!(!panel.contains(liquide_layout::geometry::Point::new(px, py)), "point is outside panel");
    g.left_click(px, py);
    let a = g.process();
    assert!(a.is_empty(), "a non-dismissable drawer ignores the scrim click");
    g.relayout();
    assert!(as_dr(&g, "dr").is_open(), "the drawer stays open");
    // But Esc still closes it (Esc is not gated by dismissable).
    let a = g.key(KeyInput::new(keys::ESCAPE, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CLOSE_ACTION);
}
