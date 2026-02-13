//! High-level input event dispatcher that ties together device polling,
//! state tracking, and event routing.

use crate::device::DeviceManager;
use crate::event::InputEvent;
use crate::router::{InputRouter, InputTarget};
use crate::state::InputState;

/// A dispatched event ready for consumption by the shell or compositor.
pub struct DispatchedEvent {
    /// The input event.
    pub event: InputEvent,
    /// Target surface ID (0 if no target found).
    pub target_surface: u64,
    /// Event sequence number.
    pub sequence: u64,
}

/// Ties together [`DeviceManager`], [`InputRouter`], and [`InputState`]
/// into a single polling + dispatch pipeline.
///
/// Usage:
/// 1. Call `poll_and_dispatch()` each frame with the current surface list
/// 2. Iterate over returned `DispatchedEvent`s
/// 3. The internal `InputState` is updated automatically
pub struct EventDispatcher {
    pub device_manager: DeviceManager,
    pub router: InputRouter,
    pub state: InputState,
}

impl EventDispatcher {
    /// Create a new dispatcher with default device manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            device_manager: DeviceManager::new(),
            router: InputRouter::new(),
            state: InputState::new(),
        }
    }

    /// Create a dispatcher with the platform-default input device.
    #[must_use]
    pub fn with_platform_default() -> Self {
        Self {
            device_manager: DeviceManager::with_platform_default(),
            router: InputRouter::new(),
            state: InputState::new(),
        }
    }

    /// Poll all devices, update input state, route events to surfaces,
    /// and return dispatched events.
    ///
    /// `surfaces` is the list of input targets (windows/surfaces) used
    /// for hit-testing mouse and touch events. The list should be in
    /// back-to-front z-order (topmost surface last).
    pub fn poll_and_dispatch(&mut self, surfaces: &[&dyn InputTarget]) -> Vec<DispatchedEvent> {
        let packets = self.device_manager.poll_all();
        let mut dispatched = Vec::with_capacity(packets.len());

        for packet in packets {
            // Update global input state
            self.state.handle_event(&packet.event);

            // Route to target surface
            let target_surface = self
                .router
                .route(&packet.event, surfaces)
                .map(|(id, _)| id)
                .unwrap_or(0);

            dispatched.push(DispatchedEvent {
                event: packet.event,
                target_surface,
                sequence: packet.sequence,
            });
        }

        dispatched
    }

    /// Feed a single external event (e.g., from a platform backend's
    /// event loop rather than from device polling).
    ///
    /// Updates state and routes the event.
    pub fn dispatch_external(
        &mut self,
        event: InputEvent,
        surfaces: &[&dyn InputTarget],
    ) -> DispatchedEvent {
        self.state.handle_event(&event);

        let target_surface = self
            .router
            .route(&event, surfaces)
            .map(|(id, _)| id)
            .unwrap_or(0);

        DispatchedEvent {
            event,
            target_surface,
            sequence: 0,
        }
    }

    /// Access the current input state.
    #[must_use]
    pub fn input_state(&self) -> &InputState {
        &self.state
    }

    /// Set the focused surface for keyboard routing.
    pub fn set_focus(&mut self, surface_id: u64) {
        self.router.set_focus(surface_id);
    }

    /// Clear keyboard focus.
    pub fn clear_focus(&mut self) {
        self.router.clear_focus();
    }

    /// Get the currently focused surface.
    #[must_use]
    pub fn focused(&self) -> Option<u64> {
        self.router.focused()
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
