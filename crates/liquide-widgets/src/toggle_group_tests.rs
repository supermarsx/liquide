//! `<lq-toggle-group>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::toggle_group::{ToggleGroup, CHANGED_ACTION};

const W: u32 = 420;
const H: u32 = 120;

fn as_grp<'a>(g: &'a Gallery, id: &str) -> &'a ToggleGroup {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<ToggleGroup>()
        .unwrap()
}

fn opts() -> Vec<(String, String)> {
    vec![
        ("bold".into(), "B".into()),
        ("italic".into(), "I".into()),
        ("underline".into(), "U".into()),
    ]
}

/// Single mode: clicking an option moves the exclusive selection.
#[test]
fn single_click_moves_selection() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("tg", Box::new(ToggleGroup::single(opts())));
    g.relayout();
    assert!(as_grp(&g, "tg").is_active(0));

    let root = g.host.root_of("tg").unwrap();
    let opt2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "opt-2").expect("opt-2 box")
    };
    g.left_click(opt2.x + opt2.width / 2.0, opt2.y + opt2.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("underline"));
    assert!(as_grp(&g, "tg").is_active(2));
    assert!(!as_grp(&g, "tg").is_active(0), "single mode is exclusive");
}

/// Multi mode: each option toggles independently; selection is the active set.
#[test]
fn multi_toggles_independently() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("tg", Box::new(ToggleGroup::multi(opts())));
    g.relayout();
    let root = g.host.root_of("tg").unwrap();
    let boxes = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (
            q.box_of_part(root, "opt-0").unwrap(),
            q.box_of_part(root, "opt-2").unwrap(),
        )
    };
    g.left_click(boxes.0.x + boxes.0.width / 2.0, boxes.0.y + boxes.0.height / 2.0);
    let _ = g.process();
    g.left_click(boxes.1.x + boxes.1.width / 2.0, boxes.1.y + boxes.1.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("bold,underline"));
    assert!(as_grp(&g, "tg").is_active(0));
    assert!(!as_grp(&g, "tg").is_active(1));
    assert!(as_grp(&g, "tg").is_active(2));

    // Re-click opt-0 to toggle it OFF.
    g.left_click(boxes.0.x + boxes.0.width / 2.0, boxes.0.y + boxes.0.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("underline"));
    assert!(!as_grp(&g, "tg").is_active(0));
}

/// NO-FAKE-GREEN tooth: per-option hit reads each option's REAL laid-out box.
/// Widen one option so a uniform-width guess would miss it.
#[test]
fn option_hit_from_layout_not_constant() {
    let mut g = Gallery::new(
        W,
        H,
        "lq-gallery { padding: 16px; } \
         lq-toggle-opt[data-value=\"underline\"] { padding-left: 60px; }",
    );
    g.mount("tg", Box::new(ToggleGroup::single(opts())));
    g.relayout();
    let root = g.host.root_of("tg").unwrap();
    let opt2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "opt-2").expect("opt-2 box")
    };
    // Click near the left of the widened option (where a uniform layout would
    // still think opt-1 lives).
    g.left_click(opt2.x + 4.0, opt2.y + opt2.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("underline"), "REAL box selects opt-2");
}

/// Arrow keys move a roving cursor; Space/Enter toggles the cursor option.
#[test]
fn keyboard_roving_cursor_and_toggle() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("tg", Box::new(ToggleGroup::multi(opts())));
    g.relayout();
    g.host.set_focus(Some("tg"), &mut g.doc, &mut g.dispatcher);
    assert_eq!(as_grp(&g, "tg").cursor(), 0);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_grp(&g, "tg").cursor(), 1);
    let a = g.key(KeyInput::new(keys::SPACE, 0));
    assert_eq!(a[0].payload.as_deref(), Some("italic"));
    assert!(as_grp(&g, "tg").is_active(1));

    // Wrap left from 1 -> 0 -> 2.
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert_eq!(as_grp(&g, "tg").cursor(), 2, "Left wraps to last");
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_grp(&g, "tg").cursor(), 2);
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_grp(&g, "tg").cursor(), 0);
}

/// Single mode without deselect: re-selecting the active option emits nothing.
#[test]
fn single_reselect_active_is_ignored() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("tg", Box::new(ToggleGroup::single(opts()).select(1)));
    g.relayout();
    let root = g.host.root_of("tg").unwrap();
    let opt1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "opt-1").unwrap()
    };
    g.left_click(opt1.x + opt1.width / 2.0, opt1.y + opt1.height / 2.0);
    let a = g.process();
    assert!(
        a.iter().all(|act| act.name != CHANGED_ACTION),
        "re-selecting the active option must not emit Changed"
    );
    assert!(as_grp(&g, "tg").is_active(1));
}

/// :checked restyles the active option's pixels.
#[test]
fn active_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("tg", Box::new(ToggleGroup::multi(opts())));
    g.relayout();
    let root = g.host.root_of("tg").unwrap();
    let opt1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "opt-1").unwrap()
    };
    let (cx, cy) = ((opt1.x + opt1.width / 2.0) as u32, (opt1.y + opt1.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);
    g.left_click(opt1.x + opt1.width / 2.0, opt1.y + opt1.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(before != after, "activating opt-1 must restyle its pixels");
}

/// Disabled toggle group swallows interaction.
#[test]
fn disabled_swallows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("tg", Box::new(ToggleGroup::single(opts()).disabled(true)));
    g.relayout();
    let root = g.host.root_of("tg").unwrap();
    let opt2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "opt-2").unwrap()
    };
    g.left_click(opt2.x + opt2.width / 2.0, opt2.y + opt2.height / 2.0);
    assert!(g.process().is_empty());
    assert!(as_grp(&g, "tg").is_active(0));
}
