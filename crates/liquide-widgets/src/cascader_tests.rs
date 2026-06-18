//! `<lq-cascader>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::cascader::{CascadeNode, Cascader, CHANGED_ACTION};
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 560;
const H: u32 = 280;

fn as_cs<'a>(g: &'a Gallery, id: &str) -> &'a Cascader {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Cascader>()
        .unwrap()
}

/// Geo hierarchy: Country → State → City.
fn geo() -> Vec<CascadeNode> {
    vec![
        CascadeNode::branch(
            "us",
            "USA",
            [
                CascadeNode::branch(
                    "ca",
                    "California",
                    [
                        CascadeNode::leaf("sf", "San Francisco"),
                        CascadeNode::leaf("la", "Los Angeles"),
                    ],
                ),
                CascadeNode::branch("ny", "New York", [CascadeNode::leaf("nyc", "New York City")]),
            ],
        ),
        CascadeNode::branch(
            "uk",
            "United Kingdom",
            [CascadeNode::branch("eng", "England", [CascadeNode::leaf("ldn", "London")])],
        ),
    ]
}

fn mount(g: &mut Gallery, id: &str) {
    g.mount(id, Box::new(Cascader::new(geo())));
    g.relayout();
    g.host.set_focus(Some(id), &mut g.doc, &mut g.dispatcher);
}

fn part_box(g: &Gallery, id: &str, part: &str) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, part).unwrap_or_else(|| panic!("part {part} box"))
}

/// Initially only column 0 (roots) is visible; picking a branch reveals column 1.
#[test]
fn picking_branch_reveals_next_column() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "cs");
    assert_eq!(as_cs(&g, "cs").column_count(), 1, "only the root column at first");
    {
        let root = g.host.root_of("cs").unwrap();
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "col-1").is_none(), "no second column yet");
    }

    // Pick USA (col 0, node 0) → reveals its states in col 1.
    let usa = part_box(&g, "cs", "node-0-0");
    g.left_click(usa.x + usa.width / 2.0, usa.y + usa.height / 2.0);
    let a = g.process();
    assert!(a.is_empty(), "picking a branch is not a final selection");
    g.relayout();
    assert_eq!(as_cs(&g, "cs").column_count(), 2, "USA's children form col 1");
    assert_eq!(as_cs(&g, "cs").path(), &[0]);
    let root = g.host.root_of("cs").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.box_of_part(root, "col-1").is_some(), "col 1 now exists");
    assert!(q.box_of_part(root, "node-1-0").is_some(), "California node in col 1");
}

/// A full drill to a leaf emits Changed(path).
#[test]
fn drill_to_leaf_emits_path() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "cs");

    // USA → California → Los Angeles.
    let usa = part_box(&g, "cs", "node-0-0");
    g.left_click(usa.x + 5.0, usa.y + usa.height / 2.0);
    let _ = g.process();
    g.relayout();
    let ca = part_box(&g, "cs", "node-1-0");
    g.left_click(ca.x + 5.0, ca.y + ca.height / 2.0);
    let _ = g.process();
    g.relayout();
    let la = part_box(&g, "cs", "node-2-1"); // Los Angeles (col 2, node 1)
    g.left_click(la.x + 5.0, la.y + la.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("0/0/1"), "path is the picked index chain");
    g.relayout();
    assert!(as_cs(&g, "cs").is_committed());
}

/// Re-picking a DIFFERENT branch in column 0 replaces the deeper columns.
#[test]
fn switching_branch_replaces_deeper_columns() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "cs");
    let usa = part_box(&g, "cs", "node-0-0");
    g.left_click(usa.x + 5.0, usa.y + usa.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_cs(&g, "cs").column_count(), 2);

    // Now pick UK (col 0, node 1) — col 1 should switch to England.
    let uk = part_box(&g, "cs", "node-0-1");
    g.left_click(uk.x + 5.0, uk.y + uk.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_cs(&g, "cs").path(), &[1]);
    assert_eq!(as_cs(&g, "cs").column_count(), 2);
    // node-1-0 in the new col 1 is England.
    let root = g.host.root_of("cs").unwrap();
    let doc = g.doc();
    let q = LayoutQuery::new(g.hit_test_engine(), doc);
    let england = q.find_part(root, "node-1-0").unwrap();
    assert_eq!(doc.get_attribute(england, "data-value").as_deref(), Some("eng"));
}

/// NO-FAKE-GREEN tooth: node hit reads each REAL laid-out box. Widen one node in
/// column 0 so nodes are non-uniform pitch; clicking node-0-1's true box picks 1.
#[test]
fn node_hit_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        W,
        H,
        "lq-gallery { padding: 16px; } lq-cascade-node[data-part=\"node-0-0\"] { height: 64px; }",
    );
    mount(&mut g, "cs");
    let n0 = part_box(&g, "cs", "node-0-0");
    let n1 = part_box(&g, "cs", "node-0-1");
    assert!(n0.height >= 60.0, "precondition: tall first node (got {})", n0.height);
    g.left_click(n1.x + 5.0, n1.y + n1.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_cs(&g, "cs").path(), &[1], "node-0-1's REAL box picked UK (index 1)");
}

/// Keyboard: Down moves the cursor, Right descends a branch, Left ascends.
#[test]
fn keyboard_drill_and_ascend() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "cs");
    // Right on USA (cursor 0) descends into col 1.
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    g.relayout();
    assert_eq!(as_cs(&g, "cs").column_count(), 2);
    assert_eq!(as_cs(&g, "cs").path(), &[0]);
    // Left ascends back to col 0.
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    g.relayout();
    assert_eq!(as_cs(&g, "cs").column_count(), 1);
    assert!(as_cs(&g, "cs").path().is_empty());
}

/// Picking a branch restyles pixels: the picked node gains the .active selection
/// background, so its own pixels change.
#[test]
fn drill_changes_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    mount(&mut g, "cs");
    let usa = part_box(&g, "cs", "node-0-0");
    let (sx, sy) = ((usa.x + 6.0) as u32, (usa.y + usa.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);

    g.left_click(usa.x + 5.0, usa.y + usa.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "the picked node must gain the active selection background");
}
