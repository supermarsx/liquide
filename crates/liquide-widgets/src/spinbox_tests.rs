//! `<lq-spinbox>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::spinbox::{Spinbox, CHANGED_ACTION};

const W: u32 = 240;
const H: u32 = 120;

fn as_spin<'a>(g: &'a Gallery, id: &str) -> &'a Spinbox {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Spinbox>()
        .unwrap()
}

/// Clicking the up box increments; the down box decrements. Hit zones from the
/// LAID-OUT up/down boxes (a constant split would mis-target).
#[test]
fn up_down_boxes_step_the_value() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sp", Box::new(Spinbox::new(0.0, 10.0, 5.0)));
    g.relayout();
    let root = g.host.root_of("sp").unwrap();
    let (up, down) = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (
            q.box_of_part(root, "up").expect("up box"),
            q.box_of_part(root, "down").expect("down box"),
        )
    };
    // The two boxes are vertically stacked: up sits above down.
    assert!(up.y < down.y, "up box must be above the down box in layout");

    g.left_click(up.x + up.width / 2.0, up.y + up.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("6"));
    assert_eq!(as_spin(&g, "sp").value(), 6.0);

    g.left_click(down.x + down.width / 2.0, down.y + down.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("5"));
    assert_eq!(as_spin(&g, "sp").value(), 5.0);
}

/// NO-FAKE-GREEN tooth: clicking the VALUE display (between/left of the buttons)
/// does NOT step — the step is gated on the real up/down boxes, not "right side".
#[test]
fn value_area_click_does_not_step() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sp", Box::new(Spinbox::new(0.0, 10.0, 5.0)));
    g.relayout();
    let root = g.host.root_of("sp").unwrap();
    let val = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "value").expect("value box")
    };
    g.left_click(val.x + 4.0, val.y + val.height / 2.0);
    let a = g.process();
    assert!(
        a.iter().all(|act| act.name != CHANGED_ACTION),
        "clicking the value display must not step (got {a:?})"
    );
    assert_eq!(as_spin(&g, "sp").value(), 5.0);
}

/// Up/Down arrows step; Home/End jump to min/max; clamps at bounds.
#[test]
fn arrows_and_clamp() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sp", Box::new(Spinbox::new(0.0, 3.0, 2.0)));
    g.relayout();
    g.host.set_focus(Some("sp"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_UP, 0));
    assert_eq!(as_spin(&g, "sp").value(), 3.0);
    // Already at max: no further increment (Ignored).
    let a = g.key(KeyInput::new(keys::ARROW_UP, 0));
    assert!(a.is_empty(), "clamped at max emits nothing");
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_spin(&g, "sp").value(), 0.0);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_spin(&g, "sp").value(), 3.0);
}

/// Step honors a non-unit step.
#[test]
fn respects_step() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sp", Box::new(Spinbox::new(0.0, 100.0, 10.0).step(5.0)));
    g.relayout();
    g.host.set_focus(Some("sp"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_UP, 0));
    assert_eq!(as_spin(&g, "sp").value(), 15.0);
}

/// Wheel scroll steps the value (scroll up increments, down decrements).
#[test]
fn wheel_steps_value() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sp", Box::new(Spinbox::new(0.0, 10.0, 5.0)));
    g.relayout();
    let root = g.host.root_of("sp").unwrap();
    let r = g.box_of(root).unwrap();
    // Scroll UP (negative dy) over the widget -> increment.
    g.scroll(r.x + r.width / 2.0, r.y + r.height / 2.0, 0.0, -1.0);
    let a = g.process();
    assert_eq!(a.last().map(|x| x.payload.as_deref()), Some(Some("6")));
    assert_eq!(as_spin(&g, "sp").value(), 6.0);

    g.scroll(r.x + r.width / 2.0, r.y + r.height / 2.0, 0.0, 1.0);
    let a = g.process();
    assert_eq!(a.last().map(|x| x.payload.as_deref()), Some(Some("5")));
}

/// Reaching the max value restyles the up button (it gains the disabled style),
/// proving the value-driven state change reaches the laid-out up box in pixels.
/// (A glyph-only assertion is avoided: the gallery font rasterizer paints digit
/// glyphs too faintly to compare reliably — the value/state changes are asserted
/// directly elsewhere; here we prove a value-driven CSS restyle lands.)
#[test]
fn reaching_max_restyles_up_button() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sp", Box::new(Spinbox::new(0.0, 3.0, 0.0)));
    g.relayout();
    let root = g.host.root_of("sp").unwrap();
    let up = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "up").unwrap()
    };
    let sum = |fb: &liquide_compositor::framebuffer::FrameBuffer| -> u64 {
        let mut acc = 0u64;
        for y in (up.y as u32)..((up.y + up.height) as u32) {
            for x in (up.x as u32)..((up.x + up.width) as u32) {
                let p = Gallery::pixel(fb, x, y);
                acc += p.r as u64 + p.g as u64 * 3 + p.b as u64 * 7;
            }
        }
        acc
    };
    let before = sum(&g.rasterize());
    g.host.set_focus(Some("sp"), &mut g.doc, &mut g.dispatcher);
    // Step up to the max (3); the up button becomes disabled-styled.
    for _ in 0..3 {
        g.key(KeyInput::new(keys::ARROW_UP, 0));
    }
    assert_eq!(as_spin(&g, "sp").value(), 3.0);
    g.relayout();
    let after = sum(&g.rasterize());
    assert!(
        before != after,
        "reaching max must restyle the up button (value-driven pixels)"
    );
}

/// Disabled spinbox swallows interaction.
#[test]
fn disabled_swallows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sp", Box::new(Spinbox::new(0.0, 10.0, 5.0).disabled(true)));
    g.relayout();
    let root = g.host.root_of("sp").unwrap();
    let up = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "up").unwrap()
    };
    g.left_click(up.x + up.width / 2.0, up.y + up.height / 2.0);
    assert!(g.process().is_empty());
    assert_eq!(as_spin(&g, "sp").value(), 5.0);
}
