use crate::event_loop::PlatformEvent;
use crate::window_host::{NativeWindowHandle, NativeWindowParams};
use crate::{NullPlatform, PlatformBackend, PlatformError};
use liquide_compositor::geometry::Rect;

// ── NullPlatform tests ─────────────────────────────────────────────

#[test]
fn null_platform_name() {
    let platform = NullPlatform::new();
    assert_eq!(platform.platform_name(), "null");
}

#[test]
fn null_platform_display_empty() {
    let platform = NullPlatform::new();
    assert!(platform.display().monitors().is_empty());
    assert!(platform.display().primary_monitor().is_none());
}

#[test]
fn null_platform_poll_event_none() {
    let mut platform = NullPlatform::new();
    assert!(platform.poll_event().is_none());
}

#[test]
fn null_platform_wait_event_quit() {
    let mut platform = NullPlatform::new();
    let event = platform.wait_event();
    assert!(matches!(event, PlatformEvent::Quit));
}

#[test]
fn null_platform_present_frame_ok() {
    use liquide_compositor::pixel::PixelFormat;
    let mut platform = NullPlatform::new();
    let handle = NativeWindowHandle(1);
    let pixels = [0u8; 32];
    let result = platform.present_frame(handle, &pixels, 2, 2, 8, PixelFormat::Bgra8);
    assert!(result.is_ok());
}

#[test]
fn null_platform_keymap_is_null() {
    let platform = NullPlatform::new();
    assert_eq!(platform.keymap().platform_name(), "null");
}

#[test]
fn null_platform_window_host_create_destroy() {
    let mut platform = NullPlatform::new();
    let params = NativeWindowParams {
        title: "test".to_string(),
        geometry: Rect::new(0.0, 0.0, 100.0, 100.0),
        window_type: "normal".to_string(),
        parent: None,
        app_id: "test".to_string(),
    };
    let handle = platform.window_host().create_window(params).unwrap();
    assert!(platform.window_host().destroy_window(handle).is_ok());
}

#[test]
fn null_platform_set_cursor_shape_returns_false() {
    let mut platform = NullPlatform::new();
    let handle = NativeWindowHandle(1);
    assert!(!platform.set_cursor_shape(handle, "pointer"));
}

#[test]
fn null_platform_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NullPlatform>();
}

#[test]
fn null_platform_debug() {
    let platform = NullPlatform::new();
    let debug = format!("{platform:?}");
    assert!(debug.contains("NullPlatform"));
}

// ── PlatformError tests ────────────────────────────────────────────

#[test]
fn platform_error_display_variant() {
    let err = PlatformError::Display("monitor not found".into());
    assert_eq!(format!("{err}"), "display error: monitor not found");
}

#[test]
fn platform_error_window_variant() {
    let err = PlatformError::Window("creation failed".into());
    assert_eq!(format!("{err}"), "window error: creation failed");
}

#[test]
fn platform_error_taskbar_variant() {
    let err = PlatformError::Taskbar("not supported".into());
    assert_eq!(format!("{err}"), "taskbar error: not supported");
}

#[test]
fn platform_error_tray_variant() {
    let err = PlatformError::Tray("icon failed".into());
    assert_eq!(format!("{err}"), "tray error: icon failed");
}

#[test]
fn platform_error_notification_variant() {
    let err = PlatformError::Notification("permission denied".into());
    assert_eq!(format!("{err}"), "notification error: permission denied");
}

#[test]
fn platform_error_dragdrop_variant() {
    let err = PlatformError::DragDrop("not available".into());
    assert_eq!(format!("{err}"), "drag-drop error: not available");
}

#[test]
fn platform_error_keymap_variant() {
    let err = PlatformError::Keymap("unknown layout".into());
    assert_eq!(format!("{err}"), "keymap error: unknown layout");
}

#[test]
fn platform_error_event_loop_variant() {
    let err = PlatformError::EventLoop("timeout".into());
    assert_eq!(format!("{err}"), "event loop error: timeout");
}

#[test]
fn platform_error_presentation_variant() {
    let err = PlatformError::Presentation("swap chain lost".into());
    assert_eq!(format!("{err}"), "presentation error: swap chain lost");
}

#[test]
fn platform_error_other_variant() {
    let err = PlatformError::Other("something".into());
    assert_eq!(format!("{err}"), "something");
}

#[test]
fn platform_error_debug_impl() {
    let err = PlatformError::Display("test".into());
    let debug = format!("{err:?}");
    assert!(debug.contains("Display"));
}

// ── Data type serde tests ──────────────────────────────────────────

#[test]
fn monitor_info_serde_roundtrip() {
    use crate::display::MonitorInfo;
    let info = MonitorInfo {
        id: 2,
        name: "DP-1".to_string(),
        geometry: Rect::new(0.0, 0.0, 2560.0, 1440.0),
        work_area: Rect::new(0.0, 48.0, 2560.0, 1392.0),
        dpi_scale: 1.5,
        primary: false,
        refresh_rate_hz: 144,
    };
    let json = serde_json::to_string(&info).unwrap();
    let deserialized: MonitorInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, 2);
    assert_eq!(deserialized.name, "DP-1");
    assert!(!deserialized.primary);
    assert_eq!(deserialized.refresh_rate_hz, 144);
    assert!((deserialized.dpi_scale - 1.5).abs() < f32::EPSILON);
}

// ── Partial-present (damage) contract tests ────────────────────────
//
// These prove the `present_frame_damaged` contract at the trait level on any
// host (no Win32 needed): `None` updates the WHOLE surface, `Some(rects)`
// updates ONLY those sub-rects (leaving the rest stale), and `Some(&[])`
// updates nothing. The test backend applies damage the same way the Win32 GDI
// path does (copy whole frame into the back-buffer, blit only the damaged
// sub-rects to the "screen"), so a backend that incorrectly treated `Some` as
// full would FAIL `damaged_some_updates_only_those_rects`.

use crate::{
    DefaultKeymap, DisplayBackend, KeymapTranslator, NativeDragDrop, NativeNotifications,
    NativeTray, NativeWindowHost, NullDisplayBackend, NullDragDrop, NullNativeNotifications,
    NullNativeTray, NullTaskbar, NullWindowHost, PlatformResult, TaskbarIntegration,
};
use liquide_compositor::pixel::PixelFormat;

/// A headless backend with an in-memory "screen" surface. `present_frame`
/// blits the full frame; `present_frame_damaged` mirrors the Win32 GDI path:
/// the whole frame is staged into `back_buffer` (authoritative, for a full
/// repaint), but only the damaged sub-rects are copied to `screen`.
struct DamageRecordingPlatform {
    display_backend: NullDisplayBackend,
    window_host: NullWindowHost,
    taskbar: NullTaskbar,
    tray: NullNativeTray,
    notifications: NullNativeNotifications,
    drag_drop: NullDragDrop,
    keymap: DefaultKeymap,
    /// The on-screen surface (what a viewer would actually see). RGBA bytes,
    /// row-major `width * height * 4`.
    screen: Vec<u8>,
    /// The authoritative back-buffer (always the full latest frame).
    back_buffer: Vec<u8>,
    width: u32,
    height: u32,
    /// Number of pixels copied to `screen` by the last present (bandwidth proxy).
    last_blitted_px: u32,
}

impl DamageRecordingPlatform {
    fn new(width: u32, height: u32) -> Self {
        let n = (width * height * 4) as usize;
        Self {
            display_backend: NullDisplayBackend,
            window_host: NullWindowHost::new(),
            taskbar: NullTaskbar,
            tray: NullNativeTray::new(),
            notifications: NullNativeNotifications::new(),
            drag_drop: NullDragDrop,
            keymap: DefaultKeymap,
            screen: vec![0u8; n],
            back_buffer: vec![0u8; n],
            width,
            height,
            last_blitted_px: 0,
        }
    }

    fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.screen[i],
            self.screen[i + 1],
            self.screen[i + 2],
            self.screen[i + 3],
        ]
    }

    fn blit_rect(&mut self, x: u32, y: u32, w: u32, h: u32) {
        let stride = (self.width * 4) as usize;
        for row in y..y + h {
            let start = (row as usize) * stride + (x * 4) as usize;
            let end = start + (w * 4) as usize;
            self.screen[start..end].copy_from_slice(&self.back_buffer[start..end]);
            self.last_blitted_px += w;
        }
    }
}

impl crate::PlatformBackend for DamageRecordingPlatform {
    fn display(&self) -> &dyn DisplayBackend {
        &self.display_backend
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
        "damage-recording"
    }

    fn present_frame(
        &mut self,
        _handle: NativeWindowHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
        _stride: u32,
        _format: PixelFormat,
    ) -> PlatformResult<()> {
        // Full present: stage + blit the whole surface.
        self.back_buffer.copy_from_slice(pixels);
        self.last_blitted_px = 0;
        self.blit_rect(0, 0, width, height);
        Ok(())
    }

    fn present_frame_damaged(
        &mut self,
        handle: NativeWindowHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        format: PixelFormat,
        damage: Option<&[Rect]>,
    ) -> PlatformResult<()> {
        match damage {
            None => self.present_frame(handle, pixels, width, height, stride, format),
            Some(rects) => {
                // Always stage the full frame into the back-buffer (authoritative
                // for a full repaint) — local memory, no "bandwidth".
                self.back_buffer.copy_from_slice(pixels);
                self.last_blitted_px = 0;
                // Blit only the damaged sub-rects, clamped to the surface.
                for r in rects {
                    let x0 = r.x.max(0.0) as u32;
                    let y0 = r.y.max(0.0) as u32;
                    let x1 = ((r.x + r.width).min(self.width as f32)).max(0.0) as u32;
                    let y1 = ((r.y + r.height).min(self.height as f32)).max(0.0) as u32;
                    if x1 > x0 && y1 > y0 {
                        self.blit_rect(x0, y0, x1 - x0, y1 - y0);
                    }
                }
                Ok(())
            }
        }
    }
}

const W: u32 = 8;
const H: u32 = 8;

fn solid_frame(byte: u8) -> Vec<u8> {
    vec![byte; (W * H * 4) as usize]
}

#[test]
fn damaged_none_updates_whole_surface() {
    let mut p = DamageRecordingPlatform::new(W, H);
    let h = NativeWindowHandle(1);
    // First, a full blue frame so the screen has known content.
    p.present_frame_damaged(h, &solid_frame(0x11), W, H, W * 4, PixelFormat::Bgra8, None)
        .unwrap();
    assert_eq!(p.last_blitted_px, W * H);
    assert_eq!(p.pixel(0, 0), [0x11; 4]);
    assert_eq!(p.pixel(W - 1, H - 1), [0x11; 4]);
}

#[test]
fn damaged_some_updates_only_those_rects() {
    let mut p = DamageRecordingPlatform::new(W, H);
    let h = NativeWindowHandle(1);
    // Seed the whole screen with 0x11.
    p.present_frame(h, &solid_frame(0x11), W, H, W * 4, PixelFormat::Bgra8)
        .unwrap();

    // New full frame of 0x22, but damage ONLY a 2x2 tile at (1,1).
    let frame = solid_frame(0x22);
    let dmg = [Rect::new(1.0, 1.0, 2.0, 2.0)];
    p.present_frame_damaged(h, &frame, W, H, W * 4, PixelFormat::Bgra8, Some(&dmg))
        .unwrap();

    // Only 4 pixels should have hit the screen — a "treat Some as full" backend
    // would have blitted W*H here and failed this assertion.
    assert_eq!(p.last_blitted_px, 4, "partial present must blit only the damaged tile");

    // Inside the damage rect: updated to 0x22.
    assert_eq!(p.pixel(1, 1), [0x22; 4]);
    assert_eq!(p.pixel(2, 2), [0x22; 4]);
    // Outside the damage rect: STILL the old 0x11 (untorn, not over-updated).
    assert_eq!(p.pixel(0, 0), [0x11; 4]);
    assert_eq!(p.pixel(5, 5), [0x11; 4]);
    assert_eq!(p.pixel(W - 1, H - 1), [0x11; 4]);
}

#[test]
fn damaged_empty_slice_presents_nothing_to_screen() {
    let mut p = DamageRecordingPlatform::new(W, H);
    let h = NativeWindowHandle(1);
    p.present_frame(h, &solid_frame(0x11), W, H, W * 4, PixelFormat::Bgra8)
        .unwrap();

    // Empty damage = nothing changed → no on-screen blit, screen stays 0x11.
    let frame = solid_frame(0x33);
    p.present_frame_damaged(h, &frame, W, H, W * 4, PixelFormat::Bgra8, Some(&[]))
        .unwrap();
    assert_eq!(p.last_blitted_px, 0, "empty damage must not blit to screen");
    assert_eq!(p.pixel(0, 0), [0x11; 4]);
    assert_eq!(p.pixel(W - 1, H - 1), [0x11; 4]);
}

#[test]
fn damaged_out_of_bounds_rect_is_clamped_safely() {
    let mut p = DamageRecordingPlatform::new(W, H);
    let h = NativeWindowHandle(1);
    p.present_frame(h, &solid_frame(0x11), W, H, W * 4, PixelFormat::Bgra8)
        .unwrap();

    // A rect overhanging the bottom-right corner must clamp, not panic / OOB.
    let frame = solid_frame(0x44);
    let dmg = [Rect::new(6.0, 6.0, 100.0, 100.0)];
    p.present_frame_damaged(h, &frame, W, H, W * 4, PixelFormat::Bgra8, Some(&dmg))
        .unwrap();
    // Clamped to the 2x2 bottom-right corner = 4 px.
    assert_eq!(p.last_blitted_px, 4);
    assert_eq!(p.pixel(W - 1, H - 1), [0x44; 4]);
    assert_eq!(p.pixel(0, 0), [0x11; 4]);
}

#[test]
fn default_present_frame_damaged_falls_back_to_full() {
    // NullPlatform does NOT override present_frame_damaged → it inherits the
    // trait default, which must route to present_frame (full present) and
    // succeed for any damage value, including out-of-bounds rects.
    let mut platform = NullPlatform::new();
    let handle = NativeWindowHandle(1);
    let pixels = [0u8; 32];
    let dmg = [Rect::new(0.0, 0.0, 1.0, 1.0)];
    assert!(platform
        .present_frame_damaged(handle, &pixels, 2, 2, 8, PixelFormat::Bgra8, None)
        .is_ok());
    assert!(platform
        .present_frame_damaged(handle, &pixels, 2, 2, 8, PixelFormat::Bgra8, Some(&dmg))
        .is_ok());
    assert!(platform
        .present_frame_damaged(handle, &pixels, 2, 2, 8, PixelFormat::Bgra8, Some(&[]))
        .is_ok());
}

#[test]
fn native_window_handle_serde_roundtrip() {
    let handle = NativeWindowHandle(12345);
    let json = serde_json::to_string(&handle).unwrap();
    let deserialized: NativeWindowHandle = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, handle);
}

#[test]
fn native_window_params_serde_roundtrip() {
    let params = NativeWindowParams {
        title: "My Window".to_string(),
        geometry: Rect::new(100.0, 200.0, 800.0, 600.0),
        window_type: "dialog".to_string(),
        parent: Some(NativeWindowHandle(1)),
        app_id: "com.example.app".to_string(),
    };
    let json = serde_json::to_string(&params).unwrap();
    let deserialized: NativeWindowParams = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.title, "My Window");
    assert_eq!(deserialized.window_type, "dialog");
    assert_eq!(deserialized.parent.unwrap().0, 1);
}

#[test]
fn jump_list_item_serde_roundtrip() {
    use crate::taskbar::JumpListItem;
    let item = JumpListItem {
        title: "Recent".to_string(),
        description: "Open recent files".to_string(),
        icon: "folder".to_string(),
        action: "open_recent".to_string(),
    };
    let json = serde_json::to_string(&item).unwrap();
    let deserialized: JumpListItem = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.title, "Recent");
    assert_eq!(deserialized.action, "open_recent");
}

#[test]
fn notification_params_serde_roundtrip() {
    use crate::notifications::NativeNotificationParams;
    let params = NativeNotificationParams {
        title: "Alert".to_string(),
        body: "Something happened".to_string(),
        icon: Some("warning".to_string()),
        urgency: "critical".to_string(),
        timeout_ms: 10000,
        actions: vec!["Dismiss".to_string(), "View".to_string()],
        sound: false,
    };
    let json = serde_json::to_string(&params).unwrap();
    let deserialized: NativeNotificationParams = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.title, "Alert");
    assert_eq!(deserialized.urgency, "critical");
    assert_eq!(deserialized.actions.len(), 2);
    assert!(!deserialized.sound);
}

#[test]
fn tray_params_serde_roundtrip() {
    use crate::tray::NativeTrayParams;
    let params = NativeTrayParams {
        tooltip: "My App".to_string(),
        icon_data: vec![1, 2, 3, 4],
        menu: vec!["Open".to_string(), "Exit".to_string()],
    };
    let json = serde_json::to_string(&params).unwrap();
    let deserialized: NativeTrayParams = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.tooltip, "My App");
    assert_eq!(deserialized.icon_data, vec![1, 2, 3, 4]);
    assert_eq!(deserialized.menu.len(), 2);
}

#[test]
fn tray_handle_serde_roundtrip() {
    use crate::tray::NativeTrayHandle;
    let handle = NativeTrayHandle(99);
    let json = serde_json::to_string(&handle).unwrap();
    let deserialized: NativeTrayHandle = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, handle);
}

// ── Handle equality / hash tests ───────────────────────────────────

#[test]
fn native_window_handle_equality() {
    let h1 = NativeWindowHandle(10);
    let h2 = NativeWindowHandle(10);
    let h3 = NativeWindowHandle(20);
    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}

#[test]
fn native_window_handle_hash_consistent() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(NativeWindowHandle(1));
    set.insert(NativeWindowHandle(2));
    set.insert(NativeWindowHandle(1)); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn native_tray_handle_equality() {
    use crate::tray::NativeTrayHandle;
    let h1 = NativeTrayHandle(5);
    let h2 = NativeTrayHandle(5);
    let h3 = NativeTrayHandle(6);
    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}

#[test]
fn native_tray_handle_hash_consistent() {
    use crate::tray::NativeTrayHandle;
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(NativeTrayHandle(10));
    set.insert(NativeTrayHandle(20));
    set.insert(NativeTrayHandle(10)); // duplicate
    assert_eq!(set.len(), 2);
}

// ── ColorScheme tests ──────────────────────────────────────────────

#[test]
fn color_scheme_as_str() {
    use crate::ColorScheme;
    assert_eq!(ColorScheme::Light.as_str(), "light");
    assert_eq!(ColorScheme::Dark.as_str(), "dark");
}

#[test]
fn color_scheme_display() {
    use crate::ColorScheme;
    assert_eq!(format!("{}", ColorScheme::Light), "light");
    assert_eq!(format!("{}", ColorScheme::Dark), "dark");
}

#[test]
fn color_scheme_default_is_light() {
    use crate::ColorScheme;
    assert_eq!(ColorScheme::default(), ColorScheme::Light);
}

#[test]
fn color_scheme_equality() {
    use crate::ColorScheme;
    assert_eq!(ColorScheme::Light, ColorScheme::Light);
    assert_eq!(ColorScheme::Dark, ColorScheme::Dark);
    assert_ne!(ColorScheme::Light, ColorScheme::Dark);
}

#[test]
fn color_scheme_debug() {
    use crate::ColorScheme;
    assert_eq!(format!("{:?}", ColorScheme::Light), "Light");
    assert_eq!(format!("{:?}", ColorScheme::Dark), "Dark");
}

#[test]
fn query_color_scheme_returns_valid_variant() {
    use crate::query_color_scheme;
    let scheme = query_color_scheme();
    assert!(scheme == crate::ColorScheme::Light || scheme == crate::ColorScheme::Dark);
}

#[test]
fn null_platform_preferred_color_scheme() {
    let platform = NullPlatform::new();
    let scheme = platform.preferred_color_scheme();
    // NullPlatform uses the default trait impl which calls query_color_scheme()
    assert!(scheme == crate::ColorScheme::Light || scheme == crate::ColorScheme::Dark);
}

#[test]
fn color_scheme_changed_event_constructible() {
    let event = PlatformEvent::ColorSchemeChanged {
        scheme: crate::ColorScheme::Dark,
    };
    assert!(matches!(
        event,
        PlatformEvent::ColorSchemeChanged {
            scheme: crate::ColorScheme::Dark
        }
    ));
}
