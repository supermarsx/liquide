use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Unique identifier for an accessible node.
pub type NodeId = u64;

/// Role of an accessible node in the UI hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Window,
    Dialog,
    Panel,
    Button,
    Label,
    TextInput,
    Checkbox,
    RadioButton,
    Slider,
    List,
    ListItem,
    Tree,
    TreeItem,
    Tab,
    TabPanel,
    Menu,
    MenuItem,
    MenuBar,
    Toolbar,
    StatusBar,
    ProgressBar,
    Separator,
    Image,
    Link,
    Table,
    TableRow,
    TableCell,
    ScrollBar,
    Tooltip,
    Alert,
    Application,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// State flags for an accessible node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum State {
    Focused,
    Selected,
    Checked,
    Expanded,
    Disabled,
    Invisible,
    ReadOnly,
    Required,
    Invalid,
}

/// Bounding rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl NodeBounds {
    #[must_use]
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }
}

/// An accessible node in the UI tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibleNode {
    pub id: NodeId,
    pub role: Role,
    pub name: String,
    pub description: String,
    pub states: HashSet<State>,
    pub value: Option<String>,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub bounds: Option<NodeBounds>,
    pub actions: Vec<String>,
}

impl AccessibleNode {
    #[must_use]
    pub fn new(id: NodeId, role: Role, name: &str) -> Self {
        Self {
            id,
            role,
            name: name.to_string(),
            description: String::new(),
            states: HashSet::new(),
            value: None,
            parent: None,
            children: Vec::new(),
            bounds: None,
            actions: Vec::new(),
        }
    }

    /// Add a state to this node.
    pub fn add_state(&mut self, state: State) {
        self.states.insert(state);
    }

    /// Remove a state from this node.
    pub fn remove_state(&mut self, state: State) {
        self.states.remove(&state);
    }

    /// Check if this node has a given state.
    #[must_use]
    pub fn has_state(&self, state: State) -> bool {
        self.states.contains(&state)
    }

    /// Check if this node can receive focus.
    #[must_use]
    pub fn is_focusable(&self) -> bool {
        if self.has_state(State::Disabled) || self.has_state(State::Invisible) {
            return false;
        }
        matches!(
            self.role,
            Role::Button
                | Role::TextInput
                | Role::Checkbox
                | Role::RadioButton
                | Role::Slider
                | Role::Link
                | Role::Tab
                | Role::MenuItem
                | Role::ListItem
                | Role::TreeItem
        )
    }

    /// Check if this node is visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        !self.has_state(State::Invisible)
    }
}

impl fmt::Display for AccessibleNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccessibleNode(id={}, role={}, name={})", self.id, self.role, self.name)
    }
}
