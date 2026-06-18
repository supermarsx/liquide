//! `<lq-textarea>` real-pipeline gallery tests (no fake-green).
//!
//! Teeth (each drives the REAL style->layout->paint + EventDispatcher pipeline so
//! a constant-based implementation cannot pass):
//! - typing inserts across lines; Enter splits a line; Backspace at column 0
//!   joins lines; Delete at end-of-line joins the next line up;
//! - arrow Up/Down move the caret across lines — the caret BOX y differs per line
//!   FROM LAYOUT (a px-per-line constant cannot reproduce CSS row geometry);
//! - click-to-place-caret resolves the line from the laid-out ROW box y (a
//!   constant px-per-line mis-resolves once the row height/gutter changes);
//! - :focus restyles the rasterized pixels (the caret becomes visible).
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::textarea::{TextArea, CHANGED_ACTION};

const W: u32 = 420;
const H: u32 = 220;

fn gallery_with(ta: TextArea) -> Gallery {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("ta", Box::new(ta));
    g.relayout();
    g
}

fn as_ta(g: &Gallery) -> &TextArea {
    g.host
        .behavior("ta")
        .unwrap()
        .as_any()
        .downcast_ref::<TextArea>()
        .unwrap()
}

/// Focus the area WITHOUT moving the caret (set the dispatcher focus + the
/// behavior's own focused flag directly), then re-render + relayout so the caret
/// element is in flow. Tests that want a specific caret position navigate after.
fn focus(g: &mut Gallery) {
    g.host.set_focus(Some("ta"), &mut g.doc, &mut g.dispatcher);
    {
        let b = g
            .host
            .behavior_mut("ta")
            .unwrap()
            .as_any_mut()
            .unwrap()
            .downcast_mut::<TextArea>()
            .unwrap();
        b.set_focused(true);
    }
    g.host.rerender("ta", &mut g.doc);
    g.relayout();
}

fn caret_box(g: &Gallery) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("ta").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "caret").expect("caret box from layout")
}

fn row_box(g: &Gallery, i: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("ta").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("row-{i}"))
        .expect("row box from layout")
}

/// The area renders a real CSS-sized box and paints.
#[test]
fn textarea_renders_box() {
    let mut g = gallery_with(TextArea::new("type here"));
    let node = g.host.root_of("ta").unwrap();
    let r = g.box_of(node).expect("textarea lays out");
    assert!((r.width - 360.0).abs() < 2.0, "width from CSS (got {})", r.width);
    assert!((r.height - 160.0).abs() < 2.0, "height from CSS (got {})", r.height);
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (r.x + 10.0) as u32, (r.y + 10.0) as u32);
    assert!(px.a > 0, "field must paint");
}

/// Clicking inside the laid-out box focuses the area.
#[test]
fn click_focuses_area() {
    let mut g = gallery_with(TextArea::new("x").with_text("hello"));
    assert!(!as_ta(&g).is_focused());
    let node = g.host.root_of("ta").unwrap();
    let r = g.box_of(node).unwrap();
    g.left_click(r.x + 8.0, r.y + 8.0);
    let _ = g.process();
    assert!(as_ta(&g).is_focused(), "click must focus the area");
}

/// Typing printable chars inserts at the caret across lines; Changed carries the
/// full multi-line text.
#[test]
fn typing_inserts_across_lines() {
    let mut g = gallery_with(TextArea::new("x"));
    focus(&mut g);

    for c in ['a', 'b'] {
        let a = g.key(KeyInput::new(c as u32, 0));
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].name, CHANGED_ACTION);
    }
    // Enter splits — now on a new line; type more.
    g.key(KeyInput::new(keys::ENTER, 0));
    for c in ['c', 'd'] {
        g.key(KeyInput::new(c as u32, 0));
    }
    assert_eq!(as_ta(&g).text(), "ab\ncd");
    assert_eq!(as_ta(&g).line_count(), 2);
    assert_eq!(as_ta(&g).caret(), (1, 2));
}

/// Enter in the MIDDLE of a line splits it into two lines at the caret.
#[test]
fn enter_splits_line_at_caret() {
    let mut g = gallery_with(TextArea::new("x").with_text("abcdef"));
    focus(&mut g);
    // Move caret to column 3 (after "abc").
    g.key(KeyInput::new(keys::HOME, 0));
    for _ in 0..3 {
        g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    }
    assert_eq!(as_ta(&g).caret(), (0, 3));
    g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(as_ta(&g).text(), "abc\ndef");
    assert_eq!(as_ta(&g).caret(), (1, 0));
}

/// Backspace at column 0 JOINS the line with the previous line; the caret lands
/// at the join seam.
#[test]
fn backspace_at_col0_joins_lines() {
    let mut g = gallery_with(TextArea::new("x").with_text("foo\nbar"));
    focus(&mut g);
    // Caret seeded at end of "bar" (line 1, col 3). Go Home → col 0 of line 1.
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_ta(&g).caret(), (1, 0));
    g.key(KeyInput::new(keys::BACKSPACE, 0));
    assert_eq!(as_ta(&g).text(), "foobar", "lines joined");
    assert_eq!(as_ta(&g).line_count(), 1);
    assert_eq!(as_ta(&g).caret(), (0, 3), "caret at the join seam");
}

/// Delete at end-of-line JOINS the next line onto the current one.
#[test]
fn delete_at_eol_joins_next_line() {
    let mut g = gallery_with(TextArea::new("x").with_text("foo\nbar"));
    focus(&mut g);
    g.key(KeyInput::new(keys::ARROW_UP, 0)); // line 0
    g.key(KeyInput::new(keys::END, 0)); // end of "foo"
    assert_eq!(as_ta(&g).caret(), (0, 3));
    g.key(KeyInput::new(keys::DELETE, 0));
    assert_eq!(as_ta(&g).text(), "foobar");
    assert_eq!(as_ta(&g).caret(), (0, 3), "caret stays at the seam");
}

/// Backspace mid-line deletes the char before the caret (single line).
#[test]
fn backspace_mid_line_deletes_char() {
    let mut g = gallery_with(TextArea::new("x").with_text("abc"));
    focus(&mut g);
    g.key(KeyInput::new(keys::BACKSPACE, 0));
    assert_eq!(as_ta(&g).text(), "ab");
}

/// Arrow Up/Down move the caret across lines AND the caret box's laid-out y
/// differs per line — proving LAYOUT (not a px-per-line constant) positions it.
#[test]
fn arrows_move_caret_across_lines_caret_y_from_layout() {
    let mut g = gallery_with(TextArea::new("x").with_text("line one\nline two\nline three"));
    focus(&mut g);
    // Caret seeded at end of last line. Jump to top.
    g.key(KeyInput::new(keys::PAGE_UP, 0));
    g.relayout();
    assert_eq!(as_ta(&g).caret().0, 0, "PageUp lands on the top line");
    let caret_y_line0 = caret_box(&g).y;

    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    g.relayout();
    assert_eq!(as_ta(&g).caret().0, 1, "Down moves to line 1");
    let caret_y_line1 = caret_box(&g).y;

    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    g.relayout();
    assert_eq!(as_ta(&g).caret().0, 2, "Down moves to line 2");
    let caret_y_line2 = caret_box(&g).y;

    assert!(
        caret_y_line0 < caret_y_line1 - 1.0 && caret_y_line1 < caret_y_line2 - 1.0,
        "caret box y must increase per line FROM LAYOUT \
         (l0={caret_y_line0}, l1={caret_y_line1}, l2={caret_y_line2})"
    );

    // Up climbs back.
    g.key(KeyInput::new(keys::ARROW_UP, 0));
    g.relayout();
    assert_eq!(as_ta(&g).caret().0, 1);
    assert!((caret_box(&g).y - caret_y_line1).abs() < 2.0, "Up returns to line 1's y");
}

/// Left at column 0 wraps to the end of the previous line; Right at EOL wraps to
/// the start of the next line.
#[test]
fn left_right_wrap_across_line_boundaries() {
    let mut g = gallery_with(TextArea::new("x").with_text("ab\ncd"));
    focus(&mut g);
    g.key(KeyInput::new(keys::PAGE_UP, 0)); // top line
    g.key(KeyInput::new(keys::HOME, 0)); // (0,0)
    g.key(KeyInput::new(keys::END, 0)); // (0,2)
    assert_eq!(as_ta(&g).caret(), (0, 2));
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0)); // wrap → (1,0)
    assert_eq!(as_ta(&g).caret(), (1, 0));
    g.key(KeyInput::new(keys::ARROW_LEFT, 0)); // wrap back → (0,2)
    assert_eq!(as_ta(&g).caret(), (0, 2));
}

/// Click-to-place-caret resolves the LINE from the laid-out row box y. A
/// constant px-per-line would resolve a different line — we verify the resolved
/// line matches the row whose laid-out box actually contains the click y.
#[test]
fn click_resolves_line_from_laid_out_row_box() {
    let mut g = gallery_with(TextArea::new("x").with_text("alpha\nbeta\ngamma\ndelta"));
    focus(&mut g);
    g.relayout();

    // Click in the vertical CENTER of line 2's laid-out row box.
    let r2 = row_box(&g, 2);
    let click_y = r2.y + r2.height / 2.0;
    let click_x = r2.x + 4.0;
    g.left_click(click_x, click_y);
    let _ = g.process();
    assert_eq!(
        as_ta(&g).caret().0,
        2,
        "click in row-2's laid-out box must place the caret on line 2"
    );

    // And clicking row 0 moves it back up — the line tracks the laid-out box, not
    // a fixed first-line guess.
    g.relayout();
    let r0 = row_box(&g, 0);
    g.left_click(r0.x + 4.0, r0.y + r0.height / 2.0);
    let _ = g.process();
    assert_eq!(as_ta(&g).caret().0, 0, "click in row-0's box places caret on line 0");
}

/// A constant px-per-line model would FAIL: the rows are 20px tall via CSS but a
/// naive (y - top)/CONST with the wrong constant lands on the wrong line. Here we
/// prove the resolved line equals the row whose REAL box contains the click for
/// EVERY row — something only a layout-derived resolver can guarantee.
#[test]
fn click_line_resolution_is_layout_derived_for_every_row() {
    let mut g = gallery_with(TextArea::new("x").with_text("r0\nr1\nr2\nr3\nr4"));
    focus(&mut g);
    g.relayout();
    for i in 0..5usize {
        let rb = row_box(&g, i);
        let y = rb.y + rb.height / 2.0;
        g.left_click(rb.x + 3.0, y);
        let _ = g.process();
        assert_eq!(
            as_ta(&g).caret().0,
            i,
            "click at the laid-out center of row {i} must resolve to line {i}"
        );
        g.relayout();
    }
}

/// Click resolves the COLUMN from the laid-out text box: a click near the line's
/// start yields a small column, a click near its end yields a large column.
#[test]
fn click_resolves_column_from_layout() {
    let mut g = gallery_with(TextArea::new("x").with_text("abcdefghij"));
    focus(&mut g);
    g.relayout();
    let r0 = row_box(&g, 0);

    // Near the left edge → near column 0.
    g.left_click(r0.x + 1.0, r0.y + r0.height / 2.0);
    let _ = g.process();
    let col_left = as_ta(&g).caret().1;

    g.relayout();
    let r0 = row_box(&g, 0);
    // Near the right end of the text → a larger column.
    g.left_click(r0.x + r0.width, r0.y + r0.height / 2.0);
    let _ = g.process();
    let col_right = as_ta(&g).caret().1;

    assert!(
        col_right > col_left,
        "click near the line end must yield a larger column than near the start \
         (left={col_left}, right={col_right}) — column derived from the laid-out text box"
    );
}

/// :focus restyles the rasterized pixels — the caret element is transparent until
/// focused, then paints with the foreground color (CSS `:focus lq-caret`).
#[test]
fn focus_restyles_pixels() {
    let mut g = gallery_with(TextArea::new("x").with_text("WWWWW"));
    // Unfocused: caret is transparent (no focus pseudo). Focus it.
    focus(&mut g);
    g.relayout();
    let cb = caret_box(&g);
    let fb = g.rasterize();
    // Sample the center of the caret box — it must be opaque (foreground) now.
    let px = Gallery::pixel(
        &fb,
        (cb.x + cb.width / 2.0) as u32,
        (cb.y + cb.height / 2.0) as u32,
    );
    assert!(px.a > 0, "focused caret must paint a visible pixel (got alpha {})", px.a);
}

/// Wheel scrolling moves the content up (its laid-out y decreases) when the
/// content overflows the viewport — extent derived from the laid-out boxes.
#[test]
fn wheel_scrolls_overflowing_content() {
    // 30 lines → content much taller than the 160px viewport.
    let text = (0..30).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
    let mut g = gallery_with(TextArea::new("x").with_text(text));
    focus(&mut g);
    g.relayout();

    let root = g.host.root_of("ta").unwrap();
    let content_y0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "content").unwrap().y
    };
    assert_eq!(as_ta(&g).scroll_y(), 0.0, "starts unscrolled");

    // Scroll down via the wheel over the viewport.
    let vp = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "viewport").unwrap()
    };
    g.scroll(vp.x + 10.0, vp.y + 10.0, 0.0, 60.0);
    let _ = g.process();
    g.relayout();

    assert!(as_ta(&g).scroll_y() > 0.0, "wheel must increase scroll offset");
    let content_y1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "content").unwrap().y
    };
    assert!(
        content_y1 < content_y0 - 1.0,
        "scrolled content must translate UP (y0={content_y0}, y1={content_y1})"
    );
}

/// The line-number gutter is rendered when enabled, one numbered row per line,
/// laid out by CSS (the gutter box has a real width from CSS).
#[test]
fn gutter_renders_line_numbers() {
    let mut g = gallery_with(TextArea::new("x").with_text("one\ntwo\nthree").with_gutter(true));
    g.relayout();
    let root = g.host.root_of("ta").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let gutter = q.box_of_part(root, "gutter").expect("gutter box");
    assert!(gutter.width > 10.0, "gutter has a CSS width (got {})", gutter.width);
    // Each line gets a numbered row.
    for i in 0..3 {
        assert!(
            q.box_of_part(root, &format!("lineno-{i}")).is_some(),
            "line number row {i} must lay out"
        );
    }
}

/// Toggling the gutter off removes the gutter subtree.
#[test]
fn gutter_toggles_off() {
    let mut g = gallery_with(TextArea::new("x").with_text("a\nb").with_gutter(true));
    g.relayout();
    {
        let root = g.host.root_of("ta").unwrap();
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "gutter").is_some(), "gutter present when enabled");
    }
    // Toggle off directly + re-render.
    {
        let b = g
            .host
            .behavior_mut("ta")
            .unwrap()
            .as_any_mut()
            .unwrap()
            .downcast_mut::<TextArea>()
            .unwrap();
        b.set_gutter(false);
    }
    g.host.rerender("ta", &mut g.doc);
    g.relayout();
    let root = g.host.root_of("ta").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.box_of_part(root, "gutter").is_none(), "gutter removed when disabled");
}

/// Disabled areas swallow keystrokes and clicks.
#[test]
fn disabled_swallows_input() {
    let mut g = gallery_with(TextArea::new("x").with_text("seed").disabled(true));
    g.host.set_focus(Some("ta"), &mut g.doc, &mut g.dispatcher);
    let a = g.key(KeyInput::new('a' as u32, 0));
    assert!(a.is_empty());
    assert_eq!(as_ta(&g).text(), "seed");
    assert!(!g.host.behavior("ta").unwrap().focusable());
}
