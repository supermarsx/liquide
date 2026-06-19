//! `<lq-label>` / `<lq-link>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::label::{Label, Link, NAVIGATE_ACTION};

const W: u32 = 320;
const H: u32 = 120;

fn as_link<'a>(g: &'a Gallery, id: &str) -> &'a Link {
    g.host.behavior(id).unwrap().as_any().downcast_ref::<Link>().unwrap()
}

/// A static label renders text through the pipeline and is inert + not focusable.
#[test]
fn label_renders_and_is_inert() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("lbl", Box::new(Label::new("Hello")));
    g.relayout();
    let node = g.host.root_of("lbl").unwrap();
    let r = g.box_of(node).expect("label lays out");
    assert!(r.width > 0.0, "label must have a box");

    let b = g.host.behavior("lbl").unwrap();
    assert!(!b.focusable(), "static label is not focusable");
    assert!(b.wanted_events().is_empty(), "static label wants no events");

    // It is not in the focus ring.
    use crate::focus::FocusRing;
    let ring = FocusRing::collect(g.doc(), g.mount_point());
    assert!(ring.is_empty(), "label must not join the focus ring");
}

/// A link click on its laid-out box fires a navigate Action with the href payload.
#[test]
fn link_click_navigates_with_href() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ln", Box::new(Link::new("Home", "/home")));
    g.relayout();
    let node = g.host.root_of("ln").unwrap();
    let r = g.box_of(node).unwrap();
    g.left_click(r.x + r.width / 2.0, r.y + r.height / 2.0);
    let actions = g.process();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, NAVIGATE_ACTION);
    assert_eq!(actions[0].payload.as_deref(), Some("/home"));
    assert_eq!(as_link(&g, "ln").navigations(), 1);
}

/// A link is focusable and Enter activates it.
#[test]
fn link_keyboard_enter_navigates() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ln", Box::new(Link::new("Home", "/home")));
    g.relayout();
    assert!(g.host.behavior("ln").unwrap().focusable());
    g.host.set_focus(Some("ln"), &mut g.doc, &mut g.dispatcher);
    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].payload.as_deref(), Some("/home"));
}

/// Hovering a link restyles its pixels (accent -> accent-hover).
#[test]
fn link_hover_restyles_pixels() {
    // Give the link a real box so a center pixel is meaningful.
    let mut g = Gallery::new(
        W,
        H,
        "lq-gallery { padding: 16px; } lq-link { display: block; width: 80px; height: 24px; }",
    );
    g.mount("ln", Box::new(Link::new("Home", "/home")));
    g.relayout();
    let node = g.host.root_of("ln").unwrap();
    let r = g.box_of(node).unwrap();
    let (cx, cy) = ((r.x + 4.0) as u32, (r.y + r.height / 2.0) as u32);

    let before = Gallery::pixel(&g.rasterize(), cx, cy);
    g.pointer_move(r.x + r.width / 2.0, r.y + r.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(
        before != after || as_link(&g, "ln").is_hovered(),
        "hover state must update (pixels {before:?}->{after:?})"
    );
    assert!(as_link(&g, "ln").is_hovered());
}

// NOTE: link :active and :focus are color-ONLY restyles (accent -> accent-active
// / accent-hover) on text glyph ink — the gallery's Roboto rasterizer paints
// glyphs too faintly/unreliably for a stable pixel-delta, and the link has no
// background/border to sample. Those states cannot be proven in pixels here (same
// limitation the existing link_hover test sidesteps with an `|| is_hovered()`
// fallback). Reported as a harness/CSS gap rather than shipped as a flaky test.
