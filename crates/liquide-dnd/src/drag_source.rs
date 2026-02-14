//! Drag source API.
//!
//! A drag source initiates a drag operation from a widget. It specifies
//! what data is being dragged, which actions are allowed, and optionally
//! provides a drag image.

use crate::data_transfer::DataTransfer;
use serde::{Deserialize, Serialize};

/// Allowed/requested drag actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DragAction {
    /// No action.
    None,
    /// Copy data.
    Copy,
    /// Move data (source should delete after drop).
    Move,
    /// Create a link/reference.
    Link,
}

/// Drag image shown under the cursor during drag.
#[derive(Debug, Clone)]
pub struct DragImage {
    /// Image data (RGBA pixels).
    pub pixels: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Hotspot X offset from top-left.
    pub hotspot_x: i32,
    /// Hotspot Y offset from top-left.
    pub hotspot_y: i32,
}

/// A drag operation in progress.
#[derive(Debug, Clone)]
pub struct DragOperation {
    /// The data being dragged.
    pub data: DataTransfer,
    /// Allowed actions.
    pub allowed_actions: Vec<DragAction>,
    /// Optional drag image.
    pub drag_image: Option<DragImage>,
    /// Current negotiated action.
    pub current_action: DragAction,
    /// Whether the drag is still in progress.
    pub active: bool,
    /// Start position (in window coords).
    pub start_x: f32,
    pub start_y: f32,
}

/// Events emitted by a drag source.
#[derive(Debug, Clone)]
pub enum DragSourceEvent {
    /// Drag started.
    DragStarted { x: f32, y: f32 },
    /// Drag moved (e.g., for visual feedback).
    DragMoved { x: f32, y: f32 },
    /// Target accepted a particular action.
    ActionChanged { action: DragAction },
    /// Drag ended (completed or cancelled).
    DragEnded { action: DragAction },
}

/// Manages drag initiation from a widget.
pub struct DragSource {
    /// Minimum pixel distance before drag starts (avoids accidental drags).
    pub drag_threshold: f32,
    /// Current operation (if any).
    operation: Option<DragOperation>,
    /// Pending events.
    events: Vec<DragSourceEvent>,
}

impl DragSource {
    #[must_use]
    pub fn new() -> Self {
        Self {
            drag_threshold: 5.0,
            operation: None,
            events: Vec::new(),
        }
    }

    /// Begin a drag operation.
    pub fn start_drag(
        &mut self,
        data: DataTransfer,
        allowed_actions: Vec<DragAction>,
        drag_image: Option<DragImage>,
        x: f32,
        y: f32,
    ) {
        let op = DragOperation {
            data,
            allowed_actions,
            drag_image,
            current_action: DragAction::None,
            active: true,
            start_x: x,
            start_y: y,
        };
        self.operation = Some(op);
        self.events.push(DragSourceEvent::DragStarted { x, y });
    }

    /// Update drag position.
    pub fn update_position(&mut self, x: f32, y: f32) {
        if self.operation.is_some() {
            self.events.push(DragSourceEvent::DragMoved { x, y });
        }
    }

    /// Set the negotiated action (called by DnD coordinator).
    pub fn set_action(&mut self, action: DragAction) {
        if let Some(op) = &mut self.operation {
            op.current_action = action;
            self.events.push(DragSourceEvent::ActionChanged { action });
        }
    }

    /// End the drag operation.
    pub fn end_drag(&mut self, action: DragAction) {
        if let Some(mut op) = self.operation.take() {
            op.active = false;
            op.current_action = action;
            self.events.push(DragSourceEvent::DragEnded { action });
        }
    }

    /// Cancel the drag operation.
    pub fn cancel(&mut self) {
        self.end_drag(DragAction::None);
    }

    /// Whether a drag is active.
    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.operation.as_ref().is_some_and(|op| op.active)
    }

    /// Get the current operation.
    #[must_use]
    pub fn operation(&self) -> Option<&DragOperation> {
        self.operation.as_ref()
    }

    /// Drain pending events.
    pub fn drain_events(&mut self) -> Vec<DragSourceEvent> {
        std::mem::take(&mut self.events)
    }
}

impl Default for DragSource {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_transfer::DataPayload;

    fn test_transfer() -> DataTransfer {
        let mut dt = DataTransfer::new();
        dt.add(DataPayload::text("hello"));
        dt
    }

    #[test]
    fn test_drag_lifecycle() {
        let mut ds = DragSource::new();
        assert!(!ds.is_dragging());

        ds.start_drag(
            test_transfer(),
            vec![DragAction::Copy, DragAction::Move],
            None,
            10.0,
            20.0,
        );
        assert!(ds.is_dragging());

        ds.update_position(15.0, 25.0);
        ds.set_action(DragAction::Copy);
        ds.end_drag(DragAction::Copy);
        assert!(!ds.is_dragging());

        let events = ds.drain_events();
        assert_eq!(events.len(), 4); // started, moved, action_changed, ended
    }

    #[test]
    fn test_cancel() {
        let mut ds = DragSource::new();
        ds.start_drag(test_transfer(), vec![DragAction::Copy], None, 0.0, 0.0);
        ds.cancel();
        assert!(!ds.is_dragging());
    }
}
