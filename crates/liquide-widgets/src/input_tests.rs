//! `<lq-input>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::input::{TextInput, CHANGED_ACTION};
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 320;
const H: u32 = 120;

fn gallery_with(input: TextInput) -> Gallery {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("inp", Box::new(input));
    g.relayout();
    g
}

fn as_input(g: &Gallery) -> &TextInput {
    g.host.behavior("inp").unwrap().as_any().downcast_ref::<TextInput>().unwrap()
}

fn focus(g: &mut Gallery) {
    g.host.set_focus(Some("inp"), &mut g.doc, &mut g.dispatcher);
    // Also click to set the behavior's own focused flag.
    let node = g.host.root_of("inp").unwrap();
    let r = g.box_of(node).unwrap();
    g.left_click(r.x + 6.0, r.y + r.height / 2.0);
    let _ = g.process();
    g.relayout();
}

/// Renders a real field box from CSS.
#[test]
fn input_renders_field_box() {
    let mut g = gallery_with(TextInput::new("type here"));
    let node = g.host.root_of("inp").unwrap();
    let r = g.box_of(node).expect("input lays out");
    assert!((r.width - 200.0).abs() < 2.0, "width from CSS (got {})", r.width);
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (r.x + 10.0) as u32, (r.y + r.height / 2.0) as u32);
    assert!(px.a > 0, "field must paint");
}

/// Clicking inside the laid-out field focuses it.
#[test]
fn click_focuses_field() {
    let mut g = gallery_with(TextInput::new("type"));
    assert!(!as_input(&g).is_focused());
    let node = g.host.root_of("inp").unwrap();
    let r = g.box_of(node).unwrap();
    g.left_click(r.x + 10.0, r.y + r.height / 2.0);
    let _ = g.process();
    assert!(as_input(&g).is_focused(), "click must focus the field");
}

/// Typing printable keys updates the buffer and emits Changed; caret advances.
#[test]
fn typing_updates_buffer_via_pipeline() {
    let mut g = gallery_with(TextInput::new("type"));
    focus(&mut g);

    for c in ['h', 'i'] {
        let a = g.key(KeyInput::new(c as u32, 0));
        assert_eq!(a.len(), 1, "each printable emits Changed");
        assert_eq!(a[0].name, CHANGED_ACTION);
    }
    assert_eq!(as_input(&g).text(), "hi");
    assert_eq!(as_input(&g).caret(), 2);
}

/// Backspace deletes before the caret; arrows + Home/End move it.
#[test]
fn backspace_and_caret_navigation() {
    let mut g = gallery_with(TextInput::new("x").with_text("abc"));
    focus(&mut g);
    assert_eq!(as_input(&g).caret(), 3, "caret seeded at end");

    g.key(KeyInput::new(keys::BACKSPACE, 0));
    assert_eq!(as_input(&g).text(), "ab");

    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_input(&g).caret(), 0);
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_input(&g).caret(), 1);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_input(&g).caret(), 2);

    // Insert at end then verify it lands where the caret is, not appended blindly.
    g.key(KeyInput::new(keys::HOME, 0));
    g.key(KeyInput::new('Z' as u32, 0));
    assert_eq!(as_input(&g).text(), "Zab");
}

/// The caret box geometry comes from LAYOUT: moving the caret moves the caret
/// box's on-screen x (it sits between the before/after text spans, so layout —
/// not a px-per-char constant — positions it). A constant-positioned caret could
/// not move with the real glyph advances.
#[test]
fn caret_box_position_comes_from_layout() {
    let mut g = gallery_with(TextInput::new("x").with_text("WWWWW"));
    focus(&mut g);

    let root = g.host.root_of("inp").unwrap();

    // Caret at end (after all 5 W's).
    let caret_end_x = {
        let hit = g.hit_test_engine();
        let q = LayoutQuery::new(hit, g.doc());
        q.box_of_part(root, "caret").expect("caret box at end").x
    };

    // Move caret to home; re-render + relayout, then read the caret box again.
    g.key(KeyInput::new(keys::HOME, 0));
    g.relayout();
    let caret_home_x = {
        let hit = g.hit_test_engine();
        let q = LayoutQuery::new(hit, g.doc());
        q.box_of_part(root, "caret").expect("caret box at home").x
    };

    assert!(
        caret_home_x < caret_end_x - 1.0,
        "caret at home must sit left of caret at end (home_x={caret_home_x}, end_x={caret_end_x}) \
         — proving layout, not a constant, positions the caret"
    );
}

/// Disabled inputs swallow keystrokes and clicks.
#[test]
fn disabled_input_swallows_input() {
    let mut g = gallery_with(TextInput::new("type").disabled(true));
    g.host.set_focus(Some("inp"), &mut g.doc, &mut g.dispatcher);
    let a = g.key(KeyInput::new('a' as u32, 0));
    assert!(a.is_empty());
    assert_eq!(as_input(&g).text(), "");
    assert!(!g.host.behavior("inp").unwrap().focusable());
}

// ── Added: :focus border + caret-reveal pixel coverage (no fake-green) ───────

/// :focus restyles the field BORDER (focus ring) — sample on the top border line.
#[test]
fn focus_restyles_border_pixels() {
    let mut g = gallery_with(TextInput::new("type here"));
    let node = g.host.root_of("inp").unwrap();
    let r = g.box_of(node).unwrap();
    let (bx, by) = ((r.x + 8.0) as u32, r.y as u32);

    let before = Gallery::pixel(&g.rasterize(), bx, by);
    focus(&mut g); // clicks + sets focus -> FOCUS pseudo + relayout
    let after = Gallery::pixel(&g.rasterize(), bx, by);
    assert!(
        before != after,
        ":focus must restyle the field border ring (before {before:?} after {after:?})"
    );
    assert!(as_input(&g).is_focused());
}

/// :focus reveals the caret — the caret element is transparent until focus, then
/// paints with the foreground colour (CSS `lq-input:focus lq-caret`). Assert the
/// caret box paints an opaque pixel once focused.
#[test]
fn focus_reveals_caret_pixels() {
    let mut g = gallery_with(TextInput::new("x").with_text("WWWWW"));
    focus(&mut g);
    g.relayout();
    let root = g.host.root_of("inp").unwrap();
    let caret = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "caret").expect("caret box from layout")
    };
    let fb = g.rasterize();
    let px = Gallery::pixel(
        &fb,
        (caret.x + caret.width / 2.0) as u32,
        (caret.y + caret.height / 2.0) as u32,
    );
    assert!(
        px.a > 0,
        "focused caret must paint a visible pixel (got alpha {})",
        px.a
    );
}
