//! `<lq-gradient-editor>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::gradient_editor::{GradientEditor, Stop, CHANGED_ACTION};
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 360;
const H: u32 = 160;

fn gallery_with(e: GradientEditor) -> Gallery {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ge", Box::new(e));
    g.relayout();
    g
}

fn as_editor(g: &Gallery) -> &GradientEditor {
    g.host
        .behavior("ge")
        .unwrap()
        .as_any()
        .downcast_ref::<GradientEditor>()
        .unwrap()
}

fn bar(g: &Gallery) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("ge").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "bar").expect("bar box")
}

/// The bar renders a real CSS-sized box and the default two stops resolve from
/// their laid-out positions.
#[test]
fn renders_bar_and_default_stops() {
    let mut g = gallery_with(GradientEditor::new());
    let b = bar(&g);
    assert!((b.width - 280.0).abs() < 3.0, "bar width from CSS (got {})", b.width);
    assert_eq!(as_editor(&g).stop_count(), 2, "two default stops");
    let root = g.host.root_of("ge").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let s0 = q.box_of_part(root, "stop-0").expect("stop-0");
    let s1 = q.box_of_part(root, "stop-1").expect("stop-1");
    // Stop 0 at the left edge, stop 1 at the right edge of the bar.
    assert!((s0.x + s0.width / 2.0 - b.x).abs() < 12.0, "stop-0 at bar left");
    assert!((s1.x + s1.width / 2.0 - (b.x + b.width)).abs() < 12.0, "stop-1 at bar right");
    let fb = g.rasterize();
    assert!(Gallery::pixel(&fb, (b.x + b.width / 2.0) as u32, (b.y + b.height / 2.0) as u32).a > 0);
}

/// Clicking the bar (away from a stop) ADDS a stop at the fraction-along-x of the
/// LAID-OUT bar — not a constant.
#[test]
fn click_bar_adds_stop_at_layout_fraction() {
    let mut g = gallery_with(GradientEditor::new());
    let b = bar(&g);
    let before = as_editor(&g).stop_count();
    // Press at ~50% of the bar.
    g.mouse_down(b.x + b.width * 0.5, b.y + b.height / 2.0);
    let actions = g.process();
    assert_eq!(actions.last().unwrap().name, CHANGED_ACTION);
    g.mouse_up(b.x + b.width * 0.5, b.y + b.height / 2.0);
    let _ = g.process();
    assert_eq!(as_editor(&g).stop_count(), before + 1, "a stop was added");
    let sel = as_editor(&g).selected_stop().expect("selected new stop");
    assert!((sel.pos - 0.5).abs() <= 0.04, "new stop near 50% (got {})", sel.pos);
}

/// The added-stop position derives from the LAID-OUT bar, not a constant: a
/// CSS-widened bar changes the mapping. A click at bar.x+150 is the midpoint of
/// a 300px bar (->0.5) but ~0.54 of a wrongly-assumed 280px bar.
#[test]
fn add_position_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        420,
        160,
        "lq-gallery { padding: 16px; } lq-gradient-bar { width: 300px; }",
    );
    g.mount("ge", Box::new(GradientEditor::new()));
    g.relayout();
    let b = bar(&g);
    assert!((b.width - 300.0).abs() < 3.0, "precondition 300px bar (got {})", b.width);
    g.mouse_down(b.x + 150.0, b.y + b.height / 2.0);
    let _ = g.process();
    g.mouse_up(b.x + 150.0, b.y + b.height / 2.0);
    let _ = g.process();
    let sel = as_editor(&g).selected_stop().expect("new stop");
    assert!(
        (sel.pos - 0.5).abs() <= 0.03,
        "stop pos must derive from the REAL 300px bar (got {}; a 280px constant gives ~0.54)",
        sel.pos
    );
}

/// Dragging a stop moves it along the bar (position from the laid-out bar).
#[test]
fn drag_moves_selected_stop() {
    // Start with a middle stop to drag.
    let mut g = gallery_with(GradientEditor::with_stops(vec![
        Stop { pos: 0.0, color: (0, 0, 0) },
        Stop { pos: 0.5, color: (255, 0, 0) },
        Stop { pos: 1.0, color: (255, 255, 255) },
    ]));
    let root = g.host.root_of("ge").unwrap();
    let mid = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "stop-1").expect("middle stop")
    };
    let b = bar(&g);
    // Press on the middle stop's handle, drag toward 25%.
    g.mouse_down(mid.x + mid.width / 2.0, mid.y + mid.height / 2.0);
    let _ = g.process();
    assert!(as_editor(&g).is_dragging(), "dragging the stop");
    g.pointer_move(b.x + b.width * 0.25, b.y + b.height / 2.0);
    let _ = g.process();
    let sel = as_editor(&g).selected_stop().expect("selected");
    assert!((sel.pos - 0.25).abs() <= 0.04, "stop moved to ~25% (got {})", sel.pos);
    g.mouse_up(b.x + b.width * 0.25, b.y + b.height / 2.0);
    let _ = g.process();
    assert!(!as_editor(&g).is_dragging(), "release clears dragging");
}

/// Clicking a palette swatch recolours the selected stop (reusing the swatch
/// model); the recolour reads the swatch's laid-out box.
#[test]
fn swatch_recolours_selected_stop() {
    let mut g = gallery_with(GradientEditor::new());
    // Select stop-0 (the black left stop) by clicking its handle.
    let root = g.host.root_of("ge").unwrap();
    let s0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "stop-0").expect("stop-0")
    };
    g.mouse_down(s0.x + s0.width / 2.0, s0.y + s0.height / 2.0);
    let _ = g.process();
    g.mouse_up(s0.x + s0.width / 2.0, s0.y + s0.height / 2.0);
    let _ = g.process();
    assert_eq!(as_editor(&g).selected_index(), 0, "stop-0 selected");

    // Click the first palette swatch (red).
    let sw = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "swatch-0").expect("swatch-0")
    };
    g.mouse_down(sw.x + sw.width / 2.0, sw.y + sw.height / 2.0);
    let actions = g.process();
    assert_eq!(actions.last().unwrap().name, CHANGED_ACTION, "recolour emits Changed");
    let stop = as_editor(&g).stops()[0];
    assert_eq!(stop.color, (239, 68, 68), "stop-0 recoloured to palette red");
}

/// Keyboard nudges the selected stop and Delete removes it (keeping >= 2 stops).
#[test]
fn keyboard_nudge_and_delete() {
    let mut g = gallery_with(GradientEditor::with_stops(vec![
        Stop { pos: 0.0, color: (0, 0, 0) },
        Stop { pos: 0.5, color: (255, 0, 0) },
        Stop { pos: 1.0, color: (255, 255, 255) },
    ]));
    // Select the middle stop via its handle.
    let root = g.host.root_of("ge").unwrap();
    let mid = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "stop-1").expect("middle stop")
    };
    g.mouse_down(mid.x + mid.width / 2.0, mid.y + mid.height / 2.0);
    let _ = g.process();
    g.mouse_up(mid.x + mid.width / 2.0, mid.y + mid.height / 2.0);
    let _ = g.process();

    g.host.set_focus(Some("ge"), &mut g.doc, &mut g.dispatcher);
    let p0 = as_editor(&g).selected_stop().unwrap().pos;
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert!(as_editor(&g).selected_stop().unwrap().pos > p0, "Right nudges position up");

    // Delete removes the (3rd) stop, leaving 2.
    let before = as_editor(&g).stop_count();
    g.key(KeyInput::new(keys::DELETE, 0));
    assert_eq!(as_editor(&g).stop_count(), before - 1, "Delete removes a stop");

    // A second Delete must NOT drop below 2 stops.
    g.key(KeyInput::new(keys::DELETE, 0));
    assert_eq!(as_editor(&g).stop_count(), 2, "kept at least 2 stops");
}

/// Locate a stop's laid-out handle box by index.
fn stop_box(g: &Gallery, i: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("ge").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("stop-{i}")).expect("stop box")
}

/// Select a stop by clicking + releasing on its handle.
fn select_stop(g: &mut Gallery, i: usize) {
    let s = stop_box(g, i);
    let (cx, cy) = (s.x + s.width / 2.0, s.y + s.height / 2.0);
    g.mouse_down(cx, cy);
    let _ = g.process();
    g.mouse_up(cx, cy);
    let _ = g.process();
    g.relayout();
}

/// :checked/.selected restyles the selected stop's BORDER pixels (CSS recolours it
/// to the accent + adds a 1px box-shadow ring). Sample the stop handle's left edge
/// (the border band) before vs after selecting it.
#[test]
fn selected_stop_restyles_border_pixels() {
    let mut g = gallery_with(GradientEditor::with_stops(vec![
        Stop { pos: 0.0, color: (0, 0, 0) },
        Stop { pos: 0.5, color: (200, 30, 30) },
        Stop { pos: 1.0, color: (255, 255, 255) },
    ]));
    // Default selection is index 0; the MIDDLE stop (1) starts unselected.
    let s1 = stop_box(&g, 1);
    // Sample on the stop handle's left border band.
    let bx = (s1.x + 1.0) as u32;
    let by = (s1.y + s1.height / 2.0) as u32;
    let before = Gallery::pixel(&g.rasterize(), bx, by);

    select_stop(&mut g, 1);
    assert_eq!(as_editor(&g).selected_index(), 1, "middle stop selected");
    let after = Gallery::pixel(&g.rasterize(), bx, by);

    assert!(
        before != after,
        ":checked stop must restyle its border (before={before:?}, after={after:?})"
    );
}

/// The selected-stop ring MOVES with selection: selecting stop 2 lights stop 2 AND
/// reverts stop 0. Two deltas at two handle boxes prove the restyle tracks the
/// selected index, not a constant.
#[test]
fn selected_ring_moves_between_stops() {
    let mut g = gallery_with(GradientEditor::with_stops(vec![
        Stop { pos: 0.0, color: (10, 10, 10) },
        Stop { pos: 0.5, color: (10, 10, 10) },
        Stop { pos: 1.0, color: (10, 10, 10) },
    ]));
    // Default selection = stop 0.
    assert_eq!(as_editor(&g).selected_index(), 0);
    let s0 = stop_box(&g, 0);
    let s2 = stop_box(&g, 2);
    let s0_pt = ((s0.x + 1.0) as u32, (s0.y + s0.height / 2.0) as u32);
    let s2_pt = ((s2.x + 1.0) as u32, (s2.y + s2.height / 2.0) as u32);

    let s0_selected = Gallery::pixel(&g.rasterize(), s0_pt.0, s0_pt.1);
    let s2_unselected = Gallery::pixel(&g.rasterize(), s2_pt.0, s2_pt.1);

    select_stop(&mut g, 2);
    assert_eq!(as_editor(&g).selected_index(), 2);
    let s0_now = Gallery::pixel(&g.rasterize(), s0_pt.0, s0_pt.1);
    let s2_now = Gallery::pixel(&g.rasterize(), s2_pt.0, s2_pt.1);

    assert!(
        s0_selected != s0_now,
        "stop 0 must LOSE its selected ring (was {s0_selected:?}, now {s0_now:?})"
    );
    assert!(
        s2_unselected != s2_now,
        "stop 2 must GAIN the selected ring (was {s2_unselected:?}, now {s2_now:?})"
    );
}

/// Each stop handle's FILL is its colour (inline `background-color`), so two stops
/// of different colours paint different interiors. Sample each handle center.
#[test]
fn stop_fill_paints_its_color() {
    let mut g = gallery_with(GradientEditor::with_stops(vec![
        Stop { pos: 0.0, color: (250, 30, 30) },   // red-ish
        Stop { pos: 1.0, color: (30, 30, 250) },   // blue-ish
    ]));
    let s0 = stop_box(&g, 0);
    let s1 = stop_box(&g, 1);
    let p0 = Gallery::pixel(&g.rasterize(), (s0.x + s0.width / 2.0) as u32, (s0.y + s0.height / 2.0) as u32);
    let p1 = Gallery::pixel(&g.rasterize(), (s1.x + s1.width / 2.0) as u32, (s1.y + s1.height / 2.0) as u32);
    assert!(
        p0 != p1,
        "differently-coloured stops must paint different fills (s0={p0:?}, s1={p1:?})"
    );
    // And the red stop reads red-dominant, the blue stop blue-dominant.
    assert!(p0.r > p0.b, "stop-0 fill is red-dominant (got {p0:?})");
    assert!(p1.b > p1.r, "stop-1 fill is blue-dominant (got {p1:?})");
}

/// A palette swatch :hover restyles its border (CSS `lq-gradient-swatch:hover`
/// lights the border to the fg colour). Sample the swatch's top border band.
#[test]
fn swatch_hover_restyles_border() {
    let mut g = gallery_with(GradientEditor::new());
    let root = g.host.root_of("ge").unwrap();
    let sw = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "swatch-2").expect("swatch-2")
    };
    let bx = (sw.x + sw.width / 2.0) as u32;
    let by = (sw.y) as u32; // top edge / border band
    let before = Gallery::pixel(&g.rasterize(), bx, by);

    g.pointer_move(sw.x + sw.width / 2.0, sw.y + sw.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), bx, by);

    assert!(
        before != after,
        "swatch :hover must restyle its border (before={before:?}, after={after:?})"
    );
}

/// A palette swatch matching the selected stop's colour is :checked/.selected and
/// gets the accent border. After recolouring stop-0 to palette red (swatch-0),
/// swatch-0's border must differ from its un-selected baseline.
#[test]
fn selected_swatch_restyles_border() {
    let mut g = gallery_with(GradientEditor::new());
    let root = g.host.root_of("ge").unwrap();
    let sw0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "swatch-0").expect("swatch-0")
    };
    // Baseline: stop-0 is black, so swatch-0 (red) is NOT selected.
    let bx = (sw0.x + sw0.width / 2.0) as u32;
    let by = (sw0.y) as u32;
    let before = Gallery::pixel(&g.rasterize(), bx, by);

    // Select stop-0, then click swatch-0 to recolour it to palette red -> swatch-0
    // becomes the :checked swatch.
    select_stop(&mut g, 0);
    g.mouse_down(sw0.x + sw0.width / 2.0, sw0.y + sw0.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_editor(&g).stops()[0].color, (239, 68, 68), "stop-0 recoloured red");
    let after = Gallery::pixel(&g.rasterize(), bx, by);

    assert!(
        before != after,
        "the selected swatch must gain the accent border (before={before:?}, after={after:?})"
    );
}

/// Disabled editor ignores input.
#[test]
fn disabled_editor_ignores_input() {
    let mut g = gallery_with(GradientEditor::new().disabled(true));
    let b = bar(&g);
    let before = as_editor(&g).stop_count();
    g.mouse_down(b.x + b.width * 0.5, b.y + b.height / 2.0);
    let _ = g.process();
    g.mouse_up(b.x + b.width * 0.5, b.y + b.height / 2.0);
    let _ = g.process();
    assert_eq!(as_editor(&g).stop_count(), before, "disabled editor adds no stop");
}
