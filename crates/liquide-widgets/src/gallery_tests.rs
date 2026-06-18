//! End-to-end S0 infrastructure-validation tests.
//!
//! These drive the REAL pipeline (style -> layout -> paint -> raster) and the
//! REAL [`EventDispatcher`](liquide_hit_test::EventDispatcher) through the
//! [`Gallery`](crate::gallery::Gallery) harness, proving the shared
//! infrastructure (WidgetHost + WidgetBehavior + LayoutQuery + focus + event
//! injection) works end-to-end on a single reference `<lq-box>`. No fake-green:
//! the gap is asserted before wiring (the geometry tooth), and pixel/action
//! assertions go through the actual render + dispatch path.
#![cfg(test)]

use liquide_compositor::pixel::Color;

use crate::behavior::WidgetBehavior;
use crate::gallery::Gallery;
use crate::reference::ReferenceBox;

const W: u32 = 320;
const H: u32 = 200;

/// The `<lq-box>` is positioned by giving the gallery a top padding so the box
/// sits at a known, non-origin location — the click point is derived from the
/// LAID-OUT box, not from this constant (we only use it to aim the pointer).
fn gallery_with_box(action: &str) -> Gallery {
    let mut g = Gallery::new(
        W,
        H,
        // Pad the gallery so the box is offset from (0,0); the test reads the
        // real laid-out rect to aim, never assuming the box is at the origin.
        "lq-gallery { padding: 24px; }",
    );
    g.mount("ref-box", Box::new(ReferenceBox::new(action)));
    g.relayout();
    g
}

fn center_of(g: &Gallery) -> (f32, f32) {
    let node = g.host.root_of("ref-box").expect("box mounted");
    let r = g.box_of(node).expect("box laid out");
    (r.x + r.width / 2.0, r.y + r.height / 2.0)
}

/// PROOF 1 — the reference `<lq-box>` renders a real, non-blank paint box at the
/// CSS-driven location through the full pipeline + rasterizer.
#[test]
fn reference_box_renders_a_paint_box_through_real_pipeline() {
    let mut g = gallery_with_box("ping");
    let node = g.host.root_of("ref-box").unwrap();
    let rect = g.box_of(node).expect("box must have a layout box");

    // CSS (widgets.css) — not Rust — set the 120x40 box size. If geometry were
    // hardcoded in Rust this would not track the stylesheet.
    assert!(
        (rect.width - 120.0).abs() < 2.0 && (rect.height - 40.0).abs() < 2.0,
        "box size must come from widgets.css (got {}x{})",
        rect.width,
        rect.height
    );
    // The gallery padding pushes the box away from the origin.
    assert!(
        rect.x >= 23.0 && rect.y >= 23.0,
        "box must be offset by the gallery padding (at {},{})",
        rect.x,
        rect.y
    );

    let fb = g.rasterize();
    let cx = (rect.x + rect.width / 2.0) as u32;
    let cy = (rect.y + rect.height / 2.0) as u32;
    let px = Gallery::pixel(&fb, cx, cy);
    // Background is rgb(39,39,42) (--widget-bg). Assert the box actually painted
    // (non-transparent, and the dark grey fill, not the black backdrop).
    assert!(px.a > 0, "box center must be painted (alpha {})", px.a);
    assert!(
        px.r > 10 && px.r < 120,
        "box fill should be the mid-grey --widget-bg, got {:?}",
        px
    );
}

/// PROOF 2 — a scripted click on the box's LAID-OUT rect dispatches a
/// WidgetAction through the real dispatcher + host.
#[test]
fn click_on_layout_box_dispatches_an_action() {
    let mut g = gallery_with_box("ping");
    let (cx, cy) = center_of(&g);

    g.left_click(cx, cy);
    let actions = g.process();

    assert_eq!(actions.len(), 1, "exactly one action from the click");
    assert_eq!(actions[0].widget, "ref-box");
    assert_eq!(actions[0].name, "ping");

    // The behavior counted the click and recorded the laid-out width it hit.
    let b = g.host.behavior("ref-box").unwrap();
    let rb = downcast(b);
    assert_eq!(rb.clicks(), 1);
}

/// PROOF 3 (the NO-FAKE-GREEN tooth) — interaction geometry is read from the
/// LAYOUT box, not a constant.
///
/// The box is restyled to an UNUSUAL CSS width (197px) that a plausible
/// hardcoded constant would not match, so the behavior can only report it by
/// actually reading the laid-out box. We assert:
///   1. the behavior observes that exact 197px CSS width (a constant fails), and
///   2. a click inside the box (which a 120px constant would think is OUTSIDE)
///      DOES fire — proving the in-bounds test uses the real, wider box.
#[test]
fn interaction_geometry_comes_from_layout_not_a_constant() {
    // 197px is deliberately not a round value a constant-based impl would guess.
    let mut g = Gallery::new(
        W,
        H,
        "lq-gallery { padding: 24px; } lq-box { width: 197px; }",
    );
    g.mount("ref-box", Box::new(ReferenceBox::new("ping")));
    g.relayout();

    let node = g.host.root_of("ref-box").unwrap();
    let rect = g.box_of(node).unwrap();
    assert!(
        (rect.width - 197.0).abs() < 2.0,
        "precondition: CSS override must widen the box to 197px (got {})",
        rect.width
    );

    // Click at x = 24 + 160 = 184: INSIDE the real 197px box, but OUTSIDE a
    // box that (wrongly) assumed the default 120px width (24..144). A
    // constant-based hit test would reject this click; the layout-based one
    // accepts it.
    let inside_only_if_layout_used = rect.x + 160.0;
    let oy = rect.y + rect.height / 2.0;
    g.left_click(inside_only_if_layout_used, oy);
    let actions = g.process();
    assert_eq!(
        actions.len(),
        1,
        "a click inside the REAL (wide) box must fire — a 120px constant would \
         reject x={inside_only_if_layout_used}"
    );

    // And the behavior must have observed the exact CSS-driven 197px width.
    let rb = downcast(g.host.behavior("ref-box").unwrap());
    let seen = rb
        .last_seen_box_width()
        .expect("behavior must have read a width from layout");
    assert!(
        (seen - 197.0).abs() < 1.0,
        "behavior must observe the LAID-OUT 197px width (got {seen}) — a \
         hardcoded constant would not match this CSS-driven size"
    );

    // A click truly OUTSIDE the real box (past 197px) must NOT fire.
    let outside_x = rect.x + rect.width + 30.0;
    assert!((outside_x as u32) < W, "outside point must stay on-screen");
    g.left_click(outside_x, oy);
    let actions = g.process();
    assert!(
        actions.is_empty(),
        "a click past the laid-out box edge must not fire (got {actions:?})"
    );
}

/// PROOF 4 — a `:hover` restyle reconciled by the host actually changes the
/// rasterized pixels (CSS-driven interactivity round-trips through the pipeline).
#[test]
fn hover_restyles_the_box_in_pixels() {
    let mut g = gallery_with_box("ping");
    let node = g.host.root_of("ref-box").unwrap();
    let rect = g.box_of(node).unwrap();
    let cx = (rect.x + rect.width / 2.0) as u32;
    let cy = (rect.y + rect.height / 2.0) as u32;

    let before: Color = Gallery::pixel(&g.rasterize(), cx, cy);

    // Move the pointer onto the box: the dispatcher sets :hover on the DOM node
    // AND queues a MouseEnter; processing flips the behavior's hovered state and
    // re-renders (pseudo_if HOVER). Either way the DOM now carries :hover.
    let (fx, fy) = center_of(&g);
    g.pointer_move(fx, fy);
    let _ = g.process();
    g.relayout();

    let after: Color = Gallery::pixel(&g.rasterize(), cx, cy);

    assert!(
        before != after,
        "hover must restyle the box (before {before:?} == after {after:?})"
    );
    // The hover background (--widget-bg-hover) is a light overlay, so the fill
    // should brighten relative to the resting mid-grey.
    assert!(
        after.r >= before.r,
        "hovered fill should be at least as bright as resting (before {before:?}, after {after:?})"
    );

    // The behavior also reflects the hover state.
    let rb = downcast(g.host.behavior("ref-box").unwrap());
    assert!(rb.is_hovered(), "behavior must track :hover");
}

/// PROOF 5 — the focus ring + dispatcher focus plumbing work: the reference box
/// joins the ring (data-focusable) and the host can focus it.
#[test]
fn reference_box_joins_focus_ring_and_can_be_focused() {
    use crate::focus::FocusRing;

    let mut g = gallery_with_box("ping");
    let mount = g.mount_point();
    let ring = FocusRing::collect(g.doc(), mount);
    let node = g.host.root_of("ref-box").unwrap();
    assert_eq!(ring.order(), &[node], "reference box must be in the focus ring");

    g.host
        .set_focus(Some("ref-box"), &mut g.doc, &mut g.dispatcher);
    assert_eq!(g.host.focused(), Some("ref-box"));
    assert_eq!(
        g.dispatcher.focus(),
        Some(node),
        "dispatcher focus must mirror the host"
    );
}

/// PROOF 6 (the F1 fix, end-to-end in REAL CSS) — `background: var(--x)`
/// shorthand paints the fill through the FULL pipeline.
///
/// The style engine used to expand the `background` shorthand BEFORE `var()`
/// substitution, so `background: var(--x)` reached the shorthand expander as an
/// unclassifiable `var(--x)` token and the fill was DROPPED (only text painted).
/// `background-color: var(--x)` worked because it is not a shorthand. The F1 fix
/// (cascade.rs, commit a579420) defers expansion of any shorthand whose value
/// text contains `var()` so the var is resolved and re-parsed into longhands at
/// apply time — matching `background-color`.
///
/// This is the FULL-PIPELINE regression guard (the F1 commit added STYLE-ENGINE
/// unit tests; this exercises style -> layout -> paint -> raster on the real
/// widgets.css base layer + a custom `--probe` token, asserting the rasterized
/// pixel carries the probe color). It is RED if the F1 fix regresses: a dropped
/// fill leaves the box backdrop (black/transparent), not the probe color.
///
/// The probe color rgb(170,80,200) is deliberately unlike any default widget
/// token, so a coincidental default match cannot make this pass.
#[test]
fn background_shorthand_with_var_paints_fill_in_real_pipeline() {
    // A distinctive probe color that matches no widgets.css default token.
    const PROBE: (u8, u8, u8) = (170, 80, 200); // #aa50c8

    // `extra_css` is unlayered, so it wins over the @layer widgets base rule for
    // `lq-box`. We define `--probe` on :root and fill the box via the SHORTHAND.
    let mut g = Gallery::new(
        W,
        H,
        "lq-gallery { padding: 24px; } \
         :root { --probe: #aa50c8; } \
         lq-box { background: var(--probe); }",
    );
    g.mount("ref-box", Box::new(ReferenceBox::new("ping")));
    g.relayout();

    let node = g.host.root_of("ref-box").unwrap();
    let rect = g.box_of(node).expect("box must have a layout box");
    let cx = (rect.x + rect.width / 2.0) as u32;
    let cy = (rect.y + rect.height / 2.0) as u32;

    let px = Gallery::pixel(&g.rasterize(), cx, cy);

    // The shorthand+var fill MUST have painted the probe color — NOT been dropped
    // (which would leave the transparent/black backdrop showing through).
    assert!(px.a > 0, "shorthand+var fill must paint (alpha {})", px.a);
    assert!(
        (px.r as i32 - PROBE.0 as i32).abs() <= 2
            && (px.g as i32 - PROBE.1 as i32).abs() <= 2
            && (px.b as i32 - PROBE.2 as i32).abs() <= 2,
        "`background: var(--probe)` must paint the probe color {PROBE:?} through \
         the real pipeline — got {px:?} (F1 shorthand+var fill regressed: dropped)"
    );
}

/// PROOF 7 (the F1 invariant the cleanup relies on) — `background: var(--x)` and
/// `background-color: var(--x)` produce the SAME rasterized fill.
///
/// This is the contract that makes the `background-color: var()` workaround
/// throughout widgets.css/themes OPTIONAL: both spellings now resolve to an
/// identical fill, so switching a workaround to the `background:` shorthand is a
/// no-op visually. If this diverges, the F1 fix regressed and the workaround
/// would once again be REQUIRED — so a CSS cleanup must not be done.
#[test]
fn background_shorthand_var_equals_background_color_var_in_pixels() {
    let probe_at_center = |decl: &str| -> Color {
        let css = format!(
            "lq-gallery {{ padding: 24px; }} \
             :root {{ --probe: #aa50c8; }} \
             lq-box {{ {decl} }}"
        );
        let mut g = Gallery::new(W, H, &css);
        g.mount("ref-box", Box::new(ReferenceBox::new("ping")));
        g.relayout();
        let node = g.host.root_of("ref-box").unwrap();
        let rect = g.box_of(node).expect("box laid out");
        let cx = (rect.x + rect.width / 2.0) as u32;
        let cy = (rect.y + rect.height / 2.0) as u32;
        Gallery::pixel(&g.rasterize(), cx, cy)
    };

    let shorthand = probe_at_center("background: var(--probe);");
    let longhand = probe_at_center("background-color: var(--probe);");

    assert_eq!(
        (shorthand.r, shorthand.g, shorthand.b, shorthand.a),
        (longhand.r, longhand.g, longhand.b, longhand.a),
        "`background: var()` ({shorthand:?}) must rasterize the SAME fill as the \
         `background-color: var()` workaround ({longhand:?}) — this equality is \
         what makes the workaround optional (cleanup safe, no visual shift)"
    );
    // And it must be the real probe, not a coincidental default match.
    assert!(
        (longhand.r as i32 - 170).abs() <= 2
            && (longhand.g as i32 - 80).abs() <= 2
            && (longhand.b as i32 - 200).abs() <= 2,
        "the shared fill must be the probe color, got {longhand:?}"
    );
}

/// Downcast a `&dyn WidgetBehavior` to `&ReferenceBox` via the trait's `as_any`
/// hook (safe; no transmute).
fn downcast(b: &dyn WidgetBehavior) -> &ReferenceBox {
    b.as_any()
        .downcast_ref::<ReferenceBox>()
        .expect("mounted behavior is a ReferenceBox")
}
