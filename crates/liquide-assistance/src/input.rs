//! Input routing for assistance sessions.

use crate::mode::AssistanceMode;

/// The source of an input event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputSource {
    /// Input from the session owner.
    Owner,
    /// Input from an observer with the given identifier.
    Observer { id: String },
}

/// The type of input event.
#[derive(Debug, Clone)]
pub enum InputEventType {
    /// A keyboard event.
    Keyboard { key: u32, pressed: bool },
    /// A mouse movement/button event.
    Mouse { x: f64, y: f64, buttons: u32 },
    /// A touch event.
    Touch { id: u32, x: f64, y: f64 },
}

/// A single input event with source and timestamp.
#[derive(Debug, Clone)]
pub struct InputEvent {
    /// Who generated the event.
    pub source: InputSource,
    /// The type of event.
    pub event_type: InputEventType,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Routes input events based on the current assistance mode.
pub struct InputCoordinator {
    exclusive_observer: Option<String>,
    active_sources: Vec<InputSource>,
}

impl InputCoordinator {
    /// Create a new input coordinator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            exclusive_observer: None,
            active_sources: vec![InputSource::Owner],
        }
    }

    /// Route an input event according to the mode.
    /// Returns the events that should be forwarded.
    #[must_use]
    pub fn route_input(&self, event: InputEvent, mode: AssistanceMode) -> Vec<InputEvent> {
        match mode {
            AssistanceMode::ViewOnly | AssistanceMode::Stealth => {
                // Only owner input is forwarded.
                if event.source == InputSource::Owner {
                    vec![event]
                } else {
                    vec![]
                }
            }
            AssistanceMode::Interactive => {
                // Both owner and observer input is forwarded.
                vec![event]
            }
            AssistanceMode::Exclusive => {
                // Only the exclusive observer's input is forwarded.
                if let Some(ref exclusive_id) = self.exclusive_observer {
                    match &event.source {
                        InputSource::Observer { id } if id == exclusive_id => vec![event],
                        _ => vec![],
                    }
                } else {
                    // No exclusive observer set, forward owner input.
                    if event.source == InputSource::Owner {
                        vec![event]
                    } else {
                        vec![]
                    }
                }
            }
        }
    }

    /// Grant exclusive input control to an observer.
    pub fn grant_exclusive(&mut self, observer_id: String) {
        self.exclusive_observer = Some(observer_id.clone());
        self.active_sources = vec![InputSource::Observer { id: observer_id }];
    }

    /// Reclaim input control for the owner.
    pub fn reclaim_owner_control(&mut self) {
        self.exclusive_observer = None;
        self.active_sources = vec![InputSource::Owner];
    }

    /// Return the currently active input sources.
    #[must_use]
    pub fn active_input_sources(&self) -> Vec<InputSource> {
        self.active_sources.clone()
    }
}

impl Default for InputCoordinator {
    fn default() -> Self {
        Self::new()
    }
}
