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
    // focus() clicks near the left edge → click-to-position lands the caret near
    // the start; jump to the end for the deletion/navigation assertions below.
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_input(&g).caret(), 3, "End puts the caret at the buffer end");

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
/// box's on-screen x (it sits between the per-glyph spans, so layout — not a
/// px-per-char constant — positions it). A constant-positioned caret could not
/// move with the real glyph advances.
#[test]
fn caret_box_position_comes_from_layout() {
    let mut g = gallery_with(TextInput::new("x").with_text("WWWWW"));
    focus(&mut g);

    let root = g.host.root_of("inp").unwrap();

    // Move the caret to the end (after all 5 W's) and read the caret box.
    g.key(KeyInput::new(keys::END, 0));
    g.relayout();
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

// ── Click-to-position + selection + navigation (geometry-from-layout teeth) ──

use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_hit_test::HitTestEngine;
use liquide_layout::geometry::Rect;
use std::sync::Arc;

/// The laid-out border rect of the glyph at byte offset `b` (`data-part="g{b}"`).
fn glyph_box(g: &Gallery, b: usize) -> Rect {
    let root = g.host.root_of("inp").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("g{b}"))
        .unwrap_or_else(|| panic!("glyph box g{b} must exist"))
}

/// The vertical middle of the field (for click y).
fn mid_y(g: &Gallery) -> f32 {
    let node = g.host.root_of("inp").unwrap();
    let r = g.box_of(node).unwrap();
    r.y + r.height / 2.0
}

/// Send a modifier-carrying mouse event straight to the behavior with the REAL
/// laid-out geometry (the gallery's click helpers do not carry modifiers). Builds
/// an independent hit-test from a fresh pipeline pass so the immutable doc borrow
/// (for `LayoutQuery`) and the mutable host borrow (for the behavior) stay
/// disjoint. Re-renders + relayouts after so the DOM/layout reflect the change.
fn send_mouse(g: &mut Gallery, kind: DomEventKind, modifiers: u32) {
    let (_n, output, _a) = g.pipeline.render_to_scene_with_output(&g.doc, 0, 0.0);
    let hit = HitTestEngine::new(Arc::clone(&output.layout), Arc::clone(&output.styles));
    let root = g.host.root_of("inp").unwrap();
    let ev = DomEvent::with_modifiers(root, kind, modifiers);
    {
        let q = LayoutQuery::new(&hit, &g.doc);
        let b = g.host.behavior_mut("inp").unwrap();
        let _ = b.on_dom_event(root, &ev, &q);
    }
    g.host.rerender("inp", &mut g.doc);
    g.relayout();
}

/// A shift-click at `(x, y)` (press+release carrying Shift) — extends selection.
fn shift_click(g: &mut Gallery, x: f32, y: f32) {
    send_mouse(
        g,
        DomEventKind::MouseDown { button: MouseButton::Left, x, y },
        keys::modifiers::SHIFT,
    );
    send_mouse(
        g,
        DomEventKind::MouseUp { button: MouseButton::Left, x, y },
        keys::modifiers::SHIFT,
    );
}

/// TEETH: a click maps to the character boundary nearest its x, resolved from the
/// LAID-OUT (shaped) glyph boxes — not a fixed pitch. One glyph is widened in CSS;
/// a click landing in its widened tail must resolve to the boundary AFTER it,
/// which a uniform px-per-char guess gets wrong (it would land several glyphs to
/// the right). RED before this fix: the click only set focus, never the caret.
#[test]
fn click_positions_caret_from_shaped_advances() {
    // Widen the glyph at byte offset 2 so its box is much larger than a fixed
    // pitch would assume.
    let extra = "lq-input lq-label[data-part=\"g2\"] { padding-right: 40px; }";
    let mut g = Gallery::new(W, H, extra);
    g.mount("inp", Box::new(TextInput::new("x").with_text("xxxxx")));
    g.relayout();
    focus(&mut g);

    let g0 = glyph_box(&g, 0);
    let g2 = glyph_box(&g, 2);
    let g3 = glyph_box(&g, 3);
    let y = mid_y(&g);

    // Click near the right edge of the WIDENED glyph 2 — past its midpoint, before
    // glyph 3's midpoint → boundary 3 (right after glyph 2).
    let click_x = g2.x + g2.width - 3.0;
    g.left_click(click_x, y);
    let _ = g.process();
    assert_eq!(
        as_input(&g).caret(),
        3,
        "click in the widened glyph's tail must land at boundary 3 (after glyph 2)"
    );

    // A fixed-pitch guess (uniform natural glyph width) would map the SAME x to a
    // different, larger index — proving the mapping reads shaped advances.
    let uniform_w = g0.width;
    let fixed_guess = (((click_x - g0.x) / uniform_w).round() as i64).clamp(0, 5);
    assert!(
        fixed_guess != 3,
        "a fixed-pitch guess must MISS the real boundary (guess={fixed_guess}); \
         the tooth: geometry from layout, not a constant. (g3.x={})",
        g3.x
    );
}

/// Click past the end of the text lands the caret at `len` (not clamped short).
/// A SINGLE click (no prior click to coalesce into a double-click) so this proves
/// plain click-to-position, not word selection.
#[test]
fn click_past_end_places_caret_at_len() {
    let mut g = gallery_with(TextInput::new("x").with_text("abc"));
    let node = g.host.root_of("inp").unwrap();
    let r = g.box_of(node).unwrap();
    // Far to the right, well past the last glyph.
    g.left_click(r.x + r.width - 4.0, r.y + r.height / 2.0);
    let _ = g.process();
    assert_eq!(as_input(&g).caret(), 3, "click past the end → caret at len");
    assert!(as_input(&g).selection().is_none(), "a plain click leaves no selection");
    assert!(as_input(&g).is_focused(), "the click focuses the field");
}

/// Click before the first glyph lands the caret at 0. Focus is set WITHOUT a click
/// (so there is no earlier same-node click to coalesce into a double-click); the
/// single positioning click below is therefore a plain click-to-position.
#[test]
fn click_before_start_places_caret_at_zero() {
    let mut g = gallery_with(TextInput::new("x").with_text("abc"));
    g.host.set_focus(Some("inp"), &mut g.doc, &mut g.dispatcher);
    g.relayout();
    // Move caret to end first so we can prove the click pulls it back to 0.
    g.key(KeyInput::new(keys::END, 0));
    g.relayout();
    // Click just LEFT of the first glyph (inside the field padding).
    let g0 = glyph_box(&g, 0);
    g.left_click(g0.x - 3.0, mid_y(&g));
    let _ = g.process();
    assert_eq!(as_input(&g).caret(), 0, "click before the first glyph → caret 0");
}

/// Click-drag selects the covered range (anchor at press, caret at release),
/// resolved from the laid-out glyph boxes.
#[test]
fn drag_selects_covered_range() {
    let mut g = gallery_with(TextInput::new("x").with_text("abcde"));
    focus(&mut g);
    let y = mid_y(&g);
    let g1 = glyph_box(&g, 1);
    let g3 = glyph_box(&g, 3);
    let press_x = g1.x + 1.0; // → boundary 1 (before 'b')
    let release_x = g3.x + g3.width - 1.0; // → boundary 4 (after 'd')

    g.mouse_down(press_x, y);
    g.pointer_move(release_x, y);
    g.mouse_up(release_x, y);
    let _ = g.process();

    assert_eq!(
        as_input(&g).selection(),
        Some((1, 4)),
        "drag must select the covered byte range"
    );
    assert_eq!(as_input(&g).selected_text(), "bcd");
}

/// Shift-click extends the selection from the existing caret to the clicked boundary.
#[test]
fn shift_click_extends_selection() {
    let mut g = gallery_with(TextInput::new("x").with_text("abcde"));
    focus(&mut g);
    let y = mid_y(&g);
    let g1 = glyph_box(&g, 1);
    let g3 = glyph_box(&g, 3);

    // Plain click → caret/anchor at boundary 1.
    g.left_click(g1.x + 1.0, y);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_input(&g).caret(), 1);
    assert!(as_input(&g).selection().is_none());

    // Shift-click at boundary 4 → selection [1, 4).
    let release_x = g3.x + g3.width - 1.0;
    shift_click(&mut g, release_x, y);
    assert_eq!(
        as_input(&g).selection(),
        Some((1, 4)),
        "shift-click must extend the selection to the clicked boundary"
    );
}

/// Shift+arrow extends the selection; a plain arrow collapses it.
#[test]
fn shift_arrow_extends_selection() {
    let mut g = gallery_with(TextInput::new("x").with_text("abcde"));
    focus(&mut g);
    g.key(KeyInput::new(keys::HOME, 0)); // caret 0, no selection
    assert!(as_input(&g).selection().is_none());

    g.key(KeyInput::new(keys::ARROW_RIGHT, keys::modifiers::SHIFT));
    g.key(KeyInput::new(keys::ARROW_RIGHT, keys::modifiers::SHIFT));
    assert_eq!(
        as_input(&g).selection(),
        Some((0, 2)),
        "shift+right twice selects the first two chars"
    );
    assert_eq!(as_input(&g).selected_text(), "ab");

    // A plain right-arrow collapses the selection to its right edge.
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert!(as_input(&g).selection().is_none());
    assert_eq!(as_input(&g).caret(), 2, "plain arrow collapses to selection edge");
}

/// Ctrl+arrow moves the caret word-wise.
#[test]
fn ctrl_arrow_moves_word_wise() {
    let mut g = gallery_with(TextInput::new("x").with_text("foo bar baz"));
    focus(&mut g);
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_input(&g).caret(), 0);

    g.key(KeyInput::new(keys::ARROW_RIGHT, keys::modifiers::CTRL));
    assert_eq!(as_input(&g).caret(), 3, "ctrl+right jumps to end of 'foo'");
    g.key(KeyInput::new(keys::ARROW_RIGHT, keys::modifiers::CTRL));
    assert_eq!(as_input(&g).caret(), 7, "ctrl+right jumps to end of 'bar'");
    g.key(KeyInput::new(keys::ARROW_LEFT, keys::modifiers::CTRL));
    assert_eq!(as_input(&g).caret(), 4, "ctrl+left jumps to start of 'bar'");
}

/// Double-click selects the word under the pointer.
#[test]
fn double_click_selects_word() {
    let mut g = gallery_with(TextInput::new("x").with_text("foo bar baz"));
    focus(&mut g);
    let y = mid_y(&g);
    // Click on a glyph inside "bar" (byte offset 5 = 'a').
    let g5 = glyph_box(&g, 5);
    g.double_click(g5.x + g5.width / 2.0, y);
    let _ = g.process();
    assert_eq!(
        as_input(&g).selection(),
        Some((4, 7)),
        "double-click selects the whole word 'bar'"
    );
    assert_eq!(as_input(&g).selected_text(), "bar");
}

/// Backspace/Delete on a selection removes the selected range (not just one char).
#[test]
fn backspace_deletes_selection() {
    let mut g = gallery_with(TextInput::new("x").with_text("abcde"));
    focus(&mut g);
    // Select "bcd" via shift-arrows from home.
    g.key(KeyInput::new(keys::HOME, 0));
    for _ in 0..3 {
        // move to boundary 1 without selecting, then select 3.
    }
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0)); // caret 1
    for _ in 0..3 {
        g.key(KeyInput::new(keys::ARROW_RIGHT, keys::modifiers::SHIFT));
    }
    assert_eq!(as_input(&g).selection(), Some((1, 4)));

    let actions = g.key(KeyInput::new(keys::BACKSPACE, 0));
    assert_eq!(as_input(&g).text(), "ae", "backspace deletes the whole selection");
    assert_eq!(actions.len(), 1, "the deletion emits a Changed action");
    assert_eq!(actions[0].name, CHANGED_ACTION);
    assert!(as_input(&g).selection().is_none());
    assert_eq!(as_input(&g).caret(), 1, "caret collapses to the deletion point");

    // Delete on a fresh selection also removes the range.
    g.key(KeyInput::new(keys::HOME, 0));
    g.key(KeyInput::new(keys::ARROW_RIGHT, keys::modifiers::SHIFT));
    assert_eq!(as_input(&g).selected_text(), "a");
    g.key(KeyInput::new(keys::DELETE, 0));
    assert_eq!(as_input(&g).text(), "e", "delete removes the selected char");
}

/// Multi-byte (UTF-8) safety: the caret index maps to real char boundaries when
/// clicking among wide/multi-byte glyphs — never splitting a codepoint.
#[test]
fn click_maps_to_utf8_char_boundaries() {
    // "é" is 2 bytes (0xC3 0xA9); "中" is 3 bytes. Buffer bytes: a(0) é(1..3) b(3) 中(4..7) c(7).
    let mut g = gallery_with(TextInput::new("x").with_text("aéb\u{4e2d}c"));
    focus(&mut g);
    let y = mid_y(&g);

    // Click on the 'b' glyph (byte offset 3) — left half → boundary 3.
    let gb = glyph_box(&g, 3);
    g.left_click(gb.x + 1.0, y);
    let _ = g.process();
    let c = as_input(&g).caret();
    assert!(
        g.doc().get(g.host.root_of("inp").unwrap()).is_some(),
        "sanity"
    );
    assert!(
        as_input(&g).text().is_char_boundary(c),
        "caret {c} must be on a UTF-8 char boundary"
    );
    assert_eq!(c, 3, "click on 'b' lands the caret at its byte boundary (3)");

    // Click on the wide '中' glyph (byte offset 4) — its right half → boundary 7.
    let gz = glyph_box(&g, 4);
    g.left_click(gz.x + gz.width - 1.0, y);
    let _ = g.process();
    let c2 = as_input(&g).caret();
    assert!(
        as_input(&g).text().is_char_boundary(c2),
        "caret {c2} must be on a UTF-8 char boundary (not mid-codepoint)"
    );
    assert_eq!(c2, 7, "click on the right half of '中' lands after it (byte 7)");
}
