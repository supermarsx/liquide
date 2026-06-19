//! `<lq-stepper>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::{KeyInput, WidgetBehavior};
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

// ── Added: deep visual-STATE / styling coverage (no fake-green) ──────────────

/// Resolve the marker (number-badge) box of a given step. The marker is the round
/// `lq-step-marker` under `step-<i>`; we find the step box and locate the marker
/// within it by walking the subtree.
fn marker_box(g: &Gallery, id: &str, step_idx: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let step = q.find_part(root, &format!("step-{step_idx}")).expect("step node");
    // The marker is the first descendant with data-part="marker".
    fn find_marker(doc: &liquide_dom::Document, node: liquide_dom::NodeId) -> Option<liquide_dom::NodeId> {
        if doc.get_attribute(node, "data-part").as_deref() == Some("marker") {
            return Some(node);
        }
        for &c in doc.children(node) {
            if let Some(m) = find_marker(doc, c) {
                return Some(m);
            }
        }
        None
    }
    let marker = find_marker(g.doc(), step).expect("step marker");
    q.box_of(marker).expect("marker box")
}

/// The current step's marker (accent fill) paints DISTINCTLY from an upcoming
/// step's marker (default bg). No-fake-green: removing `.current lq-step-marker`
/// makes the two markers identical.
#[test]
fn current_marker_differs_from_upcoming_marker() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "st"); // current = 0
    let m0 = marker_box(&g, "st", 0); // current
    let m2 = marker_box(&g, "st", 2); // upcoming
    let fb = g.rasterize();
    let cur = Gallery::pixel(&fb, (m0.x + m0.width / 2.0) as u32, (m0.y + m0.height / 2.0) as u32);
    let upc = Gallery::pixel(&fb, (m2.x + m2.width / 2.0) as u32, (m2.y + m2.height / 2.0) as u32);
    assert!(
        cur != upc,
        "the current marker (accent fill) must differ from an upcoming marker (cur {cur:?} upc {upc:?})"
    );
    // The current marker is the macOS-dark graphite accent (bright + neutral).
    assert!(Gallery::is_graphite_accent(cur), "current marker fill is the graphite accent (got {cur:?})");
}

/// A COMPLETED step's marker (`:checked`, accent-active fill) paints distinctly
/// from the current step's marker AND from an upcoming one — three visual classes.
#[test]
fn completed_marker_differs_from_current_and_upcoming() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("st", Box::new(Stepper::new(steps()).start_at(2)));
    g.relayout();
    let completed = marker_box(&g, "st", 0); // i < current → completed
    let current = marker_box(&g, "st", 2); // current
    let upcoming = marker_box(&g, "st", 3); // upcoming
    let fb = g.rasterize();
    let comp = Gallery::pixel(&fb, (completed.x + completed.width / 2.0) as u32, (completed.y + completed.height / 2.0) as u32);
    let cur = Gallery::pixel(&fb, (current.x + current.width / 2.0) as u32, (current.y + current.height / 2.0) as u32);
    let upc = Gallery::pixel(&fb, (upcoming.x + upcoming.width / 2.0) as u32, (upcoming.y + upcoming.height / 2.0) as u32);
    assert!(comp != upc, "completed marker differs from upcoming (comp {comp:?} upc {upc:?})");
    assert!(
        comp != cur,
        "completed (accent-active) marker differs from current (accent) marker (comp {comp:?} cur {cur:?})"
    );
}

/// The connector between steps fills (accent) once the step before it is completed.
/// No-fake-green: the same connector paints differently filled vs unfilled.
#[test]
fn step_connector_fills_with_progress() {
    // At step 0: the first connector is NOT filled.
    let mut g0 = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g0.mount("st", Box::new(Stepper::new(steps()).start_at(0)));
    g0.relayout();
    // The connector sits between the step-0 and step-1 markers. It is a thin (2px)
    // bar; sample it at the gap midpoint, scanning a vertical band to catch it.
    // Compute the gap x PER gallery (marker positions shift slightly between
    // states) and capture the connector's strongest pixel in each.
    // The connector is a flex-grow bar that sits AFTER step 0's label, just before
    // step 1's marker. Scan the whole horizontal gap between the markers across the
    // marker-centre band and keep the most-saturated opaque pixel (the bar).
    let connector_px = |g: &mut Gallery| -> liquide_compositor::pixel::Color {
        let s0 = marker_box(g, "st", 0);
        let s1 = marker_box(g, "st", 1);
        let fb = g.rasterize();
        let mut best = liquide_compositor::pixel::Color { r: 0, g: 0, b: 0, a: 0 };
        let cy = (s0.y + s0.height / 2.0) as u32;
        for y in cy.saturating_sub(2)..=(cy + 2) {
            for x in (s0.right() as u32 + 1)..(s1.x as u32) {
                let p = Gallery::pixel(&fb, x, y);
                if p.a > 0
                    && (p.r as u32 + p.g as u32 + p.b as u32)
                        > (best.r as u32 + best.g as u32 + best.b as u32)
                {
                    best = p;
                }
            }
        }
        best
    };
    let unfilled = connector_px(&mut g0);

    // At step 1: step 0 is completed → the first connector fills (accent).
    let mut g1 = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g1.mount("st", Box::new(Stepper::new(steps()).start_at(1)));
    g1.relayout();
    let filled = connector_px(&mut g1);

    assert!(
        unfilled != filled,
        "the connector before a completed step must change once that step is completed \
         (unfilled {unfilled:?} filled {filled:?})"
    );
    // The filled connector is the graphite accent (bright + neutral, opaque).
    assert!(
        Gallery::is_graphite_accent(filled),
        "the filled connector must paint the graphite accent colour (got {filled:?})"
    );
}

/// The step-marker badge actually PAINTS (it is the round number badge, not an
/// empty box): its centre pixel is opaque.
#[test]
fn step_marker_badge_paints() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "st");
    let m0 = marker_box(&g, "st", 0);
    let px = Gallery::pixel(&g.rasterize(), (m0.x + m0.width / 2.0) as u32, (m0.y + m0.height / 2.0) as u32);
    assert!(px.a > 0, "the step marker badge must paint (alpha {})", px.a);
    // The badge is round/sized from CSS (28px), not zero.
    assert!(m0.width >= 24.0 && m0.height >= 24.0, "marker badge is CSS-sized (got {}x{})", m0.width, m0.height);
}

/// The Next control is disabled (dimmed, opacity 0.4) on the LAST step and the
/// click is inert. The Back control is disabled on the FIRST step. Both reflect
/// the `:disabled` styling and gate the action.
#[test]
fn next_disabled_on_last_step_and_inert() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("st", Box::new(Stepper::new(steps()).start_at(3))); // last step
    g.relayout();
    let root = g.host.root_of("st").unwrap();
    let next = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "next").expect("next box")
    };
    g.left_click(next.x + next.width / 2.0, next.y + next.height / 2.0);
    let a = g.process();
    assert!(a.is_empty(), "Next on the last step is inert");
    g.relayout();
    assert_eq!(as_st(&g, "st").current(), 3, "stays on the last step");
}

/// The disabled Next control paints a DIMMED style vs an enabled Next — the
/// `:disabled { opacity }` rule lands in pixels. Compare the same control region
/// at step 3 (disabled) vs step 0 (enabled).
#[test]
fn disabled_next_renders_dimmed() {
    let mut g_en = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g_en.mount("st", Box::new(Stepper::new(steps()).start_at(0)));
    g_en.relayout();
    let en = {
        let root = g_en.host.root_of("st").unwrap();
        let q = LayoutQuery::new(g_en.hit_test_engine(), g_en.doc());
        q.box_of_part(root, "next").unwrap()
    };
    let fb_en = g_en.rasterize();

    let mut g_dis = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g_dis.mount("st", Box::new(Stepper::new(steps()).start_at(3))); // Next disabled
    g_dis.relayout();
    let dis = {
        let root = g_dis.host.root_of("st").unwrap();
        let q = LayoutQuery::new(g_dis.hit_test_engine(), g_dis.doc());
        q.box_of_part(root, "next").unwrap()
    };
    let fb_dis = g_dis.rasterize();
    assert!((en.x - dis.x).abs() < 1.0 && (en.width - dis.width).abs() < 1.0, "same Next geometry");
    // Scan the control band; the dimmed (opacity) Next must differ somewhere.
    let y = (en.y + en.height / 2.0) as u32;
    let mut differs = false;
    for x in (en.x as u32 + 2)..((en.x + en.width) as u32 - 2) {
        if Gallery::pixel(&fb_en, x, y) != Gallery::pixel(&fb_dis, x, y) {
            differs = true;
            break;
        }
    }
    assert!(differs, "the disabled Next control must render dimmed vs the enabled one");
}

/// A fully disabled stepper swallows every click and drops out of the focus ring.
#[test]
fn disabled_stepper_swallows_clicks() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("st", Box::new(Stepper::new(steps()).disabled(true)));
    g.relayout();
    let root = g.host.root_of("st").unwrap();
    let next = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "next").expect("next box")
    };
    g.left_click(next.x + next.width / 2.0, next.y + next.height / 2.0);
    let a = g.process();
    assert!(a.is_empty(), "a disabled stepper emits nothing");
    g.relayout();
    assert_eq!(as_st(&g, "st").current(), 0, "no advance");
    assert!(!as_st(&g, "st").focusable(), "a disabled stepper is not focusable");
}
