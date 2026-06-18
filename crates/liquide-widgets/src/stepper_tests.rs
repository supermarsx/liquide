//! `<lq-stepper>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::stepper::{Stepper, CHANGED_ACTION};

const W: u32 = 640;
const H: u32 = 240;

fn as_st<'a>(g: &'a Gallery, id: &str) -> &'a Stepper {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Stepper>()
        .unwrap()
}

fn steps() -> Vec<&'static str> {
    vec!["Account", "Profile", "Payment", "Confirm"]
}

fn mount(g: &mut Gallery, id: &str) {
    g.mount(id, Box::new(Stepper::new(steps())));
    g.relayout();
    g.host.set_focus(Some(id), &mut g.doc, &mut g.dispatcher);
}

/// Clicking Next advances one step + emits Changed(new index).
#[test]
fn next_advances_and_emits() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "st");
    assert_eq!(as_st(&g, "st").current(), 0);

    let root = g.host.root_of("st").unwrap();
    let next = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "next").expect("next box")
    };
    g.left_click(next.x + next.width / 2.0, next.y + next.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("1"));
    g.relayout();
    assert_eq!(as_st(&g, "st").current(), 1);
}

/// Back retreats; Back at step 0 is inert.
#[test]
fn back_retreats_and_is_inert_at_start() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("st", Box::new(Stepper::new(steps()).start_at(2)));
    g.relayout();
    let root = g.host.root_of("st").unwrap();
    let back = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "back").expect("back box")
    };
    g.left_click(back.x + back.width / 2.0, back.y + back.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("1"));
    g.relayout();
    assert_eq!(as_st(&g, "st").current(), 1);

    // Now go to 0 then try Back again — inert.
    g.mount("st", Box::new(Stepper::new(steps()).start_at(0)));
    g.relayout();
    let back0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "back").unwrap()
    };
    g.left_click(back0.x + 4.0, back0.y + back0.height / 2.0);
    let a = g.process();
    assert!(a.is_empty(), "Back at step 0 emits nothing");
    g.relayout();
    assert_eq!(as_st(&g, "st").current(), 0);
}

/// Clicking a reachable step marker jumps to it; an unreachable one is inert.
#[test]
fn click_reachable_step_jumps_unreachable_inert() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "st"); // current 0; reachable = 0 and 1 only.
    let root = g.host.root_of("st").unwrap();

    // Step 3 is NOT reachable from step 0 (can't skip ahead).
    let s3 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "step-3").expect("step-3 box")
    };
    g.left_click(s3.x + s3.width / 2.0, s3.y + s3.height / 2.0);
    let a = g.process();
    assert!(a.is_empty(), "jumping ahead to step 3 is inert");
    g.relayout();
    assert_eq!(as_st(&g, "st").current(), 0);

    // Step 1 IS reachable (the immediate next).
    let s1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "step-1").expect("step-1 box")
    };
    g.left_click(s1.x + s1.width / 2.0, s1.y + s1.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("1"));
    g.relayout();
    assert_eq!(as_st(&g, "st").current(), 1);

    // Now step 0 (a completed step) is reachable again — jump back.
    let s0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "step-0").expect("step-0 box")
    };
    g.left_click(s0.x + s0.width / 2.0, s0.y + s0.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("0"), "completed step is reachable");
}

/// NO-FAKE-GREEN tooth: the step hit reads each marker's REAL laid-out box. Widen
/// step 0's label so the markers are NOT uniform-pitch; clicking step 1's true
/// box still selects 1 (a `i * uniform_width` guess from step-0 would mis-map).
#[test]
fn step_hit_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        W,
        H,
        "lq-gallery { padding: 16px; } lq-step[data-part=\"step-0\"] lq-step-label { width: 180px; }",
    );
    mount(&mut g, "st");
    let root = g.host.root_of("st").unwrap();
    let s0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "step-0").expect("step-0 box")
    };
    let s1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "step-1").expect("step-1 box")
    };
    // The widened step 0 makes the markers non-uniform: step-1 starts well past
    // 2 * step-0.width-from-origin.
    assert!(
        s1.x - s0.x > s0.width,
        "precondition: non-uniform step pitch (s0.x={}, s1.x={}, s0.w={})",
        s0.x,
        s1.x,
        s0.width
    );
    g.left_click(s1.x + s1.width / 2.0, s1.y + s1.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("1"), "click in step-1's REAL box selects 1");
}

/// Keyboard: Right advances, Left retreats.
#[test]
fn keyboard_navigation() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "st");
    let a = g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(a[0].payload.as_deref(), Some("1"));
    g.relayout();
    assert_eq!(as_st(&g, "st").current(), 1);
    let a = g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert_eq!(a[0].payload.as_deref(), Some("0"));
    g.relayout();
    assert_eq!(as_st(&g, "st").current(), 0);
}

/// Advancing restyles pixels (the current marker fill moves to the new step).
#[test]
fn advance_changes_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("st", Box::new(Stepper::new(steps()).start_at(0)));
    g.relayout();
    let root = g.host.root_of("st").unwrap();
    let s1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "step-1").unwrap()
    };
    let (sx, sy) = ((s1.x + s1.width * 0.15) as u32, (s1.y + s1.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);

    g.mount("st", Box::new(Stepper::new(steps()).start_at(1)));
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "the current-step marker fill must move to step 1");
}
