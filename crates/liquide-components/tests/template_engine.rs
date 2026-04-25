//! Extensive template engine tests — external integration tests.
//!
//! Covers: inline styles in create/apply, unkeyed cleanup (memory leak),
//! structural pseudo-states not clobbered, set_id("") clears ID index,
//! text node handling, keyed reordering, full lifecycle scenarios,
//! and component trait usage.
//!
//! Uses only the public TemplateRenderer API:
//! - `apply_to_node` (patches an existing node to match a template)
//! - `apply_or_create` (finds by id or creates a subtree under parent)
//! - `apply` (component-level render via mount_point)

use liquide_components::template::{Component, TemplateNode, TemplateRenderer};
use liquide_dom::{Document, NodeId, PseudoStateFlags};

// ── Inline styles ────────────────────────────────────────────────────────

#[test]
fn create_subtree_applies_inline_styles() {
    let mut doc = Document::new();
    let root = doc.root();

    let template = TemplateNode::el("div")
        .id("styled-div")
        .style("color", "red")
        .style("font-size", "16px")
        .style("background", "blue");

    let node = TemplateRenderer::apply_or_create(&mut doc, root, "styled-div", &template);

    // Verify styles were applied
    assert_eq!(
        doc.get_inline_style(node, "color").as_deref(),
        Some("red"),
        "should have color: red inline style"
    );
    assert_eq!(
        doc.get_inline_style(node, "font-size").as_deref(),
        Some("16px"),
        "should have font-size: 16px inline style"
    );
    assert_eq!(
        doc.get_inline_style(node, "background").as_deref(),
        Some("blue"),
        "should have background: blue inline style"
    );
}

#[test]
fn patch_node_updates_inline_styles() {
    let mut doc = Document::new();
    let root = doc.root();

    // Create node with initial styles
    let template_v1 = TemplateNode::el("div")
        .id("patch-styles")
        .style("color", "red")
        .style("width", "100px");

    let node = TemplateRenderer::apply_or_create(&mut doc, root, "patch-styles", &template_v1);

    // Patch with new styles
    let template_v2 = TemplateNode::el("div")
        .style("color", "blue")
        .style("height", "50px");

    TemplateRenderer::apply_to_node(&mut doc, node, &template_v2);

    // color should be updated
    assert_eq!(
        doc.get_inline_style(node, "color").as_deref(),
        Some("blue"),
        "color should be updated to blue"
    );
    // height should be added
    assert_eq!(
        doc.get_inline_style(node, "height").as_deref(),
        Some("50px"),
        "height should be added"
    );
    // width should be removed (not in new template)
    assert_eq!(
        doc.get_inline_style(node, "width"),
        None,
        "width should be removed"
    );
}

// ── Inline styles on nested subtrees ─────────────────────────────────────

#[test]
fn create_subtree_applies_inline_styles_to_children() {
    let mut doc = Document::new();
    let root = doc.root();

    let template = TemplateNode::el("parent")
        .id("nested-parent")
        .style("display", "flex")
        .child(
            TemplateNode::el("child")
                .style("flex", "1")
                .style("margin", "10px"),
        );

    let parent = TemplateRenderer::apply_or_create(&mut doc, root, "nested-parent", &template);
    let children = doc.children(parent).to_vec();
    assert_eq!(children.len(), 1);

    let child = children[0];
    assert_eq!(
        doc.get_inline_style(child, "flex").as_deref(),
        Some("1"),
        "child should have flex: 1"
    );
    assert_eq!(
        doc.get_inline_style(child, "margin").as_deref(),
        Some("10px"),
        "child should have margin: 10px"
    );
}

// ── Unkeyed reconciliation — cleanup ─────────────────────────────────────

#[test]
fn unkeyed_reconcile_destroys_all_surplus() {
    let mut doc = Document::new();
    let root = doc.root();
    let parent = doc.create_element("container");
    doc.append_child(root, parent);

    // Create 5 unkeyed children with different tags
    let tags = ["div", "span", "p", "section", "article"];
    for tag in &tags {
        let child = doc.create_element(tag);
        doc.append_child(parent, child);
    }
    assert_eq!(doc.children(parent).len(), 5);

    // Reconcile to 2 children by patching the parent with a template having only 2 children
    let parent_template = TemplateNode::el("container")
        .child(TemplateNode::el("div"))
        .child(TemplateNode::el("span"));
    TemplateRenderer::apply_to_node(&mut doc, parent, &parent_template);

    assert_eq!(
        doc.children(parent).len(),
        2,
        "should have exactly 2 children after reconciliation"
    );
}

#[test]
fn unkeyed_tag_mismatch_creates_new_node() {
    let mut doc = Document::new();
    let root = doc.root();
    let parent = doc.create_element("container");
    doc.append_child(root, parent);

    // Create an old child with tag "div"
    let old_child = doc.create_element("div");
    doc.append_child(parent, old_child);

    // Reconcile with desired "span" — tag mismatch, should create new
    let parent_template = TemplateNode::el("container").child(TemplateNode::el("span"));
    TemplateRenderer::apply_to_node(&mut doc, parent, &parent_template);

    let children = doc.children(parent).to_vec();
    assert_eq!(children.len(), 1);
    // The child should be a new node (different from old_child)
    let new_child = children[0];
    assert_ne!(
        new_child, old_child,
        "tag mismatch should create a new node"
    );
}

// ── Text node reconciliation ─────────────────────────────────────────────

#[test]
fn text_node_content_updated() {
    let mut doc = Document::new();
    let root = doc.root();
    let parent = doc.create_element("container");
    doc.append_child(root, parent);

    let text_node = doc.create_text("Hello");
    doc.append_child(parent, text_node);

    // Reconcile with updated text
    let parent_template = TemplateNode::el("container").child(TemplateNode::text("World"));
    TemplateRenderer::apply_to_node(&mut doc, parent, &parent_template);

    let children = doc.children(parent).to_vec();
    assert_eq!(children.len(), 1);
    let content = doc
        .get(children[0])
        .and_then(|n| n.text_content().map(String::from));
    assert_eq!(content.as_deref(), Some("World"));
}

#[test]
fn text_node_to_element_replacement() {
    let mut doc = Document::new();
    let root = doc.root();
    let parent = doc.create_element("container");
    doc.append_child(root, parent);

    // Start with text node
    let text_node = doc.create_text("Hello");
    doc.append_child(parent, text_node);

    // Reconcile to element — should create new
    let parent_template = TemplateNode::el("container").child(TemplateNode::el("div"));
    TemplateRenderer::apply_to_node(&mut doc, parent, &parent_template);

    let children = doc.children(parent).to_vec();
    assert_eq!(children.len(), 1);
    // New child should be an element, not text
    assert!(
        !doc.get(children[0]).unwrap().is_text(),
        "should be element after replacing text node"
    );
}

#[test]
fn element_to_text_replacement() {
    let mut doc = Document::new();
    let root = doc.root();
    let parent = doc.create_element("container");
    doc.append_child(root, parent);

    // Start with element
    let elem = doc.create_element("div");
    doc.append_child(parent, elem);

    // Reconcile to text node — should create new
    let parent_template = TemplateNode::el("container").child(TemplateNode::text("Hello"));
    TemplateRenderer::apply_to_node(&mut doc, parent, &parent_template);

    let children = doc.children(parent).to_vec();
    assert_eq!(children.len(), 1);
    assert!(
        doc.get(children[0]).unwrap().is_text(),
        "should be text node after replacement"
    );
}

// ── Structural pseudo-states not clobbered ───────────────────────────────

#[test]
fn structural_pseudo_states_preserved_after_patch() {
    let mut doc = Document::new();
    let root = doc.root();
    let parent = doc.create_element("list");
    doc.append_child(root, parent);

    let child = doc.create_element("item");
    doc.append_child(parent, child);

    // Set structural pseudo-states (managed by Document)
    doc.set_pseudo_state(child, PseudoStateFlags::FIRST_CHILD, true);
    doc.set_pseudo_state(child, PseudoStateFlags::LAST_CHILD, true);

    // Patch with a template that only sets HOVER
    let template = TemplateNode::el("item").pseudo(PseudoStateFlags::HOVER);
    TemplateRenderer::apply_to_node(&mut doc, child, &template);

    // FIRST_CHILD and LAST_CHILD should still be set
    assert!(
        doc.get(child)
            .unwrap()
            .has_pseudo_state(PseudoStateFlags::FIRST_CHILD),
        "FIRST_CHILD should NOT be clobbered by template patch"
    );
    assert!(
        doc.get(child)
            .unwrap()
            .has_pseudo_state(PseudoStateFlags::LAST_CHILD),
        "LAST_CHILD should NOT be clobbered by template patch"
    );
    // HOVER should be set
    assert!(
        doc.get(child)
            .unwrap()
            .has_pseudo_state(PseudoStateFlags::HOVER),
        "HOVER should be set by template"
    );
}

#[test]
fn interactive_pseudo_states_cleared_by_template() {
    let mut doc = Document::new();
    let root = doc.root();
    let node = doc.create_element("button");
    doc.append_child(root, node);

    // Set interactive pseudo-states
    doc.set_pseudo_state(node, PseudoStateFlags::HOVER, true);
    doc.set_pseudo_state(node, PseudoStateFlags::ACTIVE, true);

    // Patch with empty pseudo-states — should clear HOVER and ACTIVE
    let template = TemplateNode::el("button");
    TemplateRenderer::apply_to_node(&mut doc, node, &template);

    assert!(
        !doc.get(node)
            .unwrap()
            .has_pseudo_state(PseudoStateFlags::HOVER),
        "HOVER should be cleared when not in template"
    );
    assert!(
        !doc.get(node)
            .unwrap()
            .has_pseudo_state(PseudoStateFlags::ACTIVE),
        "ACTIVE should be cleared when not in template"
    );
}

// ── Element ID management ────────────────────────────────────────────────

#[test]
fn set_id_empty_clears_id_without_corrupting_index() {
    let mut doc = Document::new();
    let root = doc.root();
    let node = doc.create_element("div");
    doc.set_id(node, "my-id");
    doc.append_child(root, node);

    assert_eq!(doc.get_element_by_id("my-id"), Some(node));

    // Clear ID
    doc.set_id(node, "");

    // Empty string should NOT be in the index
    assert_eq!(
        doc.get_element_by_id(""),
        None,
        "empty string should not be in ID index"
    );
    // Old ID should be removed
    assert_eq!(
        doc.get_element_by_id("my-id"),
        None,
        "old ID should be removed from index"
    );
    // Node should have no element ID
    assert!(
        doc.get(node).unwrap().element_id.is_none(),
        "element_id should be None after clearing"
    );
}

#[test]
fn template_clears_id_when_removed() {
    let mut doc = Document::new();
    let root = doc.root();
    let node = doc.create_element("div");
    doc.set_id(node, "old-id");
    doc.append_child(root, node);

    // Patch with no id — should clear
    let template = TemplateNode::el("div");
    TemplateRenderer::apply_to_node(&mut doc, node, &template);

    assert_eq!(
        doc.get_element_by_id("old-id"),
        None,
        "ID should be cleared when template has no id"
    );
}

#[test]
fn template_changes_id() {
    let mut doc = Document::new();
    let root = doc.root();
    let node = doc.create_element("div");
    doc.set_id(node, "old-id");
    doc.append_child(root, node);

    let template = TemplateNode::el("div").id("new-id");
    TemplateRenderer::apply_to_node(&mut doc, node, &template);

    assert_eq!(doc.get_element_by_id("old-id"), None, "old ID gone");
    assert_eq!(doc.get_element_by_id("new-id"), Some(node), "new ID set");
}

// ── Keyed children reordering ────────────────────────────────────────────

#[test]
fn keyed_children_insert_at_beginning() {
    let mut doc = Document::new();
    let root = doc.root();
    let parent = doc.create_element("list");
    doc.append_child(root, parent);

    // Create B, C
    let b = doc.create_element("item");
    doc.set_attribute(b, "data-key", "b");
    doc.append_child(parent, b);
    let c = doc.create_element("item");
    doc.set_attribute(c, "data-key", "c");
    doc.append_child(parent, c);

    // Reconcile to A, B, C (insert A at beginning)
    let desired = vec![
        TemplateNode::el("item").key("a"),
        TemplateNode::el("item").key("b"),
        TemplateNode::el("item").key("c"),
    ];
    let parent_template = TemplateNode::el("list").children(desired);
    TemplateRenderer::apply_to_node(&mut doc, parent, &parent_template);

    let children = doc.children(parent).to_vec();
    assert_eq!(children.len(), 3);
    // B and C should be reused
    assert_eq!(children[1], b);
    assert_eq!(children[2], c);
}

#[test]
fn keyed_children_remove_middle() {
    let mut doc = Document::new();
    let root = doc.root();
    let parent = doc.create_element("list");
    doc.append_child(root, parent);

    let a = doc.create_element("item");
    doc.set_attribute(a, "data-key", "a");
    doc.append_child(parent, a);
    let b = doc.create_element("item");
    doc.set_attribute(b, "data-key", "b");
    doc.append_child(parent, b);
    let c = doc.create_element("item");
    doc.set_attribute(c, "data-key", "c");
    doc.append_child(parent, c);

    // Remove B
    let desired = vec![
        TemplateNode::el("item").key("a"),
        TemplateNode::el("item").key("c"),
    ];
    let parent_template = TemplateNode::el("list").children(desired);
    TemplateRenderer::apply_to_node(&mut doc, parent, &parent_template);

    let children = doc.children(parent).to_vec();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0], a);
    assert_eq!(children[1], c);
}

// ── Empty template ───────────────────────────────────────────────────────

#[test]
fn reconcile_to_empty_removes_all() {
    let mut doc = Document::new();
    let root = doc.root();
    let parent = doc.create_element("container");
    doc.append_child(root, parent);

    for _ in 0..5 {
        let child = doc.create_element("item");
        doc.append_child(parent, child);
    }
    assert_eq!(doc.children(parent).len(), 5);

    let empty_parent = TemplateNode::el("container");
    TemplateRenderer::apply_to_node(&mut doc, parent, &empty_parent);
    assert_eq!(
        doc.children(parent).len(),
        0,
        "all children should be removed"
    );
}

// ── Multiple attributes ──────────────────────────────────────────────────

#[test]
fn create_subtree_sets_multiple_attributes() {
    let mut doc = Document::new();
    let root = doc.root();

    let template = TemplateNode::el("input")
        .attr("type", "text")
        .attr("placeholder", "Enter name")
        .attr("maxlength", "100")
        .attr("data-validation", "required");

    let node = TemplateRenderer::apply_or_create(
        &mut doc,
        root,
        "multi-attr-input",
        &template.id("multi-attr-input"),
    );

    assert_eq!(doc.get_attribute(node, "type").as_deref(), Some("text"));
    assert_eq!(
        doc.get_attribute(node, "placeholder").as_deref(),
        Some("Enter name")
    );
    assert_eq!(doc.get_attribute(node, "maxlength").as_deref(), Some("100"));
    assert_eq!(
        doc.get_attribute(node, "data-validation").as_deref(),
        Some("required")
    );
}

// ── Complex nesting ──────────────────────────────────────────────────────

#[test]
fn deeply_nested_template_creation() {
    let mut doc = Document::new();
    let root = doc.root();

    let template = TemplateNode::el("nav").class("main-nav").child(
        TemplateNode::el("ul")
            .child(
                TemplateNode::el("li").class("active").child(
                    TemplateNode::el("a")
                        .attr("href", "/home")
                        .child(TemplateNode::text("Home")),
                ),
            )
            .child(
                TemplateNode::el("li").child(
                    TemplateNode::el("a")
                        .attr("href", "/about")
                        .child(TemplateNode::text("About")),
                ),
            ),
    );

    let nav =
        TemplateRenderer::apply_or_create(&mut doc, root, "deep-nav", &template.id("deep-nav"));

    // Verify structure: nav > ul > [li > a > text, li > a > text]
    assert!(doc.get(nav).unwrap().has_class("main-nav"));
    let ul = doc.children(nav)[0];
    let lis = doc.children(ul).to_vec();
    assert_eq!(lis.len(), 2);

    let first_a = doc.children(lis[0])[0];
    assert_eq!(doc.get_attribute(first_a, "href").as_deref(), Some("/home"));
    let first_text = doc.children(first_a)[0];
    assert_eq!(
        doc.get(first_text)
            .and_then(|n| n.text_content().map(String::from)),
        Some("Home".to_string())
    );
}

// ── Full lifecycle: create → patch → reorder → delete ────────────────────

#[test]
fn full_lifecycle_scenario() {
    let mut doc = Document::new();
    let root = doc.root();

    // Phase 1: Create
    let mount = doc.create_element("app");
    doc.set_id(mount, "app-root");
    doc.append_child(root, mount);

    let v1 = TemplateNode::el("app")
        .id("app-root")
        .child(
            TemplateNode::el("header")
                .key("header")
                .child(TemplateNode::text("Title")),
        )
        .child(
            TemplateNode::el("main")
                .key("main")
                .child(TemplateNode::text("Content")),
        )
        .child(
            TemplateNode::el("footer")
                .key("footer")
                .child(TemplateNode::text("Footer")),
        );

    TemplateRenderer::apply_to_node(&mut doc, mount, &v1);
    assert_eq!(doc.children(mount).len(), 3);

    // Phase 2: Update content
    let v2 = TemplateNode::el("app")
        .id("app-root")
        .child(
            TemplateNode::el("header")
                .key("header")
                .child(TemplateNode::text("New Title")),
        )
        .child(
            TemplateNode::el("main")
                .key("main")
                .child(TemplateNode::text("New Content")),
        )
        .child(
            TemplateNode::el("footer")
                .key("footer")
                .child(TemplateNode::text("New Footer")),
        );

    TemplateRenderer::apply_to_node(&mut doc, mount, &v2);
    assert_eq!(doc.children(mount).len(), 3);

    // Phase 3: Remove footer
    let v3 = TemplateNode::el("app")
        .id("app-root")
        .child(
            TemplateNode::el("header")
                .key("header")
                .child(TemplateNode::text("Title")),
        )
        .child(
            TemplateNode::el("main")
                .key("main")
                .child(TemplateNode::text("Content")),
        );

    TemplateRenderer::apply_to_node(&mut doc, mount, &v3);
    assert_eq!(doc.children(mount).len(), 2);

    // Phase 4: Reorder — main before header
    let v4 = TemplateNode::el("app")
        .id("app-root")
        .child(
            TemplateNode::el("main")
                .key("main")
                .child(TemplateNode::text("Content")),
        )
        .child(
            TemplateNode::el("header")
                .key("header")
                .child(TemplateNode::text("Title")),
        );

    TemplateRenderer::apply_to_node(&mut doc, mount, &v4);
    assert_eq!(doc.children(mount).len(), 2);
}

// ── Component rendering scenario ─────────────────────────────────────────

#[test]
fn component_multiple_renders() {
    struct Counter {
        count: u32,
    }

    impl Component for Counter {
        fn render(&self) -> TemplateNode {
            TemplateNode::el("counter")
                .id("counter-widget")
                .class_if("zero", self.count == 0)
                .style("display", "flex")
                .child(TemplateNode::text(&format!("Count: {}", self.count)))
        }

        fn mount_point(&self) -> &str {
            "counter-widget"
        }
    }

    let mut doc = Document::new();
    let root = doc.root();
    let mount = doc.create_element("counter");
    doc.set_id(mount, "counter-widget");
    doc.append_child(root, mount);

    // Render at 0
    TemplateRenderer::apply(&mut doc, &Counter { count: 0 });
    assert!(doc.get(mount).unwrap().has_class("zero"));

    // Render at 5
    TemplateRenderer::apply(&mut doc, &Counter { count: 5 });
    assert!(!doc.get(mount).unwrap().has_class("zero"));

    // Render at 10
    TemplateRenderer::apply(&mut doc, &Counter { count: 10 });

    let text_node = doc.children(mount)[0];
    let text = doc
        .get(text_node)
        .and_then(|n| n.text_content().map(String::from));
    assert_eq!(text.as_deref(), Some("Count: 10"));
}
