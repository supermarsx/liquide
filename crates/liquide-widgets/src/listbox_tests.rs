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
