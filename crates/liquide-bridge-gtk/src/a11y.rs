//! GTK accessibility bridge (AT-SPI2 / ATK).
//!
//! Maps Liquide's accessibility tree to the Linux AT-SPI2 accessibility
//! protocol, primarily through ATK (Accessibility Toolkit).
//!
//! Every Liquide widget that participates in accessibility exposes an
//! accessible object with a role, name, description, and state.

use serde::{Deserialize, Serialize};

/// ATK role mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtkRole {
    Window,
    Dialog,
    Button,
    CheckBox,
    RadioButton,
    Label,
    TextInput,
    PasswordText,
    ComboBox,
    List,
    ListItem,
    Tree,
    TreeItem,
    Table,
    TableCell,
    TableRow,
    TableColumnHeader,
    Menu,
    MenuBar,
    MenuItem,
    Separator,
    ToolBar,
    ToolBarButton,
    TabPanel,
    Tab,
    ScrollBar,
    ScrollPane,
    Slider,
    SpinButton,
    ProgressBar,
    StatusBar,
    Tooltip,
    Image,
    Link,
    Panel,
    Unknown,
}

/// Accessible state flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtkState {
    Enabled,
    Focused,
    Selected,
    Checked,
    Expanded,
    Collapsed,
    Pressed,
    ReadOnly,
    Editable,
    Visible,
    Showing,
    Sensitive,
    Multiselectable,
}

/// An accessible node in the AT-SPI tree.
#[derive(Debug, Clone)]
pub struct AtkNode {
    /// Unique identifier.
    pub id: u64,
    /// ATK role.
    pub role: AtkRole,
    /// Accessible name (e.g., button label).
    pub name: String,
    /// Accessible description.
    pub description: String,
    /// Active states.
    pub states: Vec<AtkState>,
    /// Child node IDs.
    pub children: Vec<u64>,
    /// Parent node ID (0 for root).
    pub parent: u64,
    /// Value (for sliders, progress bars, etc.).
    pub value: Option<f64>,
    /// Minimum value.
    pub value_min: Option<f64>,
    /// Maximum value.
    pub value_max: Option<f64>,
}

impl AtkNode {
    #[must_use]
    pub fn new(id: u64, role: AtkRole, name: impl Into<String>) -> Self {
        Self {
            id,
            role,
            name: name.into(),
            description: String::new(),
            states: vec![AtkState::Enabled, AtkState::Visible, AtkState::Showing],
            children: Vec::new(),
            parent: 0,
            value: None,
            value_min: None,
            value_max: None,
        }
    }

    pub fn has_state(&self, state: AtkState) -> bool {
        self.states.contains(&state)
    }

    pub fn add_state(&mut self, state: AtkState) {
        if !self.has_state(state) {
            self.states.push(state);
        }
    }

    pub fn remove_state(&mut self, state: AtkState) {
        self.states.retain(|s| *s != state);
    }
}

/// The AT-SPI bridge that exposes Liquide's widget tree to assistive technologies.
pub struct AtkBridge {
    /// All accessible nodes indexed by ID.
    nodes: std::collections::HashMap<u64, AtkNode>,
    /// Root node ID.
    root_id: u64,
    /// Next node ID to allocate.
    next_id: u64,
    /// Whether the bridge is connected to AT-SPI bus.
    connected: bool,
}

impl AtkBridge {
    #[must_use]
    pub fn new() -> Self {
        let root = AtkNode::new(1, AtkRole::Window, "Application");
        let mut nodes = std::collections::HashMap::new();
        nodes.insert(1, root);
        Self {
            nodes,
            root_id: 1,
            next_id: 2,
            connected: false,
        }
    }

    /// Connect to the AT-SPI bus.
    pub fn connect(&mut self) -> Result<(), String> {
        // In real implementation: dbus connection to AT-SPI registry
        self.connected = true;
        tracing::info!("Connected to AT-SPI bus");
        Ok(())
    }

    /// Register a new accessible node.
    pub fn register_node(&mut self, role: AtkRole, name: impl Into<String>, parent: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let mut node = AtkNode::new(id, role, name);
        node.parent = parent;

        if let Some(parent_node) = self.nodes.get_mut(&parent) {
            parent_node.children.push(id);
        }

        self.nodes.insert(id, node);
        id
    }

    /// Remove a node and its children.
    pub fn unregister_node(&mut self, id: u64) {
        if let Some(node) = self.nodes.remove(&id) {
            // Remove from parent's children
            if let Some(parent) = self.nodes.get_mut(&node.parent) {
                parent.children.retain(|c| *c != id);
            }
            // Recursively remove children
            for child_id in node.children {
                self.unregister_node(child_id);
            }
        }
    }

    /// Get a node by ID.
    #[must_use]
    pub fn node(&self, id: u64) -> Option<&AtkNode> {
        self.nodes.get(&id)
    }

    /// Get a mutable node by ID.
    pub fn node_mut(&mut self, id: u64) -> Option<&mut AtkNode> {
        self.nodes.get_mut(&id)
    }

    /// Root node ID.
    #[must_use]
    pub fn root_id(&self) -> u64 {
        self.root_id
    }

    /// Total node count.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Notify assistive technologies of a state change.
    pub fn notify_state_change(&self, _node_id: u64, _state: AtkState, _active: bool) {
        // In real implementation: emit AT-SPI signal
    }

    /// Notify of a text change (for text editors).
    pub fn notify_text_change(&self, _node_id: u64, _offset: usize, _text: &str) {
        // In real implementation: emit AT-SPI text-changed signal
    }
}

impl Default for AtkBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atk_bridge() {
        let mut bridge = AtkBridge::new();
        assert_eq!(bridge.node_count(), 1); // root

        let btn = bridge.register_node(AtkRole::Button, "OK", bridge.root_id());
        let lbl = bridge.register_node(AtkRole::Label, "Status", bridge.root_id());

        assert_eq!(bridge.node_count(), 3);
        assert_eq!(bridge.node(btn).unwrap().name, "OK");
        assert_eq!(bridge.node(lbl).unwrap().role, AtkRole::Label);

        let root = bridge.node(bridge.root_id()).unwrap();
        assert_eq!(root.children.len(), 2);
    }

    #[test]
    fn test_unregister() {
        let mut bridge = AtkBridge::new();
        let btn = bridge.register_node(AtkRole::Button, "Test", bridge.root_id());
        assert_eq!(bridge.node_count(), 2);
        bridge.unregister_node(btn);
        assert_eq!(bridge.node_count(), 1);
    }

    #[test]
    fn test_node_state() {
        let mut node = AtkNode::new(1, AtkRole::CheckBox, "Check");
        assert!(node.has_state(AtkState::Enabled));
        assert!(!node.has_state(AtkState::Checked));
        node.add_state(AtkState::Checked);
        assert!(node.has_state(AtkState::Checked));
        node.remove_state(AtkState::Checked);
        assert!(!node.has_state(AtkState::Checked));
    }
}
