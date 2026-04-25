//! Drag-and-drop support for the Liquide UI toolkit.
//!
//! This crate provides:
//! - **Data transfer** via MIME-typed payloads ([`data_transfer`])
//! - **Typed drag data** via multi-format payloads ([`drag_data`])
//! - **Drag sources** for initiating drag operations ([`drag_source`])
//! - **Drop targets** for accepting dropped data ([`drop_target`])
//! - **Drag/drop traits** for widgets and windows ([`traits`])
//! - **Drag preview** visuals ([`preview`])
//! - **Drag session** state tracking ([`session`])
//! - **Drag manager** — central coordinator ([`manager`])
//! - **Auto-scroll** when dragging near edges ([`auto_scroll`])
//! - **Spring-loaded folders** for auto-opening on hover ([`spring_loading`])
//!
//! The implementation is platform-agnostic; platform bridges translate
//! between native DnD protocols (X11 XDND, Wayland, Win32 OLE, etc.)
//! and this unified API.

pub mod auto_scroll;
pub mod cursor_link;
pub mod data_transfer;
pub mod drag_data;
pub mod drag_source;
pub mod drop_target;
pub mod manager;
pub mod preview;
pub mod session;
pub mod spring_loading;
pub mod traits;

pub use auto_scroll::{
    AutoScrollConfig, AutoScrollState, AutoScrollZone, ScrollBounds, ScrollDelta, ScrollDirection,
};
pub use cursor_link::{BufferedCursorLink, CursorLink, DndCursorShape, NullCursorLink};
pub use data_transfer::{DataPayload, DataTransfer, MimeType};
pub use drag_data::{DragData, DragDataStore, DragFormat};
pub use drag_source::{DragAction, DragImage, DragOperation, DragSource, DragSourceEvent};
pub use drop_target::{
    DropAction, DropEffect, DropIndicator, DropResult, DropTarget, DropTargetEvent,
    DropTargetRegion, DropTargetRegistry,
};
pub use manager::{DragEvent, DragManager};
pub use preview::{DragPreview, DragPreviewConfig, DragPreviewStyle, PreviewRect};
pub use session::DragSession;
pub use spring_loading::{SpringLoadAction, SpringLoadConfig, SpringLoadState};
pub use traits::{DragSourceHandler, DropTargetHandler, SimpleDragSource, SimpleDropTarget};

use thiserror::Error;

/// Errors that can occur during drag-and-drop operations.
#[derive(Debug, Error)]
pub enum DndError {
    #[error("drag operation cancelled")]
    Cancelled,
    #[error("no compatible data format")]
    IncompatibleFormat,
    #[error("platform DnD not available: {0}")]
    PlatformUnavailable(String),
    #[error("data transfer error: {0}")]
    TransferError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let e = DndError::Cancelled;
        assert_eq!(format!("{e}"), "drag operation cancelled");
    }

    #[test]
    fn test_reexports_accessible() {
        // Verify key types are accessible through the crate root
        let _data = DragData::text("test");
        let _preview = DragPreview::text_label("test");
        let _mgr = DragManager::new();
        let _zone = AutoScrollZone::default();
        let _effect = DropEffect::Copy;
        let _format = DragFormat::Text("hi".to_string());

        // New re-exports
        let _cfg = AutoScrollConfig::default();
        let _state = AutoScrollState::with_defaults();
        let _delta = ScrollDelta::new(0.0, 0.0);
        let _store = DragDataStore::new();
        let _pcfg = DragPreviewConfig::new(DragPreview::icon("test"));
        let _style = DragPreviewStyle::Icon;
        let _rect = PreviewRect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        let _result = DropResult::Rejected;
        let _action = DropAction::Copy;
        let _indicator = DropIndicator::None;
        let _region = DropTargetRegion::new(1, 0.0, 0.0, 10.0, 10.0, vec![]);
        let _registry = DropTargetRegistry::new();
        let _spring_cfg = SpringLoadConfig::default();
        let _spring_state = SpringLoadState::with_defaults();
        let _spring_action = SpringLoadAction::CancelOpen;
    }
}
