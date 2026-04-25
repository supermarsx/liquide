//! Integration tests for the accordion/collapsible component.
//!
//! Tests the full lifecycle: rendering to DOM, toggling sections,
//! verifying DOM structure, keyed reconciliation across re-renders,
//! disabled state handling, and inline style propagation.

use liquide_components::accordion::{
    toggle_section, AccordionComponent, AccordionMode, AccordionSection,
};
use liquide_components::template::{Component, TemplateNode, TemplateRenderer};
use liquide_dom::{Document, PseudoStateFlags};

// ── Helpers ──────────────────────────────────────────────────────────────

fn create_doc_with_mount(mount_id: &str) -> (Document, liquide_dom::NodeId) {
    let mut doc = Document::new();
    let root = doc.root();
    let mount = doc.create_element("accordion");
    doc.set_id(mount, mount_id);
    doc.append_child(root, mount);
    (doc, mount)
}

// ── Full DOM rendering ───────────────────────────────────────────────────

#[test]
fn accordion_renders_to_dom() {
    let sections = vec![
        AccordionSection::new("general", "General")
            .expanded(true)
            .child(TemplateNode::text("General content")),
        AccordionSection::new("advanced", "Advanced")
            .expanded(false)
            .child(TemplateNode::text("Advanced content")),
    ];

    let (mut doc, mount) = create_doc_with_mount("accordion-settings");
    let component = AccordionComponent::new("settings", &sections);
    let template = component.render();

    TemplateRenderer::apply_to_node(&mut doc, mount, &template);

    let items = doc.children(mount).to_vec();
    assert_eq!(items.len(), 2, "should have 2 accordion items");

    // First item should be expanded
    assert!(doc.get(items[0]).unwrap().has_class("expanded"));

    // Second item should be collapsed
    assert!(doc.get(items[1]).unwrap().has_class("collapsed"));
}

#[test]
fn accordion_expanded_section_has_visible_content() {
    let sections = vec![AccordionSection::new("s1", "Section 1")
        .expanded(true)
        .child(TemplateNode::text("Visible text"))];

    let (mut doc, mount) = create_doc_with_mount("accordion-test");
    let component = AccordionComponent::new("test", &sections);
    TemplateRenderer::apply_to_node(&mut doc, mount, &component.render());

    let item = doc.children(mount)[0];
    let item_children = doc.children(item).to_vec();
    // Should have header + content
    assert_eq!(item_children.len(), 2);

    let content = item_children[1]; // accordion-content
                                    // Content should have display: block
    assert_eq!(
        doc.get_inline_style(content, "display").as_deref(),
        Some("block"),
        "expanded content should have display: block"
    );
    // Content should have the text child
    let content_children = doc.children(content).to_vec();
    assert!(
        !content_children.is_empty(),
        "expanded content should have children"
    );
}

#[test]
fn accordion_collapsed_section_hides_content() {
    let sections = vec![AccordionSection::new("s1", "Section 1")
        .expanded(false)
        .child(TemplateNode::text("Hidden text"))];

    let (mut doc, mount) = create_doc_with_mount("accordion-test");
    let component = AccordionComponent::new("test", &sections);
    TemplateRenderer::apply_to_node(&mut doc, mount, &component.render());

    let item = doc.children(mount)[0];
    let content = doc.children(item).to_vec()[1];
    assert_eq!(
        doc.get_inline_style(content, "display").as_deref(),
        Some("none"),
        "collapsed content should have display: none"
    );
}

// ── Toggle + re-render cycle ─────────────────────────────────────────────

#[test]
fn accordion_toggle_and_rerender() {
    let mut sections = vec![
        AccordionSection::new("s1", "First").expanded(true),
        AccordionSection::new("s2", "Second").expanded(false),
    ];

    let (mut doc, mount) = create_doc_with_mount("accordion-toggle");

    // Initial render
    let c1 = AccordionComponent::new("toggle", &sections);
    TemplateRenderer::apply_to_node(&mut doc, mount, &c1.render());

    let items = doc.children(mount).to_vec();
    assert!(
        doc.get(items[0]).unwrap().has_class("expanded"),
        "s1 starts expanded"
    );
    assert!(
        doc.get(items[1]).unwrap().has_class("collapsed"),
        "s2 starts collapsed"
    );

    // Toggle s2 (single mode → s1 closes, s2 opens)
    toggle_section(&mut sections, 1, AccordionMode::Single);
    assert!(!sections[0].expanded);
    assert!(sections[1].expanded);

    // Re-render
    let c2 = AccordionComponent::new("toggle", &sections);
    TemplateRenderer::apply_to_node(&mut doc, mount, &c2.render());

    let items = doc.children(mount).to_vec();
    assert!(
        doc.get(items[0]).unwrap().has_class("collapsed"),
        "s1 should be collapsed after toggle"
    );
    assert!(
        doc.get(items[1]).unwrap().has_class("expanded"),
        "s2 should be expanded after toggle"
    );
}

#[test]
fn accordion_multiple_mode_toggle() {
    let mut sections = vec![
        AccordionSection::new("a", "A").expanded(false),
        AccordionSection::new("b", "B").expanded(false),
        AccordionSection::new("c", "C").expanded(false),
    ];

    let (mut doc, mount) = create_doc_with_mount("accordion-multi");

    // Open A and C
    toggle_section(&mut sections, 0, AccordionMode::Multiple);
    toggle_section(&mut sections, 2, AccordionMode::Multiple);

    let component = AccordionComponent::new("multi", &sections);
    TemplateRenderer::apply_to_node(&mut doc, mount, &component.render());

    let items = doc.children(mount).to_vec();
    assert!(
        doc.get(items[0]).unwrap().has_class("expanded"),
        "A expanded"
    );
    assert!(
        doc.get(items[1]).unwrap().has_class("collapsed"),
        "B collapsed"
    );
    assert!(
        doc.get(items[2]).unwrap().has_class("expanded"),
        "C expanded"
    );
}

// ── Keyed reconciliation across renders ──────────────────────────────────

#[test]
fn accordion_keyed_sections_reused() {
    let sections_v1 = vec![
        AccordionSection::new("s1", "First"),
        AccordionSection::new("s2", "Second"),
        AccordionSection::new("s3", "Third"),
    ];

    let (mut doc, mount) = create_doc_with_mount("accordion-keyed");
    let c1 = AccordionComponent::new("keyed", &sections_v1);
    TemplateRenderer::apply_to_node(&mut doc, mount, &c1.render());

    let items_v1 = doc.children(mount).to_vec();
    assert_eq!(items_v1.len(), 3);

    // Remove middle section, reorder
    let sections_v2 = vec![
        AccordionSection::new("s3", "Third"),
        AccordionSection::new("s1", "First"),
    ];
    let c2 = AccordionComponent::new("keyed", &sections_v2);
    TemplateRenderer::apply_to_node(&mut doc, mount, &c2.render());

    let items_v2 = doc.children(mount).to_vec();
    assert_eq!(
        items_v2.len(),
        2,
        "should have 2 items after removing middle"
    );

    // Keyed nodes should be reused (same NodeIds)
    assert_eq!(items_v2[0], items_v1[2], "s3 should be reused");
    assert_eq!(items_v2[1], items_v1[0], "s1 should be reused");
}

// ── Disabled sections ────────────────────────────────────────────────────

#[test]
fn accordion_disabled_section_in_dom() {
    let sections = vec![
        AccordionSection::new("s1", "Enabled").enabled(true),
        AccordionSection::new("s2", "Disabled").enabled(false),
    ];

    let (mut doc, mount) = create_doc_with_mount("accordion-disabled");
    let component = AccordionComponent::new("disabled", &sections);
    TemplateRenderer::apply_to_node(&mut doc, mount, &component.render());

    let items = doc.children(mount).to_vec();

    // Enabled section should NOT have disabled class
    assert!(!doc.get(items[0]).unwrap().has_class("disabled"));

    // Disabled section should have disabled class and pseudo-state
    assert!(doc.get(items[1]).unwrap().has_class("disabled"));
    assert!(
        doc.get(items[1])
            .unwrap()
            .has_pseudo_state(PseudoStateFlags::DISABLED),
        "disabled accordion item should have :disabled pseudo-state"
    );
}

#[test]
fn toggle_disabled_section_noop() {
    let mut sections = vec![AccordionSection::new("s1", "Disabled")
        .expanded(false)
        .enabled(false)];

    toggle_section(&mut sections, 0, AccordionMode::Multiple);
    assert!(!sections[0].expanded, "disabled section should not toggle");
}

// ── Empty accordion ──────────────────────────────────────────────────────

#[test]
fn accordion_empty_sections() {
    let sections: Vec<AccordionSection> = vec![];

    let (mut doc, mount) = create_doc_with_mount("accordion-empty");
    let component = AccordionComponent::new("empty", &sections);
    TemplateRenderer::apply_to_node(&mut doc, mount, &component.render());

    let items = doc.children(mount).to_vec();
    assert!(items.is_empty(), "empty accordion should have no items");
}

// ── Header structure in DOM ──────────────────────────────────────────────

#[test]
fn accordion_header_contains_icon_and_title_in_dom() {
    let sections = vec![AccordionSection::new("s1", "My Section")];

    let (mut doc, mount) = create_doc_with_mount("accordion-structure");
    let component = AccordionComponent::new("structure", &sections);
    TemplateRenderer::apply_to_node(&mut doc, mount, &component.render());

    let item = doc.children(mount)[0];
    let header = doc.children(item)[0]; // accordion-header

    let header_children = doc.children(header).to_vec();
    assert_eq!(header_children.len(), 2, "header should have icon + title");

    // First child: accordion-icon
    assert_eq!(
        doc.get(header_children[0]).unwrap().tag_name(),
        "accordion-icon"
    );
    // Second child: accordion-title
    assert_eq!(
        doc.get(header_children[1]).unwrap().tag_name(),
        "accordion-title"
    );

    // Title should contain text
    let title_text = doc.children(header_children[1])[0];
    assert!(doc.get(title_text).unwrap().is_text());
}

// ── Hover state in DOM ───────────────────────────────────────────────────

#[test]
fn accordion_hover_applied_to_dom_header() {
    let sections = vec![
        AccordionSection::new("s1", "A"),
        AccordionSection::new("s2", "B"),
    ];

    let (mut doc, mount) = create_doc_with_mount("accordion-hover");
    let component = AccordionComponent::new("hover", &sections).hover(Some(0));
    TemplateRenderer::apply_to_node(&mut doc, mount, &component.render());

    let items = doc.children(mount).to_vec();
    let header0 = doc.children(items[0])[0];
    let header1 = doc.children(items[1])[0];

    assert!(
        doc.get(header0)
            .unwrap()
            .has_pseudo_state(PseudoStateFlags::HOVER),
        "first header should be hovered"
    );
    assert!(
        !doc.get(header1)
            .unwrap()
            .has_pseudo_state(PseudoStateFlags::HOVER),
        "second header should NOT be hovered"
    );
}

// ── Settings panel scenario ──────────────────────────────────────────────

#[test]
fn accordion_settings_panel_scenario() {
    let mut sections = vec![
        AccordionSection::new("appearance", "Appearance")
            .expanded(true)
            .child(TemplateNode::el("setting-row").child(TemplateNode::text("Theme: Dark")))
            .child(TemplateNode::el("setting-row").child(TemplateNode::text("Wallpaper: Forest"))),
        AccordionSection::new("network", "Network")
            .expanded(false)
            .child(TemplateNode::el("setting-row").child(TemplateNode::text("WiFi: Connected"))),
        AccordionSection::new("about", "About")
            .expanded(false)
            .child(TemplateNode::el("setting-row").child(TemplateNode::text("Version: 1.0"))),
    ];

    let (mut doc, mount) = create_doc_with_mount("accordion-settings");

    // First render
    let c1 = AccordionComponent::new("settings", &sections).mode(AccordionMode::Single);
    TemplateRenderer::apply_to_node(&mut doc, mount, &c1.render());

    assert_eq!(doc.children(mount).len(), 3);

    // Navigate to Network
    toggle_section(&mut sections, 1, AccordionMode::Single);
    assert!(!sections[0].expanded);
    assert!(sections[1].expanded);

    // Re-render
    let c2 = AccordionComponent::new("settings", &sections).mode(AccordionMode::Single);
    TemplateRenderer::apply_to_node(&mut doc, mount, &c2.render());

    let items = doc.children(mount).to_vec();
    assert!(doc.get(items[0]).unwrap().has_class("collapsed"));
    assert!(doc.get(items[1]).unwrap().has_class("expanded"));

    // Navigate to About
    toggle_section(&mut sections, 2, AccordionMode::Single);
    let c3 = AccordionComponent::new("settings", &sections).mode(AccordionMode::Single);
    TemplateRenderer::apply_to_node(&mut doc, mount, &c3.render());

    let items = doc.children(mount).to_vec();
    assert!(doc.get(items[0]).unwrap().has_class("collapsed"));
    assert!(doc.get(items[1]).unwrap().has_class("collapsed"));
    assert!(doc.get(items[2]).unwrap().has_class("expanded"));
}
