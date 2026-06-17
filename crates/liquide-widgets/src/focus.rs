//! Focus-ring helpers — a `tabindex`-equivalent ordered traversal over widgets
//! marked focusable, reusing the dispatcher's `set_focus` for the actual DOM
//! `:focus` state.
//!
//! Widgets opt in by carrying a `data-focusable` attribute on their root (the
//! `Component` adds it). [`FocusRing`] computes the document-order list of
//! focusable widget roots and answers "what's next / previous" so shared
//! Tab / Shift-Tab and arrow-key navigation behave consistently across families.
//!
//! This is intentionally a thin, pure helper: it does NOT own focus state (the
//! [`WidgetHost`](crate::host::WidgetHost) + [`EventDispatcher`] do); it only
//! computes ordering from the DOM so the host can pick the next target.
//!
//! [`EventDispatcher`]: liquide_hit_test::EventDispatcher

use liquide_dom::{Document, NodeId};

/// The attribute a widget root carries to join the focus ring.
pub const FOCUSABLE_ATTR: &str = "data-focusable";

/// An ordered, read-only view of the focusable widget roots in a document.
pub struct FocusRing {
    /// Focusable roots in document (pre-order) order.
    order: Vec<NodeId>,
}

impl FocusRing {
    /// Build the ring by walking `doc` from a root in document order, collecting
    /// every element whose `data-focusable` attribute is present and not
    /// `"false"` (a disabled widget sets `data-focusable="false"` to drop out).
    pub fn collect(doc: &Document, from: NodeId) -> Self {
        let mut order = Vec::new();
        Self::walk(doc, from, &mut order);
        Self { order }
    }

    fn walk(doc: &Document, node: NodeId, out: &mut Vec<NodeId>) {
        if Self::is_focusable(doc, node) {
            out.push(node);
        }
        for &child in doc.children(node) {
            Self::walk(doc, child, out);
        }
    }

    fn is_focusable(doc: &Document, node: NodeId) -> bool {
        match doc.get_attribute(node, FOCUSABLE_ATTR) {
            Some(v) => v != "false",
            None => false,
        }
    }

    /// The focusable roots, in tab order.
    pub fn order(&self) -> &[NodeId] {
        &self.order
    }

    /// Whether the ring has any focusable widgets.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// The first focusable widget (where focus lands on initial Tab).
    pub fn first(&self) -> Option<NodeId> {
        self.order.first().copied()
    }

    /// The last focusable widget (where focus lands on initial Shift-Tab).
    pub fn last(&self) -> Option<NodeId> {
        self.order.last().copied()
    }

    /// The widget after `current` in tab order, wrapping to the first. Returns
    /// the first widget when `current` is not in the ring (or is `None`).
    pub fn next(&self, current: Option<NodeId>) -> Option<NodeId> {
        self.step(current, 1)
    }

    /// The widget before `current` in tab order, wrapping to the last.
    pub fn prev(&self, current: Option<NodeId>) -> Option<NodeId> {
        self.step(current, -1)
    }

    fn step(&self, current: Option<NodeId>, dir: isize) -> Option<NodeId> {
        if self.order.is_empty() {
            return None;
        }
        let len = self.order.len() as isize;
        let idx = current.and_then(|c| self.order.iter().position(|&n| n == c));
        match idx {
            Some(i) => {
                let next = (i as isize + dir).rem_euclid(len) as usize;
                Some(self.order[next])
            }
            None => {
                // Not currently in the ring: forward -> first, backward -> last.
                if dir >= 0 {
                    self.first()
                } else {
                    self.last()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_dom::Document;

    fn focusable(doc: &mut Document, parent: NodeId, tag: &str, on: bool) -> NodeId {
        let el = doc.create_element(tag);
        doc.set_attribute(el, FOCUSABLE_ATTR, if on { "true" } else { "false" });
        doc.append_child(parent, el);
        el
    }

    #[test]
    fn ring_collects_in_document_order_skipping_disabled() {
        let mut doc = Document::new();
        let root = doc.root();
        let a = focusable(&mut doc, root, "lq-button", true);
        let _disabled = focusable(&mut doc, root, "lq-button", false);
        let b = focusable(&mut doc, root, "lq-input", true);
        // A plain element without the attr is not in the ring.
        let plain = doc.create_element("lq-panel");
        doc.append_child(root, plain);

        let ring = FocusRing::collect(&doc, root);
        assert_eq!(ring.order(), &[a, b]);
    }

    #[test]
    fn next_prev_wrap_around() {
        let mut doc = Document::new();
        let root = doc.root();
        let a = focusable(&mut doc, root, "lq-button", true);
        let b = focusable(&mut doc, root, "lq-input", true);
        let ring = FocusRing::collect(&doc, root);

        assert_eq!(ring.next(None), Some(a));
        assert_eq!(ring.next(Some(a)), Some(b));
        assert_eq!(ring.next(Some(b)), Some(a)); // wrap
        assert_eq!(ring.prev(None), Some(b));
        assert_eq!(ring.prev(Some(a)), Some(b)); // wrap
        assert_eq!(ring.prev(Some(b)), Some(a));
    }
}
