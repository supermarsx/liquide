//! Integration tests for W3C capture → target → bubble event dispatch.
//!
//! Exercises the full propagation pipeline through `EventDispatcher`'s public API.

use std::sync::{Arc, Mutex};

use liquide_dom::NodeId;
use liquide_hit_test::dispatch::{EventDispatcher, EventHandler};
use liquide_hit_test::event::{DomEvent, DomEventKind, EventPhase, MouseButton, Propagation};

// ── Helpers ──────────────────────────────────────────────────────────────

/// Build a Click event targeting `target` with the given ancestor path (root-first).
fn click_event(target: NodeId, path: Vec<NodeId>) -> DomEvent {
    let mut ev = DomEvent::new(
        target,
        DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        },
    );
    ev.event_path = path;
    ev
}

/// Build a non-bubbling Focus event targeting `target` with the given path.
fn focus_event(target: NodeId, path: Vec<NodeId>) -> DomEvent {
    let mut ev = DomEvent::new(target, DomEventKind::Focus);
    ev.event_path = path;
    ev
}

/// Build a KeyDown event targeting `target` with the given path.
fn keydown_event(target: NodeId, path: Vec<NodeId>) -> DomEvent {
    let mut ev = DomEvent::new(target, DomEventKind::KeyDown { key: 65, modifiers: 0 });
    ev.event_path = path;
    ev
}

/// Build a MouseMove event targeting `target` with the given path.
fn mousemove_event(target: NodeId, path: Vec<NodeId>) -> DomEvent {
    let mut ev = DomEvent::new(target, DomEventKind::MouseMove { x: 10.0, y: 20.0 });
    ev.event_path = path;
    ev
}

/// Handler that records a label into a shared log.
fn tracking_handler(log: Arc<Mutex<Vec<String>>>, label: &str) -> EventHandler {
    let label = label.to_string();
    Box::new(move |_ev: &DomEvent| {
        log.lock().unwrap().push(label.clone());
        Propagation::Continue
    })
}

/// Handler that records a label and returns a specific propagation result.
fn stopping_handler(
    log: Arc<Mutex<Vec<String>>>,
    label: &str,
    result: Propagation,
) -> EventHandler {
    let label = label.to_string();
    Box::new(move |_ev: &DomEvent| {
        log.lock().unwrap().push(label.clone());
        result
    })
}

/// Handler that records the event phase alongside a label.
fn phase_tracking_handler(
    log: Arc<Mutex<Vec<(String, EventPhase)>>>,
    label: &str,
) -> EventHandler {
    let label = label.to_string();
    Box::new(move |ev: &DomEvent| {
        log.lock().unwrap().push((label.clone(), ev.phase));
        Propagation::Continue
    })
}

// ── 1. Full capture → target → bubble ordering (3-level hierarchy) ──────

#[test]
fn test_capture_target_bubble_ordering_three_levels() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let parent: NodeId = 2;
    let child: NodeId = 3;

    // Root: capture + bubble
    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    // Parent: bubble only
    dispatcher.add_event_listener(parent, None, false, tracking_handler(log.clone(), "parent-bubble"));

    // Child (target): capture + bubble (both fire at-target in registration order)
    dispatcher.add_event_listener(child, None, true, tracking_handler(log.clone(), "child-capture"));
    dispatcher.add_event_listener(child, None, false, tracking_handler(log.clone(), "child-bubble"));

    let event = click_event(child, vec![root, parent]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec![
            "root-capture",
            "child-capture",
            "child-bubble",
            "parent-bubble",
            "root-bubble",
        ],
        "Expected: root-capture → child-capture → child-bubble → parent-bubble → root-bubble"
    );
}

// ── 2. stopPropagation in capture phase stops everything after ───────────

#[test]
fn test_stop_propagation_in_capture_stops_bubble() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let parent: NodeId = 2;
    let child: NodeId = 3;

    // Root capture: calls stopPropagation
    dispatcher.add_event_listener(
        root,
        None,
        true,
        stopping_handler(log.clone(), "root-capture", Propagation::StopPropagation),
    );

    // These should NOT be called
    dispatcher.add_event_listener(parent, None, true, tracking_handler(log.clone(), "parent-capture"));
    dispatcher.add_event_listener(child, None, true, tracking_handler(log.clone(), "child-capture"));
    dispatcher.add_event_listener(child, None, false, tracking_handler(log.clone(), "child-bubble"));
    dispatcher.add_event_listener(parent, None, false, tracking_handler(log.clone(), "parent-bubble"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    let event = click_event(child, vec![root, parent]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec!["root-capture"],
        "Only root-capture should fire; parent-capture, target, and bubble should all be stopped"
    );
}

#[test]
fn test_stop_propagation_mid_capture_stops_remaining() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let parent: NodeId = 2;
    let child: NodeId = 3;

    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture"));
    // Parent capture stops propagation
    dispatcher.add_event_listener(
        parent,
        None,
        true,
        stopping_handler(log.clone(), "parent-capture", Propagation::StopPropagation),
    );
    dispatcher.add_event_listener(child, None, false, tracking_handler(log.clone(), "child-target"));
    dispatcher.add_event_listener(parent, None, false, tracking_handler(log.clone(), "parent-bubble"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    let event = click_event(child, vec![root, parent]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec!["root-capture", "parent-capture"],
        "Only root-capture and parent-capture should fire"
    );
}

// ── 3. stopImmediatePropagation ─────────────────────────────────────────

#[test]
fn test_stop_immediate_propagation_at_target() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let target: NodeId = 2;

    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture"));

    // Target has 2 listeners — first calls stopImmediatePropagation
    dispatcher.add_event_listener(
        target,
        None,
        false,
        stopping_handler(log.clone(), "target-1", Propagation::StopImmediate),
    );
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target-2"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    let event = click_event(target, vec![root]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec!["root-capture", "target-1"],
        "Second target listener and bubble should NOT fire after stopImmediatePropagation"
    );
}

#[test]
fn test_stop_immediate_propagation_in_capture() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let target: NodeId = 2;

    // Root has 2 capture listeners — first calls stopImmediatePropagation
    dispatcher.add_event_listener(
        root,
        None,
        true,
        stopping_handler(log.clone(), "root-capture-1", Propagation::StopImmediate),
    );
    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture-2"));
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    let event = click_event(target, vec![root]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec!["root-capture-1"],
        "stopImmediatePropagation should prevent second capture listener, target, and bubble"
    );
}

// ── 4. Non-bubbling events ──────────────────────────────────────────────

#[test]
fn test_non_bubbling_event_capture_runs_but_bubble_does_not() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let parent: NodeId = 2;
    let target: NodeId = 3;

    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture"));
    dispatcher.add_event_listener(parent, None, true, tracking_handler(log.clone(), "parent-capture"));
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target"));
    dispatcher.add_event_listener(parent, None, false, tracking_handler(log.clone(), "parent-bubble"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    // Focus does NOT bubble (per W3C spec)
    let event = focus_event(target, vec![root, parent]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec!["root-capture", "parent-capture", "target"],
        "Non-bubbling Focus event: capture and at-target should fire, but NOT bubble"
    );
}

#[test]
fn test_non_bubbling_mouse_enter_no_bubble() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let target: NodeId = 2;

    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture"));
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    let mut ev = DomEvent::new(target, DomEventKind::MouseEnter);
    ev.event_path = vec![root];
    dispatcher.dispatch_events(&[ev]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec!["root-capture", "target"],
        "MouseEnter should capture but not bubble"
    );
}

// ── 5. Event.eventPhase correctness ─────────────────────────────────────

#[test]
fn test_event_phase_capture_at_target_bubble() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<(String, EventPhase)>::new()));

    let root: NodeId = 1;
    let parent: NodeId = 2;
    let target: NodeId = 3;

    dispatcher.add_event_listener(root, None, true, phase_tracking_handler(log.clone(), "root-capture"));
    dispatcher.add_event_listener(parent, None, true, phase_tracking_handler(log.clone(), "parent-capture"));
    dispatcher.add_event_listener(target, None, true, phase_tracking_handler(log.clone(), "target-capture"));
    dispatcher.add_event_listener(target, None, false, phase_tracking_handler(log.clone(), "target-bubble"));
    dispatcher.add_event_listener(parent, None, false, phase_tracking_handler(log.clone(), "parent-bubble"));
    dispatcher.add_event_listener(root, None, false, phase_tracking_handler(log.clone(), "root-bubble"));

    let event = click_event(target, vec![root, parent]);
    dispatcher.dispatch_events(&[event]);

    let recorded = log.lock().unwrap().clone();
    assert_eq!(recorded.len(), 6);

    // Capture phase
    assert_eq!(recorded[0], ("root-capture".to_string(), EventPhase::Capturing));
    assert_eq!(recorded[1], ("parent-capture".to_string(), EventPhase::Capturing));

    // At target — both capture and bubble listeners fire with AtTarget phase
    assert_eq!(recorded[2], ("target-capture".to_string(), EventPhase::AtTarget));
    assert_eq!(recorded[3], ("target-bubble".to_string(), EventPhase::AtTarget));

    // Bubble phase
    assert_eq!(recorded[4], ("parent-bubble".to_string(), EventPhase::Bubbling));
    assert_eq!(recorded[5], ("root-bubble".to_string(), EventPhase::Bubbling));
}

#[test]
fn test_event_phase_for_non_bubbling_event() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<(String, EventPhase)>::new()));

    let root: NodeId = 1;
    let target: NodeId = 2;

    dispatcher.add_event_listener(root, None, true, phase_tracking_handler(log.clone(), "root-capture"));
    dispatcher.add_event_listener(target, None, false, phase_tracking_handler(log.clone(), "target"));
    dispatcher.add_event_listener(root, None, false, phase_tracking_handler(log.clone(), "root-bubble"));

    let event = focus_event(target, vec![root]);
    dispatcher.dispatch_events(&[event]);

    let recorded = log.lock().unwrap().clone();
    assert_eq!(recorded.len(), 2, "Non-bubbling event should only fire capture + at-target");
    assert_eq!(recorded[0], ("root-capture".to_string(), EventPhase::Capturing));
    assert_eq!(recorded[1], ("target".to_string(), EventPhase::AtTarget));
}

// ── 6. Deep hierarchy stress test ───────────────────────────────────────

#[test]
fn test_deep_hierarchy_20_levels() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    // Create 20 ancestor nodes (1..=20) and target node (21)
    let ancestors: Vec<NodeId> = (1..=20).map(NodeId::from).collect();
    let target: NodeId = 21;

    // Add capture listener on every ancestor
    for (i, &node) in ancestors.iter().enumerate() {
        dispatcher.add_event_listener(
            node,
            None,
            true,
            tracking_handler(log.clone(), &format!("capture-{}", i + 1)),
        );
    }

    // Target listener
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target"));

    // Add bubble listener on every ancestor (in reverse so root-bubble is last)
    for (i, &node) in ancestors.iter().enumerate() {
        dispatcher.add_event_listener(
            node,
            None,
            false,
            tracking_handler(log.clone(), &format!("bubble-{}", i + 1)),
        );
    }

    let event = click_event(target, ancestors.clone());
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();

    // Expected: capture-1, capture-2, ..., capture-20, target, bubble-20, bubble-19, ..., bubble-1
    let mut expected = Vec::new();
    for i in 1..=20 {
        expected.push(format!("capture-{}", i));
    }
    expected.push("target".to_string());
    for i in (1..=20).rev() {
        expected.push(format!("bubble-{}", i));
    }

    assert_eq!(calls.len(), 41, "Should be 20 capture + 1 target + 20 bubble = 41 calls");
    assert_eq!(calls, expected, "Deep hierarchy propagation order mismatch");
}

#[test]
fn test_deep_hierarchy_stop_propagation_at_level_10() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let ancestors: Vec<NodeId> = (1..=20).map(NodeId::from).collect();
    let target: NodeId = 21;

    for (i, &node) in ancestors.iter().enumerate() {
        if i == 9 {
            // Level 10 (index 9) stops propagation during capture
            dispatcher.add_event_listener(
                node,
                None,
                true,
                stopping_handler(log.clone(), &format!("capture-{}", i + 1), Propagation::StopPropagation),
            );
        } else {
            dispatcher.add_event_listener(
                node,
                None,
                true,
                tracking_handler(log.clone(), &format!("capture-{}", i + 1)),
            );
        }
    }

    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target"));

    for (i, &node) in ancestors.iter().enumerate() {
        dispatcher.add_event_listener(
            node,
            None,
            false,
            tracking_handler(log.clone(), &format!("bubble-{}", i + 1)),
        );
    }

    let event = click_event(target, ancestors);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    // Should fire capture-1 through capture-10, then stop
    assert_eq!(calls.len(), 10);
    for i in 1..=10 {
        assert_eq!(calls[i - 1], format!("capture-{}", i));
    }
}

// ── 7. Multiple event types ─────────────────────────────────────────────

#[test]
fn test_dispatch_works_for_mouse_events() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let target: NodeId = 2;

    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture"));
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    let event = click_event(target, vec![root]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(calls, vec!["root-capture", "target", "root-bubble"]);
}

#[test]
fn test_dispatch_works_for_keyboard_events() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let target: NodeId = 2;

    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture"));
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    let event = keydown_event(target, vec![root]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(calls, vec!["root-capture", "target", "root-bubble"]);
}

#[test]
fn test_dispatch_works_for_focus_events() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let target: NodeId = 2;

    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture"));
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    // Focus does NOT bubble
    let event = focus_event(target, vec![root]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec!["root-capture", "target"],
        "Focus should capture + at-target but NOT bubble"
    );
}

#[test]
fn test_dispatch_works_for_mousemove_events() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let target: NodeId = 2;

    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture"));
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    let event = mousemove_event(target, vec![root]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec!["root-capture", "target", "root-bubble"],
        "MouseMove should capture + at-target + bubble"
    );
}

#[test]
fn test_kind_filter_works_across_event_types() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let target: NodeId = 2;

    // Filter: only Click events
    dispatcher.add_event_listener(
        root,
        Some(DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        }),
        true,
        tracking_handler(log.clone(), "root-click-capture"),
    );
    // Filter: only KeyDown events
    dispatcher.add_event_listener(
        root,
        Some(DomEventKind::KeyDown { key: 0, modifiers: 0 }),
        true,
        tracking_handler(log.clone(), "root-keydown-capture"),
    );

    // Dispatch a Click — only click filter should match
    let event = click_event(target, vec![root]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(calls, vec!["root-click-capture"]);

    // Dispatch a KeyDown — only keydown filter should match
    log.lock().unwrap().clear();
    let event = keydown_event(target, vec![root]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(calls, vec!["root-keydown-capture"]);
}

// ── 8. Dynamic listener add/remove during dispatch ──────────────────────

#[test]
fn test_listener_added_during_dispatch_not_called_for_current_event() {
    // Since `dispatch_events` takes `&self`, handlers cannot mutate the dispatcher
    // to add new listeners during dispatch. This test verifies the API enforces this
    // structural guarantee: the handler list is immutable during fire_handlers.
    //
    // We simulate the scenario by verifying that a second dispatch call sees the
    // original handler set, and that any mutations happen between dispatches.

    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let target: NodeId = 2;

    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture"));
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target"));

    // First dispatch — 2 handlers fire
    let event = click_event(target, vec![root]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(calls, vec!["root-capture", "target"]);

    // Add a new handler between dispatches
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble-new"));

    // Second dispatch — new handler is now visible
    log.lock().unwrap().clear();
    let event = click_event(target, vec![root]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec!["root-capture", "target", "root-bubble-new"],
        "New handler should fire on subsequent dispatch"
    );
}

// ── Additional edge cases ───────────────────────────────────────────────

#[test]
fn test_empty_event_path_only_target_fires() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let target: NodeId = 1;

    dispatcher.add_event_listener(target, None, true, tracking_handler(log.clone(), "target-capture"));
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target-bubble"));

    // No ancestors — just target
    let event = click_event(target, vec![]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    // Both fire in registration order at-target
    assert_eq!(calls, vec!["target-capture", "target-bubble"]);
}

#[test]
fn test_multiple_events_dispatched_sequentially() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let target: NodeId = 2;

    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture"));
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    // Dispatch 3 events at once
    let events = vec![
        click_event(target, vec![root]),
        click_event(target, vec![root]),
        click_event(target, vec![root]),
    ];
    dispatcher.dispatch_events(&events);

    let calls = log.lock().unwrap().clone();
    assert_eq!(calls.len(), 9, "3 events × 3 handlers each = 9 calls");
    // Each event follows the same order
    for chunk in calls.chunks(3) {
        assert_eq!(chunk, &["root-capture", "target", "root-bubble"]);
    }
}

#[test]
fn test_stop_propagation_at_target_allows_remaining_target_listeners_but_stops_bubble() {
    let mut dispatcher = EventDispatcher::new();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));

    let root: NodeId = 1;
    let target: NodeId = 2;

    dispatcher.add_event_listener(root, None, true, tracking_handler(log.clone(), "root-capture"));
    dispatcher.add_event_listener(
        target,
        None,
        false,
        stopping_handler(log.clone(), "target-1", Propagation::StopPropagation),
    );
    // stopPropagation allows remaining listeners on same node
    dispatcher.add_event_listener(target, None, false, tracking_handler(log.clone(), "target-2"));
    dispatcher.add_event_listener(root, None, false, tracking_handler(log.clone(), "root-bubble"));

    let event = click_event(target, vec![root]);
    dispatcher.dispatch_events(&[event]);

    let calls = log.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec!["root-capture", "target-1", "target-2"],
        "stopPropagation at target: remaining target listeners fire, but bubble does NOT"
    );
}

#[test]
fn test_current_target_changes_during_propagation() {
    let mut dispatcher = EventDispatcher::new();
    let targets = Arc::new(Mutex::new(Vec::<(NodeId, NodeId)>::new()));

    let root: NodeId = 1;
    let parent: NodeId = 2;
    let child: NodeId = 3;

    // Record (current_target, target) at each phase
    let t = targets.clone();
    dispatcher.add_event_listener(
        root,
        None,
        true,
        Box::new(move |ev: &DomEvent| {
            t.lock().unwrap().push((ev.current_target, ev.target));
            Propagation::Continue
        }),
    );
    let t = targets.clone();
    dispatcher.add_event_listener(
        child,
        None,
        false,
        Box::new(move |ev: &DomEvent| {
            t.lock().unwrap().push((ev.current_target, ev.target));
            Propagation::Continue
        }),
    );
    let t = targets.clone();
    dispatcher.add_event_listener(
        parent,
        None,
        false,
        Box::new(move |ev: &DomEvent| {
            t.lock().unwrap().push((ev.current_target, ev.target));
            Propagation::Continue
        }),
    );
    let t = targets.clone();
    dispatcher.add_event_listener(
        root,
        None,
        false,
        Box::new(move |ev: &DomEvent| {
            t.lock().unwrap().push((ev.current_target, ev.target));
            Propagation::Continue
        }),
    );

    let event = click_event(child, vec![root, parent]);
    dispatcher.dispatch_events(&[event]);

    let recorded = targets.lock().unwrap().clone();
    assert_eq!(recorded.len(), 4);
    // target is always child (3)
    for &(_, target) in &recorded {
        assert_eq!(target, child, "event.target should always be the original target");
    }
    // current_target changes
    assert_eq!(recorded[0].0, root, "capture phase: current_target = root");
    assert_eq!(recorded[1].0, child, "at-target: current_target = child");
    assert_eq!(recorded[2].0, parent, "bubble phase: current_target = parent");
    assert_eq!(recorded[3].0, root, "bubble phase: current_target = root");
}
