use crate::node::*;

#[test]
fn test_creation() {
    let node = AccessibleNode::new(1, Role::Button, "OK");
    assert_eq!(node.id, 1);
    assert_eq!(node.role, Role::Button);
    assert_eq!(node.name, "OK");
    assert!(node.states.is_empty());
}

#[test]
fn test_roles() {
    let w = AccessibleNode::new(1, Role::Window, "Main");
    let b = AccessibleNode::new(2, Role::Button, "Submit");
    assert_eq!(w.role, Role::Window);
    assert_eq!(b.role, Role::Button);
}

#[test]
fn test_states() {
    let mut node = AccessibleNode::new(1, Role::Checkbox, "Agree");
    node.add_state(State::Checked);
    node.add_state(State::Focused);
    assert!(node.has_state(State::Checked));
    assert!(node.has_state(State::Focused));
    assert!(!node.has_state(State::Disabled));
}

#[test]
fn test_add_remove_state() {
    let mut node = AccessibleNode::new(1, Role::Button, "Test");
    node.add_state(State::Focused);
    assert!(node.has_state(State::Focused));
    node.remove_state(State::Focused);
    assert!(!node.has_state(State::Focused));
}

#[test]
fn test_focusable_check() {
    let btn = AccessibleNode::new(1, Role::Button, "OK");
    assert!(btn.is_focusable());

    let label = AccessibleNode::new(2, Role::Label, "Info");
    assert!(!label.is_focusable());

    let mut disabled_btn = AccessibleNode::new(3, Role::Button, "Disabled");
    disabled_btn.add_state(State::Disabled);
    assert!(!disabled_btn.is_focusable());
}

#[test]
fn test_bounds() {
    let mut node = AccessibleNode::new(1, Role::Button, "OK");
    node.bounds = Some(NodeBounds::new(10, 20, 100, 50));
    let b = node.bounds.unwrap();
    assert_eq!(b.x, 10);
    assert_eq!(b.y, 20);
    assert_eq!(b.width, 100);
    assert_eq!(b.height, 50);
}

#[test]
fn test_actions() {
    let mut node = AccessibleNode::new(1, Role::Button, "OK");
    node.actions.push("click".to_string());
    node.actions.push("focus".to_string());
    assert_eq!(node.actions.len(), 2);
}

#[test]
fn test_display() {
    let node = AccessibleNode::new(42, Role::TextInput, "Search");
    let s = format!("{node}");
    assert!(s.contains("42"));
    assert!(s.contains("TextInput"));
    assert!(s.contains("Search"));
}

#[test]
fn test_serde() {
    let mut node = AccessibleNode::new(1, Role::Button, "OK");
    node.add_state(State::Focused);
    let json = serde_json::to_string(&node).unwrap();
    let d: AccessibleNode = serde_json::from_str(&json).unwrap();
    assert_eq!(d.id, 1);
    assert_eq!(d.role, Role::Button);
    assert!(d.has_state(State::Focused));
}

#[test]
fn test_children() {
    let mut node = AccessibleNode::new(1, Role::Panel, "Main");
    node.children.push(2);
    node.children.push(3);
    assert_eq!(node.children, vec![2, 3]);
}
