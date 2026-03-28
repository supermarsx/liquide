//! Accessibility tree for the bridge layer.
//!
//! Provides [`AccessibleNode`] with WAI-ARIA roles, states, and relationships,
//! and [`AccessibleTree`] for managing the full accessibility hierarchy.  This
//! is the bridge-level representation — it mirrors the in-process
//! [`liquide_a11y::AccessibilityTree`] but is designed for serialisation to
//! platform assistive-technology APIs (AT-SPI, etc.).

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Role (WAI-ARIA)
// ---------------------------------------------------------------------------

/// WAI-ARIA role for an accessible node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibleRole {
    Window,
    Dialog,
    Button,
    Label,
    TextInput,
    Checkbox,
    RadioButton,
    Slider,
    List,
    ListItem,
    Menu,
    MenuItem,
    Toolbar,
    StatusBar,
    ScrollBar,
    Table,
    TableRow,
    TableCell,
    Image,
    Link,
    ProgressBar,
    Separator,
    Tab,
    TabPanel,
    TreeItem,
    Alert,
    Tooltip,
}

impl std::fmt::Display for AccessibleRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

// ---------------------------------------------------------------------------
// State flags
// ---------------------------------------------------------------------------

/// State flags for an accessible node (WAI-ARIA states and properties).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibleState {
    Focusable,
    Focused,
    Selected,
    Expanded,
    Collapsed,
    Checked,
    Disabled,
    Editable,
    ReadOnly,
    Required,
    Modal,
}

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------

/// Bounding rectangle in screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    #[must_use]
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    /// Check if a point is inside this bounding rectangle.
    #[must_use]
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x
            && px < self.x + self.width
            && py >= self.y
            && py < self.y + self.height
    }
}

/// Unique identifier for a bridge-level accessible node.
pub type NodeId = u64;

/// A single node in the bridge-level accessibility tree.
#[derive(Debug, Clone)]
pub struct AccessibleNode {
    pub id: NodeId,
    pub role: AccessibleRole,
    pub name: String,
    pub description: String,
    pub states: Vec<AccessibleState>,
    pub value: Option<String>,
    pub bounds: Option<Bounds>,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
}

impl AccessibleNode {
    #[must_use]
    pub fn new(id: NodeId, role: AccessibleRole, name: &str) -> Self {
        Self {
            id,
            role,
            name: name.to_string(),
            description: String::new(),
            states: Vec::new(),
            value: None,
            bounds: None,
            children: Vec::new(),
            parent: None,
        }
    }

    /// Set the description.
    pub fn set_description(&mut self, desc: &str) {
        self.description = desc.to_string();
    }

    /// Set the value.
    pub fn set_value(&mut self, val: &str) {
        self.value = Some(val.to_string());
    }

    /// Set the bounding rectangle.
    pub fn set_bounds(&mut self, bounds: Bounds) {
        self.bounds = Some(bounds);
    }

    /// Add a state flag.  Duplicates are ignored.
    pub fn add_state(&mut self, state: AccessibleState) {
        if !self.states.contains(&state) {
            self.states.push(state);
        }
    }

    /// Remove a state flag.
    pub fn remove_state(&mut self, state: AccessibleState) {
        self.states.retain(|s| *s != state);
    }

    /// Check whether a state flag is present.
    #[must_use]
    pub fn has_state(&self, state: AccessibleState) -> bool {
        self.states.contains(&state)
    }
}

// ---------------------------------------------------------------------------
// Tree
// ---------------------------------------------------------------------------

/// The bridge-level accessibility tree.
///
/// Manages all [`AccessibleNode`]s, tracks the root and the currently focused
/// node, and provides lookup helpers aligned with WAI-ARIA semantics.
#[derive(Debug, Clone)]
pub struct AccessibleTree {
    nodes: HashMap<NodeId, AccessibleNode>,
    root: Option<NodeId>,
    focused: Option<NodeId>,
    next_id: NodeId,
}

impl AccessibleTree {
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root: None,
            focused: None,
            next_id: 1,
        }
    }

    /// Allocate a unique node ID.
    pub fn allocate_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Set the root node.  The node is inserted into the tree.
    pub fn set_root(&mut self, node: AccessibleNode) {
        let id = node.id;
        self.nodes.insert(id, node);
        self.root = Some(id);
    }

    /// Get the root node ID.
    #[must_use]
    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    /// Number of nodes in the tree.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Add a child node under `parent_id`.  Returns `false` if the parent
    /// does not exist.
    pub fn add_node(&mut self, parent_id: NodeId, mut node: AccessibleNode) -> bool {
        if !self.nodes.contains_key(&parent_id) {
            return false;
        }
        node.parent = Some(parent_id);
        let child_id = node.id;
        self.nodes.insert(child_id, node);
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.push(child_id);
        }
        true
    }

    /// Remove a node and all of its descendants.  Returns `false` if the
    /// node does not exist.
    pub fn remove_node(&mut self, id: NodeId) -> bool {
        if !self.nodes.contains_key(&id) {
            return false;
        }

        // Collect the subtree.
        let mut to_remove = vec![id];
        let mut i = 0;
        while i < to_remove.len() {
            let nid = to_remove[i];
            if let Some(node) = self.nodes.get(&nid) {
                to_remove.extend(node.children.iter().copied());
            }
            i += 1;
        }

        // Unlink from parent.
        if let Some(node) = self.nodes.get(&id) {
            if let Some(pid) = node.parent {
                if let Some(parent) = self.nodes.get_mut(&pid) {
                    parent.children.retain(|c| *c != id);
                }
            }
        }

        for nid in &to_remove {
            self.nodes.remove(nid);
        }

        if self.root == Some(id) {
            self.root = None;
        }
        if let Some(fid) = self.focused {
            if to_remove.contains(&fid) {
                self.focused = None;
            }
        }

        true
    }

    /// Get a node by ID.
    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<&AccessibleNode> {
        self.nodes.get(&id)
    }

    /// Get a mutable reference to a node.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut AccessibleNode> {
        self.nodes.get_mut(&id)
    }

    /// Update a node's name.  Returns `false` if the node does not exist.
    pub fn update_name(&mut self, id: NodeId, name: &str) -> bool {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.name = name.to_string();
            true
        } else {
            false
        }
    }

    /// Update a node's value.  Returns `false` if the node does not exist.
    pub fn update_value(&mut self, id: NodeId, value: &str) -> bool {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.value = Some(value.to_string());
            true
        } else {
            false
        }
    }

    /// Update a node's bounds.  Returns `false` if the node does not exist.
    pub fn update_bounds(&mut self, id: NodeId, bounds: Bounds) -> bool {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.bounds = Some(bounds);
            true
        } else {
            false
        }
    }

    // -- Focus --------------------------------------------------------------

    /// Set focus to a node.
    pub fn set_focused(&mut self, id: NodeId) {
        if let Some(old) = self.focused {
            if let Some(n) = self.nodes.get_mut(&old) {
                n.remove_state(AccessibleState::Focused);
            }
        }
        if let Some(n) = self.nodes.get_mut(&id) {
            n.add_state(AccessibleState::Focused);
        }
        self.focused = Some(id);
    }

    /// Clear focus.
    pub fn clear_focused(&mut self) {
        if let Some(old) = self.focused.take() {
            if let Some(n) = self.nodes.get_mut(&old) {
                n.remove_state(AccessibleState::Focused);
            }
        }
    }

    /// Get the currently focused node.
    #[must_use]
    pub fn focused_node(&self) -> Option<&AccessibleNode> {
        self.focused.and_then(|id| self.nodes.get(&id))
    }

    // -- Search -------------------------------------------------------------

    /// Find all nodes with the given role.
    #[must_use]
    pub fn find_by_role(&self, role: AccessibleRole) -> Vec<NodeId> {
        self.nodes
            .values()
            .filter(|n| n.role == role)
            .map(|n| n.id)
            .collect()
    }

    /// Find all nodes whose name contains `substring`.
    #[must_use]
    pub fn find_by_name(&self, substring: &str) -> Vec<NodeId> {
        self.nodes
            .values()
            .filter(|n| n.name.contains(substring))
            .map(|n| n.id)
            .collect()
    }

    /// Return the path from `id` up to the root (inclusive), ordered
    /// \[id, parent, …, root\].
    #[must_use]
    pub fn path_to_root(&self, id: NodeId) -> Vec<NodeId> {
        let mut path = Vec::new();
        let mut current = Some(id);
        while let Some(nid) = current {
            if self.nodes.contains_key(&nid) {
                path.push(nid);
                current = self.nodes.get(&nid).and_then(|n| n.parent);
            } else {
                break;
            }
        }
        path
    }

    /// Depth-first walk from the root, calling `callback` for each node.
    pub fn walk<F>(&self, mut callback: F)
    where
        F: FnMut(&AccessibleNode),
    {
        if let Some(rid) = self.root {
            self.walk_impl(rid, &mut callback);
        }
    }

    fn walk_impl<F>(&self, id: NodeId, callback: &mut F)
    where
        F: FnMut(&AccessibleNode),
    {
        if let Some(node) = self.nodes.get(&id) {
            callback(node);
            let children: Vec<NodeId> = node.children.clone();
            for cid in children {
                self.walk_impl(cid, callback);
            }
        }
    }

    /// Return the node at screen coordinates `(x, y)`, searching
    /// depth-first (deepest hit wins).
    #[must_use]
    pub fn hit_test(&self, x: f64, y: f64) -> Option<NodeId> {
        self.root.and_then(|rid| self.hit_test_impl(rid, x, y))
    }

    fn hit_test_impl(&self, id: NodeId, x: f64, y: f64) -> Option<NodeId> {
        let node = self.nodes.get(&id)?;
        // Check children first (depth-first, last painted = on top).
        for &cid in node.children.iter().rev() {
            if let Some(hit) = self.hit_test_impl(cid, x, y) {
                return Some(hit);
            }
        }
        // Then self.
        if let Some(b) = &node.bounds {
            if b.contains(x, y) {
                return Some(id);
            }
        }
        None
    }
}

impl Default for AccessibleTree {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> AccessibleTree {
        let mut tree = AccessibleTree::new();
        let root_id = tree.allocate_id();
        let mut root = AccessibleNode::new(root_id, AccessibleRole::Window, "Main Window");
        root.set_bounds(Bounds::new(0.0, 0.0, 800.0, 600.0));
        tree.set_root(root);

        let btn_id = tree.allocate_id();
        let mut btn = AccessibleNode::new(btn_id, AccessibleRole::Button, "OK");
        btn.set_bounds(Bounds::new(100.0, 200.0, 80.0, 30.0));
        tree.add_node(root_id, btn);

        let lbl_id = tree.allocate_id();
        let lbl = AccessibleNode::new(lbl_id, AccessibleRole::Label, "Hello");
        tree.add_node(root_id, lbl);

        tree
    }

    #[test]
    fn tree_creation_and_count() {
        let tree = sample_tree();
        assert_eq!(tree.node_count(), 3);
        assert!(tree.root().is_some());
    }

    #[test]
    fn add_and_get_node() {
        let tree = sample_tree();
        let root_id = tree.root().unwrap();
        let root = tree.get(root_id).unwrap();
        assert_eq!(root.name, "Main Window");
        assert_eq!(root.role, AccessibleRole::Window);
        assert_eq!(root.children.len(), 2);
    }

    #[test]
    fn add_to_nonexistent_parent_fails() {
        let mut tree = AccessibleTree::new();
        let node = AccessibleNode::new(99, AccessibleRole::Button, "orphan");
        assert!(!tree.add_node(42, node));
    }

    #[test]
    fn remove_subtree() {
        let mut tree = sample_tree();
        let root_id = tree.root().unwrap();
        tree.remove_node(root_id);
        assert_eq!(tree.node_count(), 0);
        assert!(tree.root().is_none());
    }

    #[test]
    fn remove_leaf() {
        let mut tree = sample_tree();
        let root_id = tree.root().unwrap();
        let btn_id = tree.get(root_id).unwrap().children[0];
        tree.remove_node(btn_id);
        assert_eq!(tree.node_count(), 2);
        assert_eq!(tree.get(root_id).unwrap().children.len(), 1);
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut tree = sample_tree();
        assert!(!tree.remove_node(999));
    }

    #[test]
    fn find_by_role() {
        let tree = sample_tree();
        let buttons = tree.find_by_role(AccessibleRole::Button);
        assert_eq!(buttons.len(), 1);
        let windows = tree.find_by_role(AccessibleRole::Window);
        assert_eq!(windows.len(), 1);
        let sliders = tree.find_by_role(AccessibleRole::Slider);
        assert!(sliders.is_empty());
    }

    #[test]
    fn find_by_name() {
        let tree = sample_tree();
        let results = tree.find_by_name("OK");
        assert_eq!(results.len(), 1);
        let none = tree.find_by_name("nonexistent");
        assert!(none.is_empty());
    }

    #[test]
    fn focused_node() {
        let mut tree = sample_tree();
        assert!(tree.focused_node().is_none());
        let root_id = tree.root().unwrap();
        let btn_id = tree.get(root_id).unwrap().children[0];
        tree.set_focused(btn_id);
        let focused = tree.focused_node().unwrap();
        assert_eq!(focused.id, btn_id);
        assert!(focused.has_state(AccessibleState::Focused));
    }

    #[test]
    fn clear_focused() {
        let mut tree = sample_tree();
        let root_id = tree.root().unwrap();
        let btn_id = tree.get(root_id).unwrap().children[0];
        tree.set_focused(btn_id);
        tree.clear_focused();
        assert!(tree.focused_node().is_none());
        assert!(!tree.get(btn_id).unwrap().has_state(AccessibleState::Focused));
    }

    #[test]
    fn path_to_root() {
        let tree = sample_tree();
        let root_id = tree.root().unwrap();
        let btn_id = tree.get(root_id).unwrap().children[0];
        let path = tree.path_to_root(btn_id);
        assert_eq!(path, vec![btn_id, root_id]);
    }

    #[test]
    fn path_to_root_of_root() {
        let tree = sample_tree();
        let root_id = tree.root().unwrap();
        let path = tree.path_to_root(root_id);
        assert_eq!(path, vec![root_id]);
    }

    #[test]
    fn path_to_root_nonexistent() {
        let tree = sample_tree();
        let path = tree.path_to_root(999);
        assert!(path.is_empty());
    }

    #[test]
    fn walk_visits_all() {
        let tree = sample_tree();
        let mut visited = Vec::new();
        tree.walk(|n| visited.push(n.id));
        assert_eq!(visited.len(), 3);
    }

    #[test]
    fn hit_test_finds_deepest() {
        let tree = sample_tree();
        // The button occupies (100,200)-(180,230) inside the window.
        let hit = tree.hit_test(110.0, 210.0);
        let root_id = tree.root().unwrap();
        let btn_id = tree.get(root_id).unwrap().children[0];
        assert_eq!(hit, Some(btn_id));
    }

    #[test]
    fn hit_test_falls_through_to_parent() {
        let tree = sample_tree();
        // A point inside the window but not inside the button.
        let hit = tree.hit_test(500.0, 500.0);
        assert_eq!(hit, tree.root());
    }

    #[test]
    fn hit_test_misses() {
        let tree = sample_tree();
        let hit = tree.hit_test(900.0, 900.0);
        assert!(hit.is_none());
    }

    #[test]
    fn update_name() {
        let mut tree = sample_tree();
        let root_id = tree.root().unwrap();
        assert!(tree.update_name(root_id, "Renamed"));
        assert_eq!(tree.get(root_id).unwrap().name, "Renamed");
        assert!(!tree.update_name(999, "nope"));
    }

    #[test]
    fn update_value() {
        let mut tree = sample_tree();
        let root_id = tree.root().unwrap();
        assert!(tree.update_value(root_id, "42"));
        assert_eq!(tree.get(root_id).unwrap().value.as_deref(), Some("42"));
        assert!(!tree.update_value(999, "nope"));
    }

    #[test]
    fn update_bounds() {
        let mut tree = sample_tree();
        let root_id = tree.root().unwrap();
        let b = Bounds::new(10.0, 20.0, 100.0, 200.0);
        assert!(tree.update_bounds(root_id, b));
        assert_eq!(tree.get(root_id).unwrap().bounds.unwrap().x, 10.0);
        assert!(!tree.update_bounds(999, b));
    }

    #[test]
    fn node_states() {
        let mut node = AccessibleNode::new(1, AccessibleRole::Checkbox, "Accept");
        assert!(!node.has_state(AccessibleState::Checked));
        node.add_state(AccessibleState::Checked);
        assert!(node.has_state(AccessibleState::Checked));
        // Adding duplicate is a no-op.
        node.add_state(AccessibleState::Checked);
        assert_eq!(node.states.len(), 1);
        node.remove_state(AccessibleState::Checked);
        assert!(!node.has_state(AccessibleState::Checked));
    }

    #[test]
    fn node_description_and_value() {
        let mut node = AccessibleNode::new(1, AccessibleRole::Slider, "Volume");
        node.set_description("Adjust volume level");
        node.set_value("75");
        assert_eq!(node.description, "Adjust volume level");
        assert_eq!(node.value.as_deref(), Some("75"));
    }

    #[test]
    fn bounds_contains() {
        let b = Bounds::new(10.0, 20.0, 100.0, 50.0);
        assert!(b.contains(10.0, 20.0));
        assert!(b.contains(50.0, 40.0));
        assert!(!b.contains(110.0, 20.0)); // x+width is exclusive
        assert!(!b.contains(10.0, 70.0)); // y+height is exclusive
        assert!(!b.contains(9.0, 20.0));
    }

    #[test]
    fn role_display() {
        assert_eq!(format!("{}", AccessibleRole::Button), "Button");
        assert_eq!(format!("{}", AccessibleRole::StatusBar), "StatusBar");
    }

    #[test]
    fn remove_focused_clears_focus() {
        let mut tree = sample_tree();
        let root_id = tree.root().unwrap();
        let btn_id = tree.get(root_id).unwrap().children[0];
        tree.set_focused(btn_id);
        tree.remove_node(btn_id);
        assert!(tree.focused_node().is_none());
    }

    #[test]
    fn allocate_ids_are_unique() {
        let mut tree = AccessibleTree::new();
        let a = tree.allocate_id();
        let b = tree.allocate_id();
        let c = tree.allocate_id();
        assert_ne!(a, b);
        assert_ne!(b, c);
    }

    #[test]
    fn default_tree_is_empty() {
        let tree = AccessibleTree::default();
        assert_eq!(tree.node_count(), 0);
        assert!(tree.root().is_none());
        assert!(tree.focused_node().is_none());
    }
}
