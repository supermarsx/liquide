//! Drag-and-drop support for the Liquide UI toolkit.
//!
//! This crate provides:
//! - **Data transfer** via MIME-typed payloads
//! - **Drag sources** for initiating drag operations
//! - **Drop targets** for accepting dropped data
//!
//! The implementation is platform-agnostic; platform bridges translate
//! between native DnD protocols (X11 XDND, Wayland, Win32 OLE, etc.)
//! and this unified API.

pub mod data_transfer;
pub mod drag_source;
pub mod drop_target;

pub use data_transfer::{DataPayload, DataTransfer, MimeType};
pub use drag_source::{DragAction, DragImage, DragOperation, DragSource, DragSourceEvent};
pub use drop_target::{DropEffect, DropTarget, DropTargetEvent};

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
}
