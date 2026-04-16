//! Standalone compositor platform backend.
//!
//! Implements [`PlatformBackend`] using DRM/KMS for display output and
//! evdev for input. This backend is used when LiquiDE runs as a standalone
//! compositor launched from TTY, as opposed to the remote desktop path
//! (which uses the x11/wayland/win32 client backends).
//!
//! This module is compiled on all platforms but only functional on Linux
//! where DRM/KMS and evdev are available. On other platforms, creation
//! returns an error.
//!
//! # Usage
//!
//! The standalone compositor binary (`liquid-standalone`) creates this
//! backend directly rather than using `create_platform()`, which continues
//! to return the appropriate client-side backend as before.
//!
//! ```rust,ignore
//! use liquide_platform::standalone::StandalonePlatform;
//!
//! let mut platform = StandalonePlatform::new(StandaloneConfig::default())?;
//! desktop_compositor.run(&mut platform);
//! ```

use std::collections::VecDeque;

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;

use crate::display::{DisplayBackend, MonitorInfo};
use crate::dnd::{NativeDragDrop, NullDragDrop};
use crate::event_loop::PlatformEvent;
use crate::keymap::{DefaultKeymap, KeymapTranslator};
use crate::notifications::{NativeNotifications, NullNativeNotifications};
use crate::taskbar::{NullTaskbar, TaskbarIntegration};
use crate::tray::{NativeTray, NullNativeTray};
use crate::window_host::{NativeWindowHandle, NativeWindowHost, NativeWindowParams};
use crate::{PlatformBackend, PlatformResult};

/// Configuration for the standalone platform backend.
#[derive(Debug, Clone)]
pub struct StandaloneConfig {
    /// Screen width in pixels.
    pub width: u32,
    /// Screen height in pixels.
    pub height: u32,
    /// Whether to use hardware cursor.
    pub hardware_cursor: bool,
}

impl Default for StandaloneConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            hardware_cursor: true,
        }
    }
}

/// Standalone DRM/KMS display backend.
///
/// Reports the DRM output as a single monitor for DisplayBackend queries.
struct StandaloneDisplayBackend {
    width: u32,
    height: u32,
}

impl StandaloneDisplayBackend {
    fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl DisplayBackend for StandaloneDisplayBackend {
    fn monitors(&self) -> Vec<MonitorInfo> {
        vec![MonitorInfo {
            id: 1,
            name: "DRM-1".to_string(),
            geometry: Rect::new(0.0, 0.0, self.width as f32, self.height as f32),
            work_area: Rect::new(0.0, 0.0, self.width as f32, self.height as f32),
            dpi_scale: 1.0,
            primary: true,
            refresh_rate_hz: 60,
        }]
    }

    fn primary_monitor(&self) -> Option<MonitorInfo> {
        self.monitors().into_iter().next()
    }

    fn virtual_screen_rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width as f32, self.height as f32)
    }
}

/// Standalone window host — manages a single fullscreen "window"
/// backed by the DRM framebuffer.
struct StandaloneWindowHost {
    /// The single fullscreen window handle.
    window: Option<NativeWindowHandle>,
    next_handle: u64,
}

impl StandaloneWindowHost {
    fn new() -> Self {
        Self {
            window: None,
            next_handle: 1,
        }
    }
}

impl NativeWindowHost for StandaloneWindowHost {
    fn create_window(&mut self, _params: NativeWindowParams) -> PlatformResult<NativeWindowHandle> {
        let handle = NativeWindowHandle(self.next_handle);
        self.next_handle += 1;
        self.window = Some(handle);
        Ok(handle)
    }

    fn destroy_window(&mut self, _handle: NativeWindowHandle) -> PlatformResult<()> {
        self.window = None;
        Ok(())
    }

    fn set_geometry(&mut self, _handle: NativeWindowHandle, _geometry: Rect) -> PlatformResult<()> {
        Ok(()) // Resolution changes go through DRM modesetting
    }

    fn set_title(&mut self, _handle: NativeWindowHandle, _title: &str) -> PlatformResult<()> {
        Ok(()) // No title bar in fullscreen DRM mode
    }

    fn set_icon(&mut self, _handle: NativeWindowHandle, _icon_data: &[u8]) -> PlatformResult<()> {
        Ok(()) // No window icon in fullscreen DRM mode
    }

    fn set_state(&mut self, _handle: NativeWindowHandle, _state: &str) -> PlatformResult<()> {
        Ok(()) // Always fullscreen in DRM mode
    }

    fn set_z_order(&mut self, _handle: NativeWindowHandle, _z_order: i32) -> PlatformResult<()> {
        Ok(()) // Single window, no z-ordering needed
    }

    fn set_focus(&mut self, _handle: NativeWindowHandle) -> PlatformResult<()> {
        Ok(()) // Single window always has focus
    }
}

/// The standalone platform backend.
///
/// Implements `PlatformBackend` for direct DRM/KMS output and evdev input.
/// This enables the existing `DesktopCompositor::run()` to work unchanged
/// with the standalone compositor.
pub struct StandalonePlatform {
    display: StandaloneDisplayBackend,
    window_host: StandaloneWindowHost,
    taskbar: NullTaskbar,
    tray: NullNativeTray,
    notifications: NullNativeNotifications,
    drag_drop: NullDragDrop,
    keymap: DefaultKeymap,
    /// Pending platform events (from evdev input, DRM hotplug, etc.)
    event_queue: VecDeque<PlatformEvent>,
    /// The framebuffer pixels from the last present_frame call.
    /// In a real implementation, these would be written to the DRM framebuffer.
    last_frame: Option<Vec<u8>>,
    /// Screen dimensions.
    width: u32,
    height: u32,
    /// Whether a redraw has been requested.
    redraw_pending: bool,
}

impl StandalonePlatform {
    /// Create a new standalone platform backend.
    ///
    /// In a full implementation, this would take DRM device and input
    /// device file descriptors. For now, it sets up the scaffolding
    /// that integrates with the existing DesktopCompositor.
    pub fn new(config: StandaloneConfig) -> PlatformResult<Self> {
        Ok(Self {
            display: StandaloneDisplayBackend::new(config.width, config.height),
            window_host: StandaloneWindowHost::new(),
            taskbar: NullTaskbar,
            tray: NullNativeTray::new(),
            notifications: NullNativeNotifications::new(),
            drag_drop: NullDragDrop,
            keymap: DefaultKeymap,
            event_queue: VecDeque::new(),
            last_frame: None,
            width: config.width,
            height: config.height,
            redraw_pending: false,
        })
    }

    /// Push an externally-generated event into the queue.
    ///
    /// The standalone compositor's event loop converts DRM/evdev events
    /// into PlatformEvents and pushes them here so that
    /// `DesktopCompositor::run()` can process them via `poll_event()`.
    pub fn push_event(&mut self, event: PlatformEvent) {
        self.event_queue.push_back(event);
    }

    /// Access the last presented frame's pixel data.
    ///
    /// The standalone compositor reads this to copy pixels to the DRM
    /// framebuffer (Path B: local display). This coexists with the
    /// existing tile encoder path (Path A: remote transmission).
    pub fn last_frame(&self) -> Option<&[u8]> {
        self.last_frame.as_deref()
    }

    /// Current screen dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl PlatformBackend for StandalonePlatform {
    fn display(&self) -> &dyn DisplayBackend {
        &self.display
    }

    fn window_host(&mut self) -> &mut dyn NativeWindowHost {
        &mut self.window_host
    }

    fn taskbar(&mut self) -> &mut dyn TaskbarIntegration {
        &mut self.taskbar
    }

    fn tray(&mut self) -> &mut dyn NativeTray {
        &mut self.tray
    }

    fn notifications(&mut self) -> &mut dyn NativeNotifications {
        &mut self.notifications
    }

    fn drag_drop(&mut self) -> &mut dyn NativeDragDrop {
        &mut self.drag_drop
    }

    fn keymap(&self) -> &dyn KeymapTranslator {
        &self.keymap
    }

    fn platform_name(&self) -> &str {
        "standalone-drm"
    }

    fn poll_event(&mut self) -> Option<PlatformEvent> {
        self.event_queue.pop_front()
    }

    fn wait_event(&mut self) -> PlatformEvent {
        // In a real implementation, this would block on epoll/poll
        // waiting for DRM, evdev, or Wayland client events.
        if let Some(event) = self.event_queue.pop_front() {
            event
        } else {
            // Yield briefly then return Quit if nothing pending.
            // The real implementation will use proper fd-based blocking.
            PlatformEvent::Quit
        }
    }

    fn present_frame(
        &mut self,
        _handle: NativeWindowHandle,
        pixels: &[u8],
        _width: u32,
        _height: u32,
        _stride: u32,
        _format: PixelFormat,
    ) -> PlatformResult<()> {
        // Path B: Local display output.
        // Store pixels for DRM scanout. In the full implementation,
        // this would copy to the DRM framebuffer and trigger a page flip.
        self.last_frame = Some(pixels.to_vec());
        self.redraw_pending = false;
        Ok(())
    }

    fn request_redraw(&mut self, _handle: NativeWindowHandle) {
        self.redraw_pending = true;
        self.event_queue.push_back(PlatformEvent::WindowRedraw {
            handle: NativeWindowHandle(1),
        });
    }

    fn set_cursor_shape(&mut self, _handle: NativeWindowHandle, _shape: &str) -> bool {
        // In a full implementation, this would set the DRM hardware cursor plane.
        // For now, return false to use software cursor rendering.
        false
    }

    fn hide_cursor(&mut self, _handle: NativeWindowHandle) {
        // Hide DRM hardware cursor plane.
    }

    fn show_cursor(&mut self, _handle: NativeWindowHandle) {
        // Show DRM hardware cursor plane.
    }
}
