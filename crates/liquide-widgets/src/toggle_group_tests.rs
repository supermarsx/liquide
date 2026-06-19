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

// ── Added: visual-STATE pixel-delta coverage (no fake-green) ─────────────────

/// Center pixel of an option box (selection/hover are background-color driven).
fn opt_px(g: &mut Gallery, id: &str, idx: usize) -> liquide_compositor::pixel::Color {
    let root = g.host.root_of(id).unwrap();
    let r = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, &format!("opt-{idx}")).unwrap()
    };
    let fb = g.rasterize();
    Gallery::pixel(&fb, (r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32)
}

/// :hover restyles a (non-selected) option's background (CSS
/// `lq-toggle-opt:hover` -> bg-hover-solid). Hover opt-1 in multi mode (initially
/// none selected) — its bg must change.
#[test]
fn hover_restyles_option_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("tg", Box::new(ToggleGroup::multi(opts())));
    g.relayout();
    let before = opt_px(&mut g, "tg", 1);
    let root = g.host.root_of("tg").unwrap();
    let opt1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "opt-1").unwrap()
    };
    g.pointer_move(opt1.x + opt1.width / 2.0, opt1.y + opt1.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = opt_px(&mut g, "tg", 1);
    assert!(before != after, ":hover must restyle opt-1 bg ({before:?} -> {after:?})");
}

/// A selected (:checked) option paints the accent (blue-dominant) bg; the
/// selection MOVES in pixels when it changes. Single mode: opt-0 selected at
/// start; clicking opt-2 makes opt-0 plain and opt-2 accent.
#[test]
fn checked_selection_moves_in_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("tg", Box::new(ToggleGroup::single(opts())));
    g.relayout();
    let sel0_before = opt_px(&mut g, "tg", 0); // selected (accent)
    let sel2_before = opt_px(&mut g, "tg", 2); // plain
    assert!(
        Gallery::is_graphite_accent(sel0_before),
        "the selected opt-0 is the graphite accent (got {sel0_before:?})"
    );

    let root = g.host.root_of("tg").unwrap();
    let opt2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "opt-2").unwrap()
    };
    g.left_click(opt2.x + opt2.width / 2.0, opt2.y + opt2.height / 2.0);
    let _ = g.process();
    g.relayout();
    let sel0_after = opt_px(&mut g, "tg", 0); // now plain
    let sel2_after = opt_px(&mut g, "tg", 2); // now accent
    assert!(sel0_after != sel0_before, "opt-0 lost the accent fill");
    assert!(sel2_after != sel2_before, "opt-2 gained the accent fill");
    assert!(Gallery::is_graphite_accent(sel2_after), "opt-2 now the graphite accent (got {sel2_after:?})");
}

/// Multi mode: TWO options can be :checked at once, both painting the accent bg
/// (the checked style is per-option, not exclusive).
#[test]
fn multi_two_checked_both_paint_accent() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("tg", Box::new(ToggleGroup::multi(opts())));
    g.relayout();
    let root = g.host.root_of("tg").unwrap();
    let (b0, b2) = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (q.box_of_part(root, "opt-0").unwrap(), q.box_of_part(root, "opt-2").unwrap())
    };
    g.left_click(b0.x + b0.width / 2.0, b0.y + b0.height / 2.0);
    let _ = g.process();
    g.left_click(b2.x + b2.width / 2.0, b2.y + b2.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert!(as_grp(&g, "tg").is_active(0) && as_grp(&g, "tg").is_active(2));
    let p0 = opt_px(&mut g, "tg", 0);
    let p1 = opt_px(&mut g, "tg", 1); // not active
    let p2 = opt_px(&mut g, "tg", 2);
    assert!(Gallery::is_graphite_accent(p0), "opt-0 graphite accent (got {p0:?})");
    assert!(Gallery::is_graphite_accent(p2), "opt-2 graphite accent (got {p2:?})");
    assert!(p1 != p0, "the inactive opt-1 differs from an active option");
}

/// The roving-cursor :focus ring restyles the focused option's border (CSS
/// `lq-toggle-opt:focus` border). Moving the cursor to opt-1 via keyboard must
/// restyle opt-1 vs opt-2 (which has no focus).
#[test]
fn roving_focus_restyles_option_border() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("tg", Box::new(ToggleGroup::multi(opts())));
    g.relayout();
    let before = opt_px(&mut g, "tg", 1);
    g.host.set_focus(Some("tg"), &mut g.doc, &mut g.dispatcher);
    // Move the roving cursor from 0 to 1; the rerender applies :focus to opt-1.
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_grp(&g, "tg").cursor(), 1);
    g.relayout();
    // Sample the option border (top edge), where the focus ring lands.
    let root = g.host.root_of("tg").unwrap();
    let opt1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "opt-1").unwrap()
    };
    let fb = g.rasterize();
    let after_border = Gallery::pixel(&fb, (opt1.x + opt1.width / 2.0) as u32, opt1.y as u32);
    // Compare to the same option's interior+border BEFORE moving the cursor there.
    let _ = before;
    // A cleaner assertion: opt-1's border pixel (focused) differs from opt-2's
    // border pixel (unfocused) at the same relative position.
    let opt2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "opt-2").unwrap()
    };
    let unfocused_border = Gallery::pixel(&fb, (opt2.x + opt2.width / 2.0) as u32, opt2.y as u32);
    assert!(
        after_border != unfocused_border,
        "the focused option border must differ from an unfocused one ({after_border:?} vs {unfocused_border:?})"
    );
}

/// :disabled dims the group (opacity .5) — the selected option differs enabled vs
/// disabled.
#[test]
fn disabled_dims_pixels() {
    let mk = |dis: bool| {
        let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
        g.mount("tg", Box::new(ToggleGroup::single(opts()).disabled(dis)));
        g.relayout();
        opt_px(&mut g, "tg", 0) // the selected option (accent fill)
    };
    let enabled = mk(false);
    let disabled = mk(true);
    assert!(
        enabled != disabled,
        ":disabled must dim the selected option (enabled {enabled:?} disabled {disabled:?})"
    );
}
