//! `<lq-listbox>` + `<lq-listitem>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::listbox::{ListBox, ListItem, CHANGED_ACTION};

const W: u32 = 320;
const H: u32 = 360;

fn as_lb<'a>(g: &'a Gallery, id: &str) -> &'a ListBox {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<ListBox>()
        .unwrap()
}

fn fruits() -> ListBox {
    ListBox::new([
        ListItem::new("apple", "Apple"),
        ListItem::new("banana", "Banana"),
        ListItem::new("cherry", "Cherry"),
        ListItem::new("date", "Date"),
    ])
}

fn item_box(g: &Gallery, root: liquide_dom::NodeId, i: usize) -> liquide_layout::geometry::Rect {
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("item-{i}")).expect("item box")
}

/// Clicking an item selects it (from its laid-out box); emits Changed(value).
#[test]
fn click_selects_item() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lb", Box::new(fruits()));
    g.relayout();
    let root = g.host.root_of("lb").unwrap();
    let it = item_box(&g, root, 2);
    g.left_click(it.x + it.width / 2.0, it.y + it.height / 2.0);
    let a = g.process();
    let c = a.iter().find(|a| a.name == CHANGED_ACTION).expect("changed");
    assert_eq!(c.payload.as_deref(), Some("cherry"));
    assert_eq!(as_lb(&g, "lb").selected_indices(), vec![2]);
}

/// ANTI-CONSTANT: with one item made much taller, clicking item-1's REAL box
/// still selects item 1 — an `index * uniform_height` guess would mis-target.
#[test]
fn item_hit_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        W,
        H,
        // Make item-0 very tall so a uniform-pitch guess for item-1 is wrong.
        "lq-listbox > lq-listitem[data-index=\"0\"] { padding-top: 48px; padding-bottom: 48px; }",
    );
    g.mount("lb", Box::new(fruits()));
    g.relayout();
    let root = g.host.root_of("lb").unwrap();
    let it0 = item_box(&g, root, 0);
    let it1 = item_box(&g, root, 1);
    assert!(it0.height > it1.height + 20.0, "precondition: item-0 is much taller");
    g.left_click(it1.x + it1.width / 2.0, it1.y + it1.height / 2.0);
    let a = g.process();
    assert_eq!(
        a.iter().find(|a| a.name == CHANGED_ACTION).unwrap().payload.as_deref(),
        Some("banana"),
        "click in item-1's REAL (post-tall-item-0) box selects item 1"
    );
}

/// Keyboard Down/Up move the cursor + single selection; Home/End jump.
#[test]
fn keyboard_navigates() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lb", Box::new(fruits().select(0)));
    g.relayout();
    g.host.set_focus(Some("lb"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_lb(&g, "lb").cursor(), Some(1));
    assert_eq!(as_lb(&g, "lb").selected_indices(), vec![1]);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_lb(&g, "lb").cursor(), Some(3));
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_lb(&g, "lb").cursor(), Some(0));
}

/// Disabled items are skipped by keyboard nav and reject clicks.
#[test]
fn disabled_items_are_skipped() {
    let mut g = Gallery::new(W, H, "");
    g.mount(
        "lb",
        Box::new(ListBox::new([
            ListItem::new("a", "Alpha"),
            ListItem::new("b", "Bravo").disabled(true),
            ListItem::new("c", "Charlie"),
        ])),
    );
    g.relayout();
    g.host.set_focus(Some("lb"), &mut g.doc, &mut g.dispatcher);

    // Cursor starts on item 0; Down skips the disabled item 1 to land on 2.
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_lb(&g, "lb").cursor(), Some(2), "Down skips the disabled middle item");

    // A click on the disabled item does not select it.
    let root = g.host.root_of("lb").unwrap();
    let it1 = item_box(&g, root, 1);
    g.left_click(it1.x + it1.width / 2.0, it1.y + it1.height / 2.0);
    let _ = g.process();
    assert!(
        !as_lb(&g, "lb").is_selected(1),
        "a disabled item rejects a click"
    );
}

/// Type-ahead: pressing a letter jumps the cursor to the next item starting with
/// it (and selects it in single-select mode).
#[test]
fn type_ahead_jumps_to_matching_item() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lb", Box::new(fruits()));
    g.relayout();
    g.host.set_focus(Some("lb"), &mut g.doc, &mut g.dispatcher);

    // Press 'c' -> Cherry (index 2).
    g.key(KeyInput::new('c' as u32, 0));
    assert_eq!(as_lb(&g, "lb").selected_indices(), vec![2], "type-ahead 'c' -> Cherry");
    // Press 'd' -> Date (index 3).
    g.key(KeyInput::new('d' as u32, 0));
    assert_eq!(as_lb(&g, "lb").selected_indices(), vec![3], "type-ahead 'd' -> Date");
}

/// Multi-select: Ctrl+Space toggles the cursor item without clearing others;
/// Shift+Down extends a contiguous range.
#[test]
fn multi_select_toggle_and_range() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lb", Box::new(fruits().multi().select(0)));
    g.relayout();
    g.host.set_focus(Some("lb"), &mut g.doc, &mut g.dispatcher);

    // Ctrl+Down moves cursor to 1 without changing selection (still {0}).
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::CTRL));
    assert_eq!(as_lb(&g, "lb").cursor(), Some(1));
    assert_eq!(as_lb(&g, "lb").selected_indices(), vec![0]);
    // Ctrl+Space toggles item 1 into the selection -> {0, 1}.
    g.key(KeyInput::new(keys::SPACE, keys::modifiers::CTRL));
    assert_eq!(as_lb(&g, "lb").selected_indices(), vec![0, 1]);

    // Shift+Down extends the range from the anchor (1) to 2 -> {1, 2}.
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::SHIFT));
    assert_eq!(as_lb(&g, "lb").selected_indices(), vec![1, 2]);
}

/// PIXELS: selecting an item restyles its rasterized pixels (selection fill).
#[test]
fn selection_restyles_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lb", Box::new(fruits()));
    g.relayout();
    let root = g.host.root_of("lb").unwrap();
    let it = item_box(&g, root, 1);
    let (sx, sy) = ((it.x + it.width / 2.0) as u32, (it.y + it.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);
    g.left_click(it.x + it.width / 2.0, it.y + it.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "selecting an item must restyle its pixels");
}

/// PIXELS :hover — hovering an item restyles its pixels (the hover fill), and the
/// delta lands on the HOVERED item only.
#[test]
fn hovered_item_restyles_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lb", Box::new(fruits()));
    g.relayout();
    let root = g.host.root_of("lb").unwrap();
    // item-2 is neither selected nor the default cursor (item-0), so its only
    // restyle source here is :hover.
    let (it2, it3) = (item_box(&g, root, 2), item_box(&g, root, 3));
    let (hx, hy) = ((it2.x + 4.0) as u32, (it2.y + it2.height / 2.0) as u32);
    let (nx, ny) = ((it3.x + 4.0) as u32, (it3.y + it3.height / 2.0) as u32);
    let before_h = Gallery::pixel(&g.rasterize(), hx, hy);
    let before_n = Gallery::pixel(&g.rasterize(), nx, ny);

    g.pointer_move(it2.x + it2.width / 2.0, it2.y + it2.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after_h = Gallery::pixel(&g.rasterize(), hx, hy);
    let after_n = Gallery::pixel(&g.rasterize(), nx, ny);
    assert!(before_h != after_h, "the hovered item must restyle (before {before_h:?} after {after_h:?})");
    assert_eq!(before_n, after_n, "a non-hovered item must not change");
}

/// PIXELS :focus — the keyboard cursor paints a focus ring (inset box-shadow) on
/// the cursor item, and that ring MOVES with the cursor (off the old item, onto
/// the new one). item-0 is the default cursor, so we drive Down to item 1.
#[test]
fn focus_ring_moves_with_cursor() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lb", Box::new(fruits()));
    g.relayout();
    g.host.set_focus(Some("lb"), &mut g.doc, &mut g.dispatcher);
    let root = g.host.root_of("lb").unwrap();
    let it1 = item_box(&g, root, 1);
    // Sample the ring region (left inset edge), away from glyph ink.
    let (sx, sy) = ((it1.x + 1.0) as u32, (it1.y + it1.height / 2.0) as u32);
    // Selection would also restyle item 1; keep selection on item 0 by using
    // Ctrl+Down (moves cursor only). First confirm the unfocused baseline.
    let baseline = Gallery::pixel(&g.rasterize(), sx, sy);
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::CTRL));
    assert_eq!(as_lb(&g, "lb").cursor(), Some(1), "cursor moved to item 1");
    assert!(as_lb(&g, "lb").selected_indices().is_empty(), "Ctrl+Down did not select");
    g.relayout();
    let focused = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(
        baseline != focused,
        "the focus ring must paint on the cursor item (before {baseline:?} after {focused:?})"
    );

    // Moving the cursor on to item 2 must clear item 1's ring (returns to baseline).
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::CTRL));
    assert_eq!(as_lb(&g, "lb").cursor(), Some(2));
    g.relayout();
    let after_move = Gallery::pixel(&g.rasterize(), sx, sy);
    assert_eq!(after_move, baseline, "the ring must leave item 1 when the cursor moves on");
}

/// PIXELS :checked — selecting a SECOND item (multi-select) restyles its pixels
/// while the first stays selected; both carry the selection fill.
#[test]
fn multi_select_both_items_restyle_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lb", Box::new(fruits().multi()));
    g.relayout();
    let root = g.host.root_of("lb").unwrap();
    let (it0, it2) = (item_box(&g, root, 0), item_box(&g, root, 2));
    let (x0, y0) = ((it0.x + it0.width / 2.0) as u32, (it0.y + it0.height / 2.0) as u32);
    let (x2, y2) = ((it2.x + 4.0) as u32, (it2.y + it2.height / 2.0) as u32);
    let base0 = Gallery::pixel(&g.rasterize(), x0, y0);
    let base2 = Gallery::pixel(&g.rasterize(), x2, y2);

    // Click item 0 then Ctrl+click... clicks coalesce; drive via keyboard instead:
    g.host.set_focus(Some("lb"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::SPACE, keys::modifiers::CTRL)); // toggle item 0 (cursor)
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::CTRL)); // cursor -> 1
    g.key(KeyInput::new(keys::ARROW_DOWN, keys::modifiers::CTRL)); // cursor -> 2
    g.key(KeyInput::new(keys::SPACE, keys::modifiers::CTRL)); // toggle item 2
    assert_eq!(as_lb(&g, "lb").selected_indices(), vec![0, 2], "both selected");
    g.relayout();
    let sel0 = Gallery::pixel(&g.rasterize(), x0, y0);
    let sel2 = Gallery::pixel(&g.rasterize(), x2, y2);
    assert!(sel0 != base0, "item 0 selection fill differs from baseline");
    assert!(sel2 != base2, "item 2 selection fill differs from baseline");
}

/// PIXELS :disabled — a disabled item renders with the muted disabled colour,
/// distinct from an enabled sibling's text colour. Sampled over the glyph band.
#[test]
fn disabled_item_restyles_pixels() {
    let mut g = Gallery::new(W, H, "");
    // Two items with identical labels except one is disabled; compare their ink.
    g.mount(
        "lb",
        Box::new(ListBox::new([
            ListItem::new("a", "WWWW"),
            ListItem::new("b", "WWWW").disabled(true),
        ])),
    );
    g.relayout();
    let root = g.host.root_of("lb").unwrap();
    let (it0, it1) = (item_box(&g, root, 0), item_box(&g, root, 1));
    // Sum the ink alpha+luma across each item's glyph band so a colour change is
    // visible even with the weak glyph rasterizer.
    let ink = |g: &mut Gallery, r: liquide_layout::geometry::Rect| -> u32 {
        let fb = g.rasterize();
        let y = (r.y + r.height / 2.0) as u32;
        let x0 = (r.x + 6.0) as u32;
        let x1 = (r.x + r.width - 6.0) as u32;
        (x0..x1).map(|x| {
            let p = Gallery::pixel(&fb, x, y);
            p.r as u32 + p.g as u32 + p.b as u32 + p.a as u32
        }).sum()
    };
    let enabled_ink = ink(&mut g, it0);
    let disabled_ink = ink(&mut g, it1);
    assert!(
        enabled_ink != disabled_ink,
        "a disabled item must render with a different ink than its enabled twin \
         (enabled {enabled_ink}, disabled {disabled_ink})"
    );
}
