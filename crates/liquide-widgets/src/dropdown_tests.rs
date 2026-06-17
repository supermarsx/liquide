//! `<lq-dropdown>` / `<lq-combobox>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::dropdown::{Dropdown, CHANGED_ACTION};
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 360;
const H: u32 = 320;

fn as_dd<'a>(g: &'a Gallery, id: &str) -> &'a Dropdown {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Dropdown>()
        .unwrap()
}

fn opts() -> Vec<(String, String)> {
    vec![
        ("a".into(), "Apple".into()),
        ("b".into(), "Banana".into()),
        ("c".into(), "Cherry".into()),
    ]
}

/// Click the trigger button's laid-out box and process; returns nothing useful
/// but leaves the dropdown re-laid-out.
fn open_via_button(g: &mut Gallery, id: &str) {
    let root = g.host.root_of(id).unwrap();
    let btn = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "button").expect("button box")
    };
    g.left_click(btn.x + 5.0, btn.y + btn.height / 2.0);
    let _ = g.process();
    g.relayout();
}

/// The closed dropdown paints NO option boxes; opening (button click) creates
/// them.
#[test]
fn click_button_opens_popup() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("dd", Box::new(Dropdown::new(opts())));
    g.relayout();
    assert!(!as_dd(&g, "dd").is_open());

    let root = g.host.root_of("dd").unwrap();
    {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "option-0").is_none(), "closed: no options");
    }
    open_via_button(&mut g, "dd");
    assert!(as_dd(&g, "dd").is_open());
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.box_of_part(root, "option-0").is_some(), "open: options exist");
}

/// Open via the button, then select an option by clicking its LAID-OUT box;
/// selection closes the popup and emits Changed(value).
#[test]
fn open_then_select_option() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("dd", Box::new(Dropdown::new(opts())));
    g.relayout();
    open_via_button(&mut g, "dd");
    assert!(as_dd(&g, "dd").is_open());

    let root = g.host.root_of("dd").unwrap();
    let opt1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "option-1").expect("option-1 box exists once open")
    };
    g.left_click(opt1.x + 5.0, opt1.y + opt1.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("b"));
    g.relayout();
    assert!(!as_dd(&g, "dd").is_open(), "selecting closes the popup");
    assert_eq!(as_dd(&g, "dd").selected_value(), Some("b"));
}

/// NO-FAKE-GREEN tooth: per-option hit reads each option's REAL laid-out box.
/// With tall (44px) options, clicking a MIDDLE option's true box selects that
/// option — a `index * constant_height` guess (e.g. a 24px pitch) would over-
/// count and mis-target a different row (and clamping can't mask a middle click).
#[test]
fn option_hit_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        W,
        420,
        "lq-gallery { padding: 16px; } lq-popup > lq-option { height: 44px; padding: 0; }",
    );
    g.mount(
        "dd",
        Box::new(Dropdown::new(vec![
            ("a".into(), "Apple".into()),
            ("b".into(), "Banana".into()),
            ("c".into(), "Cherry".into()),
            ("d".into(), "Date".into()),
        ])),
    );
    g.relayout();
    open_via_button(&mut g, "dd");

    let root = g.host.root_of("dd").unwrap();
    let opt1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "option-1").expect("option-1 box")
    };
    assert!(opt1.height >= 40.0, "precondition: tall option (got {})", opt1.height);
    // Clicking option-1's REAL centre selects 1. A 24px-pitch constant from the
    // first option's top would compute row (44+22)/24 ≈ 2 -> the WRONG option.
    g.left_click(opt1.x + 5.0, opt1.y + opt1.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("b"), "click in option-1's REAL box selects 1");
}

/// Esc dismisses an open dropdown WITHOUT changing an existing selection.
/// (Dismiss-on-click-OUTSIDE the widget is the embedding surface's job — a click
/// outside never reaches the widget handler in isolation; a click inside but not
/// on an option falls to the close path in `on_dom_event`, covered by the
/// behavior's Click handling.)
#[test]
fn escape_preserves_selection() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("dd", Box::new(Dropdown::new(opts()).select(0)));
    g.relayout();
    g.host.set_focus(Some("dd"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // open
    g.relayout();
    assert!(as_dd(&g, "dd").is_open());

    let a = g.key(KeyInput::new(keys::ESCAPE, 0));
    assert!(a.is_empty(), "Esc emits no Changed");
    assert!(!as_dd(&g, "dd").is_open(), "Esc closed the popup");
    assert_eq!(as_dd(&g, "dd").selected_index(), Some(0), "selection unchanged");
}

/// Keyboard: Down opens + highlights, Down moves, Enter selects.
#[test]
fn keyboard_navigation_selects() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("dd", Box::new(Dropdown::new(opts())));
    g.relayout();
    g.host.set_focus(Some("dd"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    g.relayout();
    assert!(as_dd(&g, "dd").is_open());
    assert_eq!(as_dd(&g, "dd").highlighted(), Some(0));

    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_dd(&g, "dd").highlighted(), Some(1));

    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].payload.as_deref(), Some("b"));
    assert!(!as_dd(&g, "dd").is_open());
}

/// Esc closes an open dropdown without selecting.
#[test]
fn escape_closes() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("dd", Box::new(Dropdown::new(opts())));
    g.relayout();
    g.host.set_focus(Some("dd"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    g.relayout();
    assert!(as_dd(&g, "dd").is_open());
    g.key(KeyInput::new(keys::ESCAPE, 0));
    assert!(!as_dd(&g, "dd").is_open());
    assert_eq!(as_dd(&g, "dd").selected_index(), None);
}

/// Combobox: typing filters the visible options; selection maps to the true value.
#[test]
fn combobox_filters_options() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("cb", Box::new(Dropdown::combobox(opts())));
    g.relayout();
    g.host.set_focus(Some("cb"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    g.relayout();
    let root = g.host.root_of("cb").unwrap();
    g.key(KeyInput::new('c' as u32, 0));
    g.key(KeyInput::new('h' as u32, 0));
    g.relayout();
    assert_eq!(as_dd(&g, "cb").filter_text(), "ch");
    {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "option-0").is_none(), "Apple filtered out");
        assert!(q.box_of_part(root, "option-1").is_none(), "Banana filtered out");
        assert!(q.box_of_part(root, "option-2").is_some(), "Cherry still visible");
    }
    let opt2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "option-2").unwrap()
    };
    g.left_click(opt2.x + 5.0, opt2.y + opt2.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("c"));
}

/// Opening restyles the rasterized pixels (the popup surface appears below).
#[test]
fn open_changes_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("dd", Box::new(Dropdown::new(opts())));
    g.relayout();
    let root = g.host.root_of("dd").unwrap();
    let btn = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "button").unwrap()
    };
    let (sx, sy) = ((btn.x + 10.0) as u32, (btn.y + btn.height + 20.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);

    open_via_button(&mut g, "dd");
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "the popup must restyle pixels below the button");
}
