use std::collections::HashMap;

use crate::node::{AccessibleNode, NodeId, Role};
use crate::{A11yError, Result};

/// The accessibility tree — manages the full hierarchy of accessible nodes.
#[derive(Debug, Clone)]
pub struct AccessibilityTree {
    nodes: HashMap<NodeId, AccessibleNode>,
    root: Option<NodeId>,
    next_id: NodeId,
}

impl AccessibilityTree {
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root: None,
            next_id: 1,
        }
    }

    /// Allocate a unique node ID.
    pub fn allocate_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Set the root node of the tree.
    pub fn set_root(&mut self, node: AccessibleNode) {
        let id = node.id;
        self.nodes.insert(id, node);
        self.root = Some(id);
    }

    /// Add a node as a child of the given parent.
    pub fn add_node(&mut self, parent_id: NodeId, mut node: AccessibleNode) -> Result<()> {
        if !self.nodes.contains_key(&parent_id) {
            return Err(A11yError::NodeNotFound { id: parent_id });
        }
        node.parent = Some(parent_id);
        let child_id = node.id;
        self.nodes.insert(child_id, node);
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.push(child_id);
        }
        Ok(())
    }

    /// Remove a node and all its descendants.
    pub fn remove_node(&mut self, id: NodeId) -> Result<()> {
        if !self.nodes.contains_key(&id) {
            return Err(A11yError::NodeNotFound { id });
        }

        // Collect descendants
        let mut to_remove = vec![id];
        let mut i = 0;
        while i < to_remove.len() {
            let nid = to_remove[i];
            if let Some(node) = self.nodes.get(&nid) {
                to_remove.extend(node.children.iter().copied());
            }
            i += 1;
        }

        // Remove parent reference
        if let Some(node) = self.nodes.get(&id) {
            if let Some(pid) = node.parent {
                if let Some(parent) = self.nodes.get_mut(&pid) {
                    parent.children.retain(|c| *c != id);
                }
            }
        }

        // Remove nodes
        for nid in &to_remove {
            self.nodes.remove(nid);
        }

        if self.root == Some(id) {
            self.root = None;
        }

        Ok(())
    }

    /// Get a node by ID.
    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<&AccessibleNode> {
        self.nodes.get(&id)
    }

    /// Get a mutable reference to a node by ID.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut AccessibleNode> {
        self.nodes.get_mut(&id)
    }

    /// Get the root node ID.
    #[must_use]
    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    /// Get the children of a node.
    #[must_use]
    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        self.nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default()
    }

    /// Get the parent of a node.
    #[must_use]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(&id).and_then(|n| n.parent)
    }

    /// Walk the tree depth-first from the root, calling the callback for each node.
    pub fn walk<F>(&self, mut callback: F)
    where
        F: FnMut(&AccessibleNode),
    {
        if let Some(root_id) = self.root {
            self.walk_subtree(root_id, &mut callback);
        }
    }

    fn walk_subtree<F>(&self, id: NodeId, callback: &mut F)
    where
        F: FnMut(&AccessibleNode),
    {
        if let Some(node) = self.nodes.get(&id) {
            callback(node);
            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                self.walk_subtree(child_id, callback);
            }
        }
    }

    /// Find all nodes with the given role.
    #[must_use]
    pub fn find_by_role(&self, role: Role) -> Vec<NodeId> {
        self.nodes
            .values()
            .filter(|n| n.role == role)
            .map(|n| n.id)
            .collect()
    }

    /// Find all nodes whose name contains the given substring.
    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Vec<NodeId> {
        self.nodes
            .values()
            .filter(|n| n.name.contains(name))
            .map(|n| n.id)
            .collect()
    }

    /// Number of nodes in the tree.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for AccessibilityTree {
    fn default() -> Self {
        Self::new()
    }
}
