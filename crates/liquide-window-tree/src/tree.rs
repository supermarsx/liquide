use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::iterators::*;
use crate::{Rect, Region, WindowExStyle, WindowFlags, WindowId, WindowNode, WindowStyle};

/// Global window ID generator.
static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a fresh unique window ID.
fn alloc_window_id() -> WindowId {
    WindowId(NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed))
}

/// The central window hierarchy.
///
/// Implements a hierarchical window tree with linked-list z-order among siblings.
/// The desktop window is the root — all top-level windows are its children.
#[derive(Debug)]
pub struct WindowTree {
    /// All window nodes, keyed by ID.
    pub(crate) nodes: HashMap<WindowId, WindowNode>,
    /// The root desktop window.
    pub desktop_id: WindowId,
}

impl WindowTree {
    /// Create a new window tree with a desktop root window of the given size.
    pub fn new(width: i32, height: i32) -> Self {
        let desktop_id = alloc_window_id();
        let mut desktop = WindowNode::new(
            desktop_id,
            None,
            0,
            WindowStyle::empty(),
            WindowExStyle::empty(),
            Rect::new(0, 0, width, height),
            String::from("Desktop"),
        );
        desktop.client_rect = Rect::new(0, 0, width, height);

        let mut nodes = HashMap::new();
        nodes.insert(desktop_id, desktop);

        WindowTree { nodes, desktop_id }
    }

    // -----------------------------------------------------------------------
    // Creation / destruction
    // -----------------------------------------------------------------------

    /// Create a new window and insert it as the topmost child of `parent`.
    ///
    /// If `parent` is `None`, the window becomes a child of the desktop.
    pub fn create_window(
        &mut self,
        parent: Option<WindowId>,
        class_id: u32,
        style: WindowStyle,
        ex_style: WindowExStyle,
        bounds: Rect,
        title: impl Into<String>,
    ) -> WindowId {
        let parent_id = parent.unwrap_or(self.desktop_id);
        let id = alloc_window_id();

        let node = WindowNode::new(id, Some(parent_id), class_id, style, ex_style, bounds, title.into());
        self.nodes.insert(id, node);

        // Link as the topmost child (prepend to child list).
        self.prepend_child(parent_id, id);

        // Sync topmost flag from ex_style.
        if ex_style.contains(WindowExStyle::TOPMOST) {
            if let Some(node) = self.nodes.get_mut(&id) {
                node.flags.insert(WindowFlags::TOPMOST);
            }
        }

        id
    }

    /// Destroy a window: unlinks it from the tree and recursively destroys
    /// all its children. Children are destroyed, NOT reparented.
    pub fn destroy_window(&mut self, id: WindowId) {
        if id == self.desktop_id {
            return; // Never destroy the desktop.
        }

        // Mark as being destroyed.
        if let Some(node) = self.nodes.get_mut(&id) {
            node.flags.insert(WindowFlags::IN_DESTROY);
        }

        // Collect descendants to destroy (depth-first).
        let descendants: Vec<WindowId> = self.descendants_dfs(id).collect();

        // Unlink from parent's child list.
        self.unlink_from_parent(id);

        // Remove all descendants (including self).
        for desc_id in descendants {
            self.nodes.remove(&desc_id);
        }
    }

    /// Reparent a window: move it to be a child of `new_parent`.
    pub fn reparent(&mut self, id: WindowId, new_parent: WindowId) {
        if id == self.desktop_id {
            return;
        }
        if !self.nodes.contains_key(&id) || !self.nodes.contains_key(&new_parent) {
            return;
        }

        // Prevent making a window its own descendant.
        if self.is_descendant(new_parent, id) {
            return;
        }

        // Unlink from old parent.
        self.unlink_from_parent(id);

        // Update parent pointer.
        if let Some(node) = self.nodes.get_mut(&id) {
            node.parent = Some(new_parent);
        }

        // Link as topmost child of new parent.
        self.prepend_child(new_parent, id);
    }

    // -----------------------------------------------------------------------
    // Z-order operations
    // -----------------------------------------------------------------------

    /// Move a window to the top of its parent's child list (topmost z-order).
    pub fn bring_to_top(&mut self, id: WindowId) {
        if id == self.desktop_id {
            return;
        }
        let parent_id = match self.nodes.get(&id).and_then(|n| n.parent) {
            Some(p) => p,
            None => return,
        };

        // Already at top?
        if self.nodes.get(&parent_id).and_then(|p| p.first_child) == Some(id) {
            return;
        }

        self.unlink_from_parent(id);

        // Re-insert parent pointer (unlink clears it conceptually, but we
        // need it for prepend_child).
        if let Some(node) = self.nodes.get_mut(&id) {
            node.parent = Some(parent_id);
        }

        self.prepend_child(parent_id, id);
    }

    /// Move a window to the bottom of its parent's child list (lowest z-order).
    pub fn send_to_bottom(&mut self, id: WindowId) {
        if id == self.desktop_id {
            return;
        }
        let parent_id = match self.nodes.get(&id).and_then(|n| n.parent) {
            Some(p) => p,
            None => return,
        };

        self.unlink_from_parent(id);

        if let Some(node) = self.nodes.get_mut(&id) {
            node.parent = Some(parent_id);
        }

        self.append_child(parent_id, id);
    }

    /// Place `id` immediately after `after_id` in z-order (lower = further back).
    ///
    /// Both windows must share the same parent.
    pub fn insert_after(&mut self, id: WindowId, after_id: WindowId) {
        if id == after_id || id == self.desktop_id {
            return;
        }

        let parent_id = match self.nodes.get(&id).and_then(|n| n.parent) {
            Some(p) => p,
            None => return,
        };

        // Verify same parent.
        if self.nodes.get(&after_id).and_then(|n| n.parent) != Some(parent_id) {
            return;
        }

        self.unlink_from_parent(id);

        if let Some(node) = self.nodes.get_mut(&id) {
            node.parent = Some(parent_id);
        }

        // Insert id after after_id in the linked list.
        let old_next = self.nodes.get(&after_id).and_then(|n| n.next_sibling);

        // after_id.next = id
        if let Some(after_node) = self.nodes.get_mut(&after_id) {
            after_node.next_sibling = Some(id);
        }
        // id.prev = after_id, id.next = old_next
        if let Some(node) = self.nodes.get_mut(&id) {
            node.prev_sibling = Some(after_id);
            node.next_sibling = old_next;
        }
        // old_next.prev = id
        if let Some(next_id) = old_next {
            if let Some(next_node) = self.nodes.get_mut(&next_id) {
                next_node.prev_sibling = Some(id);
            }
        }
    }

    /// Place `id` immediately before `before_id` in z-order (higher = more in front).
    ///
    /// Both windows must share the same parent.
    pub fn insert_before(&mut self, id: WindowId, before_id: WindowId) {
        if id == before_id || id == self.desktop_id {
            return;
        }

        let parent_id = match self.nodes.get(&id).and_then(|n| n.parent) {
            Some(p) => p,
            None => return,
        };

        // Verify same parent.
        if self.nodes.get(&before_id).and_then(|n| n.parent) != Some(parent_id) {
            return;
        }

        self.unlink_from_parent(id);

        if let Some(node) = self.nodes.get_mut(&id) {
            node.parent = Some(parent_id);
        }

        let old_prev = self.nodes.get(&before_id).and_then(|n| n.prev_sibling);

        // before_id.prev = id
        if let Some(before_node) = self.nodes.get_mut(&before_id) {
            before_node.prev_sibling = Some(id);
        }
        // id.next = before_id, id.prev = old_prev
        if let Some(node) = self.nodes.get_mut(&id) {
            node.next_sibling = Some(before_id);
            node.prev_sibling = old_prev;
        }
        // old_prev.next = id (or parent.first_child = id if before was first)
        match old_prev {
            Some(prev_id) => {
                if let Some(prev_node) = self.nodes.get_mut(&prev_id) {
                    prev_node.next_sibling = Some(id);
                }
            }
            None => {
                // before_id was the first child — id becomes first.
                if let Some(parent) = self.nodes.get_mut(&parent_id) {
                    parent.first_child = Some(id);
                }
            }
        }
    }

    /// Toggle always-on-top status. When enabling topmost, the window is
    /// moved in front of all non-topmost siblings.
    pub fn set_topmost(&mut self, id: WindowId, topmost: bool) {
        if let Some(node) = self.nodes.get_mut(&id) {
            if topmost {
                node.flags.insert(WindowFlags::TOPMOST);
            } else {
                node.flags.remove(WindowFlags::TOPMOST);
            }
        }

        // Reposition: topmost windows should be at the front of the sibling
        // list. Bring to top if making topmost, otherwise move behind all
        // topmost siblings.
        if topmost {
            self.bring_to_top(id);
        } else {
            // Find the last topmost sibling and insert after it.
            let parent_id = match self.nodes.get(&id).and_then(|n| n.parent) {
                Some(p) => p,
                None => return,
            };
            let mut last_topmost: Option<WindowId> = None;
            let mut child = self.nodes.get(&parent_id).and_then(|p| p.first_child);
            while let Some(cid) = child {
                if cid == id {
                    child = self.nodes.get(&cid).and_then(|n| n.next_sibling);
                    continue;
                }
                if self.nodes.get(&cid).is_some_and(|n| n.is_topmost()) {
                    last_topmost = Some(cid);
                }
                child = self.nodes.get(&cid).and_then(|n| n.next_sibling);
            }

            if let Some(after_id) = last_topmost {
                self.insert_after(id, after_id);
            }
            // If no topmost siblings, leave position as-is.
        }
    }

    // -----------------------------------------------------------------------
    // Traversal
    // -----------------------------------------------------------------------

    /// Iterate children of a window in z-order (front-to-back).
    pub fn children(&self, id: WindowId) -> WindowChildIter<'_> {
        let first = self.nodes.get(&id).and_then(|n| n.first_child);
        WindowChildIter { nodes: &self.nodes, current: first }
    }

    /// Iterate children in reverse z-order (back-to-front).
    pub fn children_back(&self, id: WindowId) -> WindowChildIterRev<'_> {
        // Walk to last child.
        let last = self.last_child(id);
        WindowChildIterRev { nodes: &self.nodes, current: last }
    }

    /// Iterate ancestors (parent, grandparent, ..., root).
    /// Does NOT include `id` itself.
    pub fn ancestors(&self, id: WindowId) -> AncestorIter<'_> {
        let parent = self.nodes.get(&id).and_then(|n| n.parent);
        AncestorIter { nodes: &self.nodes, current: parent }
    }

    /// Depth-first pre-order traversal of the subtree rooted at `id`
    /// (includes `id` itself).
    pub fn descendants_dfs(&self, id: WindowId) -> DfsIter<'_> {
        DfsIter { nodes: &self.nodes, stack: vec![id] }
    }

    /// Iterate siblings of `id` (excluding itself), front-to-back.
    pub fn siblings(&self, id: WindowId) -> SiblingIter<'_> {
        let parent = self.nodes.get(&id).and_then(|n| n.parent);
        let first = parent.and_then(|pid| self.nodes.get(&pid).and_then(|p| p.first_child));
        SiblingIter { nodes: &self.nodes, self_id: id, current: first }
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Check if `id` is a direct child of `parent_id`.
    pub fn is_child(&self, id: WindowId, parent_id: WindowId) -> bool {
        self.nodes.get(&id).and_then(|n| n.parent) == Some(parent_id)
    }

    /// Check if `id` is a descendant (child, grandchild, ...) of `ancestor_id`.
    pub fn is_descendant(&self, id: WindowId, ancestor_id: WindowId) -> bool {
        let mut current = self.nodes.get(&id).and_then(|n| n.parent);
        while let Some(pid) = current {
            if pid == ancestor_id {
                return true;
            }
            current = self.nodes.get(&pid).and_then(|n| n.parent);
        }
        false
    }

    /// All visible top-level windows in z-order (front-to-back).
    pub fn visible_windows(&self) -> Vec<WindowId> {
        self.children(self.desktop_id)
            .filter(|id| self.nodes.get(id).is_some_and(|n| n.is_visible()))
            .collect()
    }

    /// All always-on-top windows in z-order (front-to-back).
    pub fn topmost_windows(&self) -> Vec<WindowId> {
        self.children(self.desktop_id)
            .filter(|id| self.nodes.get(id).is_some_and(|n| n.is_topmost()))
            .collect()
    }

    /// Get an immutable reference to a window node.
    pub fn get(&self, id: WindowId) -> Option<&WindowNode> {
        self.nodes.get(&id)
    }

    /// Get a mutable reference to a window node.
    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut WindowNode> {
        self.nodes.get_mut(&id)
    }

    /// Total number of windows (including desktop).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tree is empty (should never be — always has desktop).
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    // -----------------------------------------------------------------------
    // Region invalidation
    // -----------------------------------------------------------------------

    /// Mark an area of a window as needing repaint.
    /// If `rect` is `None`, the entire client area is invalidated.
    pub fn invalidate(&mut self, id: WindowId, rect: Option<Rect>) {
        if let Some(node) = self.nodes.get_mut(&id) {
            let dirty = rect.unwrap_or(node.client_rect);
            node.update_region = Some(match node.update_region {
                Some(existing) => existing.union(&dirty),
                None => dirty,
            });
            node.flags.insert(WindowFlags::UPDATE_DIRTY);
        }
    }

    /// Mark an area as having been painted (validated).
    /// If `rect` is `None`, the entire update region is cleared.
    pub fn validate(&mut self, id: WindowId, rect: Option<Rect>) {
        if let Some(node) = self.nodes.get_mut(&id) {
            match rect {
                None => {
                    node.update_region = None;
                    node.flags.remove(WindowFlags::UPDATE_DIRTY);
                }
                Some(validated) => {
                    if let Some(existing) = node.update_region {
                        // Subtract the validated rect from the update region.
                        let remaining = Region::Rect(existing).subtract(&Region::Rect(validated));
                        node.update_region = remaining.bounding_rect();
                        if node.update_region.is_none() {
                            node.flags.remove(WindowFlags::UPDATE_DIRTY);
                        }
                    }
                }
            }
        }
    }

    /// Get the current invalid (dirty) region of a window.
    pub fn update_region(&self, id: WindowId) -> Option<Rect> {
        self.nodes.get(&id).and_then(|n| n.update_region)
    }

    // -----------------------------------------------------------------------
    // Internal linked-list helpers
    // -----------------------------------------------------------------------

    /// Prepend `child_id` as the first child (topmost z-order) of `parent_id`.
    fn prepend_child(&mut self, parent_id: WindowId, child_id: WindowId) {
        let old_first = self.nodes.get(&parent_id).and_then(|p| p.first_child);

        // child.prev = None, child.next = old_first
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.prev_sibling = None;
            child.next_sibling = old_first;
        }

        // old_first.prev = child
        if let Some(old_first_id) = old_first {
            if let Some(old_first_node) = self.nodes.get_mut(&old_first_id) {
                old_first_node.prev_sibling = Some(child_id);
            }
        }

        // parent.first_child = child
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.first_child = Some(child_id);
        }
    }

    /// Append `child_id` as the last child (bottom z-order) of `parent_id`.
    fn append_child(&mut self, parent_id: WindowId, child_id: WindowId) {
        let last = self.last_child(parent_id);

        match last {
            Some(last_id) => {
                // last.next = child
                if let Some(last_node) = self.nodes.get_mut(&last_id) {
                    last_node.next_sibling = Some(child_id);
                }
                // child.prev = last, child.next = None
                if let Some(child) = self.nodes.get_mut(&child_id) {
                    child.prev_sibling = Some(last_id);
                    child.next_sibling = None;
                }
            }
            None => {
                // No children — this is the first.
                if let Some(parent) = self.nodes.get_mut(&parent_id) {
                    parent.first_child = Some(child_id);
                }
                if let Some(child) = self.nodes.get_mut(&child_id) {
                    child.prev_sibling = None;
                    child.next_sibling = None;
                }
            }
        }
    }

    /// Unlink a window from its parent's child list.
    fn unlink_from_parent(&mut self, id: WindowId) {
        let (parent_id, prev, next) = {
            let node = match self.nodes.get(&id) {
                Some(n) => n,
                None => return,
            };
            (node.parent, node.prev_sibling, node.next_sibling)
        };

        // prev.next = next
        if let Some(prev_id) = prev {
            if let Some(prev_node) = self.nodes.get_mut(&prev_id) {
                prev_node.next_sibling = next;
            }
        } else if let Some(parent_id) = parent_id {
            // We were the first child — update parent.first_child.
            if let Some(parent) = self.nodes.get_mut(&parent_id) {
                parent.first_child = next;
            }
        }

        // next.prev = prev
        if let Some(next_id) = next {
            if let Some(next_node) = self.nodes.get_mut(&next_id) {
                next_node.prev_sibling = prev;
            }
        }

        // Clear our own sibling links.
        if let Some(node) = self.nodes.get_mut(&id) {
            node.prev_sibling = None;
            node.next_sibling = None;
        }
    }

    /// Find the last child (lowest z-order) of a parent.
    fn last_child(&self, parent_id: WindowId) -> Option<WindowId> {
        let mut current = self.nodes.get(&parent_id)?.first_child?;
        loop {
            match self.nodes.get(&current).and_then(|n| n.next_sibling) {
                Some(next) => current = next,
                None => return Some(current),
            }
        }
    }
}
