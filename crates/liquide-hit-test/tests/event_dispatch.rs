//! Extensive event dispatch tests.
//!
//! Covers: bubbling semantics, non-bubbling events, prevent_default /
//! stop_propagation independence, handler registration, hover chain
//! management, and click event generation.

use std::sync::{Arc, Mutex};

use liquide_dom::{Document, NodeId};
use liquide_hit_test::dispatch::EventDispatcher;
use liquide_hit_test::engine::HitTestEngine;
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton, Propagation};
use liquide_layout::geometry::{Point, Rect};
use liquide_layout::tree::{BoxType, LayoutTree};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::ComputedStyle;

// ── Helpers ──────────────────────────────────────────────────────────────

/// Build a document + layout + styles with a single clickable child.
fn simple_setup() -> (Document, HitTestEngine, NodeId, NodeId) {
    let mut doc = Document::new();
    let root = doc.root();
    let child = doc.create_element("div");
    doc.append_child(root, child);

    let mut tree = LayoutTree::new();
    let root_box = tree.alloc(root, BoxType::Block);
    {
        let r = tree.get_mut(root_box).unwrap();
        r.content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.padding_rect = r.content_rect;
        r.border_rect = r.content_rect;
        r.margin_rect = r.content_rect;
    }
    let child_box = tree.alloc(child, BoxType::Block);
    {
        let c = tree.get_mut(child_box).unwrap();
        c.content_rect = Rect::new(100.0, 100.0, 200.0, 200.0);
        c.padding_rect = c.content_rect;
        c.border_rect = c.content_rect;
        c.margin_rect = c.content_rect;
    }
    tree.add_child(root_box, child_box);
    tree.root = root_box;

    let mut styles = StyleMap::new();
    styles.insert(root, ComputedStyle::default());
    styles.insert(child, ComputedStyle::default());

    let engine = HitTestEngine::from_owned(tree, styles);
    (doc, engine, root, child)
}

// ── DomEvent construction ────────────────────────────────────────────────

#[test]
fn event_new_sets_bubbles_correctly() {
    let node = NodeId::from(42u64);

    // MouseDown bubbles
    let e = DomEvent::new(
        node,
        DomEventKind::MouseDown {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        },
    );
    assert!(e.bubbles, "MouseDown should bubble");

    // MouseEnter does NOT bubble
    let e = DomEvent::new(node, DomEventKind::MouseEnter);
    assert!(!e.bubbles, "MouseEnter should NOT bubble");

    // Focus does NOT bubble
    let e = DomEvent::new(node, DomEventKind::Focus);
    assert!(!e.bubbles, "Focus should NOT bubble");

    // Click bubbles
    let e = DomEvent::new(
        node,
        DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        },
    );
    assert!(e.bubbles, "Click should bubble");
}

#[test]
fn event_new_sets_cancelable_correctly() {
    let node = NodeId::from(42u64);

    // Click is cancelable
    let e = DomEvent::new(
        node,
        DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        },
    );
    assert!(e.cancelable, "Click should be cancelable");

    // Scroll is NOT cancelable
    let e = DomEvent::new(node, DomEventKind::Scroll { dx: 0.0, dy: 10.0 });
    assert!(!e.cancelable, "Scroll should NOT be cancelable");

    // Focus is NOT cancelable
    let e = DomEvent::new(node, DomEventKind::Focus);
    assert!(!e.cancelable, "Focus should NOT be cancelable");
}

#[test]
fn event_target_equals_current_target_initially() {
    let node = NodeId::from(42u64);
    let e = DomEvent::new(node, DomEventKind::Focus);
    assert_eq!(e.target, e.current_target);
}

// ── stop_propagation / prevent_default independence ──────────────────────

#[test]
fn prevent_default_does_not_stop_propagation() {
    let node = NodeId::from(1u64);
    let mut event = DomEvent::new(
        node,
        DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        },
    );

    event.prevent_default();
    assert!(event.default_prevented, "default should be prevented");
    assert_eq!(
        event.propagation,
        Propagation::Continue,
        "prevent_default must NOT change propagation"
    );
}

#[test]
fn stop_propagation_does_not_prevent_default() {
    let node = NodeId::from(1u64);
    let mut event = DomEvent::new(
        node,
        DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        },
    );

    event.stop_propagation();
    assert_eq!(event.propagation, Propagation::StopPropagation);
    assert!(
        !event.default_prevented,
        "stop_propagation must NOT prevent default"
    );
}

#[test]
fn prevent_default_and_stop_propagation_are_independent() {
    let node = NodeId::from(1u64);
    let mut event = DomEvent::new(
        node,
        DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        },
    );

    event.prevent_default();
    event.stop_propagation();

    assert!(event.default_prevented, "default should still be prevented");
    assert_eq!(
        event.propagation,
        Propagation::StopPropagation,
        "propagation should be stopped"
    );
}

#[test]
fn prevent_default_on_non_cancelable_is_noop() {
    let node = NodeId::from(1u64);
    let mut event = DomEvent::new(node, DomEventKind::Scroll { dx: 0.0, dy: 10.0 });

    event.prevent_default();
    assert!(
        !event.default_prevented,
        "non-cancelable event should ignore prevent_default"
    );
}

#[test]
fn stop_immediate_propagation_is_strongest() {
    let node = NodeId::from(1u64);
    let mut event = DomEvent::new(
        node,
        DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        },
    );

    event.stop_immediate_propagation();
    assert_eq!(event.propagation, Propagation::StopImmediate);
}

// ── Bubbling semantics ───────────────────────────────────────────────────

#[test]
fn non_bubbling_events_list() {
    // Verify all expected non-bubbling events
    assert!(!DomEventKind::MouseEnter.bubbles());
    assert!(!DomEventKind::MouseLeave.bubbles());
    assert!(!DomEventKind::Focus.bubbles());
    assert!(!DomEventKind::Blur.bubbles());
}

#[test]
fn bubbling_events_list() {
    assert!(DomEventKind::MouseMove { x: 0.0, y: 0.0 }.bubbles());
    assert!(
        DomEventKind::MouseDown {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0
        }
        .bubbles()
    );
    assert!(
        DomEventKind::MouseUp {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0
        }
        .bubbles()
    );
    assert!(
        DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0
        }
        .bubbles()
    );
    assert!(DomEventKind::Scroll { dx: 0.0, dy: 1.0 }.bubbles());
    assert!(
        DomEventKind::KeyDown {
            key: 0,
            modifiers: 0
        }
        .bubbles()
    );
}

// ── Dispatcher: hover chain management ───────────────────────────────────

#[test]
fn mouse_move_generates_enter_leave_events() {
    let (mut doc, engine, _root, _child) = simple_setup();
    let mut dispatcher = EventDispatcher::new();

    // Move to child
    let events = dispatcher.dispatch_mouse_move(Point::new(150.0, 150.0), &mut doc, &engine);

    // Should have MouseEnter events (at least for child)
    let enter_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.kind, DomEventKind::MouseEnter))
        .collect();
    assert!(
        !enter_events.is_empty(),
        "should generate MouseEnter on hover"
    );

    // Move away from child to root-only area
    let events = dispatcher.dispatch_mouse_move(Point::new(50.0, 50.0), &mut doc, &engine);

    // Should have MouseLeave for child
    let leave_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.kind, DomEventKind::MouseLeave))
        .collect();
    assert!(
        !leave_events.is_empty(),
        "should generate MouseLeave when leaving node"
    );
}

#[test]
fn mouse_move_updates_hover_chain() {
    let (mut doc, engine, _root, child) = simple_setup();
    let mut dispatcher = EventDispatcher::new();

    // Initially empty
    assert!(dispatcher.hover_chain().is_empty());

    // Move to child
    dispatcher.dispatch_mouse_move(Point::new(150.0, 150.0), &mut doc, &engine);
    assert!(
        !dispatcher.hover_chain().is_empty(),
        "hover chain should be populated"
    );
    assert!(
        dispatcher.hover_chain().contains(&child),
        "hover chain should contain the child"
    );

    // Move outside everything
    dispatcher.dispatch_mouse_move(Point::new(900.0, 900.0), &mut doc, &engine);
    assert!(
        dispatcher.hover_chain().is_empty(),
        "hover chain should be empty after leaving all elements"
    );
}

// ── Dispatcher: click generation ─────────────────────────────────────────

#[test]
fn mouse_down_up_generates_click() {
    let (mut doc, engine, _root, _child) = simple_setup();
    let mut dispatcher = EventDispatcher::new();

    // Move to the child first to establish hover chain
    dispatcher.dispatch_mouse_move(Point::new(150.0, 150.0), &mut doc, &engine);

    // Mouse down
    let events = dispatcher.dispatch_mouse_down(
        Point::new(150.0, 150.0),
        MouseButton::Left,
        &mut doc,
        &engine,
    );
    let has_mousedown = events
        .iter()
        .any(|e| matches!(e.kind, DomEventKind::MouseDown { .. }));
    assert!(has_mousedown, "should generate MouseDown event");

    // Mouse up → should generate Click
    let events = dispatcher.dispatch_mouse_up(
        Point::new(150.0, 150.0),
        MouseButton::Left,
        &mut doc,
        &engine,
    );
    let has_mouseup = events
        .iter()
        .any(|e| matches!(e.kind, DomEventKind::MouseUp { .. }));
    assert!(has_mouseup, "should generate MouseUp event");

    let has_click = events
        .iter()
        .any(|e| matches!(e.kind, DomEventKind::Click { .. }));
    assert!(has_click, "should generate Click event after mouse down+up");
}

#[test]
fn click_event_targets_correct_node() {
    let (mut doc, engine, _root, child) = simple_setup();
    let mut dispatcher = EventDispatcher::new();

    dispatcher.dispatch_mouse_move(Point::new(150.0, 150.0), &mut doc, &engine);
    dispatcher.dispatch_mouse_down(
        Point::new(150.0, 150.0),
        MouseButton::Left,
        &mut doc,
        &engine,
    );
    let events = dispatcher.dispatch_mouse_up(
        Point::new(150.0, 150.0),
        MouseButton::Left,
        &mut doc,
        &engine,
    );

    let click = events
        .iter()
        .find(|e| matches!(e.kind, DomEventKind::Click { .. }));
    assert!(click.is_some());
    assert_eq!(
        click.unwrap().target,
        child,
        "click target should be the child node"
    );
}

// ── Handler registration ─────────────────────────────────────────────────

#[test]
fn handler_receives_events_for_target() {
    let (mut doc, engine, _root, child) = simple_setup();
    let mut dispatcher = EventDispatcher::new();

    let counter = Arc::new(Mutex::new(0u32));
    let counter_clone = counter.clone();
    dispatcher.add_handler(
        child,
        Some(DomEventKind::MouseMove { x: 0.0, y: 0.0 }),
        Box::new(move |_event| {
            *counter_clone.lock().unwrap() += 1;
            Propagation::Continue
        }),
    );

    // Move into child
    dispatcher.dispatch_mouse_move(Point::new(150.0, 150.0), &mut doc, &engine);

    assert!(
        *counter.lock().unwrap() > 0,
        "handler should have been called"
    );
}

// ── Focus management ─────────────────────────────────────────────────────

#[test]
fn focus_is_initially_none() {
    let dispatcher = EventDispatcher::new();
    assert!(dispatcher.focus().is_none());
}

#[test]
fn mouse_down_sets_focus() {
    let (mut doc, engine, _root, child) = simple_setup();
    let mut dispatcher = EventDispatcher::new();

    dispatcher.dispatch_mouse_move(Point::new(150.0, 150.0), &mut doc, &engine);
    dispatcher.dispatch_mouse_down(
        Point::new(150.0, 150.0),
        MouseButton::Left,
        &mut doc,
        &engine,
    );

    assert_eq!(dispatcher.focus(), Some(child), "clicking should set focus");
}
