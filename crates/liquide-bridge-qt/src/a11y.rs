//! Qt accessibility bridge (QAccessible / UIA on Windows).
//!
//! Maps Liquide's accessibility tree to Qt's `QAccessibleInterface`.
//! On Windows, Qt delegates to UIA (UI Automation); on Linux, to AT-SPI.

use serde::{Deserialize, Serialize};

/// Qt accessible role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QtA11yRole {
    Window,
    Dialog,
    Button,
    CheckBox,
    RadioButton,
    StaticText,
    EditableText,
    ComboBox,
    List,
    ListItem,
    Tree,
    TreeItem,
    Table,
    Cell,
    Row,
    ColumnHeader,
    MenuBar,
    Menu,
    MenuItem,
    Separator,
    ToolBar,
    TabList,
    Tab,
    TabPanel,
    ScrollBar,
    Slider,
    SpinBox,
    ProgressBar,
    StatusBar,
    ToolTip,
    Graphic,
    Link,
    Pane,
    Unknown,
}

/// Accessible state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QtA11yState {
    Disabled,
    Focused,
    Selected,
    Checked,
    Expanded,
    Collapsed,
    ReadOnly,
    Editable,
    Invisible,
    Offscreen,
    Pressed,
}

/// An accessible node.
#[derive(Debug, Clone)]
pub struct QtA11yNode {
    pub id: u64,
    pub role: QtA11yRole,
    pub name: String,
    pub description: String,
    pub states: Vec<QtA11yState>,
    pub children: Vec<u64>,
    pub parent: u64,
}

impl QtA11yNode {
    #[must_use]
    pub fn new(id: u64, role: QtA11yRole, name: impl Into<String>) -> Self {
        Self {
            id,
            role,
            name: name.into(),
            description: String::new(),
            states: Vec::new(),
            children: Vec::new(),
            parent: 0,
        }
    }
}

/// The Qt accessibility bridge.
pub struct QtA11yBridge {
    nodes: std::collections::HashMap<u64, QtA11yNode>,
    root_id: u64,
    next_id: u64,
}

impl QtA11yBridge {
    #[must_use]
    pub fn new() -> Self {
        let root = QtA11yNode::new(1, QtA11yRole::Window, "Application");
        let mut nodes = std::collections::HashMap::new();
        nodes.insert(1, root);
        Self {
            nodes,
            root_id: 1,
            next_id: 2,
        }
    }

    pub fn register(&mut self, role: QtA11yRole, name: impl Into<String>, parent: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut node = QtA11yNode::new(id, role, name);
        node.parent = parent;
        if let Some(p) = self.nodes.get_mut(&parent) {
            p.children.push(id);
        }
        self.nodes.insert(id, node);
        id
    }

    pub fn unregister(&mut self, id: u64) {
        if let Some(node) = self.nodes.remove(&id) {
            if let Some(p) = self.nodes.get_mut(&node.parent) {
                p.children.retain(|c| *c != id);
            }
            for child in node.children {
                self.unregister(child);
            }
        }
    }

    #[must_use]
    pub fn node(&self, id: u64) -> Option<&QtA11yNode> {
        self.nodes.get(&id)
    }

    #[must_use]
    pub fn root_id(&self) -> u64 {
        self.root_id
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for QtA11yBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qt_a11y() {
        let mut bridge = QtA11yBridge::new();
        let btn = bridge.register(QtA11yRole::Button, "Save", bridge.root_id());
        assert_eq!(bridge.node_count(), 2);
        assert_eq!(bridge.node(btn).unwrap().name, "Save");
        bridge.unregister(btn);
        assert_eq!(bridge.node_count(), 1);
    }
}
