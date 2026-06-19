//! `<lq-hotkey-input>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::{KeyInput, WidgetBehavior};
use crate::gallery::Gallery;
use crate::hotkey_input::{Chord, HotkeyInput, CHANGED_ACTION};
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 280;
const H: u32 = 100;

fn as_hk<'a>(g: &'a Gallery, id: &str) -> &'a HotkeyInput {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<HotkeyInput>()
        .unwrap()
}

/// Unit: a chord renders its canonical display string in modifier order.
#[test]
fn chord_display_is_canonical() {
    let c = Chord {
        modifiers: keys::modifiers::CTRL | keys::modifiers::SHIFT,
        key: Some('k' as u32),
    };
    assert_eq!(c.display(), "Ctrl+Shift+K");
    let plain = Chord {
        modifiers: 0,
        key: Some(keys::ENTER),
    };
    assert_eq!(plain.display(), "Enter");
}

/// Capturing a chord while focused records modifiers + key and emits Changed.
#[test]
fn capture_records_chord() {
    let mut g = Gallery::new(W, H, "");
    g.mount("hk", Box::new(HotkeyInput::new()));
    g.relayout();
    g.host.set_focus(Some("hk"), &mut g.doc, &mut g.dispatcher);

    let a = g.key(KeyInput::new(
        'k' as u32,
        keys::modifiers::CTRL | keys::modifiers::SHIFT,
    ));
    let c = a.iter().find(|a| a.name == CHANGED_ACTION).expect("changed");
    assert_eq!(c.payload.as_deref(), Some("Ctrl+Shift+K"));
    let chord = as_hk(&g, "hk").chord();
    assert!(chord.is_complete());
    assert_eq!(chord.key, Some('k' as u32));
    assert_eq!(chord.modifiers, keys::modifiers::CTRL | keys::modifiers::SHIFT);
}

/// Backspace/Delete clears the captured chord (emits Changed("")).
#[test]
fn backspace_clears_chord() {
    let mut g = Gallery::new(W, H, "");
    g.mount(
        "hk",
        Box::new(HotkeyInput::with_chord(keys::modifiers::ALT, 'j' as u32)),
    );
    g.relayout();
    g.host.set_focus(Some("hk"), &mut g.doc, &mut g.dispatcher);
    assert!(as_hk(&g, "hk").chord().is_complete());

    let a = g.key(KeyInput::new(keys::BACKSPACE, 0));
    let c = a.iter().find(|a| a.name == CHANGED_ACTION).expect("changed");
    assert_eq!(c.payload.as_deref(), Some(""));
    assert!(!as_hk(&g, "hk").chord().is_complete(), "chord cleared");
}

/// Escape cancels capture (drops focus) without changing the stored chord.
#[test]
fn escape_cancels_capture() {
    let mut g = Gallery::new(W, H, "");
    g.mount(
        "hk",
        Box::new(HotkeyInput::with_chord(keys::modifiers::CTRL, 's' as u32)),
    );
    g.relayout();
    g.host.set_focus(Some("hk"), &mut g.doc, &mut g.dispatcher);
    // Begin capturing by clicking the field (sets the behavior's focused flag).
    let root = g.host.root_of("hk").unwrap();
    let field = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "field").expect("field box")
    };
    g.left_click(field.x + field.width / 2.0, field.y + field.height / 2.0);
    let _ = g.process();
    assert!(as_hk(&g, "hk").is_focused(), "click begins capture");

    let before = as_hk(&g, "hk").chord();
    g.key(KeyInput::new(keys::ESCAPE, 0));
    assert!(!as_hk(&g, "hk").is_focused(), "Escape drops capture focus");
    assert_eq!(as_hk(&g, "hk").chord(), before, "the stored chord is unchanged");
}

/// Clicking the field's LAID-OUT box begins capture (focuses it). Anti-constant:
/// the field box is read from layout, so a CSS size change cannot break the hit.
#[test]
fn click_field_begins_capture() {
    let mut g = Gallery::new(W, H, "lq-hotkey-field { min-width: 240px; }");
    g.mount("hk", Box::new(HotkeyInput::new()));
    g.relayout();
    let root = g.host.root_of("hk").unwrap();
    let field = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "field").expect("field box")
    };
    assert!(field.width >= 200.0, "precondition: wide field from CSS (got {})", field.width);
    // Click near the right edge — inside the laid-out box but outside a default
    // narrow-field guess.
    g.left_click(field.x + field.width - 12.0, field.y + field.height / 2.0);
    let _ = g.process();
    assert!(
        as_hk(&g, "hk").is_focused(),
        "a click in the field's REAL box begins capture"
    );
}

/// PIXELS: beginning capture (clicking the field → the .capturing glow) restyles
/// the rendered field border pixels.
#[test]
fn capture_restyles_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("hk", Box::new(HotkeyInput::new()));
    g.relayout();
    g.host.set_focus(Some("hk"), &mut g.doc, &mut g.dispatcher);
    let root = g.host.root_of("hk").unwrap();
    let field = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "field").expect("field box")
    };
    // Sample the border region (top edge) where the capture glow lands.
    let (sx, sy) = ((field.x + 2.0) as u32, (field.y + 1.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);
    // Click to begin capture (sets .capturing on the field → border glow), then
    // capture a chord.
    g.left_click(field.x + field.width / 2.0, field.y + field.height / 2.0);
    let _ = g.process();
    g.key(KeyInput::new('m' as u32, keys::modifiers::CTRL));
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "capturing must restyle the field border pixels");
}

/// PIXELS :focus moves — beginning capture lights the field border glow; pressing
/// Escape (cancels capture, drops the .capturing class) returns the border to its
/// resting pixels. The glow is a STATE, not a one-way paint.
#[test]
fn capture_glow_clears_on_escape() {
    let mut g = Gallery::new(W, H, "");
    g.mount("hk", Box::new(HotkeyInput::new()));
    g.relayout();
    g.host.set_focus(Some("hk"), &mut g.doc, &mut g.dispatcher);
    let root = g.host.root_of("hk").unwrap();
    let field = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "field").expect("field box")
    };
    let (sx, sy) = ((field.x + 2.0) as u32, (field.y + 1.0) as u32);
    let resting = Gallery::pixel(&g.rasterize(), sx, sy);

    // Click to begin capture → border glow.
    g.left_click(field.x + field.width / 2.0, field.y + field.height / 2.0);
    let _ = g.process();
    g.relayout();
    let glowing = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(resting != glowing, "capture must light the border glow");

    // Escape cancels capture (drops .capturing) → border returns to resting.
    g.key(KeyInput::new(keys::ESCAPE, 0));
    assert!(!as_hk(&g, "hk").is_focused(), "Escape drops capture");
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert_eq!(after, resting, "the glow must clear when capture is cancelled");
}

/// DISABLED: a disabled hotkey input swallows a field click (no capture) AND a
/// key press (no chord change), and drops out of the focus ring.
#[test]
fn disabled_hotkey_swallows_interaction() {
    let mut g = Gallery::new(W, H, "");
    g.mount(
        "hk",
        Box::new(HotkeyInput::with_chord(keys::modifiers::CTRL, 's' as u32).disabled(true)),
    );
    g.relayout();
    let root = g.host.root_of("hk").unwrap();
    let field = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "field").expect("field box")
    };
    g.left_click(field.x + field.width / 2.0, field.y + field.height / 2.0);
    let acts = g.process();
    assert!(acts.is_empty(), "disabled field click emits nothing");
    assert!(!as_hk(&g, "hk").is_focused(), "disabled field does not begin capture");

    // A key press is swallowed: the stored chord is unchanged.
    let before = as_hk(&g, "hk").chord();
    g.host.set_focus(Some("hk"), &mut g.doc, &mut g.dispatcher);
    let a = g.key(KeyInput::new('x' as u32, keys::modifiers::ALT));
    assert!(a.is_empty(), "disabled hotkey ignores keys");
    assert_eq!(as_hk(&g, "hk").chord(), before, "the chord is unchanged");
    assert!(!as_hk(&g, "hk").focusable(), "disabled hotkey is not focusable");
}
