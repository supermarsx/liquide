//! Drop target API.
//!
//! A drop target is a widget region that can accept data from a drag operation.
//! It inspects offered MIME types to decide whether to accept, and specifies
//! the resulting effect (copy/move/link).

use crate::data_transfer::{DataTransfer, MimeType};
use crate::drag_source::DragAction;
use serde::{Deserialize, Serialize};

/// The visual effect shown when hovering over a drop target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DropEffect {
    None,
    Copy,
    Move,
    Link,
}

impl From<DragAction> for DropEffect {
    fn from(action: DragAction) -> Self {
        match action {
            DragAction::None => DropEffect::None,
            DragAction::Copy => DropEffect::Copy,
            DragAction::Move => DropEffect::Move,
            DragAction::Link => DropEffect::Link,
        }
    }
}

/// Events received by a drop target.
#[derive(Debug, Clone)]
pub enum DropTargetEvent {
    /// Drag entered the target area.
    DragEnter {
        /// MIME types offered by the source.
        offered_types: Vec<MimeType>,
        x: f32,
        y: f32,
    },
    /// Drag moved within the target area.
    DragOver {
        x: f32,
        y: f32,
    },
    /// Drag left the target area.
    DragLeave,
    /// Data was dropped.
    Drop {
        data: DataTransfer,
        action: DragAction,
        x: f32,
        y: f32,
    },
}

/// A drop target that can accept dragged data.
pub struct DropTarget {
    /// MIME types this target accepts.
    accepted_types: Vec<String>,
    /// Whether a drag is currently over this target.
    drag_over: bool,
    /// Current drop effect.
    effect: DropEffect,
    /// Whether the target is enabled.
    enabled: bool,
    /// Pending events.
    events: Vec<DropTargetEvent>,
}

impl DropTarget {
    #[must_use]
    pub fn new(accepted_types: Vec<String>) -> Self {
        Self {
            accepted_types,
            drag_over: false,
            effect: DropEffect::None,
            enabled: true,
            events: Vec::new(),
        }
    }

    /// Create a target that accepts text.
    #[must_use]
    pub fn text() -> Self {
        Self::new(vec![MimeType::TEXT_PLAIN.to_string()])
    }

    /// Create a target that accepts files (URI list).
    #[must_use]
    pub fn files() -> Self {
        Self::new(vec![MimeType::TEXT_URI_LIST.to_string()])
    }

    /// Set whether the drop target is enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if a drag is currently over this target.
    #[must_use]
    pub fn is_drag_over(&self) -> bool {
        self.drag_over
    }

    /// Get the current drop effect.
    #[must_use]
    pub fn effect(&self) -> DropEffect {
        self.effect
    }

    /// Test whether this target can accept any of the offered types.
    #[must_use]
    pub fn can_accept(&self, offered: &[MimeType]) -> bool {
        if !self.enabled {
            return false;
        }
        offered
            .iter()
            .any(|m| self.accepted_types.iter().any(|a| *a == m.0))
    }

    /// Handle drag entering the target area.
    pub fn handle_drag_enter(&mut self, offered_types: Vec<MimeType>, x: f32, y: f32) -> DropEffect {
        if self.can_accept(&offered_types) {
            self.drag_over = true;
            self.effect = DropEffect::Copy;
            self.events.push(DropTargetEvent::DragEnter {
                offered_types,
                x,
                y,
            });
            DropEffect::Copy
        } else {
            self.effect = DropEffect::None;
            DropEffect::None
        }
    }

    /// Handle drag moving over the target.
    pub fn handle_drag_over(&mut self, x: f32, y: f32) -> DropEffect {
        if self.drag_over {
            self.events.push(DropTargetEvent::DragOver { x, y });
        }
        self.effect
    }

    /// Handle drag leaving the target.
    pub fn handle_drag_leave(&mut self) {
        self.drag_over = false;
        self.effect = DropEffect::None;
        self.events.push(DropTargetEvent::DragLeave);
    }

    /// Handle a drop.
    pub fn handle_drop(&mut self, data: DataTransfer, action: DragAction, x: f32, y: f32) -> bool {
        let accepted = self.drag_over;
        self.drag_over = false;
        self.effect = DropEffect::None;

        if accepted {
            self.events.push(DropTargetEvent::Drop {
                data,
                action,
                x,
                y,
            });
        }
        accepted
    }

    /// Drain pending events.
    pub fn drain_events(&mut self) -> Vec<DropTargetEvent> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_transfer::DataPayload;

    #[test]
    fn test_accept_check() {
        let target = DropTarget::text();
        assert!(target.can_accept(&[MimeType::text_plain()]));
        assert!(!target.can_accept(&[MimeType::text_html()]));
    }

    #[test]
    fn test_drag_enter_leave() {
        let mut target = DropTarget::text();
        let eff = target.handle_drag_enter(vec![MimeType::text_plain()], 10.0, 20.0);
        assert_eq!(eff, DropEffect::Copy);
        assert!(target.is_drag_over());

        target.handle_drag_leave();
        assert!(!target.is_drag_over());
    }

    #[test]
    fn test_drop_accepted() {
        let mut target = DropTarget::text();
        target.handle_drag_enter(vec![MimeType::text_plain()], 5.0, 5.0);

        let mut data = DataTransfer::new();
        data.add(DataPayload::text("dropped text"));
        let accepted = target.handle_drop(data, DragAction::Copy, 5.0, 5.0);
        assert!(accepted);
        assert!(!target.is_drag_over());
    }

    #[test]
    fn test_disabled_target() {
        let mut target = DropTarget::text();
        target.set_enabled(false);
        assert!(!target.can_accept(&[MimeType::text_plain()]));
    }
}
