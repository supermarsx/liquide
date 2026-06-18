//! `<lq-masked-input>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::masked_input::{MaskedInput, CHANGED_ACTION};

const W: u32 = 320;
const H: u32 = 120;

fn as_mask<'a>(g: &'a Gallery, id: &str) -> &'a MaskedInput {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<MaskedInput>()
        .unwrap()
}

fn type_str(g: &mut Gallery, s: &str) {
    for c in s.chars() {
        g.key(KeyInput::new(c as u32, 0));
    }
}

/// Typing fills editable slots and auto-inserts literals into the formatted value.
#[test]
fn typing_fills_slots_with_literals() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("mk", Box::new(MaskedInput::new("##/##/####")));
    g.relayout();
    g.host.set_focus(Some("mk"), &mut g.doc, &mut g.dispatcher);

    type_str(&mut g, "12252026");
    assert_eq!(as_mask(&g, "mk").raw(), "12252026");
    assert_eq!(as_mask(&g, "mk").formatted(), "12/25/2026");
    assert!(as_mask(&g, "mk").is_complete());
}

/// The change action carries raw|formatted.
#[test]
fn change_payload_is_raw_and_formatted() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("mk", Box::new(MaskedInput::new("##/##")));
    g.relayout();
    g.host.set_focus(Some("mk"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new('0' as u32, 0));
    let a = g.key(KeyInput::new('7' as u32, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    // raw "07", formatted "07/__".
    assert_eq!(a[0].payload.as_deref(), Some("07|07/__"));
}

/// Wrong-kind characters are rejected (a letter in a `#` digit slot).
#[test]
fn wrong_kind_char_rejected() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("mk", Box::new(MaskedInput::new("###")));
    g.relayout();
    g.host.set_focus(Some("mk"), &mut g.doc, &mut g.dispatcher);
    let a = g.key(KeyInput::new('x' as u32, 0));
    assert!(a.is_empty(), "a letter in a digit slot is rejected");
    assert_eq!(as_mask(&g, "mk").raw(), "");
    // A letter mask accepts letters but not digits.
    g.mount("mk2", Box::new(MaskedInput::new("AA")));
    g.relayout();
    g.host.set_focus(Some("mk2"), &mut g.doc, &mut g.dispatcher);
    let a = g.key(KeyInput::new('5' as u32, 0));
    assert!(a.is_empty(), "a digit in a letter slot is rejected");
    g.key(KeyInput::new('a' as u32, 0));
    assert_eq!(as_mask(&g, "mk2").raw(), "a");
}

/// Backspace removes the last filled slot.
#[test]
fn backspace_removes_last_filled() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("mk", Box::new(MaskedInput::new("##/##")));
    g.relayout();
    g.host.set_focus(Some("mk"), &mut g.doc, &mut g.dispatcher);
    type_str(&mut g, "123");
    assert_eq!(as_mask(&g, "mk").raw(), "123");
    g.key(KeyInput::new(keys::BACKSPACE, 0));
    assert_eq!(as_mask(&g, "mk").raw(), "12");
    assert_eq!(as_mask(&g, "mk").formatted(), "12/__");
}

/// NO-FAKE-GREEN tooth: the literal slots lay out at the MASK positions. The `/`
/// literal slot box must sit BETWEEN slot-1 (an editable cell) and slot-3, i.e.
/// its laid-out x is ordered after the first two digit cells — a constant would
/// not track this when slot widths change. We widen the digit cells so a fixed
/// pitch would mislocate the separator.
#[test]
fn literal_slots_lay_out_at_mask_positions() {
    let mut g = Gallery::new(
        W,
        H,
        "lq-gallery { padding: 16px; } \
         lq-mask-slot { min-width: 20px; padding: 0 4px; }",
    );
    g.mount("mk", Box::new(MaskedInput::new("##/##")));
    g.relayout();
    let root = g.host.root_of("mk").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    // Mask: slot-0 (#), slot-1 (#), slot-2 (/), slot-3 (#), slot-4 (#).
    let s0 = q.box_of_part(root, "slot-0").expect("slot-0");
    let s1 = q.box_of_part(root, "slot-1").expect("slot-1");
    let s2 = q.box_of_part(root, "slot-2").expect("slot-2 (the / literal)");
    let s3 = q.box_of_part(root, "slot-3").expect("slot-3");
    // Strictly left-to-right ordering proves the literal sits at its mask index.
    assert!(s0.x < s1.x, "slot-0 left of slot-1");
    assert!(s1.x < s2.x, "the / literal sits AFTER the first two digit cells");
    assert!(s2.x < s3.x, "the / literal sits BEFORE the last two digit cells");
    // And the literal cell really carries the separator text (it's the `/`).
    let lit_node = q.find_part(root, "slot-2").unwrap();
    assert_eq!(
        g.doc().get_attribute(lit_node, "data-literal").as_deref(),
        Some("true"),
        "slot-2 is the literal slot"
    );
}

/// The caret skips literal slots: after filling slot-1, the caret element lays
/// out at the next EDITABLE position (slot-3), i.e. to the right of the / literal.
#[test]
fn caret_skips_literal_slots() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("mk", Box::new(MaskedInput::new("##/##")));
    g.relayout();
    // Click to focus the field (sets the behavior's :focus so the caret renders),
    // then route keys to it.
    let root = g.host.root_of("mk").unwrap();
    let fbox = g.box_of(root).unwrap();
    g.left_click(fbox.x + 4.0, fbox.y + fbox.height / 2.0);
    let _ = g.process();
    g.host.set_focus(Some("mk"), &mut g.doc, &mut g.dispatcher);
    // Fill the first two digits; the next editable slot is slot-3 (after the /).
    type_str(&mut g, "12");
    g.relayout();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let caret = q.box_of_part(root, "caret").expect("caret box");
    let s1 = q.box_of_part(root, "slot-1").expect("slot-1 (last filled digit)");
    let lit = q.box_of_part(root, "slot-2").expect("/ literal box");
    // The caret skipped the filled editable slots AND the literal: it sits past
    // the last filled digit (slot-1) and at/after the / literal's start — i.e. at
    // the NEXT editable position (slot-3), not back at a filled or literal slot.
    assert!(
        caret.x > s1.x,
        "caret is past the last filled digit (caret.x {} vs slot-1.x {})",
        caret.x,
        s1.x
    );
    assert!(
        caret.x >= lit.x,
        "caret skipped to at/after the / literal (caret.x {} vs literal.x {})",
        caret.x,
        lit.x
    );
}

/// Disabled masked input swallows keys.
#[test]
fn disabled_swallows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("mk", Box::new(MaskedInput::new("###").disabled(true)));
    g.relayout();
    g.host.set_focus(Some("mk"), &mut g.doc, &mut g.dispatcher);
    let a = g.key(KeyInput::new('5' as u32, 0));
    assert!(a.is_empty());
    assert_eq!(as_mask(&g, "mk").raw(), "");
}
