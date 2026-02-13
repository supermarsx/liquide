//! Native window creation and management.
//!
//! Provides the [`NativeWindowHost`] trait for creating and manipulating
//! platform windows, and a [`NullWindowHost`] for testing.

use liquide_compositor::geometry::Rect;
use serde::{Deserialize, Serialize};

use crate::PlatformResult;

/// An opaque handle to a native platform window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NativeWindowHandle(pub u64);

/// Parameters for creating a new native window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeWindowParams {
    /// Window title text.
    pub title: String,
    /// Initial position and size.
    pub geometry: Rect,
    /// Window type hint (e.g. "normal", "dialog", "splash").
    pub window_type: String,
    /// Optional parent window for transient / child windows.
    pub parent: Option<NativeWindowHandle>,
    /// Application identifier (e.g. reverse-DNS name).
    pub app_id: String,
}

/// Backend for creating and manipulating native platform windows.
pub trait NativeWindowHost: Send {
    /// Create a new native window and return its handle.
    fn create_window(&mut self, params: NativeWindowParams) -> PlatformResult<NativeWindowHandle>;

    /// Destroy a previously created window.
    fn destroy_window(&mut self, handle: NativeWindowHandle) -> PlatformResult<()>;

    /// Set the position and size of a window.
    fn set_geometry(&mut self, handle: NativeWindowHandle, geometry: Rect) -> PlatformResult<()>;

    /// Set the title bar text of a window.
    fn set_title(&mut self, handle: NativeWindowHandle, title: &str) -> PlatformResult<()>;

    /// Set the window icon from raw image data.
    fn set_icon(&mut self, handle: NativeWindowHandle, icon_data: &[u8]) -> PlatformResult<()>;

    /// Set the window state (e.g. "maximized", "minimized", "fullscreen").
    fn set_state(&mut self, handle: NativeWindowHandle, state: &str) -> PlatformResult<()>;

    /// Set the Z-order (stacking level) of a window.
    fn set_z_order(&mut self, handle: NativeWindowHandle, z_order: i32) -> PlatformResult<()>;

    /// Request input focus for a window.
    fn set_focus(&mut self, handle: NativeWindowHandle) -> PlatformResult<()>;
}

/// A [`NativeWindowHost`] that tracks windows in memory without
/// creating real platform windows.
#[derive(Debug, Default)]
pub struct NullWindowHost {
    windows: Vec<NativeWindowHandle>,
    next_handle: u64,
}

impl NullWindowHost {
    /// Create a new null window host.
    #[must_use]
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            next_handle: 1,
        }
    }
}

impl NativeWindowHost for NullWindowHost {
    fn create_window(&mut self, _params: NativeWindowParams) -> PlatformResult<NativeWindowHandle> {
        let handle = NativeWindowHandle(self.next_handle);
        self.next_handle += 1;
        self.windows.push(handle);
        Ok(handle)
    }

    fn destroy_window(&mut self, handle: NativeWindowHandle) -> PlatformResult<()> {
        self.windows.retain(|h| *h != handle);
        Ok(())
    }

    fn set_geometry(&mut self, _handle: NativeWindowHandle, _geometry: Rect) -> PlatformResult<()> {
        Ok(())
    }

    fn set_title(&mut self, _handle: NativeWindowHandle, _title: &str) -> PlatformResult<()> {
        Ok(())
    }

    fn set_icon(&mut self, _handle: NativeWindowHandle, _icon_data: &[u8]) -> PlatformResult<()> {
        Ok(())
    }

    fn set_state(&mut self, _handle: NativeWindowHandle, _state: &str) -> PlatformResult<()> {
        Ok(())
    }

    fn set_z_order(&mut self, _handle: NativeWindowHandle, _z_order: i32) -> PlatformResult<()> {
        Ok(())
    }

    fn set_focus(&mut self, _handle: NativeWindowHandle) -> PlatformResult<()> {
        Ok(())
    }
}
