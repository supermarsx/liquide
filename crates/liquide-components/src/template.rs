//! Template engine — declarative, zero-overhead UI templates for native shell components.
//!
//! Instead of hand-coding imperative DOM operations (`create_element`, `append_child`,
//! `set_attribute`, `add_class`, …), shell components declare **what** the DOM tree should
//! look like and let the [`TemplateRenderer`] figure out the minimal diff.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────┐    render()    ┌──────────────┐    apply()    ┌──────────┐
//! │  Component    │ ─────────────►│ TemplateNode  │ ────────────►│ Document │
//! │  (Rust data)  │               │ (declarative) │              │  (DOM)   │
//! └──────────────┘                └──────────────┘              └──────────┘
//! ```
//!
//! - **TemplateNode**: a lightweight struct tree describing elements, text,
//!   classes, attributes, and pseudo-states.  Cheap to build every frame.
//! - **Component**: a trait that takes live application state and produces
//!   a `TemplateNode` tree.
//! - **TemplateRenderer**: applies a `TemplateNode` tree to a `Document`,
//!   performing keyed reconciliation to reuse existing DOM nodes and only
//!   touch what changed.
//!
//! ## Zero overhead
//!
//! No string parsing, no virtual DOM diffing, no allocations for unchanged
//! subtrees.  The template is a plain Rust struct that compiles to direct
//! DOM API calls.  Keyed children use a `data-key` attribute for O(n)
//! reconciliation instead of O(n²) brute-force.

use std::collections::HashMap;

use liquide_dom::{Document, NodeId, PseudoStateFlags};

// ── TemplateNode ─────────────────────────────────────────────────

/// A declarative description of a DOM subtree.
///
/// Build with the fluent API:
/// ```rust,ignore
/// TemplateNode::el("dock")
///     .id("shell-dock")
///     .class("visible")
///     .children(items.iter().map(|item|
///         TemplateNode::el("dock-item")
///             .key(&item.app_id)
///             .class_if("active", item.is_running)
///             .class_if("pinned", item.is_pinned)
///             .attr("data-app-id", &item.app_id)
///             .attr("data-icon", &item.icon)
///             .pseudo_if(PseudoStateFlags::HOVER, item.is_hovered)
///             .child(TemplateNode::text(&item.label))
///     ))
/// ```
#[derive(Debug, Clone)]
pub struct TemplateNode {
    /// Element tag name, or empty for text nodes.
    pub tag: String,
    /// Element id (`#id` selector target).
    pub element_id: Option<String>,
    /// CSS classes.
    pub classes: Vec<String>,
    /// HTML attributes.
    pub attrs: Vec<(String, String)>,
    /// Inline CSS styles (property, value) — applied with highest specificity.
    pub inline_styles: Vec<(String, String)>,
    /// Pseudo-state flags to set.
    pub pseudo_states: PseudoStateFlags,
    /// Reconciliation key (maps to `data-key` attribute).
    /// Keyed children are matched by key instead of position.
    pub key: Option<String>,
    /// Child nodes.
    pub children: Vec<TemplateNode>,
    /// If this is a text node, the text content.
    pub text: Option<String>,
}

impl TemplateNode {
    // ── Constructors ─────────────────────────────────────────

    /// Create an element node.
    pub fn el(tag: &str) -> Self {
        Self {
            tag: tag.to_string(),
            element_id: None,
            classes: Vec::new(),
            attrs: Vec::new(),
            inline_styles: Vec::new(),
            pseudo_states: PseudoStateFlags::empty(),
            key: None,
            children: Vec::new(),
            text: None,
        }
    }

    /// Create a text node.
    pub fn text(content: &str) -> Self {
        Self {
            tag: String::new(),
            element_id: None,
            classes: Vec::new(),
            attrs: Vec::new(),
            inline_styles: Vec::new(),
            pseudo_states: PseudoStateFlags::empty(),
            key: None,
            children: Vec::new(),
            text: Some(content.to_string()),
        }
    }

    // ── Fluent setters ───────────────────────────────────────

    /// Set the element id.
    pub fn id(mut self, id: &str) -> Self {
        self.element_id = Some(id.to_string());
        self
    }

    /// Add an inline CSS style property.
    /// These are applied with the highest specificity, overriding selectors.
    pub fn style(mut self, property: &str, value: &str) -> Self {
        self.inline_styles.push((property.to_string(), value.to_string()));
        self
    }

    /// Add a CSS class.
    pub fn class(mut self, class: &str) -> Self {
        self.classes.push(class.to_string());
        self
    }

    /// Conditionally add a CSS class.
    pub fn class_if(mut self, class: &str, condition: bool) -> Self {
        if condition {
            self.classes.push(class.to_string());
        }
        self
    }

    /// Set an HTML attribute.
    pub fn attr(mut self, key: &str, value: &str) -> Self {
        self.attrs.push((key.to_string(), value.to_string()));
        self
    }

    /// Set a pseudo-state flag.
    pub fn pseudo(mut self, flag: PseudoStateFlags) -> Self {
        self.pseudo_states |= flag;
        self
    }

    /// Conditionally set a pseudo-state flag.
    pub fn pseudo_if(mut self, flag: PseudoStateFlags, condition: bool) -> Self {
        if condition {
            self.pseudo_states |= flag;
        }
        self
    }

    /// Set the reconciliation key.
    pub fn key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }

    /// Add a single child.
    pub fn child(mut self, child: TemplateNode) -> Self {
        self.children.push(child);
        self
    }

    /// Add multiple children from an iterator.
    pub fn children(mut self, children: impl IntoIterator<Item = TemplateNode>) -> Self {
        self.children.extend(children);
        self
    }

    /// Check if this is a text node.
    pub fn is_text(&self) -> bool {
        self.text.is_some()
    }
}

// ── Component trait ──────────────────────────────────────────────

/// A shell component that produces a declarative DOM template from live state.
///
/// Components are stateless renderers — all mutable state lives in the shell
/// subsystem (Dock, StatusBar, etc.) and the component reads it to produce
/// a [`TemplateNode`] tree.
///
/// ```rust,ignore
/// struct DockComponent<'a> {
///     items: &'a [DockItemInfo],
///     hover_index: Option<usize>,
/// }
///
/// impl Component for DockComponent<'_> {
///     fn render(&self) -> TemplateNode {
///         TemplateNode::el("dock")
///             .id("shell-dock")
///             .children(self.items.iter().enumerate().map(|(i, item)| {
///                 TemplateNode::el("dock-item")
///                     .key(&item.app_id)
///                     .class_if("active", item.is_running)
///                     .pseudo_if(PseudoStateFlags::HOVER, self.hover_index == Some(i))
///                     .attr("data-app-id", &item.app_id)
///                     .child(TemplateNode::text(&item.label))
///             }))
///     }
///
///     fn mount_point(&self) -> &str {
///         "shell-dock"  // element id of the anchor node
///     }
/// }
/// ```
pub trait Component {
    /// Produce the declarative template tree from current state.
    fn render(&self) -> TemplateNode;

    /// The element id of the DOM node to render into.
    ///
    /// The renderer will find or create this node and reconcile
    /// its children against the template.
    fn mount_point(&self) -> &str;
}

// ── TemplateRenderer ─────────────────────────────────────────────

/// Applies a [`TemplateNode`] tree to a [`Document`] with minimal DOM mutations.
///
/// The renderer performs keyed reconciliation:
/// 1. Children with a `key` are matched by their `data-key` attribute.
/// 2. Unkeyed children are matched by position.
/// 3. New children are created; surplus old children are destroyed.
/// 4. Matched children are patched in-place (attributes, classes, pseudo-states,
///    then recurse into their children).
pub struct TemplateRenderer;

impl TemplateRenderer {
    /// Apply a component's template to the document.
    ///
    /// Finds the component's mount point in the document and reconciles
    /// the mount node's properties + children against the template.
    pub fn apply(doc: &mut Document, component: &dyn Component) {
        let template = component.render();
        let mount_id = component.mount_point();

        if let Some(node_id) = doc.get_element_by_id(mount_id) {
            // Reconcile the mount point node itself
            Self::patch_node(doc, node_id, &template);
        }
        // If mount point doesn't exist, the component isn't mounted yet.
        // The desktop_dom setup should have created the anchor elements.
    }

    /// Apply a template to a specific node in the document.
    ///
    /// This reconciles the node and all its descendants.
    pub fn apply_to_node(doc: &mut Document, node_id: NodeId, template: &TemplateNode) {
        Self::patch_node(doc, node_id, template);
    }

    /// Apply a template as children of a parent node, creating the root from the template
    /// if it doesn't already exist under the parent.
    ///
    /// Returns the root node ID.
    pub fn apply_or_create(
        doc: &mut Document,
        parent: NodeId,
        element_id: &str,
        template: &TemplateNode,
    ) -> NodeId {
        if let Some(existing) = doc.get_element_by_id(element_id) {
            Self::patch_node(doc, existing, template);
            existing
        } else {
            Self::create_subtree(doc, parent, template)
        }
    }

    /// Remove a mounted element by id.
    pub fn unmount(doc: &mut Document, element_id: &str) {
        if let Some(node_id) = doc.get_element_by_id(element_id) {
            if let Some(parent) = doc.parent(node_id) {
                doc.remove_child(parent, node_id);
            }
            doc.destroy_node(node_id);
        }
    }

    // ── Internal reconciliation ──────────────────────────────

    /// Patch a live DOM node to match a template node.
    fn patch_node(doc: &mut Document, node_id: NodeId, template: &TemplateNode) {
        // Handle text nodes
        if let Some(ref text) = template.text {
            // If the existing node is a text node, just update content
            if doc.get(node_id).map_or(false, |n| n.is_text()) {
                if doc.get(node_id).and_then(|n| n.text_content()) != Some(text.as_str()) {
                    doc.set_text_content(node_id, text);
                }
            }
            return;
        }

        // ── Patch element id ────────────────────────────────
        if let Some(ref id) = template.element_id {
            let needs_set = doc
                .get(node_id)
                .and_then(|n| n.element_id.as_deref())
                .map_or(true, |existing| existing != id.as_str());
            if needs_set {
                doc.set_id(node_id, id);
            }
        } else {
            // Clear stale element id if the template no longer specifies one
            if doc.get(node_id).and_then(|n| n.element_id.as_deref()).is_some() {
                doc.set_id(node_id, "");
            }
        }

        // ── Patch classes ───────────────────────────────────
        Self::patch_classes(doc, node_id, &template.classes);

        // ── Patch attributes ────────────────────────────────
        Self::patch_attributes(doc, node_id, &template.attrs);

        // ── Patch inline styles ─────────────────────────────
        Self::patch_inline_styles(doc, node_id, &template.inline_styles);

        // ── Patch pseudo-states ─────────────────────────────
        Self::patch_pseudo_states(doc, node_id, template.pseudo_states);

        // ── Reconcile children ──────────────────────────────
        Self::reconcile_children(doc, node_id, &template.children);
    }

    /// Patch CSS classes: add missing, remove extra.
    fn patch_classes(doc: &mut Document, node_id: NodeId, desired: &[String]) {
        // Get current classes
        let current: Vec<String> = doc
            .get(node_id)
            .map(|n| n.classes.iter().map(String::from).collect())
            .unwrap_or_default();

        // Remove classes not in desired
        for cls in &current {
            if !desired.iter().any(|d| d == cls) {
                doc.remove_class(node_id, cls);
            }
        }

        // Add classes not in current
        for cls in desired {
            if !current.iter().any(|c| c == cls) {
                doc.add_class(node_id, cls);
            }
        }
    }

    /// Patch HTML attributes: set new/changed, remove stale.
    fn patch_attributes(doc: &mut Document, node_id: NodeId, desired: &[(String, String)]) {
        // Collect current attribute keys
        let current_keys: Vec<String> = doc
            .get(node_id)
            .map(|n| n.attrs.iter().map(|(k, _)| k.to_string()).collect())
            .unwrap_or_default();

        // Build desired map for easy lookup
        let desired_map: HashMap<&str, &str> = desired
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        // Remove attributes not in desired (skip internal data-key managed by reconciliation)
        for key in &current_keys {
            if key == "data-key" {
                continue;
            }
            if !desired_map.contains_key(key.as_str()) {
                doc.remove_attribute(node_id, key);
            }
        }

        // Set all desired attributes (set_attribute is a no-op if unchanged
        // due to dirty tracking, so we can safely call it always)
        for (key, value) in desired {
            let needs_set = doc
                .get_attribute(node_id, key)
                .map_or(true, |v| v != *value);
            if needs_set {
                doc.set_attribute(node_id, key, value);
            }
        }
    }

    /// Patch inline CSS styles: set new/changed, remove stale.
    fn patch_inline_styles(doc: &mut Document, node_id: NodeId, desired: &[(String, String)]) {
        // Collect current inline style keys
        let current_keys: Vec<String> = doc
            .get(node_id)
            .map(|n| n.inline_styles.iter().map(|(k, _)| k.to_string()).collect())
            .unwrap_or_default();

        // Build desired map for easy lookup
        let desired_map: std::collections::HashMap<&str, &str> = desired
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        // Remove styles not in desired
        for key in &current_keys {
            if !desired_map.contains_key(key.as_str()) {
                doc.remove_inline_style(node_id, key);
            }
        }

        // Set all desired inline styles
        for (prop, value) in desired {
            let needs_set = doc
                .get_inline_style(node_id, prop)
                .map_or(true, |v| v != *value);
            if needs_set {
                doc.set_inline_style(node_id, prop, value);
            }
        }
    }

    /// Patch pseudo-states to match the desired flags.
    fn patch_pseudo_states(doc: &mut Document, node_id: NodeId, desired: PseudoStateFlags) {
        let current = doc
            .get(node_id)
            .map(|n| n.pseudo_states)
            .unwrap_or(PseudoStateFlags::empty());

        // Only touch bits that differ
        let changed = current ^ desired;
        if changed.is_empty() {
            return;
        }

        // We need to set/clear individual flags.
        // Only manage interactive states; structural ones (FIRST_CHILD, LAST_CHILD, ROOT)
        // are auto-managed by Document.
        let all_flags = [
            PseudoStateFlags::HOVER,
            PseudoStateFlags::ACTIVE,
            PseudoStateFlags::FOCUS,
            PseudoStateFlags::DISABLED,
            PseudoStateFlags::CHECKED,
        ];
        for flag in &all_flags {
            if changed.contains(*flag) {
                doc.set_pseudo_state(node_id, *flag, desired.contains(*flag));
            }
        }
    }

    /// Reconcile children of a parent node against a list of template children.
    ///
    /// Uses keyed reconciliation for children with `key`, positional for the rest.
    fn reconcile_children(
        doc: &mut Document,
        parent: NodeId,
        desired_children: &[TemplateNode],
    ) {
        let old_children: Vec<NodeId> = doc.children(parent).to_vec();

        // Build key→NodeId map from existing children
        let mut key_map: HashMap<String, NodeId> = HashMap::new();
        let mut unkeyed_old: Vec<NodeId> = Vec::new();

        for &child_id in &old_children {
            if let Some(key) = doc.get_attribute(child_id, "data-key") {
                key_map.insert(key, child_id);
            } else {
                unkeyed_old.push(child_id);
            }
        }

        let mut used_old: Vec<NodeId> = Vec::new();
        let mut new_children: Vec<NodeId> = Vec::new();
        let mut unkeyed_idx = 0;

        for desired in desired_children {
            let matched = if let Some(ref key) = desired.key {
                key_map.remove(key)
            } else {
                // Match by position among unkeyed — but only if tag names match
                let m = unkeyed_old.get(unkeyed_idx).copied().filter(|&nid| {
                    // For text nodes, check both are text; for elements, compare tag names
                    if desired.is_text() {
                        doc.get(nid).map_or(false, |n| n.is_text())
                    } else {
                        doc.get(nid)
                            .map_or(false, |n| n.tag_name() == desired.tag)
                    }
                });
                if m.is_some() {
                    unkeyed_idx += 1;
                } else if unkeyed_old.get(unkeyed_idx).is_some() {
                    // Tag mismatch — skip this old node (will be removed later)
                    unkeyed_idx += 1;
                }
                m
            };

            if let Some(existing) = matched {
                // Patch existing node
                Self::patch_node(doc, existing, desired);
                // Set the key attribute if it has one
                if let Some(ref key) = desired.key {
                    doc.set_attribute(existing, "data-key", key);
                }
                used_old.push(existing);
                new_children.push(existing);
            } else {
                // Create new node
                let child_id = Self::create_subtree(doc, parent, desired);
                new_children.push(child_id);
            }
        }

        // Remove surplus keyed children
        for (_, node_id) in key_map {
            doc.remove_child(parent, node_id);
            doc.destroy_node(node_id);
        }

        // Remove surplus unkeyed children
        for &child_id in &unkeyed_old {
            if !used_old.contains(&child_id) {
                doc.remove_child(parent, child_id);
                doc.destroy_node(child_id);
            }
        }

        // Reorder children to match desired order.
        // We do this by detaching all children and re-appending in the right order.
        // This is efficient because Document::append_child detaches first.
        let current: Vec<NodeId> = doc.children(parent).to_vec();
        if current != new_children {
            // Detach all existing children
            for &child in &current {
                doc.remove_child(parent, child);
            }
            // Re-append in desired order
            for &child in &new_children {
                doc.append_child(parent, child);
            }
        }
    }

    /// Create a new subtree from a template and attach it to a parent.
    fn create_subtree(
        doc: &mut Document,
        parent: NodeId,
        template: &TemplateNode,
    ) -> NodeId {
        // Text node
        if let Some(ref text) = template.text {
            let id = doc.create_text(text);
            doc.append_child(parent, id);
            return id;
        }

        // Element node
        let id = doc.create_element(&template.tag);

        if let Some(ref eid) = template.element_id {
            doc.set_id(id, eid);
        }

        for cls in &template.classes {
            doc.add_class(id, cls);
        }

        for (key, val) in &template.attrs {
            doc.set_attribute(id, key, val);
        }

        if let Some(ref key) = template.key {
            doc.set_attribute(id, "data-key", key);
        }

        // Set pseudo-states (only interactive states; structural ones are Document-managed)
        let all_flags = [
            PseudoStateFlags::HOVER,
            PseudoStateFlags::ACTIVE,
            PseudoStateFlags::FOCUS,
            PseudoStateFlags::DISABLED,
            PseudoStateFlags::CHECKED,
        ];
        for flag in &all_flags {
            if template.pseudo_states.contains(*flag) {
                doc.set_pseudo_state(id, *flag, true);
            }
        }

        // Apply inline styles
        for (prop, val) in &template.inline_styles {
            doc.set_inline_style(id, prop, val);
        }

        // Create children recursively
        for child_template in &template.children {
            Self::create_subtree(doc, id, child_template);
        }

        doc.append_child(parent, id);
        id
    }
}

// ── Convenience macros ───────────────────────────────────────────

/// Shorthand for `TemplateNode::el(tag)`.
#[macro_export]
macro_rules! el {
    ($tag:expr) => {
        $crate::template::TemplateNode::el($tag)
    };
}

/// Shorthand for `TemplateNode::text(content)`.
#[macro_export]
macro_rules! text {
    ($content:expr) => {
        $crate::template::TemplateNode::text($content)
    };
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_node_builder() {
        let node = TemplateNode::el("dock")
            .id("shell-dock")
            .class("visible")
            .attr("data-count", "5")
            .child(
                TemplateNode::el("dock-item")
                    .key("files")
                    .class("active")
                    .class("pinned")
                    .attr("data-app-id", "files")
                    .pseudo(PseudoStateFlags::HOVER)
                    .child(TemplateNode::text("Files")),
            );

        assert_eq!(node.tag, "dock");
        assert_eq!(node.element_id.as_deref(), Some("shell-dock"));
        assert_eq!(node.classes, vec!["visible"]);
        assert_eq!(node.children.len(), 1);
        assert_eq!(node.children[0].tag, "dock-item");
        assert_eq!(node.children[0].key.as_deref(), Some("files"));
        assert!(node.children[0].pseudo_states.contains(PseudoStateFlags::HOVER));
        assert_eq!(node.children[0].children[0].text.as_deref(), Some("Files"));
    }

    #[test]
    fn template_class_if() {
        let active = TemplateNode::el("item").class_if("active", true);
        assert_eq!(active.classes, vec!["active"]);

        let inactive = TemplateNode::el("item").class_if("active", false);
        assert!(inactive.classes.is_empty());
    }

    #[test]
    fn template_pseudo_if() {
        let hovered = TemplateNode::el("item").pseudo_if(PseudoStateFlags::HOVER, true);
        assert!(hovered.pseudo_states.contains(PseudoStateFlags::HOVER));

        let not_hovered = TemplateNode::el("item").pseudo_if(PseudoStateFlags::HOVER, false);
        assert!(!not_hovered.pseudo_states.contains(PseudoStateFlags::HOVER));
    }

    #[test]
    fn create_subtree_basic() {
        let mut doc = Document::new();
        let root = doc.root();

        let template = TemplateNode::el("dock")
            .id("test-dock")
            .class("visible")
            .child(
                TemplateNode::el("dock-item")
                    .key("files")
                    .attr("data-app-id", "files")
                    .child(TemplateNode::text("Files")),
            )
            .child(
                TemplateNode::el("dock-item")
                    .key("terminal")
                    .attr("data-app-id", "terminal")
                    .child(TemplateNode::text("Terminal")),
            );

        let dock_id = TemplateRenderer::create_subtree(&mut doc, root, &template);

        // Verify structure
        assert_eq!(doc.get(dock_id).unwrap().element_id.as_deref(), Some("test-dock"));
        assert!(doc.get(dock_id).unwrap().has_class("visible"));
        assert_eq!(doc.children(dock_id).len(), 2);

        let first_item = doc.children(dock_id)[0];
        assert_eq!(doc.get_attribute(first_item, "data-app-id").as_deref(), Some("files"));
        assert_eq!(doc.get_attribute(first_item, "data-key").as_deref(), Some("files"));
    }

    #[test]
    fn reconcile_adds_new_children() {
        let mut doc = Document::new();
        let root = doc.root();
        let parent = doc.create_element("dock");
        doc.append_child(root, parent);

        // Start empty
        assert_eq!(doc.children(parent).len(), 0);

        // Apply template with 2 children
        let children = vec![
            TemplateNode::el("dock-item").key("a").child(TemplateNode::text("A")),
            TemplateNode::el("dock-item").key("b").child(TemplateNode::text("B")),
        ];
        TemplateRenderer::reconcile_children(&mut doc, parent, &children);

        assert_eq!(doc.children(parent).len(), 2);
    }

    #[test]
    fn reconcile_removes_surplus_children() {
        let mut doc = Document::new();
        let root = doc.root();
        let parent = doc.create_element("dock");
        doc.append_child(root, parent);

        // Create 3 unkeyed children
        for label in &["A", "B", "C"] {
            let child = doc.create_element("dock-item");
            let txt = doc.create_text(label);
            doc.append_child(child, txt);
            doc.append_child(parent, child);
        }
        assert_eq!(doc.children(parent).len(), 3);

        // Reconcile to 1 child
        let children = vec![
            TemplateNode::el("dock-item").child(TemplateNode::text("A")),
        ];
        TemplateRenderer::reconcile_children(&mut doc, parent, &children);

        assert_eq!(doc.children(parent).len(), 1);
    }

    #[test]
    fn reconcile_keyed_reuse() {
        let mut doc = Document::new();
        let root = doc.root();
        let parent = doc.create_element("dock");
        doc.append_child(root, parent);

        // Create keyed children A, B, C
        let mut ids = Vec::new();
        for key in &["a", "b", "c"] {
            let child = doc.create_element("dock-item");
            doc.set_attribute(child, "data-key", key);
            doc.append_child(parent, child);
            ids.push(child);
        }

        // Reconcile to B, C, A (reorder)
        let children = vec![
            TemplateNode::el("dock-item").key("b"),
            TemplateNode::el("dock-item").key("c"),
            TemplateNode::el("dock-item").key("a"),
        ];
        TemplateRenderer::reconcile_children(&mut doc, parent, &children);

        let new_order: Vec<NodeId> = doc.children(parent).to_vec();
        assert_eq!(new_order.len(), 3);
        // B should be first, C second, A third
        assert_eq!(new_order[0], ids[1]); // b
        assert_eq!(new_order[1], ids[2]); // c
        assert_eq!(new_order[2], ids[0]); // a
    }

    #[test]
    fn patch_updates_classes() {
        let mut doc = Document::new();
        let root = doc.root();
        let node = doc.create_element("item");
        doc.add_class(node, "old-class");
        doc.add_class(node, "keep");
        doc.append_child(root, node);

        let template = TemplateNode::el("item")
            .class("keep")
            .class("new-class");

        TemplateRenderer::patch_node(&mut doc, node, &template);

        assert!(!doc.get(node).unwrap().has_class("old-class"));
        assert!(doc.get(node).unwrap().has_class("keep"));
        assert!(doc.get(node).unwrap().has_class("new-class"));
    }

    #[test]
    fn patch_updates_attributes() {
        let mut doc = Document::new();
        let root = doc.root();
        let node = doc.create_element("item");
        doc.set_attribute(node, "data-old", "x");
        doc.set_attribute(node, "data-keep", "y");
        doc.append_child(root, node);

        let template = TemplateNode::el("item")
            .attr("data-keep", "y")
            .attr("data-new", "z");

        TemplateRenderer::patch_node(&mut doc, node, &template);

        assert!(doc.get_attribute(node, "data-old").is_none());
        assert_eq!(doc.get_attribute(node, "data-keep").as_deref(), Some("y"));
        assert_eq!(doc.get_attribute(node, "data-new").as_deref(), Some("z"));
    }

    #[test]
    fn component_trait_basic() {
        struct TestComponent;

        impl Component for TestComponent {
            fn render(&self) -> TemplateNode {
                TemplateNode::el("test-element")
                    .id("test-mount")
                    .class("test-class")
                    .child(TemplateNode::text("hello"))
            }

            fn mount_point(&self) -> &str {
                "test-mount"
            }
        }

        let mut doc = Document::new();
        let root = doc.root();
        let mount = doc.create_element("test-element");
        doc.set_id(mount, "test-mount");
        doc.append_child(root, mount);

        let component = TestComponent;
        TemplateRenderer::apply(&mut doc, &component);

        let mount_node = doc.get_element_by_id("test-mount").unwrap();
        assert!(doc.get(mount_node).unwrap().has_class("test-class"));
        assert_eq!(doc.children(mount_node).len(), 1);
    }

    #[test]
    fn apply_or_create_creates_new() {
        let mut doc = Document::new();
        let root = doc.root();

        let template = TemplateNode::el("overlay")
            .id("my-overlay")
            .class("visible")
            .child(TemplateNode::text("content"));

        let node_id = TemplateRenderer::apply_or_create(
            &mut doc, root, "my-overlay", &template,
        );

        assert!(doc.get_element_by_id("my-overlay").is_some());
        assert_eq!(doc.children(node_id).len(), 1);
    }

    #[test]
    fn apply_or_create_patches_existing() {
        let mut doc = Document::new();
        let root = doc.root();

        // Create initial
        let node = doc.create_element("overlay");
        doc.set_id(node, "my-overlay");
        doc.add_class(node, "old");
        doc.append_child(root, node);

        // Apply template — should patch, not recreate
        let template = TemplateNode::el("overlay")
            .id("my-overlay")
            .class("new");

        let node_id = TemplateRenderer::apply_or_create(
            &mut doc, root, "my-overlay", &template,
        );

        assert_eq!(node_id, node); // Same node reused
        assert!(!doc.get(node).unwrap().has_class("old"));
        assert!(doc.get(node).unwrap().has_class("new"));
    }

    #[test]
    fn unmount_removes_node() {
        let mut doc = Document::new();
        let root = doc.root();
        let node = doc.create_element("overlay");
        doc.set_id(node, "removal-test");
        doc.append_child(root, node);

        assert!(doc.get_element_by_id("removal-test").is_some());
        TemplateRenderer::unmount(&mut doc, "removal-test");
        assert!(doc.get_element_by_id("removal-test").is_none());
    }

    #[test]
    fn full_dock_component_scenario() {
        // Simulate a full dock component render cycle
        let mut doc = Document::new();
        let root = doc.root();

        // Setup: create dock mount point (like DesktopDocument does)
        let dock = doc.create_element("dock");
        doc.set_id(dock, "shell-dock");
        doc.append_child(root, dock);

        // First render: 3 apps
        let items = vec![
            ("files", "Files", true, true),
            ("terminal", "Terminal", false, true),
            ("browser", "Browser", true, false),
        ];
        let template = TemplateNode::el("dock")
            .id("shell-dock")
            .children(items.iter().enumerate().map(|(i, (id, label, running, pinned))| {
                TemplateNode::el("dock-item")
                    .key(id)
                    .class_if("active", *running)
                    .class_if("pinned", *pinned)
                    .attr("data-app-id", id)
                    .attr("data-icon", id)
                    .attr("data-index", &i.to_string())
                    .pseudo_if(PseudoStateFlags::HOVER, false)
                    .child(TemplateNode::text(label))
            }));

        TemplateRenderer::apply_to_node(&mut doc, dock, &template);
        assert_eq!(doc.children(dock).len(), 3);

        let first = doc.children(dock)[0];
        assert!(doc.get(first).unwrap().has_class("active"));
        assert!(doc.get(first).unwrap().has_class("pinned"));

        // Second render: browser closed, hover on terminal
        let items2 = vec![
            ("files", "Files", true, true),
            ("terminal", "Terminal", false, true),
        ];
        let template2 = TemplateNode::el("dock")
            .id("shell-dock")
            .children(items2.iter().enumerate().map(|(i, (id, label, running, pinned))| {
                TemplateNode::el("dock-item")
                    .key(id)
                    .class_if("active", *running)
                    .class_if("pinned", *pinned)
                    .attr("data-app-id", id)
                    .attr("data-icon", id)
                    .attr("data-index", &i.to_string())
                    .pseudo_if(PseudoStateFlags::HOVER, i == 1)
                    .child(TemplateNode::text(label))
            }));

        TemplateRenderer::apply_to_node(&mut doc, dock, &template2);
        assert_eq!(doc.children(dock).len(), 2); // browser removed

        let terminal = doc.children(dock)[1];
        assert!(doc.get(terminal).unwrap().has_pseudo_state(PseudoStateFlags::HOVER));
    }
}
