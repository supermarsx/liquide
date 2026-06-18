//! `<lq-transfer>` real-pipeline gallery tests.
#![cfg(test)]

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
