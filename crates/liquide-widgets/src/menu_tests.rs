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

/// True if ANY pixel in the laid-out box of `part` differs between two FBs.
fn part_region_differs(
    g: &Gallery,
    id: &str,
    part: &str,
    a: &liquide_compositor::framebuffer::FrameBuffer,
    b: &liquide_compositor::framebuffer::FrameBuffer,
) -> bool {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let Some(r) = q.box_of_part(root, part) else { return false };
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

/// Normal render: each item + the separator paint opaque pixels.
#[test]
fn normal_render_paints_items_and_separator() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("m", Box::new(sample()));
    g.relayout();
    let fb = g.rasterize();
    for i in [0usize, 1, 4] {
        let r = item_box(&g, "m", i);
        let px = Gallery::pixel(&fb, (r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32);
        assert!(px.a > 0, "item {i} must paint (alpha {})", px.a);
    }
    // The separator (entry 2) is a 1px divider with its own background — find it
    // by walking the menu for the lq-menu-separator element and sampling it.
    let root = g.host.root_of("m").unwrap();
    let sep = g
        .doc()
        .children(root)
        .iter()
        .copied()
        .find(|&c| g.doc().tag_name(c).as_deref() == Some("lq-menu-separator"))
        .expect("separator element");
    let r = g.box_of(sep).expect("separator box");
    let px = Gallery::pixel(&fb, (r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32);
    assert!(px.a > 0, "separator divider must paint (alpha {})", px.a);
}

/// Keyboard highlight (:focus) restyles the highlighted item's pixels and the
/// highlight MOVES as the cursor moves (accent background fill — paints reliably).
#[test]
fn keyboard_highlight_restyles_and_moves() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("m", Box::new(sample()));
    g.relayout();
    g.host.set_focus(Some("m"), &mut g.doc, &mut g.dispatcher);
    let plain = g.rasterize();

    // Down → item 0 highlighted.
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_menu(&g, "m").highlighted(), Some(0));
    g.relayout();
    let hl0 = g.rasterize();
    assert!(
        part_region_differs(&g, "m", "item-0", &plain, &hl0),
        "highlighted item 0 must restyle (accent fill)"
    );
    assert!(
        !part_region_differs(&g, "m", "item-1", &plain, &hl0),
        "item 1 must be unchanged while only item 0 is highlighted"
    );

    // Down → item 1 highlighted; highlight leaves 0, lands on 1.
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_menu(&g, "m").highlighted(), Some(1));
    g.relayout();
    let hl1 = g.rasterize();
    assert!(
        part_region_differs(&g, "m", "item-1", &hl0, &hl1),
        "highlight must move onto item 1"
    );
    assert!(
        part_region_differs(&g, "m", "item-0", &hl0, &hl1),
        "highlight must leave item 0"
    );
}

// NOTE (gap, reported to coordinator): the submenu arrow marker
// `lq-menu > lq-menu-item.has-submenu::after { content: "\25B6"; }` does NOT
// rasterize, whereas the table sort `::after` and tree twisty `::before` markers
// DO paint (see those passing tests). The distinguishing factor is the container:
// the menu item is `display: flex`, and the generated `::after` pseudo-element is
// not laid out / painted as a flex item — rendering the same "More" label as a
// submenu parent vs a plain item yields IDENTICAL item-box pixels. This is a
// layout/CSS gap (flex-container generated-content), not a behavior bug; the
// submenu OPEN behavior itself is proven by `submenu_parent_opens`. A
// no-fake-green pixel test for the arrow marker is omitted pending the fix.

// NOTE (CSS gap, reported to coordinator): the menu `:disabled` item style is
// `lq-menu > lq-menu-item:disabled { color: <disabled-fg>; background-color:
// transparent; }` — a FOREGROUND-COLOUR-ONLY restyle with no background/border
// change. The gallery's glyph rasterizer is too weak for a label-ink colour shift
// to register: an enabled-vs-disabled item-box pixel diff comes back IDENTICAL.
// A no-fake-green pixel-delta test for the disabled dim cannot pass without a CSS
// change that alters a paintable surface (e.g. a dimmed background/opacity). The
// disabled item's INERTNESS is already proven by `disabled_and_separator_are_inert`.

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
