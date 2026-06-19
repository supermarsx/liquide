//! `<lq-ip-input>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::ip_input::{IpInput, CHANGED_ACTION};
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 280;
const H: u32 = 120;

fn as_ip<'a>(g: &'a Gallery, id: &str) -> &'a IpInput {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<IpInput>()
        .unwrap()
}

fn octet_box(g: &Gallery, root: liquide_dom::NodeId, i: usize) -> liquide_layout::geometry::Rect {
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("octet-{i}")).expect("octet box")
}

/// Typing digits fills the active octet; 3 digits auto-advance to the next octet.
#[test]
fn digits_fill_and_auto_advance() {
    let mut g = Gallery::new(W, H, "");
    g.mount("ip", Box::new(IpInput::new()));
    g.relayout();
    g.host.set_focus(Some("ip"), &mut g.doc, &mut g.dispatcher);

    for c in ['1', '9', '2'] {
        g.key(KeyInput::new(c as u32, 0));
    }
    assert_eq!(as_ip(&g, "ip").octet(0), 192);
    assert_eq!(as_ip(&g, "ip").active_octet(), 1, "3 digits auto-advance to octet 1");

    for c in ['1', '6', '8'] {
        g.key(KeyInput::new(c as u32, 0));
    }
    assert_eq!(as_ip(&g, "ip").octet(1), 168);
    assert_eq!(as_ip(&g, "ip").active_octet(), 2);
}

/// A `.` commits the active octet and advances even with fewer than 3 digits.
#[test]
fn dot_advances_octet() {
    let mut g = Gallery::new(W, H, "");
    g.mount("ip", Box::new(IpInput::new()));
    g.relayout();
    g.host.set_focus(Some("ip"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new('1' as u32, 0));
    g.key(KeyInput::new('0' as u32, 0));
    g.key(KeyInput::new('.' as u32, 0)); // commit + advance
    assert_eq!(as_ip(&g, "ip").octet(0), 10);
    assert_eq!(as_ip(&g, "ip").active_octet(), 1, ". advances to octet 1");
}

/// An octet clamps to 0..=255: typing 999 clamps to 255 and emits the clamped
/// address.
#[test]
fn octet_clamps_to_255() {
    let mut g = Gallery::new(W, H, "");
    g.mount("ip", Box::new(IpInput::new()));
    g.relayout();
    g.host.set_focus(Some("ip"), &mut g.doc, &mut g.dispatcher);

    let mut last = None;
    for c in ['9', '9', '9'] {
        let a = g.key(KeyInput::new(c as u32, 0));
        if let Some(act) = a.into_iter().find(|a| a.name == CHANGED_ACTION) {
            last = act.payload;
        }
    }
    assert_eq!(as_ip(&g, "ip").octet(0), 255, "999 clamps to 255");
    assert_eq!(last.as_deref(), Some("255.0.0.0"));
}

/// Up/Down arrows step the active octet (clamped); the full address updates.
#[test]
fn arrows_step_octet() {
    let mut g = Gallery::new(W, H, "");
    g.mount("ip", Box::new(IpInput::with(10, 0, 0, 1)));
    g.relayout();
    g.host.set_focus(Some("ip"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_UP, 0)); // octet 0: 10 -> 11
    assert_eq!(as_ip(&g, "ip").octet(0), 11);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // 11 -> 10
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // 10 -> 9
    assert_eq!(as_ip(&g, "ip").octet(0), 9);
}

/// ANTI-CONSTANT: clicking octet-2's REAL laid-out box focuses octet 2 — with
/// octet widths made unequal, a fixed x-split would target the wrong octet.
#[test]
fn click_focuses_octet_from_layout() {
    let mut g = Gallery::new(
        W,
        H,
        // Make octet-0 very wide so a uniform-split guess for octet-2 is wrong.
        "lq-ip-octet[data-index=\"0\"] { width: 90px; }",
    );
    g.mount("ip", Box::new(IpInput::with(1, 2, 3, 4)));
    g.relayout();
    let root = g.host.root_of("ip").unwrap();
    let o0 = octet_box(&g, root, 0);
    let o2 = octet_box(&g, root, 2);
    assert!(o0.width > o2.width + 30.0, "precondition: octet-0 is much wider");
    g.left_click(o2.x + o2.width / 2.0, o2.y + o2.height / 2.0);
    let _ = g.process();
    assert_eq!(
        as_ip(&g, "ip").active_octet(),
        2,
        "click in octet-2's REAL box focuses octet 2"
    );
}

/// Backspace deletes a digit; at an empty octet it hops to the previous octet.
#[test]
fn backspace_deletes_then_hops() {
    let mut g = Gallery::new(W, H, "");
    g.mount("ip", Box::new(IpInput::new()));
    g.relayout();
    g.host.set_focus(Some("ip"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new('5' as u32, 0)); // octet 0 = "5"
    g.key(KeyInput::new('.' as u32, 0)); // advance to octet 1
    assert_eq!(as_ip(&g, "ip").active_octet(), 1);
    // Octet 1 is empty: Backspace hops back to octet 0.
    g.key(KeyInput::new(keys::BACKSPACE, 0));
    assert_eq!(as_ip(&g, "ip").active_octet(), 0, "backspace at empty octet hops back");
    // Now Backspace deletes the '5'.
    g.key(KeyInput::new(keys::BACKSPACE, 0));
    assert_eq!(as_ip(&g, "ip").octet_text(0), "", "the digit is deleted");
}

/// PIXELS: focusing an octet restyles it (the active-octet fill).
#[test]
fn active_octet_restyles_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("ip", Box::new(IpInput::with(1, 2, 3, 4)));
    g.relayout();
    let root = g.host.root_of("ip").unwrap();
    let o2 = octet_box(&g, root, 2);
    let (sx, sy) = ((o2.x + o2.width / 2.0) as u32, (o2.y + o2.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);
    g.left_click(o2.x + o2.width / 2.0, o2.y + o2.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "focusing an octet must restyle its pixels");
}

/// PIXELS :focus (widget) — focusing the whole control recolours its outer border
/// (`lq-ip-input:focus { border-color: accent }`). Sampled on the top border edge.
#[test]
fn widget_focus_restyles_border_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("ip", Box::new(IpInput::with(1, 2, 3, 4)));
    g.relayout();
    let root = g.host.root_of("ip").unwrap();
    let r = g.box_of(root).expect("ip laid out");
    // The 1px border on the top edge.
    let (sx, sy) = ((r.x + r.width / 2.0) as u32, r.y as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);
    // Click the widget body (between octets) to focus WITHOUT changing the active
    // octet's own fill at the sampled border pixel.
    g.left_click(r.x + r.width / 2.0, r.y + r.height / 2.0);
    let _ = g.process();
    assert!(as_ip(&g, "ip").is_focused(), "click focuses the widget");
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(
        before != after,
        "focusing the IP control must recolour its border (before {before:?} after {after:?})"
    );
}

/// ::between octets — the literal dot separators (`lq-ip-dot`) render between the
/// four octet boxes. There are exactly 3, each laid out between consecutive octets
/// and carrying paintable ink.
#[test]
fn dot_separators_render_between_octets() {
    let mut g = Gallery::new(W, H, "");
    g.mount("ip", Box::new(IpInput::with(1, 2, 3, 4)));
    g.relayout();
    let root = g.host.root_of("ip").unwrap();
    let o0 = octet_box(&g, root, 0);
    let o1 = octet_box(&g, root, 1);
    // The dot element sits horizontally between octet 0 and octet 1.
    assert!(o1.x > o0.x + o0.width - 1.0, "octet 1 follows octet 0");
    let gap_x = (o0.x + o0.width + (o1.x - (o0.x + o0.width)) / 2.0) as u32;
    let gap_y = (o0.y + o0.height / 2.0) as u32;
    // Scan the gap column for any ink (the dot glyph / box).
    let fb = g.rasterize();
    let painted = ((gap_y.saturating_sub(6))..(gap_y + 6)).any(|y| Gallery::pixel(&fb, gap_x, y).a > 0);
    assert!(painted, "a dot separator must paint between octet 0 and octet 1");
}

/// ACTIVE MOVES: focusing octet 3 after octet 1 clears octet 1's active fill (the
/// active styling rides the current octet, it does not accumulate).
#[test]
fn active_fill_moves_off_previous_octet() {
    let mut g = Gallery::new(W, H, "");
    g.mount("ip", Box::new(IpInput::with(1, 2, 3, 4)));
    g.relayout();
    let root = g.host.root_of("ip").unwrap();
    let o1 = octet_box(&g, root, 1);
    let o3 = octet_box(&g, root, 3);
    let (x1, y1) = ((o1.x + o1.width / 2.0) as u32, (o1.y + o1.height / 2.0) as u32);
    let baseline_o1 = Gallery::pixel(&g.rasterize(), x1, y1);

    // Focus octet 1.
    g.left_click(o1.x + o1.width / 2.0, o1.y + o1.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_ip(&g, "ip").active_octet(), 1);
    let active_o1 = Gallery::pixel(&g.rasterize(), x1, y1);
    assert!(active_o1 != baseline_o1, "octet 1 active fill differs from baseline");

    // Focus octet 3 — octet 1 must return to its unfocused pixels.
    g.left_click(o3.x + o3.width / 2.0, o3.y + o3.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_ip(&g, "ip").active_octet(), 3);
    let after_o1 = Gallery::pixel(&g.rasterize(), x1, y1);
    assert_eq!(after_o1, baseline_o1, "the active fill must leave octet 1 when focus moves");
}

/// DISABLED: a disabled IP input swallows an octet click — no focus, no action.
#[test]
fn disabled_ip_swallows_click() {
    let mut g = Gallery::new(W, H, "");
    g.mount("ip", Box::new(IpInput::with(1, 2, 3, 4).disabled(true)));
    g.relayout();
    let root = g.host.root_of("ip").unwrap();
    let o2 = octet_box(&g, root, 2);
    g.left_click(o2.x + o2.width / 2.0, o2.y + o2.height / 2.0);
    let acts = g.process();
    assert!(acts.is_empty(), "disabled IP input must emit nothing");
    assert!(!as_ip(&g, "ip").is_focused(), "disabled IP input does not focus on click");
}
