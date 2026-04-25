//! Central drag-and-drop coordinator.
//!
//! [`DragManager`] manages the lifecycle of drag-and-drop operations,
//! routing events to registered drop targets and maintaining the active
//! [`DragSession`].

use std::collections::HashMap;

use crate::drag_data::DragData;
use crate::drop_target::DropEffect;
use crate::preview::DragPreview;
use crate::session::DragSession;
use crate::traits::DropTargetHandler;

/// Events emitted by the drag manager.
#[derive(Debug, Clone)]
pub enum DragEvent {
    /// A drag operation started.
    Started { source_window: Option<u64> },
    /// The drag cursor moved.
    Moved { x: f32, y: f32 },
    /// The drag entered a new target window.
    EnteredTarget { window_id: u64, effect: DropEffect },
    /// The drag left a target window.
    LeftTarget { window_id: u64 },
    /// The drop effect changed.
    EffectChanged { effect: DropEffect },
    /// The drag completed with a drop.
    Dropped {
        target_window: Option<u64>,
        success: bool,
    },
    /// The drag was cancelled.
    Cancelled,
}

/// Central coordinator for drag-and-drop operations.
///
/// Manages the active [`DragSession`], routes cursor movement to registered
/// [`DropTargetHandler`]s, and enforces the drag threshold.
pub struct DragManager {
    /// Minimum distance in pixels before a drag is considered "started"
    /// (prevents accidental drags from small mouse movements).
    pub drag_threshold: f32,
    /// The current active session, if any.
    session: Option<DragSession>,
    /// Whether the threshold has been exceeded (drag is "committed").
    threshold_met: bool,
    /// Registered drop target handlers, keyed by window ID.
    targets: HashMap<u64, Box<dyn DropTargetHandler>>,
    /// Pending events.
    events: Vec<DragEvent>,
}

impl DragManager {
    /// Create a new drag manager with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            drag_threshold: 5.0,
            session: None,
            threshold_met: false,
            targets: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Create a new drag manager with a custom threshold.
    #[must_use]
    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            drag_threshold: threshold,
            ..Self::new()
        }
    }

    /// Register a drop target for a window.
    pub fn register_target(&mut self, window_id: u64, handler: Box<dyn DropTargetHandler>) {
        self.targets.insert(window_id, handler);
    }

    /// Unregister a drop target.
    pub fn unregister_target(&mut self, window_id: u64) {
        self.targets.remove(&window_id);
    }

    /// Begin a drag operation.
    ///
    /// Returns `true` if the drag was started (no other drag is active).
    /// The drag is not "committed" until the cursor moves past `drag_threshold`
    /// pixels from `start_pos`.
    pub fn begin_drag(
        &mut self,
        source_window: Option<u64>,
        data: DragData,
        preview: DragPreview,
        start_pos: (f32, f32),
    ) -> bool {
        if self.session.is_some() {
            return false; // Already dragging
        }
        if data.is_empty() {
            return false; // Nothing to drag
        }

        self.session = Some(DragSession::new(source_window, data, preview, start_pos));
        self.threshold_met = false;
        self.events.push(DragEvent::Started { source_window });
        true
    }

    /// Update the drag position and optionally set the target window.
    ///
    /// This fires enter/leave/over events on registered drop targets as
    /// appropriate. Does nothing if no drag is active.
    pub fn update_position(&mut self, x: f32, y: f32, target_window: Option<u64>) {
        let Some(session) = &mut self.session else {
            return;
        };

        session.update_pos(x, y);

        // Check threshold
        if !self.threshold_met {
            if session.distance() < self.drag_threshold {
                return; // Haven't moved far enough yet
            }
            self.threshold_met = true;
        }

        self.events.push(DragEvent::Moved { x, y });

        let old_target = session.current_target;
        let new_target = target_window;

        // Handle target transitions
        if old_target != new_target {
            // Leave old target
            if let Some(old_id) = old_target {
                if let Some(handler) = self.targets.get_mut(&old_id) {
                    handler.on_drag_leave();
                }
                self.events
                    .push(DragEvent::LeftTarget { window_id: old_id });
            }

            // Enter new target
            if let Some(new_id) = new_target {
                let effect = if let Some(handler) = self.targets.get_mut(&new_id) {
                    if handler.accepts(&session.data) {
                        handler.on_drag_enter(&session.data)
                    } else {
                        DropEffect::None
                    }
                } else {
                    DropEffect::None
                };

                session.set_effect(effect);
                self.events.push(DragEvent::EnteredTarget {
                    window_id: new_id,
                    effect,
                });
            } else {
                session.set_effect(DropEffect::None);
            }

            session.set_target(new_target);
        } else if let Some(target_id) = new_target {
            // Same target — fire drag_over
            let effect = if let Some(handler) = self.targets.get_mut(&target_id) {
                handler.on_drag_over(x, y, &session.data)
            } else {
                DropEffect::None
            };

            let old_effect = session.effect;
            session.set_effect(effect);
            if effect != old_effect {
                self.events.push(DragEvent::EffectChanged { effect });
            }
        }
    }

    /// Complete the drag at the current position.
    ///
    /// Calls `on_drop` on the current target handler (if any). Returns `true`
    /// if the drop was accepted.
    pub fn drop_drag(&mut self) -> bool {
        let Some(mut session) = self.session.take() else {
            return false;
        };

        if !self.threshold_met {
            // Never exceeded threshold — treat as cancel
            session.end();
            self.threshold_met = false;
            self.events.push(DragEvent::Cancelled);
            return false;
        }

        let target_id = session.current_target;
        let (x, y) = session.current_pos;
        let data = session.data.clone();

        let success = if let Some(tid) = target_id {
            if let Some(handler) = self.targets.get_mut(&tid) {
                handler.on_drop(x, y, data)
            } else {
                false
            }
        } else {
            false
        };

        session.end();
        self.threshold_met = false;
        self.events.push(DragEvent::Dropped {
            target_window: target_id,
            success,
        });

        success
    }

    /// Cancel the current drag operation.
    pub fn cancel(&mut self) {
        if let Some(mut session) = self.session.take() {
            // Leave current target
            if let Some(target_id) = session.current_target {
                if let Some(handler) = self.targets.get_mut(&target_id) {
                    handler.on_drag_leave();
                }
            }
            session.end();
            self.threshold_met = false;
            self.events.push(DragEvent::Cancelled);
        }
    }

    /// Get a reference to the active drag session, if any.
    #[must_use]
    pub fn active_session(&self) -> Option<&DragSession> {
        self.session.as_ref()
    }

    /// Whether a drag is currently active.
    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.session.is_some()
    }

    /// Whether the drag threshold has been met (drag is "committed").
    #[must_use]
    pub fn is_threshold_met(&self) -> bool {
        self.threshold_met
    }

    /// Drain pending events.
    pub fn drain_events(&mut self) -> Vec<DragEvent> {
        std::mem::take(&mut self.events)
    }

    /// Number of registered drop targets.
    #[must_use]
    pub fn target_count(&self) -> usize {
        self.targets.len()
    }
}

impl Default for DragManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drag_data::{DragData, DragFormat};
    use crate::preview::DragPreview;
    use crate::traits::SimpleDropTarget;

    fn text_data() -> DragData {
        DragData::text("hello")
    }

    fn text_preview() -> DragPreview {
        DragPreview::text_label("hello")
    }

    #[test]
    fn test_begin_drag() {
        let mut mgr = DragManager::new();
        let ok = mgr.begin_drag(Some(1), text_data(), text_preview(), (0.0, 0.0));
        assert!(ok);
        assert!(mgr.is_dragging());
        assert!(mgr.active_session().is_some());
    }

    #[test]
    fn test_begin_drag_rejects_when_active() {
        let mut mgr = DragManager::new();
        mgr.begin_drag(Some(1), text_data(), text_preview(), (0.0, 0.0));
        let ok = mgr.begin_drag(Some(2), text_data(), text_preview(), (10.0, 10.0));
        assert!(!ok); // Already dragging
    }

    #[test]
    fn test_begin_drag_rejects_empty_data() {
        let mut mgr = DragManager::new();
        let ok = mgr.begin_drag(Some(1), DragData::new(), text_preview(), (0.0, 0.0));
        assert!(!ok);
    }

    #[test]
    fn test_drag_threshold() {
        let mut mgr = DragManager::with_threshold(10.0);
        mgr.begin_drag(Some(1), text_data(), text_preview(), (0.0, 0.0));

        // Move less than threshold — no events beyond Started
        mgr.update_position(3.0, 4.0, None); // distance = 5 < 10
        assert!(!mgr.is_threshold_met());

        // Move past threshold
        mgr.update_position(8.0, 6.0, None); // distance = 10
        assert!(mgr.is_threshold_met());
    }

    #[test]
    fn test_drop_before_threshold_cancels() {
        let mut mgr = DragManager::with_threshold(10.0);
        mgr.begin_drag(Some(1), text_data(), text_preview(), (0.0, 0.0));
        mgr.update_position(1.0, 1.0, None); // below threshold

        let success = mgr.drop_drag();
        assert!(!success);
        assert!(!mgr.is_dragging());

        let events = mgr.drain_events();
        assert!(events.iter().any(|e| matches!(e, DragEvent::Cancelled)));
    }

    #[test]
    fn test_cancel() {
        let mut mgr = DragManager::new();
        mgr.begin_drag(Some(1), text_data(), text_preview(), (0.0, 0.0));
        mgr.cancel();
        assert!(!mgr.is_dragging());

        let events = mgr.drain_events();
        assert!(events.iter().any(|e| matches!(e, DragEvent::Cancelled)));
    }

    #[test]
    fn test_target_enter_leave() {
        let mut mgr = DragManager::with_threshold(0.0);
        mgr.register_target(10, Box::new(SimpleDropTarget::text()));
        mgr.register_target(20, Box::new(SimpleDropTarget::text()));

        mgr.begin_drag(Some(1), text_data(), text_preview(), (0.0, 0.0));

        // Enter target 10
        mgr.update_position(50.0, 50.0, Some(10));
        let events = mgr.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, DragEvent::EnteredTarget { window_id: 10, .. }))
        );

        // Move to target 20 (leave 10, enter 20)
        mgr.update_position(150.0, 50.0, Some(20));
        let events = mgr.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, DragEvent::LeftTarget { window_id: 10 }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, DragEvent::EnteredTarget { window_id: 20, .. }))
        );
    }

    #[test]
    fn test_drop_on_target() {
        let mut mgr = DragManager::with_threshold(0.0);
        mgr.register_target(10, Box::new(SimpleDropTarget::text()));

        mgr.begin_drag(Some(1), text_data(), text_preview(), (0.0, 0.0));
        mgr.update_position(50.0, 50.0, Some(10)); // enter
        mgr.update_position(55.0, 55.0, Some(10)); // over

        let success = mgr.drop_drag();
        assert!(success);
        assert!(!mgr.is_dragging());
    }

    #[test]
    fn test_drop_on_incompatible_target() {
        let mut mgr = DragManager::with_threshold(0.0);
        // Target accepts file-paths, but we're dragging text
        mgr.register_target(10, Box::new(SimpleDropTarget::file_paths()));

        mgr.begin_drag(Some(1), text_data(), text_preview(), (0.0, 0.0));
        mgr.update_position(50.0, 50.0, Some(10));

        let success = mgr.drop_drag();
        assert!(!success);
    }

    #[test]
    fn test_drop_no_target() {
        let mut mgr = DragManager::with_threshold(0.0);
        mgr.begin_drag(Some(1), text_data(), text_preview(), (0.0, 0.0));
        mgr.update_position(50.0, 50.0, None);

        let success = mgr.drop_drag();
        assert!(!success);

        let events = mgr.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            DragEvent::Dropped {
                target_window: None,
                success: false
            }
        )));
    }

    #[test]
    fn test_cancel_leaves_target() {
        let mut mgr = DragManager::with_threshold(0.0);
        mgr.register_target(10, Box::new(SimpleDropTarget::text()));

        mgr.begin_drag(Some(1), text_data(), text_preview(), (0.0, 0.0));
        mgr.update_position(50.0, 50.0, Some(10));
        mgr.cancel();

        let events = mgr.drain_events();
        assert!(events.iter().any(|e| matches!(e, DragEvent::Cancelled)));
    }

    #[test]
    fn test_register_unregister_target() {
        let mut mgr = DragManager::new();
        mgr.register_target(10, Box::new(SimpleDropTarget::text()));
        assert_eq!(mgr.target_count(), 1);

        mgr.unregister_target(10);
        assert_eq!(mgr.target_count(), 0);
    }

    #[test]
    fn test_session_tracks_position() {
        let mut mgr = DragManager::with_threshold(0.0);
        mgr.begin_drag(Some(1), text_data(), text_preview(), (10.0, 20.0));
        mgr.update_position(30.0, 40.0, None);

        let session = mgr.active_session().unwrap();
        assert_eq!(session.current_pos, (30.0, 40.0));
        assert_eq!(session.start_pos, (10.0, 20.0));
    }

    #[test]
    fn test_effect_change_event() {
        let mut mgr = DragManager::with_threshold(0.0);
        mgr.register_target(10, Box::new(SimpleDropTarget::text()));

        mgr.begin_drag(Some(1), text_data(), text_preview(), (0.0, 0.0));
        mgr.update_position(50.0, 50.0, Some(10));
        mgr.drain_events(); // clear

        // Move within same target — effect stays the same, no EffectChanged
        mgr.update_position(55.0, 55.0, Some(10));
        let events = mgr.drain_events();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, DragEvent::EffectChanged { .. }))
        );
    }

    #[test]
    fn test_multi_format_drop() {
        let mut mgr = DragManager::with_threshold(0.0);
        mgr.register_target(10, Box::new(SimpleDropTarget::text()));

        // Drag data with multiple formats
        let mut data = DragData::text("file:///doc.txt");
        data.add_format(DragFormat::FilePaths(vec!["/doc.txt".into()]));

        mgr.begin_drag(Some(1), data, text_preview(), (0.0, 0.0));
        mgr.update_position(50.0, 50.0, Some(10));

        let success = mgr.drop_drag();
        assert!(success); // text target accepts because data has Text format
    }

    #[test]
    fn test_drag_after_cancel_can_restart() {
        let mut mgr = DragManager::new();
        mgr.begin_drag(Some(1), text_data(), text_preview(), (0.0, 0.0));
        mgr.cancel();

        // Should be able to start a new drag
        let ok = mgr.begin_drag(Some(2), text_data(), text_preview(), (10.0, 10.0));
        assert!(ok);
        assert!(mgr.is_dragging());
    }
}
