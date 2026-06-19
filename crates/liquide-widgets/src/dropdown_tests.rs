//! `<lq-dropdown>` / `<lq-combobox>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::{KeyInput, WidgetBehavior};
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

// ── added: per-state styling proofs (no-fake-green pixel deltas) ───────────

/// An open option restyles to the :hover background when the pointer is over it.
/// The hovered option's centre changes colour vs its resting (popup-bg) fill —
/// `lq-popup > lq-option:hover { background: #3f3f46 }`. Remove that rule and the
/// pixel would not move.
#[test]
fn option_hover_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("dd", Box::new(Dropdown::new(opts())));
    g.relayout();
    open_via_button(&mut g, "dd");
    let root = g.host.root_of("dd").unwrap();
    // option-1 is NOT selected/highlighted by default selection (nothing
    // selected; highlight is on option-0 after open), so a clean hover delta.
    let opt1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "option-1").expect("option-1 box")
    };
    let (cx, cy) = ((opt1.x + opt1.width / 2.0) as u32, (opt1.y + opt1.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    g.pointer_move(opt1.x + opt1.width / 2.0, opt1.y + opt1.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert_eq!(as_dd(&g, "dd").is_open(), true, "still open while hovering");
    assert!(before != after, "hovering an option must restyle it (before {before:?} after {after:?})");
}

/// The :checked/selected option paints the accent fill, and that fill MOVES with
/// the selection: selecting option-0 paints option-0 accent (graphite) while
/// option-2 stays the resting fill; a second dropdown selecting option-2 paints
/// the accent there instead. Proves `lq-option:checked { background: accent }`.
#[test]
fn selected_option_paints_accent_and_moves() {
    // Dropdown A: option-0 selected.
    let mut a = Gallery::new(W, 420, "lq-gallery { padding: 16px; }");
    a.mount("dd", Box::new(Dropdown::new(opts()).select(0)));
    a.relayout();
    open_via_button(&mut a, "dd");
    let aroot = a.host.root_of("dd").unwrap();
    let (a0, a2) = {
        let q = LayoutQuery::new(a.hit_test_engine(), a.doc());
        (
            q.box_of_part(aroot, "option-0").expect("a option-0"),
            q.box_of_part(aroot, "option-2").expect("a option-2"),
        )
    };
    let afb = a.rasterize();
    let a0px = Gallery::pixel(&afb, (a0.x + a0.width / 2.0) as u32, (a0.y + a0.height / 2.0) as u32);
    let a2px = Gallery::pixel(&afb, (a2.x + a2.width / 2.0) as u32, (a2.y + a2.height / 2.0) as u32);
    assert!(
        Gallery::is_graphite_accent(a0px),
        "selected option-0 must paint the graphite accent (got {a0px:?})"
    );
    assert!(a0px != a2px, "the unselected option-2 must differ from selected option-0");

    // Dropdown B: option-2 selected — the accent fill moves to option-2.
    let mut b = Gallery::new(W, 420, "lq-gallery { padding: 16px; }");
    b.mount("dd", Box::new(Dropdown::new(opts()).select(2)));
    b.relayout();
    open_via_button(&mut b, "dd");
    let broot = b.host.root_of("dd").unwrap();
    let b2 = {
        let q = LayoutQuery::new(b.hit_test_engine(), b.doc());
        q.box_of_part(broot, "option-2").expect("b option-2")
    };
    let b2px = Gallery::pixel(&b.rasterize(), (b2.x + b2.width / 2.0) as u32, (b2.y + b2.height / 2.0) as u32);
    assert!(Gallery::is_graphite_accent(b2px), "selection moved: option-2 now the graphite accent (got {b2px:?})");
    assert!(
        b2px != a2px,
        "option-2's fill differs once it is the selected one (resting {a2px:?} selected {b2px:?})"
    );
}

/// Keyboard highlight (:focus/.highlighted) restyles the highlighted option. The
/// FIRST visible option is highlighted on open; an additional ArrowDown moves the
/// highlight to option-1, restyling it to the hover/highlight background.
#[test]
fn highlighted_option_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("dd", Box::new(Dropdown::new(opts())));
    g.relayout();
    g.host.set_focus(Some("dd"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // open + highlight 0
    g.relayout();
    let root = g.host.root_of("dd").unwrap();
    let opt1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "option-1").expect("option-1 box")
    };
    let (cx, cy) = ((opt1.x + opt1.width / 2.0) as u32, (opt1.y + opt1.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy); // option-1 resting

    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // highlight -> option-1
    assert_eq!(as_dd(&g, "dd").highlighted(), Some(1));
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(before != after, "highlighting option-1 must restyle it (before {before:?} after {after:?})");
}

/// Opening restyles the TRIGGER border to the accent — `lq-dropdown.open >
/// lq-dropdown-button { border-color: accent }`. Sample the button's top border
/// ring (1px in) before/after opening.
#[test]
fn open_restyles_trigger_border() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("dd", Box::new(Dropdown::new(opts())));
    g.relayout();
    let root = g.host.root_of("dd").unwrap();
    let btn = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "button").expect("button box")
    };
    // A point ON the top border ring of the trigger.
    let (bx, by) = ((btn.x + btn.width / 2.0) as u32, (btn.y + 0.5) as u32);
    let before = Gallery::pixel(&g.rasterize(), bx, by);

    open_via_button(&mut g, "dd");
    let after = Gallery::pixel(&g.rasterize(), bx, by);
    assert!(as_dd(&g, "dd").is_open());
    assert!(before != after, "opening must recolour the trigger border (before {before:?} after {after:?})");
}

/// A disabled dropdown swallows the trigger click — it never opens and emits
/// nothing — and drops out of the focus ring.
#[test]
fn disabled_dropdown_swallows_click() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("dd", Box::new(Dropdown::new(opts()).disabled(true)));
    g.relayout();
    let root = g.host.root_of("dd").unwrap();
    let btn = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "button").expect("button box")
    };
    g.left_click(btn.x + 5.0, btn.y + btn.height / 2.0);
    assert!(g.process().is_empty(), "disabled trigger emits nothing");
    assert!(!as_dd(&g, "dd").is_open(), "disabled dropdown must not open");
    assert!(!as_dd(&g, "dd").focusable(), "disabled dropdown is not focusable");
}

/// The caret affordance (`data-part="arrow"`, a CSS `::before` ▼) reserves a real
/// laid-out box in the trigger. (The glyph ink itself is not asserted — the
/// gallery glyph rasterizer does not reliably paint the dingbat; the box presence
/// is the structural proof the affordance exists.)
#[test]
fn caret_arrow_has_a_layout_box() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("dd", Box::new(Dropdown::new(opts())));
    g.relayout();
    let root = g.host.root_of("dd").unwrap();
    let arrow = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "arrow").expect("arrow box")
    };
    assert!(arrow.width > 0.0 && arrow.height > 0.0, "caret reserves a box (got {arrow:?})");
}
