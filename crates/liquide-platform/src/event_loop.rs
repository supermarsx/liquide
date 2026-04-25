//! Platform event loop types and abstractions.
//!
//! Defines the [`PlatformEvent`] enum for all events dispatched by the
//! native windowing system, and [`ControlFlow`] for controlling the
//! event loop lifecycle.

use liquide_compositor::pixel::PixelFormat;
use liquide_input::keyboard::KeyEvent;
use liquide_input::mouse::MouseEvent;
use liquide_input::touch::TouchEvent;

use crate::PlatformResult;
use crate::window_host::NativeWindowHandle;

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
    WindowCloseRequested { handle: NativeWindowHandle },

    /// A window was destroyed by the platform.
    WindowDestroyed { handle: NativeWindowHandle },

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
    WindowRedraw { handle: NativeWindowHandle },

    /// A window gained keyboard focus.
    FocusGained { handle: NativeWindowHandle },

    /// A window lost keyboard focus.
    FocusLost { handle: NativeWindowHandle },

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
    WindowMinimized { handle: NativeWindowHandle },

    /// A window was maximized.
    WindowMaximized { handle: NativeWindowHandle },

    /// A window was restored from minimized / maximized state.
    WindowRestored { handle: NativeWindowHandle },

    /// A file was dropped onto a window.
    FileDrop {
        handle: NativeWindowHandle,
        paths: Vec<String>,
    },

    /// The system color scheme (light/dark mode) changed.
    ColorSchemeChanged { scheme: crate::ColorScheme },

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_flow_continue_eq() {
        assert_eq!(ControlFlow::Continue, ControlFlow::Continue);
        assert_ne!(ControlFlow::Continue, ControlFlow::Exit);
    }

    #[test]
    fn control_flow_exit_eq() {
        assert_eq!(ControlFlow::Exit, ControlFlow::Exit);
    }

    #[test]
    fn null_frame_presenter_supports_bgra8() {
        let presenter = NullFramePresenter;
        assert!(presenter.supports_format(PixelFormat::Bgra8));
    }

    #[test]
    fn null_frame_presenter_supports_rgba8() {
        let presenter = NullFramePresenter;
        assert!(presenter.supports_format(PixelFormat::Rgba8));
    }

    #[test]
    fn null_frame_presenter_present_ok() {
        let mut presenter = NullFramePresenter;
        let handle = NativeWindowHandle(1);
        let pixels = [0u8; 16];
        let result = presenter.present_frame(handle, &pixels, 2, 2, 8, PixelFormat::Bgra8);
        assert!(result.is_ok());
    }

    #[test]
    fn null_frame_presenter_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<NullFramePresenter>();
    }

    #[test]
    fn null_frame_presenter_debug() {
        let presenter = NullFramePresenter;
        let debug = format!("{presenter:?}");
        assert!(debug.contains("NullFramePresenter"));
    }

    #[test]
    fn platform_event_quit_debug() {
        let event = PlatformEvent::Quit;
        let debug = format!("{event:?}");
        assert!(debug.contains("Quit"));
    }

    #[test]
    fn platform_event_window_created_fields() {
        let handle = NativeWindowHandle(42);
        let event = PlatformEvent::WindowCreated {
            handle,
            width: 800,
            height: 600,
        };
        if let PlatformEvent::WindowCreated {
            handle: h,
            width,
            height,
        } = event
        {
            assert_eq!(h.0, 42);
            assert_eq!(width, 800);
            assert_eq!(height, 600);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn platform_event_dpi_changed() {
        let event = PlatformEvent::DpiChanged {
            handle: NativeWindowHandle(1),
            dpi_scale: 2.0,
        };
        if let PlatformEvent::DpiChanged { dpi_scale, .. } = event {
            assert!((dpi_scale - 2.0).abs() < f32::EPSILON);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn platform_event_file_drop() {
        let event = PlatformEvent::FileDrop {
            handle: NativeWindowHandle(1),
            paths: vec!["/tmp/test.txt".to_string(), "/tmp/other.txt".to_string()],
        };
        if let PlatformEvent::FileDrop { paths, .. } = event {
            assert_eq!(paths.len(), 2);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn platform_event_clone() {
        let event = PlatformEvent::WindowResized {
            handle: NativeWindowHandle(5),
            width: 1024,
            height: 768,
        };
        let cloned = event.clone();
        if let PlatformEvent::WindowResized {
            handle,
            width,
            height,
        } = cloned
        {
            assert_eq!(handle.0, 5);
            assert_eq!(width, 1024);
            assert_eq!(height, 768);
        } else {
            panic!("wrong variant");
        }
    }
}
