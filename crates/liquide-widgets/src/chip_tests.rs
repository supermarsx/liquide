//! `<lq-chip>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::{KeyInput, WidgetBehavior};
use crate::chip::{Chip, CHANGED_ACTION, REMOVE_ACTION};
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 280;
const H: u32 = 120;

fn as_chip<'a>(g: &'a Gallery, id: &str) -> &'a Chip {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Chip>()
        .unwrap()
}

/// Clicking the × box of a removable chip emits Remove.
#[test]
fn remove_hits_the_x_box() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ch", Box::new(Chip::new("Tag").removable(true)));
    g.relayout();
    let root = g.host.root_of("ch").unwrap();
    let x = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "remove").expect("remove (×) box")
    };
    g.left_click(x.x + x.width / 2.0, x.y + x.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, REMOVE_ACTION);
}

/// NO-FAKE-GREEN tooth: clicking the chip BODY (the label, away from the ×) does
/// NOT remove — the × hit is read from its real laid-out box, not "right side of
/// the chip" by a constant.
#[test]
fn body_click_does_not_remove() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ch", Box::new(Chip::new("Removable").removable(true)));
    g.relayout();
    let root = g.host.root_of("ch").unwrap();
    let label = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "label").expect("label box")
    };
    // Click the centre of the LABEL (definitely not inside the × box).
    g.left_click(label.x + label.width / 2.0, label.y + label.height / 2.0);
    let a = g.process();
    assert!(
        a.iter().all(|act| act.name != REMOVE_ACTION),
        "clicking the body must not remove (got {a:?})"
    );
}

/// A selectable chip toggles selection on a body click; emits Changed.
#[test]
fn selectable_chip_toggles() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ch", Box::new(Chip::new("Filter").selectable(true)));
    g.relayout();
    assert!(!as_chip(&g, "ch").is_selected());

    let root = g.host.root_of("ch").unwrap();
    let label = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "label").unwrap()
    };
    g.left_click(label.x + label.width / 2.0, label.y + label.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("true"));
    assert!(as_chip(&g, "ch").is_selected());
}

/// :selected restyles the chip's pixels.
#[test]
fn selected_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ch", Box::new(Chip::new("Pick").selectable(true)));
    g.relayout();
    let root = g.host.root_of("ch").unwrap();
    let r = g.box_of(root).unwrap();
    let (cx, cy) = ((r.x + 4.0) as u32, (r.y + r.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    // Toggle via keyboard (Space) to avoid double-click coalescing concerns.
    g.host.set_focus(Some("ch"), &mut g.doc, &mut g.dispatcher);
    let a = g.key(KeyInput::new(keys::SPACE, 0));
    assert_eq!(a.len(), 1);
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(before != after, ":selected must restyle the chip");
}

/// Delete key removes a removable chip.
#[test]
fn delete_key_removes() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ch", Box::new(Chip::new("Tag").removable(true)));
    g.relayout();
    g.host.set_focus(Some("ch"), &mut g.doc, &mut g.dispatcher);
    let a = g.key(KeyInput::new(keys::DELETE, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, REMOVE_ACTION);
}

/// A plain (non-interactive) chip is inert and not focusable.
#[test]
fn plain_chip_is_inert() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ch", Box::new(Chip::new("Static")));
    g.relayout();
    assert!(!as_chip(&g, "ch").is_removable());
    let root = g.host.root_of("ch").unwrap();
    let r = g.box_of(root).unwrap();
    g.left_click(r.x + r.width / 2.0, r.y + r.height / 2.0);
    assert!(g.process().is_empty(), "a plain chip emits nothing on click");
}

// ── added: per-state styling proofs ───────────────────────────────────────

// NOTE (reported CSS gap): chip `:hover` is NOT pixel-provable in the gallery.
// `lq-chip { background: var(--widget-bg-elevated, #3f3f46) }` and
// `lq-chip:hover { background: var(--widget-bg-hover-solid, #52525b) }` both
// resolve to #3f3f46 here: `--widget-bg-elevated` is undefined (-> its #3f3f46
// fallback) and `--widget-bg-hover-solid` is `var(--bg-elevated, #3f3f46)` with
// `--bg-elevated` also undefined in the widget layer (it lives in variables.css,
// which the gallery does not load) -> #3f3f46. Resting == hover, so no delta.
// A hover pixel test would be fake-green; deferred to the coordinator (define
// `--widget-bg-elevated` distinct from the hover token, or point the resting
// chip bg at a token that differs from `--widget-bg-hover-solid`). The chip's
// hover STATE is still exercised by the behavior path (is_hovered round-trip)
// elsewhere; only the visible delta is blocked.

/// The remove (×) affordance is a real element with its OWN background
/// (`lq-chip-remove { background: rgba(255,255,255,0.10) }`) painted over the
/// chip body — so the × box centre differs from the chip-body fill. This proves
/// the remove affordance actually paints (not just lays out).
#[test]
fn remove_box_paints_distinct_from_body() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ch", Box::new(Chip::new("Tag").removable(true)));
    g.relayout();
    let root = g.host.root_of("ch").unwrap();
    let (label, rem) = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (
            q.box_of_part(root, "label").expect("label box"),
            q.box_of_part(root, "remove").expect("remove box"),
        )
    };
    let fb = g.rasterize();
    let body_px = Gallery::pixel(&fb, (label.x + 2.0) as u32, (label.y + label.height / 2.0) as u32);
    let x_px = Gallery::pixel(&fb, (rem.x + rem.width / 2.0) as u32, (rem.y + rem.height / 2.0) as u32);
    assert!(rem.width > 0.0 && rem.height > 0.0, "× reserves a real box");
    assert!(
        body_px != x_px,
        "the × box paints its own fill over the body (body {body_px:?} × {x_px:?})"
    );
}

/// Hovering the × box restyles it via the dispatcher-driven `:hover`
/// (`lq-chip-remove:hover { background: rgba(255,255,255,0.25) }`). The pointer
/// is parked over the × box and the scene restyled WITHOUT a behavior re-render
/// (which would reset the DOM hover pseudo), so the dispatcher's HOVER on the ×
/// node lands in the pixels.
#[test]
fn remove_box_hover_restyles() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ch", Box::new(Chip::new("Tag").removable(true)));
    g.relayout();
    let root = g.host.root_of("ch").unwrap();
    let rem = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "remove").expect("remove box")
    };
    let (cx, cy) = ((rem.x + rem.width / 2.0) as u32, (rem.y + rem.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    // Move the pointer onto the × — the dispatcher sets HOVER on the × node.
    // Do NOT process() (a chip re-render would drop the dispatcher's pseudo);
    // relayout restyles directly from the DOM's hover flag.
    g.pointer_move(rem.x + rem.width / 2.0, rem.y + rem.height / 2.0);
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(before != after, "hovering the × must restyle it (before {before:?} after {after:?})");
}

/// A selected chip paints the blue accent fill (`lq-chip:checked { background:
/// accent; border-color: accent }`) — deepens the flag-only selected test to a
/// colour assertion.
#[test]
fn selected_chip_paints_accent() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ch", Box::new(Chip::new("Pick").selected(true)));
    g.relayout();
    let root = g.host.root_of("ch").unwrap();
    let r = g.box_of(root).unwrap();
    let px = Gallery::pixel(&g.rasterize(), (r.x + 3.0) as u32, (r.y + r.height / 2.0) as u32);
    assert!(as_chip(&g, "ch").is_selected());
    assert!(px.b > px.r, "selected chip must paint the blue-dominant accent (got {px:?})");
}

/// A disabled chip swallows clicks AND dims its pixels (`lq-chip:disabled
/// { opacity: 0.5 }`). It is also dropped from the focus ring.
#[test]
fn disabled_chip_swallows_and_dims() {
    // Swallow: a disabled selectable chip ignores a body click.
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ch", Box::new(Chip::new("No").selectable(true).disabled(true)));
    g.relayout();
    let root = g.host.root_of("ch").unwrap();
    let r = g.box_of(root).unwrap();
    g.left_click(r.x + r.width / 2.0, r.y + r.height / 2.0);
    assert!(g.process().is_empty(), "disabled chip emits nothing");
    assert!(!as_chip(&g, "ch").is_selected(), "disabled chip does not toggle");
    assert!(!as_chip(&g, "ch").focusable(), "disabled chip is not focusable");

    // Dim: compare an enabled vs disabled chip centre.
    let mut on = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    on.mount("ch", Box::new(Chip::new("No").selectable(true)));
    on.relayout();
    let onr = on.box_of(on.host.root_of("ch").unwrap()).unwrap();
    let (sx, sy) = ((onr.x + 3.0) as u32, (onr.y + onr.height / 2.0) as u32);
    let on_px = Gallery::pixel(&on.rasterize(), sx, sy);
    let off_px = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(on_px != off_px, ":disabled must dim the chip (enabled {on_px:?} disabled {off_px:?})");
}
