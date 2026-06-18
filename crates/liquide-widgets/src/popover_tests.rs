//! `<lq-popover>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::popover::{Placement, Popover, CLOSE_ACTION};

const W: u32 = 600;
const H: u32 = 400;

fn as_po<'a>(g: &'a Gallery, id: &str) -> &'a Popover {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Popover>()
        .unwrap()
}

fn part_box(g: &Gallery, id: &str, part: &str) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, part).unwrap_or_else(|| panic!("part {part} box"))
}

/// The standard measure-then-place second pass an embedding surface runs: read
/// the laid-out trigger + panel boxes, feed them to the popover's `reposition`,
/// re-render, and re-lay-out so the panel lands at its geometric placement.
fn place(g: &mut Gallery, id: &str) {
    if part_box_opt(g, id, "panel").is_none() {
        return; // closed: nothing to place.
    }
    let trig = part_box(g, id, "trigger");
    let panel = part_box(g, id, "panel");
    let b = g.host.behavior_mut(id).unwrap();
    let po = b.as_any_mut().unwrap().downcast_mut::<Popover>().unwrap();
    po.reposition(trig, panel);
    g.host.rerender(id, &mut g.doc);
    g.relayout();
}

fn part_box_opt(g: &Gallery, id: &str, part: &str) -> Option<liquide_layout::geometry::Rect> {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, part)
}

/// Clicking the trigger toggles the panel; a closed popover paints no panel.
#[test]
fn trigger_click_toggles_panel() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 40px; }");
    g.mount(
        "po",
        Box::new(Popover::new("Menu", Placement::Bottom).content("Panel body")),
    );
    g.relayout();
    let root = g.host.root_of("po").unwrap();
    {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "panel").is_none(), "closed: no panel");
    }
    let trig = part_box(&g, "po", "trigger");
    g.left_click(trig.x + trig.width / 2.0, trig.y + trig.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert!(as_po(&g, "po").is_open());
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.box_of_part(root, "panel").is_some(), "open: panel exists");
    drop(q);
}

/// PLACEMENT FROM TRIGGER BOX: a Bottom popover's laid-out panel sits BELOW the
/// laid-out trigger, and `panel_offset` (the pure placement math) agrees with the
/// real laid-out relationship for the actual trigger + panel boxes.
#[test]
fn bottom_placement_derives_from_trigger_box() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 40px; }");
    g.mount(
        "po",
        Box::new(Popover::new("Menu", Placement::Bottom).content("Panel body").open(true)),
    );
    g.relayout();
    place(&mut g, "po");
    let trig = part_box(&g, "po", "trigger");
    let panel = part_box(&g, "po", "panel");
    // The panel is BELOW the trigger.
    assert!(panel.y >= trig.bottom() - 1.0, "bottom panel below trigger (panel.y={}, trig.bottom={})", panel.y, trig.bottom());
    // And it is horizontally centered on the trigger (within a tolerance).
    let trig_cx = trig.x + trig.width / 2.0;
    let panel_cx = panel.x + panel.width / 2.0;
    assert!(
        (trig_cx - panel_cx).abs() < 3.0,
        "bottom panel centers on the trigger (trig_cx={trig_cx}, panel_cx={panel_cx})"
    );
    // The pure offset math matches the laid-out delta.
    let (dx, dy) = Popover::panel_offset(Placement::Bottom, trig, panel);
    let expect_x = trig.x + dx;
    let expect_y = trig.y + dy;
    assert!(
        (panel.x - expect_x).abs() < 3.0 && (panel.y - expect_y).abs() < 3.0,
        "panel_offset must predict the laid-out panel position (got ({},{}) expected ({},{}))",
        panel.x,
        panel.y,
        expect_x,
        expect_y
    );
}

/// A Right popover's panel sits to the RIGHT of the trigger — different geometry,
/// so the placement is genuinely derived, not a fixed offset.
#[test]
fn right_placement_differs_from_bottom() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 40px; }");
    g.mount(
        "po",
        Box::new(Popover::new("Menu", Placement::Right).content("Panel body").open(true)),
    );
    g.relayout();
    place(&mut g, "po");
    let trig = part_box(&g, "po", "trigger");
    let panel = part_box(&g, "po", "panel");
    assert!(panel.x >= trig.right() - 1.0, "right panel is to the right of the trigger");
    // Vertically centered on the trigger.
    let trig_cy = trig.y + trig.height / 2.0;
    let panel_cy = panel.y + panel.height / 2.0;
    assert!((trig_cy - panel_cy).abs() < 3.0, "right panel vertically centers on trigger");
}

/// NO-FAKE-GREEN tooth: the placement tracks the trigger BOX. Widen the trigger;
/// the bottom panel's center follows the new (wider) trigger center — a constant
/// horizontal offset would NOT recentre.
#[test]
fn placement_follows_trigger_width() {
    // Narrow trigger.
    let mut g1 = Gallery::new(W, H, "lq-gallery { padding: 40px; } lq-popover-trigger { width: 80px; }");
    g1.mount("po", Box::new(Popover::new("M", Placement::Bottom).content("Body").open(true)));
    g1.relayout();
    place(&mut g1, "po");
    let t1 = part_box(&g1, "po", "trigger");
    let p1 = part_box(&g1, "po", "panel");
    let center_delta_1 = (t1.x + t1.width / 2.0) - (p1.x + p1.width / 2.0);

    // Wide trigger.
    let mut g2 = Gallery::new(W, H, "lq-gallery { padding: 40px; } lq-popover-trigger { width: 240px; }");
    g2.mount("po", Box::new(Popover::new("M", Placement::Bottom).content("Body").open(true)));
    g2.relayout();
    place(&mut g2, "po");
    let t2 = part_box(&g2, "po", "trigger");
    let p2 = part_box(&g2, "po", "panel");
    let center_delta_2 = (t2.x + t2.width / 2.0) - (p2.x + p2.width / 2.0);

    // The wider trigger really is wider.
    assert!(t2.width > t1.width + 100.0, "precondition: trigger widened");
    // In BOTH cases the panel stays centered on its trigger (delta ≈ 0). A fixed
    // pixel offset would make the centers diverge as the trigger grows.
    assert!(center_delta_1.abs() < 4.0, "narrow: panel centered (delta {center_delta_1})");
    assert!(center_delta_2.abs() < 4.0, "wide: panel still centered (delta {center_delta_2})");
}

/// Esc closes an open popover (outside-click dismiss is the embedding surface's
/// job — a click that lands outside the widget never reaches the widget handler
/// in isolation, mirroring the dropdown's documented contract).
#[test]
fn escape_closes() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 40px; }");
    g.mount("po", Box::new(Popover::new("Menu", Placement::Bottom).content("Body").open(true)));
    g.relayout();
    g.host.set_focus(Some("po"), &mut g.doc, &mut g.dispatcher);
    assert!(as_po(&g, "po").is_open());
    let a = g.key(KeyInput::new(keys::ESCAPE, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CLOSE_ACTION);
    g.relayout();
    assert!(!as_po(&g, "po").is_open());
}

/// A click inside the panel does NOT close the popover.
#[test]
fn panel_click_keeps_open() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 40px; }");
    g.mount("po", Box::new(Popover::new("Menu", Placement::Bottom).content("Body").open(true)));
    g.relayout();
    place(&mut g, "po");
    let panel = part_box(&g, "po", "panel");
    g.left_click(panel.x + panel.width / 2.0, panel.y + panel.height / 2.0);
    let a = g.process();
    assert!(a.is_empty(), "panel click emits nothing");
    g.relayout();
    assert!(as_po(&g, "po").is_open(), "panel click keeps the popover open");
}

/// Opening restyles pixels (the panel appears below the trigger).
#[test]
fn open_changes_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 40px; }");
    g.mount("po", Box::new(Popover::new("Menu", Placement::Bottom).content("Body")));
    g.relayout();
    let trig = part_box(&g, "po", "trigger");
    let (sx, sy) = ((trig.x + 10.0) as u32, (trig.bottom() + 30.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);

    g.mount("po", Box::new(Popover::new("Menu", Placement::Bottom).content("Body").open(true)));
    g.relayout();
    place(&mut g, "po");
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "the panel must paint below the trigger when open");
}
