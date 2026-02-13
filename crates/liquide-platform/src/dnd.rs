//! Drag-and-drop support.
//!
//! Provides the [`NativeDragDrop`] trait for initiating and accepting
//! drag-and-drop operations, and a [`NullDragDrop`] for testing.

use crate::PlatformResult;

/// Backend for native drag-and-drop operations.
pub trait NativeDragDrop: Send {
    /// Begin a drag operation with the given MIME types and payload.
    fn start_drag(&mut self, mime_types: &[String], data: &[u8]) -> PlatformResult<()>;

    /// Check whether a drop with the given MIME type can be accepted.
    fn accept_drop(&mut self, mime_type: &str) -> PlatformResult<bool>;

    /// Cancel the current drag operation.
    fn cancel_drag(&mut self) -> PlatformResult<()>;
}

/// A [`NativeDragDrop`] that accepts all operations as no-ops.
#[derive(Debug, Default)]
pub struct NullDragDrop;

impl NativeDragDrop for NullDragDrop {
    fn start_drag(&mut self, _mime_types: &[String], _data: &[u8]) -> PlatformResult<()> {
        Ok(())
    }

    fn accept_drop(&mut self, _mime_type: &str) -> PlatformResult<bool> {
        Ok(true)
    }

    fn cancel_drag(&mut self) -> PlatformResult<()> {
        Ok(())
    }
}
