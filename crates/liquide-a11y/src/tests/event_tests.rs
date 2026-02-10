use crate::event::*;
use crate::node::State;

#[test]
fn test_create_events() {
    let e1 = AccessibilityEvent::NodeAdded { id: 1, parent: 0 };
    let e2 = AccessibilityEvent::StateChanged { id: 1, state: State::Focused, value: true };
    // Just ensure they can be created
    assert!(matches!(e1, AccessibilityEvent::NodeAdded { .. }));
    assert!(matches!(e2, AccessibilityEvent::StateChanged { .. }));
}

#[test]
fn test_queue_push_drain() {
    let mut q = EventQueue::new(100);
    q.push(AccessibilityEvent::TreeUpdated);
    q.push(AccessibilityEvent::NodeRemoved { id: 1 });
    assert_eq!(q.len(), 2);
    let events = q.drain();
    assert_eq!(events.len(), 2);
    assert!(q.is_empty());
}

#[test]
fn test_max_size_overflow() {
    let mut q = EventQueue::new(3);
    q.push(AccessibilityEvent::TreeUpdated);
    q.push(AccessibilityEvent::TreeUpdated);
    q.push(AccessibilityEvent::TreeUpdated);
    q.push(AccessibilityEvent::NodeRemoved { id: 99 });
    assert_eq!(q.len(), 3);
    let events = q.drain();
    // The last event should be NodeRemoved
    assert!(matches!(events[2], AccessibilityEvent::NodeRemoved { id: 99 }));
}

#[test]
fn test_clear() {
    let mut q = EventQueue::new(100);
    q.push(AccessibilityEvent::TreeUpdated);
    q.push(AccessibilityEvent::TreeUpdated);
    q.clear();
    assert!(q.is_empty());
    assert_eq!(q.len(), 0);
}

#[test]
fn test_is_empty() {
    let q = EventQueue::new(10);
    assert!(q.is_empty());
}

#[test]
fn test_serde() {
    let event = AccessibilityEvent::FocusChanged { old: Some(1), new_focus: Some(2) };
    let json = serde_json::to_string(&event).unwrap();
    let d: AccessibilityEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(d, AccessibilityEvent::FocusChanged { old: Some(1), new_focus: Some(2) }));
}
