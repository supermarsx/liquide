//! `<lq-chip>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
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
