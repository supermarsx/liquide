//! Integration tests for `::before` / `::after` pseudo-element BOX GENERATION.
//!
//! These tests pin the CSS contract that a generated-content pseudo-element with
//! a `content` declaration synthesizes a real layout box that:
//!   - is generated even when `content: ""` (icons / focus rings),
//!   - honors explicit `width` / `height` (not only text metrics),
//!   - is ordered before / after the element's in-flow children,
//!   - is ABSENT when `content: none`.
//!
//! They are written to FAIL against the pre-fix behavior (empty content dropped;
//! explicit size ignored) so they have real teeth.

use liquide_dom::Document;
use liquide_layout::tree::{BoxType, PseudoElementKind};
use liquide_layout::{DefaultImageMeasurer, DefaultTextMeasurer, LayoutEngine, Size};
use liquide_style_engine::engine::StyleEngine;

/// Lay out a single `<host>` element under the given stylesheet and return the
/// (document, layout-tree, host-node-id) so tests can inspect the box subtree.
fn layout_host(css: &str) -> (Document, liquide_layout::LayoutTree, liquide_dom::NodeId) {
    let mut doc = Document::new();
    let root = doc.root();
    let host = doc.create_element("host");
    doc.append_child(root, host);

    let mut se = StyleEngine::default();
    se.add_stylesheet(css);
    let styles = se.restyle_all(&doc);

    let mut le = LayoutEngine::new(Size::new(800.0, 600.0), 16.0);
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);
    (doc, tree, host)
}

/// Collect the pseudo-element child boxes of the host node, in child order.
fn pseudo_children(
    tree: &liquide_layout::LayoutTree,
    host: liquide_dom::NodeId,
) -> Vec<(PseudoElementKind, String, liquide_layout::geometry::Rect)> {
    let host_box = tree.find_by_node(host).expect("host must have a box");
    let mut out = Vec::new();
    for &cid in &host_box.children {
        if let Some(b) = tree.get(cid) {
            if let BoxType::PseudoElement { kind, content } = &b.box_type {
                out.push((*kind, content.clone(), b.border_rect));
            }
        }
    }
    out
}

#[test]
fn before_with_string_content_generates_box() {
    let (_doc, tree, host) = layout_host(r#"host { display: block; } host::before { content: "x"; }"#);
    let pe = pseudo_children(&tree, host);
    assert_eq!(pe.len(), 1, "::before with content must generate one box");
    assert_eq!(pe[0].0, PseudoElementKind::Before);
    assert_eq!(pe[0].1, "x");
    assert!(pe[0].2.width > 0.0, "text box must have non-zero width");
}

#[test]
fn content_none_generates_no_box() {
    // Teeth: an explicit `content: none` must NOT generate a pseudo box.
    let (_doc, tree, host) = layout_host(r#"host { display: block; } host::before { content: none; }"#);
    let pe = pseudo_children(&tree, host);
    assert!(pe.is_empty(), "content:none must NOT generate a pseudo box");
}

#[test]
fn no_content_declaration_generates_no_box() {
    // A pseudo rule with no `content` at all generates nothing.
    let (_doc, tree, host) = layout_host(r#"host { display: block; } host::before { color: red; }"#);
    let pe = pseudo_children(&tree, host);
    assert!(pe.is_empty(), "missing content must NOT generate a pseudo box");
}

#[test]
fn empty_content_with_explicit_size_generates_sized_box() {
    // The icon / focus-ring case: `content: ""` plus explicit dimensions must
    // still produce a real, correctly-SIZED box. Pre-fix this was dropped
    // entirely (empty string -> no box), and even if generated it was sized to
    // zero text metrics.
    let (_doc, tree, host) = layout_host(
        r#"host { display: block; }
           host::before { content: ""; width: 24px; height: 16px; }"#,
    );
    let pe = pseudo_children(&tree, host);
    assert_eq!(
        pe.len(),
        1,
        "content:\"\" must STILL generate a box (icon/focus-ring case)"
    );
    let rect = pe[0].2;
    assert!(
        (rect.width - 24.0).abs() < 0.5,
        "explicit width must be honored, got {}",
        rect.width
    );
    assert!(
        (rect.height - 16.0).abs() < 0.5,
        "explicit height must be honored, got {}",
        rect.height
    );
}

#[test]
fn before_and_after_order_around_children() {
    // ::before must come first, real children in the middle, ::after last.
    let mut doc = Document::new();
    let root = doc.root();
    let host = doc.create_element("host");
    doc.append_child(root, host);
    let child = doc.create_element("kid");
    doc.append_child(host, child);

    let mut se = StyleEngine::default();
    se.add_stylesheet(
        r#"host { display: block; }
           kid { display: block; height: 10px; }
           host::before { content: "B"; }
           host::after { content: "A"; }"#,
    );
    let styles = se.restyle_all(&doc);
    let mut le = LayoutEngine::new(Size::new(800.0, 600.0), 16.0);
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let host_box = tree.find_by_node(host).expect("host box");
    let kinds: Vec<Option<PseudoElementKind>> = host_box
        .children
        .iter()
        .map(|&cid| match &tree.get(cid).unwrap().box_type {
            BoxType::PseudoElement { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect();

    // First child is ::before, last is ::after, the kid block is in between.
    assert_eq!(kinds.first().copied().flatten(), Some(PseudoElementKind::Before));
    assert_eq!(kinds.last().copied().flatten(), Some(PseudoElementKind::After));
    assert!(
        kinds.iter().any(|k| k.is_none()),
        "the real child box must sit between the pseudo boxes"
    );
}

#[test]
fn explicit_size_overrides_text_metrics() {
    // Even with text content, explicit width/height take precedence over the
    // measured glyph box (a styleable pseudo box, not just an autosized run).
    let (_doc, tree, host) = layout_host(
        r#"host { display: block; }
           host::before { content: "i"; width: 40px; height: 40px; }"#,
    );
    let pe = pseudo_children(&tree, host);
    assert_eq!(pe.len(), 1);
    let rect = pe[0].2;
    assert!(
        (rect.width - 40.0).abs() < 0.5 && (rect.height - 40.0).abs() < 0.5,
        "explicit size must override text metrics, got {}x{}",
        rect.width,
        rect.height
    );
}
