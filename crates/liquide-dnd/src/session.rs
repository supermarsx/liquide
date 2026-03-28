//! Active drag session state.
//!
//! [`DragSession`] tracks the full state of an in-progress drag operation:
//! source window, payload data, preview, cursor positions, current target,
//! and negotiated drop effect.

use crate::drag_data::DragData;
use crate::drop_target::DropEffect;
use crate::preview::DragPreview;

/// An active drag-and-drop session.
///
/// Created by [`DragManager::begin_drag`](crate::manager::DragManager::begin_drag)
/// and destroyed when the drag completes or is cancelled.
#[derive(Debug, Clone)]
pub struct DragSession {
    /// The window that initiated the drag (if known).
    pub source_window: Option<u64>,
    /// The data being dragged.
    pub data: DragData,
    /// The visual preview for this drag.
    pub preview: DragPreview,
    /// Current cursor position.
    pub current_pos: (f32, f32),
    /// Position where the drag started.
    pub start_pos: (f32, f32),
    /// The window currently under the cursor (if any).
    pub current_target: Option<u64>,
    /// The negotiated drop effect.
    pub effect: DropEffect,
    /// Whether the session is still active.
    active: bool,
}

impl DragSession {
    /// Create a new drag session.
    pub(crate) fn new(
        source_window: Option<u64>,
        data: DragData,
        preview: DragPreview,
        start_pos: (f32, f32),
    ) -> Self {
        Self {
            source_window,
            data,
            preview,
            current_pos: start_pos,
            start_pos,
            current_target: None,
            effect: DropEffect::None,
            active: true,
        }
    }

    /// Whether the drag session is still active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Euclidean distance from the start position to the current position.
    #[must_use]
    pub fn distance(&self) -> f32 {
        let dx = self.current_pos.0 - self.start_pos.0;
        let dy = self.current_pos.1 - self.start_pos.1;
        (dx * dx + dy * dy).sqrt()
    }

    /// Mark the session as ended.
    pub(crate) fn end(&mut self) {
        self.active = false;
    }

    /// Update the current cursor position.
    pub(crate) fn update_pos(&mut self, x: f32, y: f32) {
        self.current_pos = (x, y);
    }

    /// Set the current target window.
    pub(crate) fn set_target(&mut self, target: Option<u64>) {
        self.current_target = target;
    }

    /// Set the negotiated drop effect.
    pub(crate) fn set_effect(&mut self, effect: DropEffect) {
        self.effect = effect;
    }

    /// The delta from start to current position.
    #[must_use]
    pub fn delta(&self) -> (f32, f32) {
        (
            self.current_pos.0 - self.start_pos.0,
            self.current_pos.1 - self.start_pos.1,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session() -> DragSession {
        DragSession::new(
            Some(42),
            DragData::text("test"),
            DragPreview::text_label("test"),
            (100.0, 200.0),
        )
    }

    #[test]
    fn test_session_initial_state() {
        let s = make_session();
        assert!(s.is_active());
        assert_eq!(s.source_window, Some(42));
        assert_eq!(s.start_pos, (100.0, 200.0));
        assert_eq!(s.current_pos, (100.0, 200.0));
        assert_eq!(s.current_target, None);
        assert_eq!(s.effect, DropEffect::None);
    }

    #[test]
    fn test_session_distance() {
        let mut s = make_session();
        // At start, distance is 0
        assert!((s.distance() - 0.0).abs() < f32::EPSILON);

        // Move 3,4 -> distance 5
        s.update_pos(103.0, 204.0);
        assert!((s.distance() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_session_delta() {
        let mut s = make_session();
        s.update_pos(110.0, 215.0);
        let (dx, dy) = s.delta();
        assert!((dx - 10.0).abs() < f32::EPSILON);
        assert!((dy - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_session_end() {
        let mut s = make_session();
        assert!(s.is_active());
        s.end();
        assert!(!s.is_active());
    }

    #[test]
    fn test_session_target_tracking() {
        let mut s = make_session();
        assert_eq!(s.current_target, None);
        s.set_target(Some(99));
        assert_eq!(s.current_target, Some(99));
        s.set_target(None);
        assert_eq!(s.current_target, None);
    }
}
