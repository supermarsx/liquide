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

// ── Added: visual-STATE pixel-delta coverage (no fake-green) ─────────────────

/// Channel-weighted sum over a sub-part box.
fn part_sum(g: &mut Gallery, id: &str, part: &str) -> u64 {
    let root = g.host.root_of(id).unwrap();
    let r = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, part).unwrap_or_else(|| panic!("part {part}"))
    };
    let fb = g.rasterize();
    let mut acc = 0u64;
    for y in (r.y as u32)..((r.y + r.height) as u32) {
        for x in (r.x as u32)..((r.x + r.width) as u32) {
            let p = Gallery::pixel(&fb, x, y);
            acc += p.r as u64 + p.g as u64 * 3 + p.b as u64 * 7 + p.a as u64 * 11;
        }
    }
    acc
}

/// FOCUS paints the caret: when focused, the caret element renders AND the
/// `lq-masked-input:focus lq-caret` rule gives it a visible (fg) background. The
/// caret box must paint a non-transparent pixel.
#[test]
fn focus_paints_caret() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("mk", Box::new(MaskedInput::new("##/##")));
    g.relayout();
    // Before focus there is NO caret element at all.
    let root = g.host.root_of("mk").unwrap();
    {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "caret").is_none(), "unfocused: no caret");
    }
    // Click to set the behavior's focused flag (renders the caret), then set the
    // :focus pseudo so the caret's focus bg rule applies.
    let fbox = g.box_of(root).unwrap();
    g.left_click(fbox.x + 4.0, fbox.y + fbox.height / 2.0);
    let _ = g.process();
    g.host.set_focus(Some("mk"), &mut g.doc, &mut g.dispatcher);
    g.relayout();
    let caret = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "caret").expect("focused: caret box")
    };
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (caret.x + caret.width / 2.0) as u32, (caret.y + caret.height / 2.0) as u32);
    assert!(px.a > 0, "the focused caret must paint a visible bar (alpha {})", px.a);
}

/// :focus paints the focus-ring border (CSS `lq-masked-input:focus` border-color).
#[test]
fn focus_restyles_border_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("mk", Box::new(MaskedInput::new("###")));
    g.relayout();
    let root = g.host.root_of("mk").unwrap();
    let r = g.box_of(root).unwrap();
    let (bx, by) = ((r.x + 8.0) as u32, r.y as u32);
    let before = Gallery::pixel(&g.rasterize(), bx, by);
    g.host.set_focus(Some("mk"), &mut g.doc, &mut g.dispatcher);
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), bx, by);
    assert!(
        before != after,
        ":focus must restyle the masked-input border (before {before:?} after {after:?})"
    );
}

/// The `.complete` state restyles the border (CSS `lq-masked-input.complete`
/// border-color: accent) — a fully-filled field differs at the border from a
/// partially-filled one. Sample the top border line; fill only changes the border.
#[test]
fn complete_restyles_border_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("mk", Box::new(MaskedInput::new("##")));
    g.relayout();
    let root = g.host.root_of("mk").unwrap();
    let r = g.box_of(root).unwrap();
    let (bx, by) = ((r.x + 8.0) as u32, r.y as u32);
    let before = Gallery::pixel(&g.rasterize(), bx, by);
    g.host.set_focus(Some("mk"), &mut g.doc, &mut g.dispatcher);
    type_str(&mut g, "12");
    assert!(as_mask(&g, "mk").is_complete());
    // Drop focus so the focus-ring rule does not mask the .complete delta we want.
    g.host.set_focus(None, &mut g.doc, &mut g.dispatcher);
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), bx, by);
    assert!(
        before != after,
        ".complete must restyle the border to accent (before {before:?} after {after:?})"
    );
}

/// A FILLED editable slot paints differently from an EMPTY one (CSS
/// `lq-mask-slot.empty` uses the dim placeholder color; a filled slot the fg).
/// Compare slot-0 (filled) vs slot-1 (empty) after typing one digit.
#[test]
fn filled_slot_differs_from_empty_slot() {
    let mut g = Gallery::new(
        W,
        H,
        "lq-gallery { padding: 16px; } lq-mask-slot { min-width: 16px; }",
    );
    g.mount("mk", Box::new(MaskedInput::new("####")));
    g.relayout();
    g.host.set_focus(Some("mk"), &mut g.doc, &mut g.dispatcher);
    type_str(&mut g, "7");
    g.relayout();
    let filled = part_sum(&mut g, "mk", "slot-0");
    let empty = part_sum(&mut g, "mk", "slot-2");
    assert!(
        filled != empty,
        "a filled slot must paint differently from an empty slot (filled {filled} empty {empty})"
    );
}

/// :disabled dims the masked input (opacity .5).
#[test]
fn disabled_dims_pixels() {
    let mk = |dis: bool| {
        let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
        g.mount("mk", Box::new(MaskedInput::new("##/##").disabled(dis)));
        g.relayout();
        let root = g.host.root_of("mk").unwrap();
        let r = g.box_of(root).unwrap();
        let fb = g.rasterize();
        // Sample the border (opaque) where the .5 opacity multiply shows.
        Gallery::pixel(&fb, (r.x + 8.0) as u32, r.y as u32)
    };
    let enabled = mk(false);
    let disabled = mk(true);
    assert!(
        enabled != disabled,
        ":disabled must dim the masked input (enabled {enabled:?} disabled {disabled:?})"
    );
}
