//! `<lq-listview>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::listview::{ListView, ViewItem, ViewMode, CHANGED_ACTION};

const W: u32 = 460;
const H: u32 = 420;

fn as_lv<'a>(g: &'a Gallery, id: &str) -> &'a ListView {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<ListView>()
        .unwrap()
}

fn files(mode: ViewMode) -> ListView {
    ListView::new(
        mode,
        [
            ViewItem::new("readme", "\u{1F4C4}", "readme.txt").subline("2 KB").columns(vec!["2 KB".into(), "Text".into()]),
            ViewItem::new("photo", "\u{1F5BC}", "photo.png").subline("1.2 MB").columns(vec!["1.2 MB".into(), "Image".into()]),
            ViewItem::new("song", "\u{1F3B5}", "song.mp3").subline("4 MB").columns(vec!["4 MB".into(), "Audio".into()]),
        ],
    )
    .detail_headers(vec!["Size".into(), "Type".into()])
}

fn item_box(g: &Gallery, root: liquide_dom::NodeId, i: usize) -> liquide_layout::geometry::Rect {
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("item-{i}")).expect("item box")
}

/// In EACH mode, clicking an item's laid-out box selects it — even though each
/// mode lays the items out completely differently (the anti-constant tooth: no
/// per-mode pitch constant, the hit comes from the box CSS produced).
#[test]
fn click_selects_in_every_mode() {
    for mode in [ViewMode::Icons, ViewMode::List, ViewMode::Details, ViewMode::Tiles] {
        let mut g = Gallery::new(W, H, "");
        g.mount("lv", Box::new(files(mode)));
        g.relayout();
        let root = g.host.root_of("lv").unwrap();
        let it = item_box(&g, root, 1);
        g.left_click(it.x + it.width / 2.0, it.y + it.height / 2.0);
        let a = g.process();
        let c = a
            .iter()
            .find(|a| a.name == CHANGED_ACTION)
            .unwrap_or_else(|| panic!("changed action expected in mode {:?}", mode));
        assert_eq!(
            c.payload.as_deref(),
            Some("photo"),
            "click item-1 selects 'photo' in mode {:?}",
            mode
        );
        assert_eq!(as_lv(&g, "lv").selected(), Some(1));
    }
}

/// Switching the mode changes the item LAYOUT: the same item's box differs
/// between Icons (square tile) and List (compact row). This proves the mode flips
/// the geometry, so the per-mode hit-test is genuinely mode-dependent.
#[test]
fn mode_switch_changes_layout() {
    let mut g_icons = Gallery::new(W, H, "");
    g_icons.mount("lv", Box::new(files(ViewMode::Icons)));
    g_icons.relayout();
    let root = g_icons.host.root_of("lv").unwrap();
    let icon_box = item_box(&g_icons, root, 0);

    let mut g_list = Gallery::new(W, H, "");
    g_list.mount("lv", Box::new(files(ViewMode::List)));
    g_list.relayout();
    let root2 = g_list.host.root_of("lv").unwrap();
    let list_box = item_box(&g_list, root2, 0);

    assert!(
        (icon_box.height - list_box.height).abs() > 10.0
            || (icon_box.width - list_box.width).abs() > 10.0,
        "Icons vs List must lay items out differently (icon {:?}, list {:?})",
        icon_box,
        list_box
    );
}

/// set_mode emits a mode Action and flips the mode; the new mode renders with a
/// `mode-<x>` root class that CSS keys off (verified via the rendered template).
#[test]
fn set_mode_switches_and_emits_action() {
    use crate::behavior::{WidgetBehavior, WidgetOutcome};
    let mut lv = files(ViewMode::Icons);
    assert_eq!(lv.mode(), ViewMode::Icons);
    let out = lv.set_mode(ViewMode::Details);
    match out {
        WidgetOutcome::Action { name, payload } => {
            assert_eq!(name, crate::listview::MODE_ACTION);
            assert_eq!(payload.as_deref(), Some("details"));
        }
        other => panic!("expected a mode Action, got {other:?}"),
    }
    assert_eq!(lv.mode(), ViewMode::Details);
    // The rendered root carries the new mode class.
    let node = lv.render();
    assert!(
        node.classes.iter().any(|c| c == "mode-details"),
        "render must carry the mode-details class, got {:?}",
        node.classes
    );
}

/// Keyboard: Right/Down move the cursor + selection; Home/End jump.
#[test]
fn keyboard_navigates() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lv", Box::new(files(ViewMode::List).select(0)));
    g.relayout();
    g.host.set_focus(Some("lv"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_lv(&g, "lv").selected(), Some(1));
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_lv(&g, "lv").selected(), Some(2));
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_lv(&g, "lv").selected(), Some(0));
}

/// PIXELS: selecting an item restyles its rasterized pixels.
#[test]
fn selection_restyles_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lv", Box::new(files(ViewMode::List)));
    g.relayout();
    let root = g.host.root_of("lv").unwrap();
    let it = item_box(&g, root, 1);
    let (sx, sy) = ((it.x + 12.0) as u32, (it.y + it.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);
    g.left_click(it.x + it.width / 2.0, it.y + it.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "selecting an item must restyle its pixels");
}

/// PIXELS :checked — selection restyles pixels in EVERY mode (each mode lays the
/// item out differently, but the selection fill paints in all of them).
#[test]
fn selection_restyles_pixels_in_every_mode() {
    for mode in [ViewMode::Icons, ViewMode::List, ViewMode::Details, ViewMode::Tiles] {
        let mut g = Gallery::new(W, H, "");
        g.mount("lv", Box::new(files(mode)));
        g.relayout();
        let root = g.host.root_of("lv").unwrap();
        let it = item_box(&g, root, 1);
        // Sample the left inset edge at mid-height where the selection fill/border
        // lands (avoids the rounded corners and the weak central glyph ink).
        let (sx, sy) = ((it.x + 2.0) as u32, (it.y + it.height / 2.0) as u32);
        let before = Gallery::pixel(&g.rasterize(), sx, sy);
        g.left_click(it.x + it.width / 2.0, it.y + it.height / 2.0);
        let _ = g.process();
        g.relayout();
        let after = Gallery::pixel(&g.rasterize(), sx, sy);
        assert!(
            before != after,
            "selection must restyle pixels in mode {:?} (before {before:?} after {after:?})",
            mode
        );
        assert_eq!(as_lv(&g, "lv").selected(), Some(1), "mode {:?}", mode);
    }
}

/// PIXELS :hover — hovering an item restyles its pixels (List mode hover fill),
/// and the delta lands on the hovered item only.
#[test]
fn hovered_item_restyles_pixels() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lv", Box::new(files(ViewMode::List)));
    g.relayout();
    let root = g.host.root_of("lv").unwrap();
    let (it1, it2) = (item_box(&g, root, 1), item_box(&g, root, 2));
    let (hx, hy) = ((it1.x + 2.0) as u32, (it1.y + it1.height / 2.0) as u32);
    let (nx, ny) = ((it2.x + 2.0) as u32, (it2.y + it2.height / 2.0) as u32);
    let before_h = Gallery::pixel(&g.rasterize(), hx, hy);
    let before_n = Gallery::pixel(&g.rasterize(), nx, ny);
    g.pointer_move(it1.x + it1.width / 2.0, it1.y + it1.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after_h = Gallery::pixel(&g.rasterize(), hx, hy);
    let after_n = Gallery::pixel(&g.rasterize(), nx, ny);
    assert!(before_h != after_h, "hovered item must restyle (before {before_h:?} after {after_h:?})");
    assert_eq!(before_n, after_n, "a non-hovered item must not change");
}

/// PIXELS :focus — the keyboard cursor paints a focus ring on the cursor item and
/// it MOVES with the cursor. Drive in List mode; cursor starts at item 0.
#[test]
fn focus_ring_moves_with_cursor() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lv", Box::new(files(ViewMode::List)));
    g.relayout();
    g.host.set_focus(Some("lv"), &mut g.doc, &mut g.dispatcher);
    let root = g.host.root_of("lv").unwrap();
    let it0 = item_box(&g, root, 0);
    // item 0 is the default cursor → it already has the focus ring. Sample its
    // inset edge, move the cursor away (Down selects+moves to 1), and assert the
    // ring leaves item 0.
    let (sx, sy) = ((it0.x + 1.0) as u32, (it0.y + it0.height / 2.0) as u32);
    let with_ring = Gallery::pixel(&g.rasterize(), sx, sy);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_lv(&g, "lv").cursor(), Some(1), "cursor moved to item 1");
    g.relayout();
    let without_ring = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(
        with_ring != without_ring,
        "the focus ring must leave item 0 when the cursor moves (with {with_ring:?} without {without_ring:?})"
    );
}

/// DISABLED: a disabled listview swallows a click — no selection, no action.
#[test]
fn disabled_listview_swallows_click() {
    let mut g = Gallery::new(W, H, "");
    g.mount("lv", Box::new(files(ViewMode::List).disabled(true)));
    g.relayout();
    let root = g.host.root_of("lv").unwrap();
    let it = item_box(&g, root, 1);
    g.left_click(it.x + it.width / 2.0, it.y + it.height / 2.0);
    let acts = g.process();
    assert!(acts.is_empty(), "disabled listview must emit nothing");
    assert_eq!(as_lv(&g, "lv").selected(), None, "disabled listview selects nothing");
}
