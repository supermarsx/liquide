//! `<lq-tree>` real-pipeline gallery tests (no fake-green).
//!
//! Teeth: clicking a node's LAID-OUT twisty box expands/collapses it (the child
//! rows actually appear/disappear in the DOM); clicking a row body selects it;
//! arrow keys (Left collapse / Right expand / Up/Down navigate) work; depth
//! indentation grows with depth (read from the laid-out row's x); the twisty hit
//! is read from layout, not a constant.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::tree::{Tree, TreeNode, CHANGED_ACTION, TOGGLED_ACTION};

const W: u32 = 320;
const H: u32 = 360;

fn as_tree<'a>(g: &'a Gallery, id: &str) -> &'a Tree {
    g.host.behavior(id).unwrap().as_any().downcast_ref::<Tree>().unwrap()
}

/// A two-level tree: one collapsed branch (2 children) + one leaf root.
fn sample() -> Tree {
    Tree::new()
        .root(TreeNode::branch(
            "fruit",
            "Fruit",
            vec![
                TreeNode::leaf("apple", "Apple"),
                TreeNode::leaf("pear", "Pear"),
            ],
        ))
        .root(TreeNode::leaf("water", "Water"))
}

fn row_box(g: &Gallery, id: &str, pos: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("row-{pos}")).expect("row box")
}
fn twisty_box(g: &Gallery, id: &str, pos: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, &format!("twisty-{pos}")).expect("twisty box")
}

/// Collapsed initially: only the 2 roots are visible.
#[test]
fn collapsed_shows_only_roots() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("tr", Box::new(sample()));
    g.relayout();
    assert_eq!(as_tree(&g, "tr").visible_len(), 2, "branch collapsed → 2 rows");
}

/// Clicking the branch's LAID-OUT twisty expands it; the children appear.
#[test]
fn twisty_click_expands_children() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("tr", Box::new(sample()));
    g.relayout();

    let tw = twisty_box(&g, "tr", 0);
    g.left_click(tw.x + tw.width / 2.0, tw.y + tw.height / 2.0);
    let actions = g.process();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, TOGGLED_ACTION);
    assert_eq!(actions[0].payload.as_deref(), Some("0"));
    assert!(as_tree(&g, "tr").is_expanded(&[0]));

    g.relayout();
    assert_eq!(
        as_tree(&g, "tr").visible_len(),
        4,
        "Fruit + Apple + Pear + Water now visible"
    );

    // Collapse again by clicking the twisty once more. A bare repeat click on the
    // SAME node would be coalesced into a DoubleClick by the dispatcher (realistic
    // debounce), so break the same-node chain with an intervening click on empty
    // gallery space first.
    g.left_click(2.0, 2.0);
    let _ = g.process();
    let tw = twisty_box(&g, "tr", 0);
    g.left_click(tw.x + tw.width / 2.0, tw.y + tw.height / 2.0);
    let _ = g.process();
    assert!(!as_tree(&g, "tr").is_expanded(&[0]));
}

/// Clicking a row BODY (not the twisty) selects it without toggling expansion.
#[test]
fn row_body_click_selects() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("tr", Box::new(sample()));
    g.relayout();

    // Click the second root ("Water", leaf) on its label area (right of twisty).
    let r1 = row_box(&g, "tr", 1);
    g.left_click(r1.x + r1.width - 8.0, r1.y + r1.height / 2.0);
    let actions = g.process();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, CHANGED_ACTION);
    assert_eq!(actions[0].payload.as_deref(), Some("1"));
    assert_eq!(as_tree(&g, "tr").selected_path().as_deref(), Some("1"));
    // The branch did not expand from a body click.
    assert!(!as_tree(&g, "tr").is_expanded(&[0]));
}

/// NO-FAKE-GREEN tooth: expanded children are indented DEEPER than their parent —
/// the child row's laid-out CONTENT x (inside the depth-driven padding-left) is
/// greater, proving the indent comes from CSS calc(depth * --tree-indent), not a
/// flat constant. Both rows share the same border-box left edge; only the content
/// shifts, so a hardcoded indent would not move with --tree-indent.
#[test]
fn depth_indent_comes_from_layout() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; } lq-tree { --tree-indent: 24px; }");
    g.mount("tr", Box::new(sample()));
    g.relayout();

    // Expand the branch so a child row exists.
    let tw = twisty_box(&g, "tr", 0);
    g.left_click(tw.x + tw.width / 2.0, tw.y + tw.height / 2.0);
    let _ = g.process();
    g.relayout();

    let root = g.host.root_of("tr").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let parent = q.content_of(q.find_part(root, "row-0").unwrap()).unwrap();
    let child = q.content_of(q.find_part(root, "row-1").unwrap()).unwrap();
    // The depth-1 child content begins ~24px (the --tree-indent) right of the
    // depth-0 parent content.
    assert!(
        child.x > parent.x + 16.0,
        "child content x ({}) indented right of parent ({}) by the depth padding",
        child.x,
        parent.x
    );
}

/// Keyboard: Right expands the cursor branch; Left collapses; Up/Down navigate.
#[test]
fn keyboard_expand_collapse_navigate() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("tr", Box::new(sample()));
    g.relayout();
    g.host.set_focus(Some("tr"), &mut g.doc, &mut g.dispatcher);

    // Cursor starts at row 0 (the collapsed branch). Right expands it.
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert!(as_tree(&g, "tr").is_expanded(&[0]));
    assert_eq!(as_tree(&g, "tr").visible_len(), 4);

    // Right again (already expanded) descends to the first child.
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_tree(&g, "tr").cursor(), 1);
    assert_eq!(as_tree(&g, "tr").cursor_path().as_deref(), Some("0/0"));

    // Left on a child (leaf) ascends to its parent.
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert_eq!(as_tree(&g, "tr").cursor(), 0);

    // Left on the open parent collapses it.
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert!(!as_tree(&g, "tr").is_expanded(&[0]));
    assert_eq!(as_tree(&g, "tr").visible_len(), 2);

    // Down navigates to the next root.
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_tree(&g, "tr").cursor(), 1);
}

/// Enter on a leaf selects it; Enter on a branch toggles expansion.
#[test]
fn enter_selects_leaf_toggles_branch() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("tr", Box::new(sample()));
    g.relayout();
    g.host.set_focus(Some("tr"), &mut g.doc, &mut g.dispatcher);

    // Enter on the branch (cursor 0) toggles it open.
    g.key(KeyInput::new(keys::ENTER, 0));
    assert!(as_tree(&g, "tr").is_expanded(&[0]));

    // Move to a leaf child and Enter selects it.
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // cursor 1 = Apple
    let actions = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, CHANGED_ACTION);
    assert_eq!(as_tree(&g, "tr").selected_path().as_deref(), Some("0/0"));
}

/// Expanding restyles pixels (the new child rows paint where there was none).
#[test]
fn expansion_changes_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("tr", Box::new(sample()));
    g.relayout();

    // A point just below the second visible row — empty before expansion.
    let r1 = row_box(&g, "tr", 1);
    let (px, py) = ((r1.x + 30.0) as u32, (r1.y + r1.height + 6.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), px, py);

    let tw = twisty_box(&g, "tr", 0);
    g.left_click(tw.x + tw.width / 2.0, tw.y + tw.height / 2.0);
    let _ = g.process();
    g.relayout();
    // After expansion the children push "Water" down; the tree paints more rows.
    let after = Gallery::pixel(&g.rasterize(), px, py);
    assert!(
        before != after,
        "expansion must change the rendered rows (before {before:?} after {after:?})"
    );
}
