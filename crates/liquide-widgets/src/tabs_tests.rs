//! `<lq-tabs>` real-pipeline gallery tests (no fake-green).
//!
//! Teeth: clicking a tab's LAID-OUT box switches the active panel (the panel
//! content actually changes in the DOM); the active tab carries :checked which
//! restyles its pixels; arrow keys move the active tab; a click in tab-1's real
//! box selects tab 1 (geometry per-tab from layout, not an index over a constant
//! tab width).
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::tabs::{Tabs, CHANGED_ACTION};

const W: u32 = 420;
const H: u32 = 240;

fn as_tabs<'a>(g: &'a Gallery, id: &str) -> &'a Tabs {
    g.host.behavior(id).unwrap().as_any().downcast_ref::<Tabs>().unwrap()
}

fn three_tabs() -> Tabs {
    Tabs::new()
        .tab("One", "first panel content")
        .tab("Two", "second panel content")
        .tab("Three", "third panel content")
}

fn tab_box(g: &Gallery, id: &str, i: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("tab-{i}")).expect("tab box")
}

/// The panel text node under the panel part (proves which panel is shown).
fn panel_text(g: &Gallery, id: &str) -> String {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let panel = q.find_part(root, "panel").expect("panel");
    // The panel's single text child.
    let mut out = String::new();
    fn collect(doc: &liquide_dom::Document, node: liquide_dom::NodeId, out: &mut String) {
        if let Some(t) = doc.get(node).and_then(|n| n.text_content()) {
            out.push_str(t);
        }
        for &c in doc.children(node) {
            collect(doc, c, out);
        }
    }
    collect(g.doc(), panel, &mut out);
    out
}

/// Initially the first tab is active and its panel is shown.
#[test]
fn first_tab_active_initially() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs()));
    g.relayout();
    assert_eq!(as_tabs(&g, "t").selected_index(), 0);
    assert!(panel_text(&g, "t").contains("first"), "first panel shown");
}

/// Clicking the second tab's LAID-OUT box switches the active panel.
#[test]
fn clicking_tab_switches_panel() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs()));
    g.relayout();

    let t1 = tab_box(&g, "t", 1);
    g.left_click(t1.x + t1.width / 2.0, t1.y + t1.height / 2.0);
    let actions = g.process();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, CHANGED_ACTION);
    assert_eq!(actions[0].payload.as_deref(), Some("1"));
    assert_eq!(as_tabs(&g, "t").selected_index(), 1);

    g.relayout();
    assert!(panel_text(&g, "t").contains("second"), "second panel now shown");
    assert!(!panel_text(&g, "t").contains("first"), "first panel hidden");
}

/// NO-FAKE-GREEN tooth: with unequal tab widths, a click in tab 2's REAL box
/// selects 2 — proving per-tab hit-test from layout, not an index over a constant
/// width. The first tab is widened so a constant-width index would mis-map.
#[test]
fn tab_hit_test_comes_from_layout_not_constant() {
    // Make tab-0 very wide; a constant tab width would put the click into the
    // wrong tab.
    let css = "lq-gallery { padding: 8px; }
               lq-tabs > lq-tablist > lq-tab:first-child { padding-left: 80px; padding-right: 80px; }";
    let mut g = Gallery::new(W, H, css);
    g.mount("t", Box::new(three_tabs()));
    g.relayout();

    let t0 = tab_box(&g, "t", 0);
    let t2 = tab_box(&g, "t", 2);
    // Precondition: tab-0 is wider than a "default" tab so geometry is unequal.
    assert!(t0.width > 100.0, "tab-0 widened (got {})", t0.width);

    // Click inside tab-2's REAL box.
    g.left_click(t2.x + t2.width / 2.0, t2.y + t2.height / 2.0);
    let _ = g.process();
    assert_eq!(
        as_tabs(&g, "t").selected_index(),
        2,
        "click in tab-2's laid-out box must select 2 (geometry from layout)"
    );
}

/// The active tab's :checked restyles its rasterized pixels.
#[test]
fn active_tab_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs()));
    g.relayout();

    let t1 = tab_box(&g, "t", 1);
    let (cx, cy) = ((t1.x + t1.width / 2.0) as u32, (t1.y + t1.height - 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    g.left_click(t1.x + t1.width / 2.0, t1.y + t1.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(before != after, "active tab must restyle (before {before:?} after {after:?})");
}

/// Arrow keys move the active tab (wrapping); Home/End jump.
#[test]
fn keyboard_moves_active_tab() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs()));
    g.relayout();
    g.host.set_focus(Some("t"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_tabs(&g, "t").selected_index(), 1);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_tabs(&g, "t").selected_index(), 2);
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0)); // wrap
    assert_eq!(as_tabs(&g, "t").selected_index(), 0);
    g.key(KeyInput::new(keys::ARROW_LEFT, 0)); // wrap back
    assert_eq!(as_tabs(&g, "t").selected_index(), 2);
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_tabs(&g, "t").selected_index(), 0);
}

/// Disabled tabs ignore clicks and keys.
#[test]
fn disabled_tabs_ignore_input() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs().disabled(true)));
    g.relayout();
    let t1 = tab_box(&g, "t", 1);
    g.left_click(t1.x + t1.width / 2.0, t1.y + t1.height / 2.0);
    let _ = g.process();
    g.host.set_focus(Some("t"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_tabs(&g, "t").selected_index(), 0, "disabled tabs hold selection");
}

// ─────────────────────────────────────────────────────────────────────────
// STATE × STYLING coverage (pixel-delta, no-fake-green). Each test below would
// FAIL if the corresponding CSS state rule were removed from widgets.css.
// ─────────────────────────────────────────────────────────────────────────

/// `normal` render correct: the selected tab paints accent ink along its bottom
/// border (`:checked { border-bottom-color: accent }`) — the indicator underline.
/// Sampling the tab-0 bottom edge row proves the base+checked style paints real
/// ink (not a tautology: an unselected tab there has a transparent bottom border).
#[test]
fn selected_tab_indicator_paints_accent_underline() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs()));
    g.relayout();

    // tab-0 is selected initially. Sample its bottom border row (the 2px accent
    // underline) vs the SAME row under an UNSELECTED tab (tab-1, transparent
    // bottom border). The selected underline must differ — proving the indicator
    // border-bottom-color paints.
    let t0 = tab_box(&g, "t", 0);
    let t1 = tab_box(&g, "t", 1);
    let row = (t0.y + t0.height - 1.0) as u32;
    let fb = g.rasterize();
    let sel_underline = Gallery::pixel(&fb, (t0.x + t0.width / 2.0) as u32, row);
    let unsel_underline = Gallery::pixel(&fb, (t1.x + t1.width / 2.0) as u32, row);
    assert!(sel_underline.a > 0, "selected tab underline must paint (alpha {})", sel_underline.a);
    assert!(
        sel_underline != unsel_underline,
        "selected tab's accent underline must differ from an unselected tab's bottom edge \
         (selected {sel_underline:?} vs unselected {unsel_underline:?})"
    );
}

/// `:checked` selection MOVES: tab-0 looks selected, then after selecting tab-1,
/// tab-0 reverts to the unselected look AND tab-1 takes the selected pixels. This
/// proves the :checked styling tracks the live selection, not a fixed first tab.
#[test]
fn checked_styling_follows_selection_movement() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs()));
    g.relayout();

    let t0 = tab_box(&g, "t", 0);
    let t1 = tab_box(&g, "t", 1);
    // Sample the bottom-border underline row for each tab.
    let row = (t0.y + t0.height - 1.0) as u32;
    let (x0, x1) = ((t0.x + t0.width / 2.0) as u32, (t1.x + t1.width / 2.0) as u32);

    let fb0 = g.rasterize();
    let t0_before = Gallery::pixel(&fb0, x0, row);
    let t1_before = Gallery::pixel(&fb0, x1, row);
    // Precondition: with tab-0 selected, the two underlines differ.
    assert!(t0_before != t1_before, "precondition: tab-0 selected underline differs from tab-1");

    // Move selection to tab-1.
    g.left_click(t1.x + t1.width / 2.0, t1.y + t1.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_tabs(&g, "t").selected_index(), 1);

    let fb1 = g.rasterize();
    let t0_after = Gallery::pixel(&fb1, x0, row);
    let t1_after = Gallery::pixel(&fb1, x1, row);

    // tab-0 reverted: its underline changed away from its selected look.
    assert!(
        t0_after != t0_before,
        "tab-0 must revert when selection leaves it (before {t0_before:?} after {t0_after:?})"
    );
    // tab-1 took the selected look: it now looks like tab-0's old selected underline.
    assert!(
        t1_after != t1_before,
        "tab-1 must take the selected look when selected (before {t1_before:?} after {t1_after:?})"
    );
    // And the two tabs swapped roles: tab-1's NEW look matches tab-0's OLD
    // selected look; tab-0's NEW look matches tab-1's OLD unselected look.
    assert_eq!(t1_after, t0_before, "tab-1 now wears the selected-underline pixels tab-0 had");
    assert_eq!(t0_after, t1_before, "tab-0 now wears the unselected pixels tab-1 had");
}

/// `:hover` restyles a tab's pixels: hovering an UNSELECTED tab swaps its
/// background to the hover-solid color (`:hover { background-color }`). Sampling
/// the tab body center proves the hover restyle paints (FAILs if :hover removed).
#[test]
fn hovering_tab_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs()));
    g.relayout();

    // tab-1 is unselected; sample its body center (above the bottom border, away
    // from glyph ink) before hover.
    let t1 = tab_box(&g, "t", 1);
    let (cx, cy) = ((t1.x + t1.width / 2.0) as u32, (t1.y + 4.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    // Move the pointer onto tab-1: the behavior sets :hover and re-renders.
    g.pointer_move(t1.x + t1.width / 2.0, t1.y + t1.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_tabs(&g, "t").selected_index(), 0, "hover must not change selection");

    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(
        before != after,
        "hovering an unselected tab must restyle its background (before {before:?} after {after:?})"
    );
}

/// Moving the pointer OFF a tab (MouseLeave) clears :hover, reverting its pixels —
/// the hover state is transient, not sticky.
#[test]
fn unhovering_tab_reverts_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("t", Box::new(three_tabs()));
    g.relayout();

    let t1 = tab_box(&g, "t", 1);
    let (cx, cy) = ((t1.x + t1.width / 2.0) as u32, (t1.y + 4.0) as u32);
    let resting = Gallery::pixel(&g.rasterize(), cx, cy);

    // Hover tab-1.
    g.pointer_move(t1.x + t1.width / 2.0, t1.y + t1.height / 2.0);
    let _ = g.process();
    g.relayout();
    let hovered = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(hovered != resting, "precondition: hover restyled tab-1");

    // Move the pointer well outside the tabs widget -> MouseLeave clears hover.
    g.pointer_move(2.0, 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert_eq!(after, resting, "leaving the tab must revert hover pixels to the resting look");
}
