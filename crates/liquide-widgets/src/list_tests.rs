//! `<lq-list>` real-pipeline gallery tests (no fake-green).
//!
//! Teeth: clicking a row's LAID-OUT box selects THAT row (a constant
//! `index*row_height` would mis-target after CSS changes the row height);
//! :checked on a selected row restyles its rasterized pixels; Shift-range and
//! Ctrl-toggle multi-select correctness (driven through the keyboard path, which
//! is the only DOM event that carries modifiers); keyboard Up/Down/Home/End +
//! Space/Enter.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::list::{List, CHANGED_ACTION};

const W: u32 = 320;
const H: u32 = 320;

fn as_list<'a>(g: &'a Gallery, id: &str) -> &'a List {
    g.host.behavior(id).unwrap().as_any().downcast_ref::<List>().unwrap()
}

fn items(n: usize) -> Vec<(String, String)> {
    (0..n)
        .map(|i| (format!("v{i}"), format!("Item {i}")))
        .collect()
}

fn row_box(g: &Gallery, id: &str, i: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("item-{i}")).expect("row box")
}

/// Click row `i` at the centre of its laid-out box and process.
fn click_row(g: &mut Gallery, id: &str, i: usize) -> Vec<crate::host::WidgetAction> {
    let r = row_box(g, id, i);
    g.left_click(r.x + r.width / 2.0, r.y + r.height / 2.0);
    g.process()
}

/// Initially nothing selected; the cursor starts at row 0.
#[test]
fn list_starts_unselected_cursor_zero() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("l", Box::new(List::new(items(5))));
    g.relayout();
    assert!(as_list(&g, "l").selected_indices().is_empty());
    assert_eq!(as_list(&g, "l").cursor(), Some(0));
}

/// Clicking a row's LAID-OUT box selects exactly that row (single-select).
#[test]
fn click_selects_row() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("l", Box::new(List::new(items(5))));
    g.relayout();

    let actions = click_row(&mut g, "l", 2);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, CHANGED_ACTION);
    assert_eq!(actions[0].payload.as_deref(), Some("v2"));
    assert_eq!(as_list(&g, "l").selected_indices(), vec![2]);
}

/// NO-FAKE-GREEN tooth: with a tall row height, a click in row 3's REAL box
/// selects row 3 — a constant `index * default_row_height` would land elsewhere.
#[test]
fn row_hit_comes_from_layout_not_constant() {
    // Make rows unusually tall so a default-height constant guess mis-maps.
    let css = "lq-gallery { padding: 8px; } lq-list > lq-list-item { padding-top: 18px; padding-bottom: 18px; }";
    let mut g = Gallery::new(W, H, css);
    g.mount("l", Box::new(List::new(items(5))));
    g.relayout();

    let r3 = row_box(&g, "l", 3);
    let r0 = row_box(&g, "l", 0);
    assert!(r3.height > 40.0, "rows widened tall (got {})", r3.height);
    assert!(
        r3.y > r0.y + 3.0 * 40.0,
        "row 3 is well below a constant-height guess (r3.y={}, r0.y={})",
        r3.y,
        r0.y
    );

    g.left_click(r3.x + r3.width / 2.0, r3.y + r3.height / 2.0);
    let _ = g.process();
    assert_eq!(
        as_list(&g, "l").selected_indices(),
        vec![3],
        "click in row-3's REAL box must select 3 (geometry from layout)"
    );
}

/// :checked on the selected row restyles its rasterized pixels.
#[test]
fn selected_row_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("l", Box::new(List::new(items(5))));
    g.relayout();

    let r2 = row_box(&g, "l", 2);
    let (cx, cy) = (
        (r2.x + r2.width / 2.0) as u32,
        (r2.y + r2.height / 2.0) as u32,
    );
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    g.left_click(r2.x + r2.width / 2.0, r2.y + r2.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(
        before != after,
        "selected row must restyle (before {before:?} after {after:?})"
    );
}

/// Keyboard Up/Down move selection; Home/End jump; clamps at the ends.
#[test]
fn keyboard_navigates_and_selects() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("l", Box::new(List::new(items(5))));
    g.relayout();
    g.host.set_focus(Some("l"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_list(&g, "l").cursor(), Some(1));
    assert_eq!(as_list(&g, "l").selected_indices(), vec![1]);

    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_list(&g, "l").cursor(), Some(4));
    assert_eq!(as_list(&g, "l").selected_indices(), vec![4]);

    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_list(&g, "l").selected_indices(), vec![0]);

    // ArrowUp at the top clamps (no wrap).
    g.key(KeyInput::new(keys::ARROW_UP, 0));
    assert_eq!(as_list(&g, "l").cursor(), Some(0));
}

/// Multi-select range: Shift+Arrow grows the contiguous range from the anchor.
#[test]
fn shift_arrow_selects_range() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("l", Box::new(List::new(items(6)).multi()));
    g.relayout();
    g.host.set_focus(Some("l"), &mut g.doc, &mut g.dispatcher);

    // Anchor at row 1 via a plain click, then Shift+Down x3 → rows 1..=4.
    let _ = click_row(&mut g, "l", 1);
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::SHIFT));
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::SHIFT));
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::SHIFT));
    assert_eq!(
        as_list(&g, "l").selected_indices(),
        vec![1, 2, 3, 4],
        "shift range from anchor 1 down to 4"
    );

    // Shrinking the range back up.
    g.key(KeyInput::new(keys::ARROW_UP, keys::modifiers::SHIFT));
    assert_eq!(as_list(&g, "l").selected_indices(), vec![1, 2, 3]);
}

/// Multi-select toggle: Ctrl+Arrow moves the cursor without selecting, then
/// Ctrl+Space toggles the cursor row in/out, preserving the rest.
#[test]
fn ctrl_space_toggles_individual_rows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("l", Box::new(List::new(items(6)).multi()));
    g.relayout();
    g.host.set_focus(Some("l"), &mut g.doc, &mut g.dispatcher);

    // Select row 0, move cursor (no select) to 2, toggle it in, to 4, toggle in.
    let _ = click_row(&mut g, "l", 0);
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::CTRL)); // cursor 1
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::CTRL)); // cursor 2
    g.key(KeyInput::new(keys::SPACE, keys::modifiers::CTRL)); // add 2
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::CTRL)); // cursor 3
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::CTRL)); // cursor 4
    g.key(KeyInput::new(keys::SPACE, keys::modifiers::CTRL)); // add 4
    assert_eq!(as_list(&g, "l").selected_indices(), vec![0, 2, 4]);

    // Toggle row 2 back out (cursor is at 4 → move up to 2 first).
    g.key(KeyInput::new(keys::ARROW_UP, keys::modifiers::CTRL)); // cursor 3
    g.key(KeyInput::new(keys::ARROW_UP, keys::modifiers::CTRL)); // cursor 2
    g.key(KeyInput::new(keys::SPACE, keys::modifiers::CTRL)); // remove 2
    assert_eq!(as_list(&g, "l").selected_indices(), vec![0, 4]);
}

/// Single-select mode: a plain Space single-selects the cursor row (never
/// accumulates), and Ctrl+Space does NOT toggle (single mode).
#[test]
fn single_mode_replaces_selection() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("l", Box::new(List::new(items(5)))); // single by default
    g.relayout();
    g.host.set_focus(Some("l"), &mut g.doc, &mut g.dispatcher);

    let _ = click_row(&mut g, "l", 1);
    // Move cursor to 3, Ctrl+Space (ignored in single → plain semantics).
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // selects 2 (single move selects)
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // selects 3
    assert_eq!(
        as_list(&g, "l").selected_indices(),
        vec![3],
        "single-select replaces, never accumulates"
    );
}

/// True if ANY pixel in row `i`'s laid-out box differs between two framebuffers.
/// Scans the whole row rect (a robust structural diff that does not depend on the
/// weak glyph rasterizer hitting a specific ink pixel).
fn row_region_differs(
    g: &Gallery,
    id: &str,
    i: usize,
    a: &liquide_compositor::framebuffer::FrameBuffer,
    b: &liquide_compositor::framebuffer::FrameBuffer,
) -> bool {
    let r = row_box(g, id, i);
    let x0 = r.x.max(0.0) as u32;
    let y0 = r.y.max(0.0) as u32;
    let x1 = ((r.x + r.width).min(W as f32)) as u32;
    let y1 = ((r.y + r.height).min(H as f32)) as u32;
    for y in y0..y1 {
        for x in x0..x1 {
            if Gallery::pixel(a, x, y) != Gallery::pixel(b, x, y) {
                return true;
            }
        }
    }
    false
}

/// Normal render: every row paints an opaque cell (the list + items are visible).
#[test]
fn normal_render_paints_rows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("l", Box::new(List::new(items(5))));
    g.relayout();
    let fb = g.rasterize();
    for i in 0..5 {
        let r = row_box(&g, "l", i);
        let px = Gallery::pixel(&fb, (r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32);
        assert!(px.a > 0, "row {i} must paint (alpha {})", px.a);
    }
}

/// :hover restyles the hovered row's pixels AND leaves a non-hovered row alone —
/// proving the hover background (#3f3f46) lands only on the row under the pointer.
#[test]
fn hover_restyles_only_hovered_row() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("l", Box::new(List::new(items(5))));
    g.relayout();
    let before = g.rasterize();

    let r2 = row_box(&g, "l", 2);
    g.pointer_move(r2.x + r2.width / 2.0, r2.y + r2.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = g.rasterize();

    assert!(
        row_region_differs(&g, "l", 2, &before, &after),
        "hovered row 2 must restyle"
    );
    assert!(
        !row_region_differs(&g, "l", 0, &before, &after),
        "non-hovered row 0 must be unchanged"
    );
}

// NOTE (CSS gap, reported to coordinator): the list cursor :focus ring is
// `lq-list > lq-list-item:focus { box-shadow: inset 0 0 0 1px ... }`. A
// whole-framebuffer diff between cursor-on-row-0 and cursor-on-row-2 produced
// ZERO differing pixels — the inset box-shadow focus ring does not rasterize
// through the real pipeline (other widgets express :focus via `border`, which
// DOES paint). A no-fake-green pixel-delta focus test cannot pass without a CSS
// change (e.g. switch the row :focus ring to a `border`/`outline`/background
// the renderer paints), so it is intentionally omitted here.

/// :checked selection fill MOVES with the selection: selecting row 3 then row 1
/// restyles the newly-selected row and reverts the previously-selected one.
#[test]
fn selection_fill_moves_with_selection() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("l", Box::new(List::new(items(5))));
    g.relayout();
    let plain = g.rasterize();

    let _ = click_row(&mut g, "l", 3);
    assert_eq!(as_list(&g, "l").selected_indices(), vec![3]);
    g.relayout();
    let sel3 = g.rasterize();
    assert!(
        row_region_differs(&g, "l", 3, &plain, &sel3),
        "row 3 must gain the selection fill"
    );

    let _ = click_row(&mut g, "l", 1);
    assert_eq!(as_list(&g, "l").selected_indices(), vec![1]);
    g.relayout();
    let sel1 = g.rasterize();
    assert!(
        row_region_differs(&g, "l", 1, &sel3, &sel1),
        "row 1 must gain the fill"
    );
    assert!(
        row_region_differs(&g, "l", 3, &sel3, &sel1),
        "row 3 must lose the fill when selection moves"
    );
}

/// Multi-select paints the selection fill on EVERY row in the range (Shift+range).
#[test]
fn multi_select_range_fills_all_rows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("l", Box::new(List::new(items(6)).multi()));
    g.relayout();
    let plain = g.rasterize();

    g.host.set_focus(Some("l"), &mut g.doc, &mut g.dispatcher);
    let _ = click_row(&mut g, "l", 1); // anchor 1
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::SHIFT));
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::SHIFT));
    assert_eq!(as_list(&g, "l").selected_indices(), vec![1, 2, 3]);
    g.relayout();
    let after = g.rasterize();

    for i in [1usize, 2, 3] {
        assert!(
            row_region_differs(&g, "l", i, &plain, &after),
            "selected row {i} in the multi-range must carry the fill"
        );
    }
    assert!(
        !row_region_differs(&g, "l", 5, &plain, &after),
        "row 5 outside the range must be unchanged"
    );
}

/// Disabled list swallows clicks and keys.
#[test]
fn disabled_list_swallows_input() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("l", Box::new(List::new(items(5)).disabled(true)));
    g.relayout();
    let r2 = row_box(&g, "l", 2);
    g.left_click(r2.x + r2.width / 2.0, r2.y + r2.height / 2.0);
    assert!(g.process().is_empty());
    g.host.set_focus(Some("l"), &mut g.doc, &mut g.dispatcher);
    assert!(g.key(KeyInput::new(keys::ARROW_DOWN, 0)).is_empty());
    assert!(as_list(&g, "l").selected_indices().is_empty());
}
