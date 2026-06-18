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
