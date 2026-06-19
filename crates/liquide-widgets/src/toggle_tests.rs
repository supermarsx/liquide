//! `<lq-checkbox>` / `<lq-switch>` / `<lq-radio>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::toggle::{RadioGroup, Toggle, CHANGED_ACTION};

const W: u32 = 320;
const H: u32 = 200;

fn as_toggle<'a>(g: &'a Gallery, id: &str) -> &'a Toggle {
    g.host.behavior(id).unwrap().as_any().downcast_ref::<Toggle>().unwrap()
}
fn as_radio<'a>(g: &'a Gallery, id: &str) -> &'a RadioGroup {
    g.host.behavior(id).unwrap().as_any().downcast_ref::<RadioGroup>().unwrap()
}

/// A checkbox renders an indicator box and a click toggles :checked.
#[test]
fn checkbox_click_toggles_checked() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("cb", Box::new(Toggle::checkbox("Enable")));
    g.relayout();
    assert!(!as_toggle(&g, "cb").is_checked());

    let node = g.host.root_of("cb").unwrap();
    let r = g.box_of(node).unwrap();
    g.left_click(r.x + 9.0, r.y + r.height / 2.0);
    let actions = g.process();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, CHANGED_ACTION);
    assert_eq!(actions[0].payload.as_deref(), Some("true"));
    assert!(as_toggle(&g, "cb").is_checked());

    // Toggle back OFF via the keyboard path (a second rapid click on the same
    // node would be coalesced into a DoubleClick by the dispatcher — realistic
    // double-click debounce — so we exercise the off-direction via Space, which
    // also proves the toggle is bidirectional through real events).
    g.host.set_focus(Some("cb"), &mut g.doc, &mut g.dispatcher);
    let actions = g.key(KeyInput::new(keys::SPACE, 0));
    assert_eq!(actions.len(), 1, "Space toggles off");
    assert_eq!(actions[0].payload.as_deref(), Some("false"));
    assert!(!as_toggle(&g, "cb").is_checked());
}

/// :checked actually restyles the indicator pixels (the check fill appears).
#[test]
fn checked_restyles_indicator_pixels() {
    use crate::layout_query::LayoutQuery;
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("cb", Box::new(Toggle::checkbox("Enable")));
    g.relayout();

    let root = g.host.root_of("cb").unwrap();
    let ind = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "indicator").expect("indicator box")
    };
    let (ix, iy) = ((ind.x + ind.width / 2.0) as u32, (ind.y + ind.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), ix, iy);

    // Toggle on via keyboard (Space), re-render, relayout, rasterize.
    g.host.set_focus(Some("cb"), &mut g.doc, &mut g.dispatcher);
    let a = g.key(KeyInput::new(keys::SPACE, 0));
    assert_eq!(a.len(), 1, "Space toggles");
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), ix, iy);
    assert!(
        before != after,
        ":checked must restyle the indicator (before {before:?} after {after:?})"
    );
    assert!(as_toggle(&g, "cb").is_checked());
}

/// Switch is the same behavior with a different element/appearance.
#[test]
fn switch_space_toggles() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sw", Box::new(Toggle::switch("Wifi").checked(true)));
    g.relayout();
    assert!(as_toggle(&g, "sw").is_checked());
    g.host.set_focus(Some("sw"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::SPACE, 0));
    assert!(!as_toggle(&g, "sw").is_checked(), "Space toggles the switch off");
}

/// Disabled toggle swallows clicks + Space.
#[test]
fn disabled_toggle_swallows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("cb", Box::new(Toggle::checkbox("x").disabled(true)));
    g.relayout();
    let node = g.host.root_of("cb").unwrap();
    let r = g.box_of(node).unwrap();
    g.left_click(r.x + 9.0, r.y + r.height / 2.0);
    assert!(g.process().is_empty());
    g.host.set_focus(Some("cb"), &mut g.doc, &mut g.dispatcher);
    assert!(g.key(KeyInput::new(keys::SPACE, 0)).is_empty());
    assert!(!as_toggle(&g, "cb").is_checked());
}

/// Radio group is EXCLUSIVE: selecting one option deselects the others, by
/// construction (single `selected` field). Geometry per option comes from the
/// laid-out per-option box (data-part), not a constant row height.
#[test]
fn radio_group_exclusive_selection_via_click() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    let rg = RadioGroup::new(
        "size",
        vec![
            ("s".into(), "Small".into()),
            ("m".into(), "Medium".into()),
            ("l".into(), "Large".into()),
        ],
    );
    g.mount("rg", Box::new(rg));
    g.relayout();
    assert_eq!(as_radio(&g, "rg").selected_index(), 0);

    // Click the THIRD option's laid-out box.
    use crate::layout_query::LayoutQuery;
    let root = g.host.root_of("rg").unwrap();
    let opt2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "option-2").expect("option-2 box")
    };
    g.left_click(opt2.x + 9.0, opt2.y + opt2.height / 2.0);
    let actions = g.process();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].payload.as_deref(), Some("l"));
    assert_eq!(as_radio(&g, "rg").selected_index(), 2, "only the clicked option is selected");
    assert_eq!(as_radio(&g, "rg").selected_value(), Some("l"));
}

/// Arrow keys move the radio selection (and wrap), one selected at a time.
#[test]
fn radio_group_arrow_keys_move_selection() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    let rg = RadioGroup::new(
        "size",
        vec![("a".into(), "A".into()), ("b".into(), "B".into())],
    );
    g.mount("rg", Box::new(rg));
    g.relayout();
    g.host.set_focus(Some("rg"), &mut g.doc, &mut g.dispatcher);

    let a = g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(as_radio(&g, "rg").selected_index(), 1);
    // Wrap back to 0.
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_radio(&g, "rg").selected_index(), 0);
    g.key(KeyInput::new(keys::ARROW_UP, 0));
    assert_eq!(as_radio(&g, "rg").selected_index(), 1, "Up wraps to last");
}

/// The NO-FAKE-GREEN tooth for the radio: per-option hit-test reads each option's
/// LAID-OUT box. A click on option-1's box selects option-1, never option-0 — a
/// constant row height would mis-target after the CSS option height changes.
#[test]
fn radio_per_option_hit_from_layout() {
    // Give options an unusual tall height so a constant guess would miss.
    let mut g = Gallery::new(
        W,
        H,
        "lq-gallery { padding: 16px; } lq-radio { height: 41px; }",
    );
    let rg = RadioGroup::new(
        "x",
        vec![("a".into(), "A".into()), ("b".into(), "B".into())],
    );
    g.mount("rg", Box::new(rg));
    g.relayout();

    use crate::layout_query::LayoutQuery;
    let root = g.host.root_of("rg").unwrap();
    let opt1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "option-1").expect("option-1 box")
    };
    assert!((opt1.height - 41.0).abs() < 2.0, "precondition: 41px option (got {})", opt1.height);
    // y must be ~ second row; a 24px constant assumption would land in option-0.
    g.left_click(opt1.x + 9.0, opt1.y + opt1.height / 2.0);
    let _ = g.process();
    assert_eq!(
        as_radio(&g, "rg").selected_index(),
        1,
        "click in option-1's REAL (tall) box must select option-1"
    );
}

// ── Added: deeper indicator pixel-delta coverage for switch / radio + the
//    :hover and :focus indicator ring (no fake-green) ──────────────────────────

use crate::layout_query::LayoutQuery;

fn indicator_center(g: &Gallery, id: &str) -> (u32, u32) {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let ind = q.box_of_part(root, "indicator").expect("indicator box");
    ((ind.x + ind.width / 2.0) as u32, (ind.y + ind.height / 2.0) as u32)
}

/// :checked restyles the SWITCH indicator (track) pixels — the accent fill appears
/// when toggled on (CSS `lq-switch:checked > lq-indicator`).
#[test]
fn switch_checked_restyles_indicator_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sw", Box::new(Toggle::switch("Wifi")));
    g.relayout();
    assert!(!as_toggle(&g, "sw").is_checked());

    let (ix, iy) = indicator_center(&g, "sw");
    let before = Gallery::pixel(&g.rasterize(), ix, iy);

    g.host.set_focus(Some("sw"), &mut g.doc, &mut g.dispatcher);
    let a = g.key(KeyInput::new(keys::SPACE, 0));
    assert_eq!(a.len(), 1, "Space toggles the switch on");
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), ix, iy);
    assert!(
        before != after,
        ":checked must restyle the switch track (before {before:?} after {after:?})"
    );
    assert!(as_toggle(&g, "sw").is_checked());
}

/// :checked restyles the RADIO indicator pixels — selecting an option fills its
/// indicator with the accent (CSS `lq-radio:checked > lq-indicator`). We compare
/// option-1's indicator before vs after it becomes selected.
#[test]
fn radio_checked_restyles_indicator_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    let rg = RadioGroup::new(
        "size",
        vec![("a".into(), "A".into()), ("b".into(), "B".into())],
    );
    g.mount("rg", Box::new(rg));
    g.relayout();
    assert_eq!(as_radio(&g, "rg").selected_index(), 0);

    // option-1's indicator is the second option's indicator (data-part nesting).
    let root = g.host.root_of("rg").unwrap();
    let ind1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        // The per-option box; sample the left side where the indicator sits.
        q.box_of_part(root, "option-1").expect("option-1 box")
    };
    let (ix, iy) = ((ind1.x + 9.0) as u32, (ind1.y + ind1.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), ix, iy);

    g.left_click(ind1.x + 9.0, ind1.y + ind1.height / 2.0);
    let _ = g.process();
    assert_eq!(as_radio(&g, "rg").selected_index(), 1);
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), ix, iy);
    assert!(
        before != after,
        ":checked must restyle the radio indicator (before {before:?} after {after:?})"
    );
}

/// :hover restyles the checkbox indicator BORDER (CSS `lq-checkbox:hover >
/// lq-indicator` -> focus-ring colour). Sample on the indicator's top border line.
#[test]
fn checkbox_hover_restyles_indicator_border() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("cb", Box::new(Toggle::checkbox("Enable")));
    g.relayout();

    let root = g.host.root_of("cb").unwrap();
    let ind = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "indicator").expect("indicator box")
    };
    // Top border line of the indicator.
    let (bx, by) = ((ind.x + ind.width / 2.0) as u32, ind.y as u32);
    let before = Gallery::pixel(&g.rasterize(), bx, by);

    g.pointer_move(ind.x + ind.width / 2.0, ind.y + ind.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), bx, by);
    assert!(as_toggle(&g, "cb").is_checked() == false);
    assert!(
        before != after,
        ":hover must restyle the indicator border (before {before:?} after {after:?})"
    );
}

/// :focus restyles the checkbox indicator BORDER (focus ring) — CSS
/// `lq-checkbox:focus > lq-indicator`. Focus via the dispatcher; no re-render
/// follows so the FOCUS pseudo survives.
#[test]
fn checkbox_focus_restyles_indicator_border() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("cb", Box::new(Toggle::checkbox("Enable")));
    g.relayout();

    let root = g.host.root_of("cb").unwrap();
    let ind = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "indicator").expect("indicator box")
    };
    let (bx, by) = ((ind.x + ind.width / 2.0) as u32, ind.y as u32);
    let before = Gallery::pixel(&g.rasterize(), bx, by);

    g.host.set_focus(Some("cb"), &mut g.doc, &mut g.dispatcher);
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), bx, by);
    assert!(
        before != after,
        ":focus must restyle the indicator border ring (before {before:?} after {after:?})"
    );
}
