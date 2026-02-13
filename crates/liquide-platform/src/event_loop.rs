//! Platform event loop types and abstractions.
//!
//! Defines the [`PlatformEvent`] enum for all events dispatched by the
//! native windowing system, and [`ControlFlow`] for controlling the
//! event loop lifecycle.

use liquide_compositor::pixel::PixelFormat;
use liquide_input::keyboard::KeyEvent;
use liquide_input::mouse::MouseEvent;
use liquide_input::touch::TouchEvent;

use crate::window_host::NativeWindowHandle;
use crate::PlatformResult;

/// Events dispatched by the platform event loop.
#[derive(Debug, Clone)]
pub enum PlatformEvent {
    /// A window has been created and is ready.
    WindowCreated {
        handle: NativeWindowHandle,
        width: u32,
        height: u32,
    },

    /// The user requested a window to close (e.g. clicked the X button).
    WindowCloseRequested {
        handle: NativeWindowHandle,
    },

    /// A window was destroyed by the platform.
    WindowDestroyed {
        handle: NativeWindowHandle,
    },

    /// A window was resized.
    WindowResized {
        handle: NativeWindowHandle,
        width: u32,
        height: u32,
    },

    /// A window was moved.
    WindowMoved {
        handle: NativeWindowHandle,
        x: i32,
        y: i32,
    },

    /// A window needs to be repainted.
    WindowRedraw {
        handle: NativeWindowHandle,
    },

    /// A window gained keyboard focus.
    FocusGained {
        handle: NativeWindowHandle,
    },

    /// A window lost keyboard focus.
    FocusLost {
        handle: NativeWindowHandle,
    },

    /// Keyboard input event.
    KeyInput {
        handle: NativeWindowHandle,
        event: KeyEvent,
    },

    /// Mouse / pointer input event.
    MouseInput {
        handle: NativeWindowHandle,
        event: MouseEvent,
    },

    /// Touch input event.
    TouchInput {
        handle: NativeWindowHandle,
        event: TouchEvent,
    },

    /// The DPI scaling factor changed for a window (e.g. moved to another
    /// monitor).
    DpiChanged {
        handle: NativeWindowHandle,
        dpi_scale: f32,
    },

    /// A window was minimized.
    WindowMinimized {
        handle: NativeWindowHandle,
    },

    /// A window was maximized.
    WindowMaximized {
        handle: NativeWindowHandle,
    },

    /// A window was restored from minimized / maximized state.
    WindowRestored {
        handle: NativeWindowHandle,
    },

    /// A file was dropped onto a window.
    FileDrop {
        handle: NativeWindowHandle,
        paths: Vec<String>,
    },

    /// The application should quit.
    Quit,
}

/// Instruction to the event loop on how to proceed after processing an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlFlow {
    /// Continue processing events.
    Continue,
    /// Exit the event loop.
    Exit,
}

/// Trait for presenting rendered pixel buffers to a native window.
///
/// Each platform backend implements this to blit compositor output
/// onto the actual display surface.
pub trait FramePresenter: Send {
    /// Present a rendered frame to the specified window.
    ///
    /// The pixel data is in the given format, with `stride` bytes per row.
    /// The implementation copies the pixel data to the window's display
    /// surface using the platform's fastest available mechanism.
    fn present_frame(
        &mut self,
        handle: NativeWindowHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        format: PixelFormat,
    ) -> PlatformResult<()>;

    /// Check whether a pixel format is supported for presentation.
    fn supports_format(&self, format: PixelFormat) -> bool;
}

/// A null frame presenter that discards all frames.
#[derive(Debug, Default)]
pub struct NullFramePresenter;

impl FramePresenter for NullFramePresenter {
    fn present_frame(
        &mut self,
        _handle: NativeWindowHandle,
        _pixels: &[u8],
        _width: u32,
        _height: u32,
        _stride: u32,
        _format: PixelFormat,
    ) -> PlatformResult<()> {
        Ok(())
    }

    fn supports_format(&self, _format: PixelFormat) -> bool {
        true
    }
}
