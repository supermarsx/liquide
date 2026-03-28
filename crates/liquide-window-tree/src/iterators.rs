use std::collections::HashMap;

use crate::{WindowId, WindowNode};

/// Iterates children of a window in z-order (front-to-back, i.e. topmost first).
pub struct WindowChildIter<'a> {
    pub(crate) nodes: &'a HashMap<WindowId, WindowNode>,
    pub(crate) current: Option<WindowId>,
}

impl<'a> Iterator for WindowChildIter<'a> {
    type Item = WindowId;

    fn next(&mut self) -> Option<WindowId> {
        let id = self.current?;
        if let Some(node) = self.nodes.get(&id) {
            self.current = node.next_sibling;
            Some(id)
        } else {
            self.current = None;
            None
        }
    }
}

/// Iterates children of a window in reverse z-order (back-to-front, i.e. bottommost first).
pub struct WindowChildIterRev<'a> {
    pub(crate) nodes: &'a HashMap<WindowId, WindowNode>,
    pub(crate) current: Option<WindowId>,
}

impl<'a> Iterator for WindowChildIterRev<'a> {
    type Item = WindowId;

    fn next(&mut self) -> Option<WindowId> {
        let id = self.current?;
        if let Some(node) = self.nodes.get(&id) {
            self.current = node.prev_sibling;
            Some(id)
        } else {
            self.current = None;
            None
        }
    }
}

/// Iterates ancestors of a window (parent, grandparent, ..., root).
pub struct AncestorIter<'a> {
    pub(crate) nodes: &'a HashMap<WindowId, WindowNode>,
    pub(crate) current: Option<WindowId>,
}

impl<'a> Iterator for AncestorIter<'a> {
    type Item = WindowId;

    fn next(&mut self) -> Option<WindowId> {
        let id = self.current?;
        if let Some(node) = self.nodes.get(&id) {
            self.current = node.parent;
            Some(id)
        } else {
            self.current = None;
            None
        }
    }
}

/// Iterates siblings of a window (excluding itself), front-to-back in z-order.
pub struct SiblingIter<'a> {
    pub(crate) nodes: &'a HashMap<WindowId, WindowNode>,
    pub(crate) self_id: WindowId,
    pub(crate) current: Option<WindowId>,
}

impl<'a> Iterator for SiblingIter<'a> {
    type Item = WindowId;

    fn next(&mut self) -> Option<WindowId> {
        loop {
            let id = self.current?;
            if let Some(node) = self.nodes.get(&id) {
                self.current = node.next_sibling;
                if id != self.self_id {
                    return Some(id);
                }
            } else {
                self.current = None;
                return None;
            }
        }
    }
}

/// Depth-first pre-order traversal of a subtree.
pub struct DfsIter<'a> {
    pub(crate) nodes: &'a HashMap<WindowId, WindowNode>,
    pub(crate) stack: Vec<WindowId>,
}

impl<'a> Iterator for DfsIter<'a> {
    type Item = WindowId;

    fn next(&mut self) -> Option<WindowId> {
        let id = self.stack.pop()?;
        let node = self.nodes.get(&id)?;

        // Push children in reverse z-order so that topmost child is visited first.
        // Walk to the last sibling, collecting along the way.
        let mut children = Vec::new();
        let mut child = node.first_child;
        while let Some(cid) = child {
            children.push(cid);
            child = self.nodes.get(&cid).and_then(|n| n.next_sibling);
        }
        // Push in reverse so topmost (first_child) ends up on top of stack.
        for cid in children.into_iter().rev() {
            self.stack.push(cid);
        }

        Some(id)
    }
}
