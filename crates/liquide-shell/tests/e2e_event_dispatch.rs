//! End-to-end tests for the DOM event dispatch pipeline.
//!
//! Validates that:
//! - EventDispatcher correctly manages hover chain and focus
//! - Mouse events generate correct DOM events (enter/leave/click/etc.)
//! - Event handlers fire on target and bubble through ancestors
//! - Propagation control (StopPropagation, StopImmediate) works
//! - Shell exposes event handler registration API

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use liquide_dom::{Document, NodeId, PseudoStateFlags};
use liquide_hit_test::dispatch::EventDispatcher;
use liquide_hit_test::engine::HitTestEngine;
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton, Propagation};
use liquide_layout::geometry::Point;
use liquide_layout::tree::{BoxType, LayoutBox, LayoutTree};
use liquide_layout::Rect;
use liquide_style_engine::StyleMap;

// ---------------------------------------------------------------------------
// Helpers — build a minimal DOM + LayoutTree for hit-testing
// ---------------------------------------------------------------------------

/// Build a Document with:
///  root (0,0 → 800×600)
///    ├── child_a (10,10 → 200×50)  — "button"
///    └── child_b (10,100 → 200×50) — "panel"
fn build_test_dom() -> (Document, NodeId, NodeId, NodeId) {
    let mut doc = Document::new();
    let root = doc.root();
    let a = doc.create_element("button");
    let b = doc.create_element("panel");
    doc.append_child(root, a);
    doc.append_child(root, b);
    (doc, root, a, b)
}

fn build_test_layout(root: NodeId, a: NodeId, b: NodeId) -> LayoutTree {
    let mut tree = LayoutTree::new();

    // Root box
    let root_id = tree.alloc(root, BoxType::Block);
    let mut root_box = tree.get_mut(root_id).unwrap();
    root_box.border_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    root_box.content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    root_box.padding_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    root_box.margin_rect = Rect::new(0.0, 0.0, 800.0, 600.0);

    // Child A — "button"
    let a_id = tree.alloc(a, BoxType::Block);
    {
        let a_box = tree.get_mut(a_id).unwrap();
        a_box.border_rect = Rect::new(10.0, 10.0, 200.0, 50.0);
        a_box.content_rect = Rect::new(10.0, 10.0, 200.0, 50.0);
        a_box.padding_rect = Rect::new(10.0, 10.0, 200.0, 50.0);
        a_box.margin_rect = Rect::new(10.0, 10.0, 200.0, 50.0);
    }

    // Child B — "panel"
    let b_id = tree.alloc(b, BoxType::Block);
    {
        let b_box = tree.get_mut(b_id).unwrap();
        b_box.border_rect = Rect::new(10.0, 100.0, 200.0, 50.0);
        b_box.content_rect = Rect::new(10.0, 100.0, 200.0, 50.0);
        b_box.padding_rect = Rect::new(10.0, 100.0, 200.0, 50.0);
        b_box.margin_rect = Rect::new(10.0, 100.0, 200.0, 50.0);
    }

    tree.add_child(root_id, a_id);
    tree.add_child(root_id, b_id);
    tree.root = root_id;

    tree
}

fn build_test_engine(root: NodeId, a: NodeId, b: NodeId) -> HitTestEngine {
    let layout = build_test_layout(root, a, b);
    let styles = StyleMap::new(); // empty styles → default pointer-events
    HitTestEngine::from_owned(layout, styles)
}

// ---------------------------------------------------------------------------
// Hover chain tests
// ---------------------------------------------------------------------------

#[test]
fn test_hover_chain_enters_element() {
    let (mut doc, root, a, b) = build_test_dom();
    let engine = build_test_engine(root, a, b);
    let mut dispatcher = EventDispatcher::new();

    // Move mouse over child_a
    let events = dispatcher.dispatch_mouse_move(Point::new(50.0, 30.0), &mut doc, &engine);

    // Should have MouseEnter events
    let enter_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.kind, DomEventKind::MouseEnter))
        .collect();

    assert!(
        !enter_events.is_empty(),
        "Moving over element 'a' should generate MouseEnter events"
    );

    // Hover chain should include child_a
    let chain = dispatcher.hover_chain();
    assert!(
        chain.contains(&a),
        "Hover chain should contain child_a after moving over it"
    );
}

#[test]
fn test_hover_chain_leaves_element() {
    let (mut doc, root, a, b) = build_test_dom();
    let engine = build_test_engine(root, a, b);
    let mut dispatcher = EventDispatcher::new();

    // Enter child_a
    dispatcher.dispatch_mouse_move(Point::new(50.0, 30.0), &mut doc, &engine);

    // Move to child_b
    let events = dispatcher.dispatch_mouse_move(Point::new(50.0, 120.0), &mut doc, &engine);

    let leave_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.kind, DomEventKind::MouseLeave))
        .collect();

    assert!(
        !leave_events.is_empty(),
        "Moving from element 'a' to 'b' should generate MouseLeave events"
    );

    // Hover chain should now contain child_b, not child_a
    let chain = dispatcher.hover_chain();
    assert!(chain.contains(&b), "Hover chain should contain child_b");
    assert!(!chain.contains(&a), "Hover chain should not contain child_a");
}

#[test]
fn test_hover_sets_pseudo_state() {
    let (mut doc, root, a, b) = build_test_dom();
    let engine = build_test_engine(root, a, b);
    let mut dispatcher = EventDispatcher::new();

    // Move over child_a
    dispatcher.dispatch_mouse_move(Point::new(50.0, 30.0), &mut doc, &engine);

    // child_a should have :hover pseudo-state
    assert!(
        doc.get(a).unwrap().has_pseudo_state(PseudoStateFlags::HOVER),
        "Element 'a' should have :hover pseudo-state"
    );

    // Move away
    dispatcher.dispatch_mouse_move(Point::new(400.0, 400.0), &mut doc, &engine);

    assert!(
        !doc.get(a).unwrap().has_pseudo_state(PseudoStateFlags::HOVER),
        "Element 'a' should lose :hover after mouse leaves"
    );
}

// ---------------------------------------------------------------------------
// Click & double-click
// ---------------------------------------------------------------------------

#[test]
fn test_mouse_down_up_generates_click() {
    let (mut doc, root, a, b) = build_test_dom();
    let engine = build_test_engine(root, a, b);
    let mut dispatcher = EventDispatcher::new();

    // Move over child_a first
    dispatcher.dispatch_mouse_move(Point::new(50.0, 30.0), &mut doc, &engine);

    // Press
    let down_events =
        dispatcher.dispatch_mouse_down(Point::new(50.0, 30.0), MouseButton::Left, &mut doc, &engine);

    let has_mouse_down = down_events
        .iter()
        .any(|e| matches!(e.kind, DomEventKind::MouseDown { .. }));
    assert!(has_mouse_down, "Should generate MouseDown event");

    // Release
    let up_events =
        dispatcher.dispatch_mouse_up(Point::new(50.0, 30.0), MouseButton::Left, &mut doc, &engine);

    let has_click = up_events
        .iter()
        .any(|e| matches!(e.kind, DomEventKind::Click { .. }));
    assert!(has_click, "Release after press should generate Click event");
}

#[test]
fn test_right_click_generates_context_menu() {
    let (mut doc, root, a, b) = build_test_dom();
    let engine = build_test_engine(root, a, b);
    let mut dispatcher = EventDispatcher::new();

    dispatcher.dispatch_mouse_move(Point::new(50.0, 30.0), &mut doc, &engine);

    let up_events = dispatcher.dispatch_mouse_up(
        Point::new(50.0, 30.0),
        MouseButton::Right,
        &mut doc,
        &engine,
    );

    let has_ctx = up_events
        .iter()
        .any(|e| matches!(e.kind, DomEventKind::ContextMenu { .. }));
    assert!(
        has_ctx,
        "Right-click should generate ContextMenu event"
    );
}

// ---------------------------------------------------------------------------
// Focus management
// ---------------------------------------------------------------------------

#[test]
fn test_click_sets_focus() {
    let (mut doc, root, a, b) = build_test_dom();
    let engine = build_test_engine(root, a, b);
    let mut dispatcher = EventDispatcher::new();

    assert_eq!(dispatcher.focus(), None, "No focus initially");

    // Click on child_a
    dispatcher.dispatch_mouse_down(Point::new(50.0, 30.0), MouseButton::Left, &mut doc, &engine);

    assert_eq!(dispatcher.focus(), Some(a), "Focus should be on child_a");

    // Click on child_b
    dispatcher.dispatch_mouse_down(Point::new(50.0, 120.0), MouseButton::Left, &mut doc, &engine);

    assert_eq!(dispatcher.focus(), Some(b), "Focus should move to child_b");

    // child_a should have lost :focus
    assert!(
        !doc.get(a).unwrap().has_pseudo_state(PseudoStateFlags::FOCUS),
        "child_a should lose :focus"
    );

    // child_b should have :focus
    assert!(
        doc.get(b).unwrap().has_pseudo_state(PseudoStateFlags::FOCUS),
        "child_b should have :focus"
    );
}

#[test]
fn test_explicit_set_focus() {
    let (mut doc, root, a, b) = build_test_dom();
    let mut dispatcher = EventDispatcher::new();

    let events = dispatcher.set_focus(Some(a), &mut doc);

    let has_focus = events.iter().any(|e| matches!(e.kind, DomEventKind::Focus));
    assert!(has_focus, "set_focus should generate Focus event");
    assert_eq!(dispatcher.focus(), Some(a));

    // Switch focus
    let events2 = dispatcher.set_focus(Some(b), &mut doc);

    let has_blur = events2.iter().any(|e| matches!(e.kind, DomEventKind::Blur));
    assert!(has_blur, "Changing focus should generate Blur on old node");
    assert_eq!(dispatcher.focus(), Some(b));
}

// ---------------------------------------------------------------------------
// Event handler registration & firing
// ---------------------------------------------------------------------------

#[test]
fn test_handler_fires_on_click() {
    let (mut doc, root, a, b) = build_test_dom();
    let engine = build_test_engine(root, a, b);
    let mut dispatcher = EventDispatcher::new();

    let click_count = Arc::new(AtomicU32::new(0));
    let counter = click_count.clone();

    dispatcher.add_handler(
        a,
        Some(DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        }),
        Box::new(move |_event| {
            counter.fetch_add(1, Ordering::SeqCst);
            Propagation::Continue
        }),
    );

    // Click on child_a
    dispatcher.dispatch_mouse_move(Point::new(50.0, 30.0), &mut doc, &engine);
    dispatcher.dispatch_mouse_down(Point::new(50.0, 30.0), MouseButton::Left, &mut doc, &engine);
    dispatcher.dispatch_mouse_up(Point::new(50.0, 30.0), MouseButton::Left, &mut doc, &engine);

    assert_eq!(
        click_count.load(Ordering::SeqCst),
        1,
        "Click handler should fire exactly once"
    );
}

#[test]
fn test_handler_does_not_fire_on_wrong_node() {
    let (mut doc, root, a, b) = build_test_dom();
    let engine = build_test_engine(root, a, b);
    let mut dispatcher = EventDispatcher::new();

    let click_count = Arc::new(AtomicU32::new(0));
    let counter = click_count.clone();

    // Register handler on child_a
    dispatcher.add_handler(
        a,
        Some(DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        }),
        Box::new(move |_event| {
            counter.fetch_add(1, Ordering::SeqCst);
            Propagation::Continue
        }),
    );

    // Click on child_b instead
    dispatcher.dispatch_mouse_move(Point::new(50.0, 120.0), &mut doc, &engine);
    dispatcher.dispatch_mouse_down(Point::new(50.0, 120.0), MouseButton::Left, &mut doc, &engine);
    dispatcher.dispatch_mouse_up(Point::new(50.0, 120.0), MouseButton::Left, &mut doc, &engine);

    assert_eq!(
        click_count.load(Ordering::SeqCst),
        0,
        "Handler on child_a should not fire for clicks on child_b"
    );
}

#[test]
fn test_stop_propagation_prevents_bubble() {
    let (mut doc, root, a, b) = build_test_dom();
    let engine = build_test_engine(root, a, b);
    let mut dispatcher = EventDispatcher::new();

    let child_count = Arc::new(AtomicU32::new(0));
    let root_count = Arc::new(AtomicU32::new(0));

    let child_counter = child_count.clone();
    let root_counter = root_count.clone();

    // Handler on child_a that stops propagation
    dispatcher.add_handler(
        a,
        None, // any event
        Box::new(move |_event| {
            child_counter.fetch_add(1, Ordering::SeqCst);
            Propagation::StopPropagation
        }),
    );

    // Handler on root
    dispatcher.add_handler(
        root,
        None,
        Box::new(move |_event| {
            root_counter.fetch_add(1, Ordering::SeqCst);
            Propagation::Continue
        }),
    );

    // Click on child_a
    dispatcher.dispatch_mouse_down(Point::new(50.0, 30.0), MouseButton::Left, &mut doc, &engine);

    assert!(
        child_count.load(Ordering::SeqCst) > 0,
        "Child handler should fire"
    );
    assert_eq!(
        root_count.load(Ordering::SeqCst),
        0,
        "Root handler should NOT fire (propagation stopped)"
    );
}

// ---------------------------------------------------------------------------
// Scroll dispatch
// ---------------------------------------------------------------------------

#[test]
fn test_scroll_event_dispatches() {
    let (mut doc, root, a, b) = build_test_dom();
    let engine = build_test_engine(root, a, b);
    let mut dispatcher = EventDispatcher::new();

    let scroll_count = Arc::new(AtomicU32::new(0));
    let counter = scroll_count.clone();

    dispatcher.add_handler(
        a,
        Some(DomEventKind::Scroll {
            dx: 0.0,
            dy: 0.0,
        }),
        Box::new(move |_event| {
            counter.fetch_add(1, Ordering::SeqCst);
            Propagation::Continue
        }),
    );

    // Scroll over child_a
    dispatcher.dispatch_scroll(Point::new(50.0, 30.0), 0.0, -10.0, &engine);

    assert_eq!(
        scroll_count.load(Ordering::SeqCst),
        1,
        "Scroll handler should fire"
    );
}

// ---------------------------------------------------------------------------
// Keyboard dispatch
// ---------------------------------------------------------------------------

#[test]
fn test_key_events_dispatch_to_focused() {
    let (mut doc, root, a, b) = build_test_dom();
    let mut dispatcher = EventDispatcher::new();

    let key_count = Arc::new(AtomicU32::new(0));
    let counter = key_count.clone();

    dispatcher.add_handler(
        a,
        Some(DomEventKind::KeyDown {
            key: 0,
            modifiers: 0,
        }),
        Box::new(move |_event| {
            counter.fetch_add(1, Ordering::SeqCst);
            Propagation::Continue
        }),
    );

    // No focus → key events should not reach any handler
    dispatcher.dispatch_key_down(65, 0); // 'A'
    assert_eq!(key_count.load(Ordering::SeqCst), 0);

    // Focus child_a
    dispatcher.set_focus(Some(a), &mut doc);

    // Now key event should fire handler
    dispatcher.dispatch_key_down(65, 0);
    assert_eq!(
        key_count.load(Ordering::SeqCst),
        1,
        "Key handler should fire when focused"
    );
}

// ---------------------------------------------------------------------------
// Active pseudo-state
// ---------------------------------------------------------------------------

#[test]
fn test_active_pseudo_state_on_press_release() {
    let (mut doc, root, a, b) = build_test_dom();
    let engine = build_test_engine(root, a, b);
    let mut dispatcher = EventDispatcher::new();

    // Move over child_a
    dispatcher.dispatch_mouse_move(Point::new(50.0, 30.0), &mut doc, &engine);

    // Press — should set :active
    dispatcher.dispatch_mouse_down(Point::new(50.0, 30.0), MouseButton::Left, &mut doc, &engine);

    let has_active = doc.get(a).unwrap().has_pseudo_state(PseudoStateFlags::ACTIVE);
    assert!(
        has_active,
        "Element should have :active on mouse down"
    );

    // Release — should clear :active
    dispatcher.dispatch_mouse_up(Point::new(50.0, 30.0), MouseButton::Left, &mut doc, &engine);

    let still_active = doc.get(a).unwrap().has_pseudo_state(PseudoStateFlags::ACTIVE);
    assert!(
        !still_active,
        "Element should lose :active on mouse up"
    );
}
