//! `<lq-menu>` real-pipeline gallery tests (no fake-green).
//!
//! Teeth: hovering an item highlights it (restyles pixels); clicking an item's
//! LAID-OUT box fires Activate(id) — the recurring menu-geometry-from-CSS guard
//! (a constant item height would mis-target after CSS changes the padding);
//! disabled items + separators are not activatable; keyboard Up/Down skips
//! separators/disabled, Enter activates, Esc dismisses.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::menu::{Menu, MenuEntry, ACTIVATE_ACTION, DISMISS_ACTION, SUBMENU_ACTION};

const W: u32 = 280;
const H: u32 = 320;

fn as_menu<'a>(g: &'a Gallery, id: &str) -> &'a Menu {
    g.host.behavior(id).unwrap().as_any().downcast_ref::<Menu>().unwrap()
}

/// Menu: Open, Save, (sep), Disabled, More→submenu.
fn sample() -> Menu {
    Menu::with_entries(vec![
        MenuEntry::item("open", "Open"),
        MenuEntry::item("save", "Save"),
        MenuEntry::separator(),
        MenuEntry::disabled_item("paste", "Paste"),
        MenuEntry::submenu("more", "More"),
    ])
}

fn item_box(g: &Gallery, id: &str, i: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("item-{i}")).expect("item box")
}

fn click_item(g: &mut Gallery, id: &str, i: usize) -> Vec<crate::host::WidgetAction> {
    let r = item_box(g, id, i);
    g.left_click(r.x + r.width / 2.0, r.y + r.height / 2.0);
    g.process()
}

/// Clicking an enabled item's LAID-OUT box fires Activate(id).
#[test]
fn click_activates_item() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("m", Box::new(sample()));
    g.relayout();

    let actions = click_item(&mut g, "m", 1); // "Save"
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, ACTIVATE_ACTION);
    assert_eq!(actions[0].payload.as_deref(), Some("save"));
}

/// NO-FAKE-GREEN tooth: with tall items, a click in item-1's REAL box activates
/// "save" — a constant item-height index would mis-map to the wrong item.
#[test]
fn item_hit_comes_from_layout_not_constant() {
    let css = "lq-gallery { padding: 8px; } lq-menu > lq-menu-item { padding-top: 16px; padding-bottom: 16px; }";
    let mut g = Gallery::new(W, H, css);
    g.mount("m", Box::new(sample()));
    g.relayout();

    let i0 = item_box(&g, "m", 0);
    let i1 = item_box(&g, "m", 1);
    assert!(i1.height > 36.0, "items tall (got {})", i1.height);
    assert!(i1.y > i0.y + 36.0, "item 1 below a constant-height guess");

    g.left_click(i1.x + i1.width / 2.0, i1.y + i1.height / 2.0);
    let actions = g.process();
    assert_eq!(actions[0].payload.as_deref(), Some("save"), "hit from layout");
}

/// Clicking a disabled item does nothing; clicking a separator does nothing.
#[test]
fn disabled_and_separator_are_inert() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("m", Box::new(sample()));
    g.relayout();

    // The disabled "Paste" is entry index 3.
    let actions = click_item(&mut g, "m", 3);
    assert!(actions.is_empty(), "disabled item not activatable");
    assert_eq!(as_menu(&g, "m").highlighted(), None);
}

/// A submenu parent fires Submenu(id) on click and on ArrowRight when highlighted.
#[test]
fn submenu_parent_opens() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("m", Box::new(sample()));
    g.relayout();

    // Click the "More" submenu parent (entry index 4).
    let actions = click_item(&mut g, "m", 4);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, SUBMENU_ACTION);
    assert_eq!(actions[0].payload.as_deref(), Some("more"));
}

/// Hovering an item highlights it and restyles its pixels.
#[test]
fn hover_highlights_and_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("m", Box::new(sample()));
    g.relayout();

    let i0 = item_box(&g, "m", 0);
    let (cx, cy) = ((i0.x + i0.width / 2.0) as u32, (i0.y + i0.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    g.pointer_move(i0.x + i0.width / 2.0, i0.y + i0.height / 2.0);
    let _ = g.process();
    assert_eq!(as_menu(&g, "m").highlighted(), Some(0));
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(before != after, "hover restyles (before {before:?} after {after:?})");
}

/// Keyboard Down/Up skip the separator + disabled item; Enter activates.
#[test]
fn keyboard_skips_inert_and_activates() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("m", Box::new(sample()));
    g.relayout();
    g.host.set_focus(Some("m"), &mut g.doc, &mut g.dispatcher);

    // Down from nothing → first activatable (0 = Open).
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_menu(&g, "m").highlighted(), Some(0));
    // Down → 1 (Save).
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_menu(&g, "m").highlighted(), Some(1));
    // Down → skips separator (2) and disabled (3) → submenu parent (4 = More).
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(
        as_menu(&g, "m").highlighted(),
        Some(4),
        "Down skips the separator + disabled item"
    );
    // Down again wraps to 0.
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_menu(&g, "m").highlighted(), Some(0));

    // Enter activates the highlighted (0 = Open).
    let actions = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, ACTIVATE_ACTION);
    assert_eq!(actions[0].payload.as_deref(), Some("open"));
}

/// Up from nothing highlights the last activatable; wraps correctly.
#[test]
fn keyboard_up_from_empty_highlights_last() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("m", Box::new(sample()));
    g.relayout();
    g.host.set_focus(Some("m"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_UP, 0));
    assert_eq!(as_menu(&g, "m").highlighted(), Some(4), "Up → last activatable");
    // Up again skips disabled(3)+separator(2) → 1 (Save).
    g.key(KeyInput::new(keys::ARROW_UP, 0));
    assert_eq!(as_menu(&g, "m").highlighted(), Some(1));
}

/// Esc emits a Dismiss action.
#[test]
fn escape_dismisses() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("m", Box::new(sample()));
    g.relayout();
    g.host.set_focus(Some("m"), &mut g.doc, &mut g.dispatcher);
    let actions = g.key(KeyInput::new(keys::ESCAPE, 0));
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, DISMISS_ACTION);
}

/// Home/End jump to first/last activatable.
#[test]
fn home_end_jump_activatable() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("m", Box::new(sample()));
    g.relayout();
    g.host.set_focus(Some("m"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_menu(&g, "m").highlighted(), Some(4));
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_menu(&g, "m").highlighted(), Some(0));
}
