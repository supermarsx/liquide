//! `<lq-tag-input>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::tag_input::{TagInput, CHANGED_ACTION};

const W: u32 = 420;
const H: u32 = 120;

fn as_tags<'a>(g: &'a Gallery, id: &str) -> &'a TagInput {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<TagInput>()
        .unwrap()
}

fn type_str(g: &mut Gallery, s: &str) {
    for c in s.chars() {
        g.key(KeyInput::new(c as u32, 0));
    }
}

/// Typing + Enter adds a token; emits Changed(tags).
#[test]
fn type_and_enter_adds_a_tag() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ti", Box::new(TagInput::new("Add tags…")));
    g.relayout();
    g.host.set_focus(Some("ti"), &mut g.doc, &mut g.dispatcher);

    type_str(&mut g, "rust");
    assert_eq!(as_tags(&g, "ti").buffer(), "rust");
    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("rust"));
    assert_eq!(as_tags(&g, "ti").tags(), &["rust".to_string()]);
    assert_eq!(as_tags(&g, "ti").buffer(), "", "buffer clears after commit");
}

/// Backspace at the start of an empty buffer removes the last token.
#[test]
fn backspace_at_start_removes_last_tag() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount(
        "ti",
        Box::new(TagInput::new("…").with_tags(["a", "b", "c"])),
    );
    g.relayout();
    g.host.set_focus(Some("ti"), &mut g.doc, &mut g.dispatcher);
    assert_eq!(as_tags(&g, "ti").tags().len(), 3);

    let a = g.key(KeyInput::new(keys::BACKSPACE, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].payload.as_deref(), Some("a,b"));
    assert_eq!(as_tags(&g, "ti").tags(), &["a".to_string(), "b".to_string()]);
}

/// Backspace mid-buffer edits the buffer, does NOT remove a token.
#[test]
fn backspace_mid_buffer_edits_text_not_tags() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ti", Box::new(TagInput::new("…").with_tags(["keep"])));
    g.relayout();
    g.host.set_focus(Some("ti"), &mut g.doc, &mut g.dispatcher);
    type_str(&mut g, "xy");
    assert_eq!(as_tags(&g, "ti").buffer(), "xy");

    g.key(KeyInput::new(keys::BACKSPACE, 0));
    assert_eq!(as_tags(&g, "ti").buffer(), "x");
    assert_eq!(as_tags(&g, "ti").tags(), &["keep".to_string()], "token untouched");
}

/// NO-FAKE-GREEN tooth: clicking a token's × box removes THAT token; the hit is
/// read from the laid-out × box, never a constant. Tokens of different label
/// widths reflow, so a constant offset would mis-target. Click token-0's × and
/// assert token-0 (not the last) is removed.
#[test]
fn remove_x_hits_the_laid_out_box() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount(
        "ti",
        // Different-width labels so a constant pitch can't predict the × x.
        Box::new(TagInput::new("…").with_tags(["short", "a-much-longer-tag", "z"])),
    );
    g.relayout();
    let root = g.host.root_of("ti").unwrap();
    let rm0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "remove-0").expect("remove-0 box")
    };
    g.left_click(rm0.x + rm0.width / 2.0, rm0.y + rm0.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(
        as_tags(&g, "ti").tags(),
        &["a-much-longer-tag".to_string(), "z".to_string()],
        "the × box of token-0 removed token-0 specifically"
    );
}

/// Clicking a token's LABEL (next to its ×) does NOT remove it.
#[test]
fn label_click_does_not_remove() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ti", Box::new(TagInput::new("…").with_tags(["alpha", "beta"])));
    g.relayout();
    let root = g.host.root_of("ti").unwrap();
    let label0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "label-0").expect("label-0 box")
    };
    g.left_click(label0.x + 2.0, label0.y + label0.height / 2.0);
    let _ = g.process();
    assert_eq!(
        as_tags(&g, "ti").tags().len(),
        2,
        "clicking the label must not remove a token"
    );
}

/// Duplicate tags are rejected (default).
#[test]
fn duplicate_tag_rejected() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ti", Box::new(TagInput::new("…").with_tags(["dup"])));
    g.relayout();
    g.host.set_focus(Some("ti"), &mut g.doc, &mut g.dispatcher);
    type_str(&mut g, "dup");
    g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(as_tags(&g, "ti").tags(), &["dup".to_string()], "no duplicate added");
}

/// A new token actually paints (token box exists in layout after add).
#[test]
fn added_token_lays_out_a_box() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ti", Box::new(TagInput::new("…")));
    g.relayout();
    let root = g.host.root_of("ti").unwrap();
    // No token yet.
    {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "token-0").is_none());
    }
    g.host.set_focus(Some("ti"), &mut g.doc, &mut g.dispatcher);
    type_str(&mut g, "new");
    g.key(KeyInput::new(keys::ENTER, 0));
    g.relayout();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let b = q.box_of_part(root, "token-0").expect("token-0 box after add");
    assert!(b.width > 0.0 && b.height > 0.0, "added token has a real box");
}

/// Disabled tag-input swallows keys + clicks.
#[test]
fn disabled_swallows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ti", Box::new(TagInput::new("…").disabled(true)));
    g.relayout();
    g.host.set_focus(Some("ti"), &mut g.doc, &mut g.dispatcher);
    type_str(&mut g, "x");
    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert!(a.is_empty());
    assert!(as_tags(&g, "ti").tags().is_empty());
}

// ── Added: visual-STATE pixel-delta coverage (no fake-green) ─────────────────

/// Helper: average the channel-weighted sum of a sub-part box (background/border
/// driven; avoids relying on glyph ink which the gallery font paints faintly).
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

/// :focus paints the focus-ring border color (CSS `lq-tag-input:focus`
/// border-color: focus-ring). Sample the top border line — focus must restyle it.
#[test]
fn focus_restyles_border_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ti", Box::new(TagInput::new("Add…")));
    g.relayout();
    let root = g.host.root_of("ti").unwrap();
    let r = g.box_of(root).unwrap();
    let (bx, by) = ((r.x + 8.0) as u32, r.y as u32);

    let before = Gallery::pixel(&g.rasterize(), bx, by);
    // set_focus sets the :focus pseudo on the widget root via the dispatcher, which
    // the CSS `lq-tag-input:focus` border-color rule matches (no rerender needed).
    g.host.set_focus(Some("ti"), &mut g.doc, &mut g.dispatcher);
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), bx, by);
    assert!(
        before != after,
        ":focus must restyle the tag-input border (before {before:?} after {after:?})"
    );
}

/// The chip remove × box PAINTS a distinct background (CSS `lq-token-remove`
/// background rgba(0,0,0,.18)) — proves the ::after-equivalent × affordance is
/// rasterized, not just present in the DOM.
#[test]
fn chip_remove_x_paints() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ti", Box::new(TagInput::new("…").with_tags(["rust"])));
    g.relayout();
    let root = g.host.root_of("ti").unwrap();
    let (rm, chip) = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (
            q.box_of_part(root, "remove-0").expect("remove-0 box"),
            q.box_of_part(root, "token-0").expect("token-0 box"),
        )
    };
    assert!(rm.width > 0.0 && rm.height > 0.0, "× box lays out");
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (rm.x + rm.width / 2.0) as u32, (rm.y + rm.height / 2.0) as u32);
    assert!(px.a > 0, "the chip × box must paint (alpha {})", px.a);
    // The chip itself is the accent (blue) fill; the × inset is darker — so the ×
    // center pixel must differ from the chip's bare label area.
    let chip_px = Gallery::pixel(&fb, (chip.x + 3.0) as u32, (chip.y + chip.height / 2.0) as u32);
    assert!(
        px != chip_px,
        "the × inset paints distinctly from the chip body (× {px:?} chip {chip_px:?})"
    );
}

/// :hover restyles the token's × box background (CSS `lq-token-remove:hover`
/// darkens to rgba(0,0,0,.40)). Hovering the × must change its rasterized pixels.
#[test]
fn remove_x_hover_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("ti", Box::new(TagInput::new("…").with_tags(["alpha"])));
    g.relayout();

    let before = part_sum(&mut g, "ti", "remove-0");
    let root = g.host.root_of("ti").unwrap();
    let rm = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "remove-0").unwrap()
    };
    g.pointer_move(rm.x + rm.width / 2.0, rm.y + rm.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = part_sum(&mut g, "ti", "remove-0");
    assert!(
        before != after,
        ":hover must darken the × box bg (before {before} after {after})"
    );
}

/// :disabled dims the whole control (CSS `lq-tag-input:disabled` opacity .5) —
/// the same tags painted disabled differ from enabled at the chip.
#[test]
fn disabled_dims_pixels() {
    let mk = |dis: bool| {
        let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
        g.mount("ti", Box::new(TagInput::new("…").with_tags(["one"]).disabled(dis)));
        g.relayout();
        let root = g.host.root_of("ti").unwrap();
        let chip = {
            let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
            q.box_of_part(root, "token-0").unwrap()
        };
        let fb = g.rasterize();
        Gallery::pixel(&fb, (chip.x + chip.width / 2.0) as u32, (chip.y + chip.height / 2.0) as u32)
    };
    let enabled = mk(false);
    let disabled = mk(true);
    assert!(
        enabled != disabled,
        ":disabled must dim the chip fill (enabled {enabled:?} disabled {disabled:?})"
    );
}
