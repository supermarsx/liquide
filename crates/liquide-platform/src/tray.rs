//! System tray / notification area icons.
//!
//! Provides the [`NativeTray`] trait for managing tray icons and a
//! [`NullNativeTray`] that tracks handles in memory for testing.

use serde::{Deserialize, Serialize};

use crate::PlatformResult;

/// An opaque handle to a system tray icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NativeTrayHandle(pub u64);

/// Parameters for creating a new tray icon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeTrayParams {
    /// Tooltip text shown on hover.
    pub tooltip: String,
    /// Raw image data for the tray icon.
    pub icon_data: Vec<u8>,
    /// Labels for the right-click context menu.
    pub menu: Vec<String>,
}

/// Partial update to an existing tray icon.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrayUpdate {
    /// New tooltip text, if changing.
    pub tooltip: Option<String>,
    /// New icon data, if changing.
    pub icon_data: Option<Vec<u8>>,
}

/// Backend for system tray icon management.
pub trait NativeTray: Send {
    /// Add a new icon to the system tray and return its handle.
    fn add_icon(&mut self, params: NativeTrayParams) -> PlatformResult<NativeTrayHandle>;

    /// Update an existing tray icon.
    fn update_icon(&mut self, handle: NativeTrayHandle, update: TrayUpdate) -> PlatformResult<()>;

    /// Remove an icon from the system tray.
    fn remove_icon(&mut self, handle: NativeTrayHandle) -> PlatformResult<()>;
}

/// A [`NativeTray`] that tracks handles in memory without creating
/// real tray icons.
#[derive(Debug, Default)]
pub struct NullNativeTray {
    handles: Vec<NativeTrayHandle>,
    next_handle: u64,
}

impl NullNativeTray {
    /// Create a new null tray backend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handles: Vec::new(),
            next_handle: 1,
        }
    }
}

impl NativeTray for NullNativeTray {
    fn add_icon(&mut self, _params: NativeTrayParams) -> PlatformResult<NativeTrayHandle> {
        let handle = NativeTrayHandle(self.next_handle);
        self.next_handle += 1;
        self.handles.push(handle);
        Ok(handle)
    }

    fn update_icon(
        &mut self,
        _handle: NativeTrayHandle,
        _update: TrayUpdate,
    ) -> PlatformResult<()> {
        Ok(())
    }

    fn remove_icon(&mut self, handle: NativeTrayHandle) -> PlatformResult<()> {
        self.handles.retain(|h| *h != handle);
        Ok(())
    }
}
