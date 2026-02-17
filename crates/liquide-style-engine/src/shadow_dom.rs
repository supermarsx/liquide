//! Shadow DOM style scoping for the CSS engine.
//!
//! Provides the logic for confining CSS rules to their shadow tree and
//! implementing the `:host`, `:host()`, `:host-context()` and `::slotted()`
//! pseudo selectors.
//!
//! ## Scoping model
//!
//! - Styles defined inside a shadow root only apply to elements within that
//!   shadow tree.  They do not leak to the light DOM or to nested shadow trees.
//! - The `:host` pseudo-class matches the shadow host element (the element the
//!   shadow root is attached to).
//! - `::slotted(simple)` matches light-DOM children distributed into `<slot>`
//!   elements.
//! - The outer document's styles do **not** pierce into shadow trees unless
//!   the property is inheritable and not overridden.

use liquide_dom::{Document, NodeData, NodeId};

// ─── scope boundary helpers ────────────────────────────────────────

/// Return the nearest shadow root ancestor of `node_id`, or `None` if the
/// element lives in the light DOM.
pub fn enclosing_shadow_root(doc: &Document, node_id: NodeId) -> Option<NodeId> {
    let mut current = node_id;
    loop {
        let parent = doc.parent(current)?;
        if let Some(node) = doc.get(parent) {
            if matches!(node.data, NodeData::ShadowRoot) {
                return Some(parent);
            }
        }
        current = parent;
    }
}

/// Return the *host* element of a shadow root.  The host is the shadow root's
/// parent in the node tree.
pub fn shadow_host(doc: &Document, shadow_root_id: NodeId) -> Option<NodeId> {
    doc.parent(shadow_root_id)
}

/// Check whether `node_id` lives inside a shadow tree.
pub fn is_in_shadow_tree(doc: &Document, node_id: NodeId) -> bool {
    enclosing_shadow_root(doc, node_id).is_some()
}

/// Check whether two node IDs share the same scope (i.e. are in the same
/// shadow tree, or are both in the light DOM).
pub fn same_scope(doc: &Document, a: NodeId, b: NodeId) -> bool {
    enclosing_shadow_root(doc, a) == enclosing_shadow_root(doc, b)
}

// ─── style application helpers ─────────────────────────────────────

/// Determine whether a style sheet whose *owning* node is `stylesheet_owner`
/// should apply to the element `target`.
///
/// * Light-DOM sheets (no shadow root ancestor) apply only to light-DOM nodes.
/// * Shadow-DOM sheets apply only to nodes inside that same shadow root.
pub fn stylesheet_applies(doc: &Document, stylesheet_owner: NodeId, target: NodeId) -> bool {
    let sheet_scope = enclosing_shadow_root(doc, stylesheet_owner);
    let target_scope = enclosing_shadow_root(doc, target);
    sheet_scope == target_scope
}

/// Check if `node_id` is a shadow host (has a ShadowRoot child).
pub fn is_shadow_host(doc: &Document, node_id: NodeId) -> bool {
    for &child_id in doc.children(node_id) {
        if let Some(child) = doc.get(child_id) {
            if matches!(child.data, NodeData::ShadowRoot) {
                return true;
            }
        }
    }
    false
}

/// Return the shadow root child of a host element, if any.
pub fn host_shadow_root(doc: &Document, host_id: NodeId) -> Option<NodeId> {
    for &child_id in doc.children(host_id) {
        if let Some(child) = doc.get(child_id) {
            if matches!(child.data, NodeData::ShadowRoot) {
                return Some(child_id);
            }
        }
    }
    None
}

/// Check `:host` — the target node must be the shadow host of the scope the
/// stylesheet lives in.
pub fn matches_host(doc: &Document, stylesheet_scope: Option<NodeId>, target: NodeId) -> bool {
    match stylesheet_scope {
        Some(shadow_id) => shadow_host(doc, shadow_id) == Some(target),
        None => false, // :host has no meaning in the light DOM
    }
}

/// Collect nodes that are slotted (light-DOM children of the host distributed
/// into `<slot>`) for `::slotted()`.
pub fn slotted_children(doc: &Document, host_id: NodeId) -> Vec<NodeId> {
    // In the simplified model, all light-DOM children of the host that are
    // *not* the shadow root are considered slotted.
    let mut result = Vec::new();
    for &child_id in doc.children(host_id) {
        if let Some(child) = doc.get(child_id) {
            if !matches!(child.data, NodeData::ShadowRoot) {
                result.push(child_id);
            }
        }
    }
    result
}

/// Whether a CSS property should pierce the shadow boundary via inheritance.
///
/// Only inheritable properties (color, font-size, line-height, etc.) cross the
/// boundary.  This returns `true` for the well-known inheritable set.
pub fn property_inherits_across_boundary(property: &str) -> bool {
    matches!(
        property,
        "color"
            | "cursor"
            | "direction"
            | "font"
            | "font-family"
            | "font-size"
            | "font-style"
            | "font-variant"
            | "font-weight"
            | "letter-spacing"
            | "line-height"
            | "list-style"
            | "list-style-image"
            | "list-style-position"
            | "list-style-type"
            | "orphans"
            | "quotes"
            | "text-align"
            | "text-indent"
            | "text-transform"
            | "visibility"
            | "white-space"
            | "widows"
            | "word-spacing"
            | "writing-mode"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_dom::Document;

    fn setup_shadow_dom() -> (Document, NodeId, NodeId, NodeId, NodeId) {
        let mut doc = Document::new();
        // host (element)
        let host = doc.create_element("div");
        doc.append_child(doc.root(), host);
        // shadow root
        let shadow = doc.create_shadow_root();
        doc.append_child(host, shadow);
        // element inside shadow
        let inner = doc.create_element("p");
        doc.append_child(shadow, inner);
        // light DOM child of host (slotted)
        let light = doc.create_element("span");
        doc.append_child(host, light);
        (doc, host, shadow, inner, light)
    }

    #[test]
    fn test_enclosing_shadow() {
        let (doc, _host, shadow, inner, light) = setup_shadow_dom();
        assert_eq!(enclosing_shadow_root(&doc, inner), Some(shadow));
        assert_eq!(enclosing_shadow_root(&doc, light), None);
    }

    #[test]
    fn test_is_shadow_host() {
        let (doc, host, _shadow, inner, _light) = setup_shadow_dom();
        assert!(is_shadow_host(&doc, host));
        assert!(!is_shadow_host(&doc, inner));
    }

    #[test]
    fn test_same_scope() {
        let (doc, _host, _shadow, inner, light) = setup_shadow_dom();
        // inner is in shadow, light is in light DOM
        assert!(!same_scope(&doc, inner, light));
        assert!(same_scope(&doc, light, light));
    }

    #[test]
    fn test_stylesheet_applies() {
        let (doc, _host, _shadow, inner, light) = setup_shadow_dom();
        // Shadow stylesheet (owner = inner) applies to inner, not light
        assert!(stylesheet_applies(&doc, inner, inner));
        assert!(!stylesheet_applies(&doc, inner, light));
        // Light DOM stylesheet (owner = light) applies to light, not inner
        assert!(stylesheet_applies(&doc, light, light));
        assert!(!stylesheet_applies(&doc, light, inner));
    }

    #[test]
    fn test_matches_host() {
        let (doc, host, shadow, inner, _light) = setup_shadow_dom();
        assert!(matches_host(&doc, Some(shadow), host));
        assert!(!matches_host(&doc, Some(shadow), inner));
        assert!(!matches_host(&doc, None, host));
    }

    #[test]
    fn test_slotted_children() {
        let (doc, host, _shadow, _inner, light) = setup_shadow_dom();
        let slotted = slotted_children(&doc, host);
        assert!(slotted.contains(&light));
        assert_eq!(slotted.len(), 1);
    }

    #[test]
    fn test_property_inheritance() {
        assert!(property_inherits_across_boundary("color"));
        assert!(property_inherits_across_boundary("font-size"));
        assert!(!property_inherits_across_boundary("margin"));
        assert!(!property_inherits_across_boundary("padding"));
        assert!(!property_inherits_across_boundary("display"));
    }
}
