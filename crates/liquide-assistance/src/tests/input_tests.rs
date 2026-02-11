use crate::input::{InputCoordinator, InputEvent, InputEventType, InputSource};
use crate::mode::AssistanceMode;

fn make_owner_event() -> InputEvent {
    InputEvent {
        source: InputSource::Owner,
        event_type: InputEventType::Keyboard { key: 65, pressed: true },
        timestamp: 1000,
    }
}

fn make_observer_event(id: &str) -> InputEvent {
    InputEvent {
        source: InputSource::Observer { id: id.to_string() },
        event_type: InputEventType::Mouse { x: 100.0, y: 200.0, buttons: 1 },
        timestamp: 1000,
    }
}

#[test]
fn test_view_only_blocks_observer() {
    let coord = InputCoordinator::new();
    let events = coord.route_input(make_observer_event("obs-1"), AssistanceMode::ViewOnly);
    assert!(events.is_empty());
}

#[test]
fn test_view_only_allows_owner() {
    let coord = InputCoordinator::new();
    let events = coord.route_input(make_owner_event(), AssistanceMode::ViewOnly);
    assert_eq!(events.len(), 1);
}

#[test]
fn test_interactive_allows_both() {
    let coord = InputCoordinator::new();
    let owner_events = coord.route_input(make_owner_event(), AssistanceMode::Interactive);
    let obs_events = coord.route_input(make_observer_event("obs-1"), AssistanceMode::Interactive);
    assert_eq!(owner_events.len(), 1);
    assert_eq!(obs_events.len(), 1);
}

#[test]
fn test_exclusive_mode() {
    let mut coord = InputCoordinator::new();
    coord.grant_exclusive("obs-1".into());

    let owner = coord.route_input(make_owner_event(), AssistanceMode::Exclusive);
    assert!(owner.is_empty());

    let obs = coord.route_input(make_observer_event("obs-1"), AssistanceMode::Exclusive);
    assert_eq!(obs.len(), 1);
}

#[test]
fn test_reclaim_control() {
    let mut coord = InputCoordinator::new();
    coord.grant_exclusive("obs-1".into());
    coord.reclaim_owner_control();

    let sources = coord.active_input_sources();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0], InputSource::Owner);
}
