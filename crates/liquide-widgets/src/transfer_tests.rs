//! `<lq-transfer>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::WidgetBehavior;
use crate::gallery::Gallery;
use crate::layout_query::LayoutQuery;
use crate::transfer::{Transfer, CHANGED_ACTION};

const W: u32 = 540;
const H: u32 = 320;

fn as_tr<'a>(g: &'a Gallery, id: &str) -> &'a Transfer {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Transfer>()
        .unwrap()
}

fn items() -> Vec<(String, String)> {
    vec![
        ("a".into(), "Alpha".into()),
        ("b".into(), "Bravo".into()),
        ("c".into(), "Charlie".into()),
        ("d".into(), "Delta".into()),
    ]
}

fn mount(g: &mut Gallery, id: &str) {
    g.mount(id, Box::new(Transfer::new(items())));
    g.relayout();
}

fn part_box(g: &Gallery, id: &str, part: &str) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, part).unwrap_or_else(|| panic!("part {part} box"))
}

/// Click a source row to select it, then move-selected → it lands in the target
/// and emits Changed(target ids).
#[test]
fn select_and_move_selected_to_target() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "tr");
    assert!(as_tr(&g, "tr").target_ids().is_empty());

    let row1 = part_box(&g, "tr", "src-1"); // Bravo
    g.left_click(row1.x + 8.0, row1.y + row1.height / 2.0);
    let _ = g.process();
    g.relayout();

    let to_tgt = part_box(&g, "tr", "to-target");
    g.left_click(to_tgt.x + to_tgt.width / 2.0, to_tgt.y + to_tgt.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("b"));
    g.relayout();
    assert_eq!(as_tr(&g, "tr").target_ids(), vec!["b".to_string()]);
    // Bravo is no longer in the source list.
    let root = g.host.root_of("tr").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.box_of_part(root, "src-1").is_none(), "Bravo left the source");
    assert!(q.box_of_part(root, "tgt-1").is_some(), "Bravo entered the target");
}

/// Move-all shuttles the whole source list.
#[test]
fn move_all_to_target() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "tr");
    let all = part_box(&g, "tr", "all-to-target");
    g.left_click(all.x + all.width / 2.0, all.y + all.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("a,b,c,d"));
    g.relayout();
    assert_eq!(as_tr(&g, "tr").target_ids().len(), 4);
    assert!(as_tr(&g, "tr").source_indices().is_empty());
}

/// Double-click a row shuttles that single item to the other list.
#[test]
fn double_click_shuttles_one() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "tr");
    let row2 = part_box(&g, "tr", "src-2"); // Charlie
    g.double_click(row2.x + 8.0, row2.y + row2.height / 2.0);
    let a = g.process();
    assert!(
        a.iter().any(|act| act.name == CHANGED_ACTION && act.payload.as_deref() == Some("c")),
        "dblclick on Charlie moves it to the target (actions: {a:?})"
    );
    g.relayout();
    assert_eq!(as_tr(&g, "tr").target_ids(), vec!["c".to_string()]);
}

/// Moving back: pre-place items in the target, select one there, move ← to source.
#[test]
fn move_selected_back_to_source() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("tr", Box::new(Transfer::new(items()).with_target([0, 1])));
    g.relayout();
    assert_eq!(as_tr(&g, "tr").target_ids(), vec!["a".to_string(), "b".to_string()]);

    let tgt0 = part_box(&g, "tr", "tgt-0"); // Alpha in target
    g.left_click(tgt0.x + 8.0, tgt0.y + tgt0.height / 2.0);
    let _ = g.process();
    g.relayout();
    let to_src = part_box(&g, "tr", "to-source");
    g.left_click(to_src.x + to_src.width / 2.0, to_src.y + to_src.height / 2.0);
    let a = g.process();
    // Alpha returns to source → target now only has Bravo.
    assert_eq!(a[0].payload.as_deref(), Some("b"));
    g.relayout();
    assert_eq!(as_tr(&g, "tr").target_ids(), vec!["b".to_string()]);
}

/// NO-FAKE-GREEN tooth: row hit reads each REAL laid-out box. Widen one source
/// row so rows are non-uniform pitch; clicking row 2's true box selects 2 (a
/// `i * row_height` guess from row 0 would mis-target).
#[test]
fn row_hit_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        W,
        420,
        "lq-gallery { padding: 16px; } lq-transfer-row[data-part=\"src-0\"] { height: 60px; }",
    );
    mount(&mut g, "tr");
    let r0 = part_box(&g, "tr", "src-0");
    let r2 = part_box(&g, "tr", "src-2");
    assert!(r0.height >= 56.0, "precondition: tall first row (got {})", r0.height);
    // With a 60px first row, a uniform pitch from r0.top would over/under-count r2.
    g.left_click(r2.x + 8.0, r2.y + r2.height / 2.0);
    let _ = g.process();
    g.relayout();
    // Now move-selected and confirm it moved Charlie (index 2), not a neighbour.
    let to_tgt = part_box(&g, "tr", "to-target");
    g.left_click(to_tgt.x + to_tgt.width / 2.0, to_tgt.y + to_tgt.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("c"), "row-2's REAL box selected Charlie");
}

/// Moving restyles the rasterized pixels: select a source row (turns selected,
/// box-shadow + selection bg) and assert the row's own pixels change.
#[test]
fn move_changes_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "tr");
    let r0 = part_box(&g, "tr", "src-0");
    let (sx, sy) = ((r0.x + 6.0) as u32, (r0.y + r0.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);

    // Select the row (selection bg/shadow) — a real interaction restyle.
    g.left_click(r0.x + 8.0, r0.y + r0.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "selecting a source row must restyle its pixels");
}

// ── Added: deep visual-STATE / styling coverage (no fake-green) ──────────────

/// :hover paints the hover background on the pointed row (the dispatcher sets the
/// :hover pseudo on the hit node; transfer's `wanted_events` is click-only so
/// `process` does not re-render and wipe it).
#[test]
fn row_hover_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "tr");
    let r1 = part_box(&g, "tr", "src-1");
    let (sx, sy) = ((r1.x + r1.width / 2.0) as u32, (r1.y + r1.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);

    g.pointer_move(r1.x + r1.width / 2.0, r1.y + r1.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(
        before != after,
        ":hover must restyle the pointed row's background (before {before:?} after {after:?})"
    );
}

/// The selected (:checked) row carries an inset accent box-shadow: sample the row
/// EDGE band (where the inset shadow lands) and confirm it differs from an
/// unselected row's edge. Proves the box-shadow ring, not just the bg, paints.
#[test]
fn selected_row_paints_inset_accent_ring() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "tr");
    let r0 = part_box(&g, "tr", "src-0");
    let r1 = part_box(&g, "tr", "src-1");
    // Select row 0 only.
    g.left_click(r0.x + 8.0, r0.y + r0.height / 2.0);
    let _ = g.process();
    g.relayout();
    let fb = g.rasterize();
    // Left edge band of each row.
    let sel_edge = Gallery::pixel(&fb, (r0.x + 1.0) as u32, (r0.y + r0.height / 2.0) as u32);
    let unsel_edge = Gallery::pixel(&fb, (r1.x + 1.0) as u32, (r1.y + r1.height / 2.0) as u32);
    assert!(
        sel_edge != unsel_edge,
        "the selected row's inset accent ring must differ from an unselected row (sel {sel_edge:?} unsel {unsel_edge:?})"
    );
}

/// The selection styling tracks WHICH row is selected: row 0's pixels are
/// selected when row 0 is the chosen row, but resting when row 1 is chosen
/// instead. No-fake-green: removing the `.selected` rule makes both samples
/// identical. (Two galleries avoid the dispatcher's same-node double-click
/// coalescing that a re-click would trigger.)
#[test]
fn selection_styling_tracks_selected_row() {
    // Gallery A: select row 0.
    let mut ga = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut ga, "tr");
    let a0 = part_box(&ga, "tr", "src-0");
    ga.left_click(a0.x + 8.0, a0.y + a0.height / 2.0);
    let _ = ga.process();
    ga.relayout();
    assert!(as_tr(&ga, "tr").source_indices().contains(&0), "row 0 stays in source (selection only)");
    let (sx, sy) = ((a0.x + 4.0) as u32, (a0.y + a0.height / 2.0) as u32);
    let row0_when_selected = Gallery::pixel(&ga.rasterize(), sx, sy);

    // Gallery B: select row 1 instead → row 0 is resting.
    let mut gb = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut gb, "tr");
    let b1 = part_box(&gb, "tr", "src-1");
    gb.left_click(b1.x + 8.0, b1.y + b1.height / 2.0);
    let _ = gb.process();
    gb.relayout();
    let b0 = part_box(&gb, "tr", "src-0");
    let row0_when_resting =
        Gallery::pixel(&gb.rasterize(), (b0.x + 4.0) as u32, (b0.y + b0.height / 2.0) as u32);

    assert!(
        row0_when_selected != row0_when_resting,
        "row 0 must be styled selected only when it is the chosen row (selected {row0_when_selected:?} resting {row0_when_resting:?})"
    );
}

/// A disabled move button (nothing selected → `to-target` is `.disabled`) paints a
/// DIMMED style (opacity 0.35) vs an enabled one, and its click is inert.
#[test]
fn disabled_move_button_is_dimmed_and_inert() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "tr");
    // Nothing selected → to-target is disabled. Capture it dimmed.
    let dis = part_box(&g, "tr", "to-target");
    let fb_dis = g.rasterize();

    // Now select a source row → to-target becomes enabled (not dimmed).
    let r0 = part_box(&g, "tr", "src-0");
    g.left_click(r0.x + 8.0, r0.y + r0.height / 2.0);
    let _ = g.process();
    g.relayout();
    let en = part_box(&g, "tr", "to-target");
    let fb_en = g.rasterize();
    assert!((dis.x - en.x).abs() < 1.0 && (dis.width - en.width).abs() < 1.0, "same button geometry");

    // The dimmed (disabled) button differs from the enabled one somewhere.
    let y = (dis.y + dis.height / 2.0) as u32;
    let mut differs = false;
    for x in (dis.x as u32 + 1)..((dis.x + dis.width) as u32 - 1) {
        if Gallery::pixel(&fb_dis, x, y) != Gallery::pixel(&fb_en, x, y) {
            differs = true;
            break;
        }
    }
    assert!(differs, "the disabled move button must render dimmed vs the enabled one");

    // And the disabled button's click was inert: at the start (nothing selected),
    // clicking to-target moved nothing.
    let mut g2 = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g2, "tr");
    let tt = part_box(&g2, "tr", "to-target");
    g2.left_click(tt.x + tt.width / 2.0, tt.y + tt.height / 2.0);
    let a = g2.process();
    assert!(a.is_empty(), "to-target with nothing selected is inert");
    g2.relayout();
    assert!(as_tr(&g2, "tr").target_ids().is_empty(), "no item moved");
}

/// A fully disabled transfer swallows row clicks and the move-all button, and is
/// not focusable.
#[test]
fn disabled_transfer_swallows_interaction() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("tr", Box::new(Transfer::new(items()).disabled(true)));
    g.relayout();
    let all = part_box(&g, "tr", "all-to-target");
    g.left_click(all.x + all.width / 2.0, all.y + all.height / 2.0);
    let a = g.process();
    assert!(a.is_empty(), "a disabled transfer emits nothing");
    g.relayout();
    assert!(as_tr(&g, "tr").target_ids().is_empty(), "nothing moved");
    assert!(!as_tr(&g, "tr").focusable(), "a disabled transfer is not focusable");
}
