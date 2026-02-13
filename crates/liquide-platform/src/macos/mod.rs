//! macOS platform backend.
//!
//! Provides a complete `PlatformBackend` implementation using raw Objective-C
//! runtime calls via FFI to Cocoa / AppKit / Core Graphics.  No external crate
//! dependencies are used -- we link directly to the system frameworks and the
//! Objective-C runtime dylib.

pub mod ffi;
pub mod input;

use std::collections::HashMap;
use std::ffi::c_void;
use std::time::{SystemTime, UNIX_EPOCH};

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;
use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState};
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent, ScrollAxis};

use crate::display::{DisplayBackend, MonitorInfo};
use crate::dnd::NullDragDrop;
use crate::event_loop::PlatformEvent;
use crate::keymap::KeymapTranslator;
use crate::notifications::{NativeNotificationParams, NativeNotifications};
use crate::taskbar::{JumpListItem, TaskbarIntegration};
use crate::tray::{NativeTray, NativeTrayHandle, NativeTrayParams, TrayUpdate};
use crate::window_host::{NativeWindowHandle, NativeWindowHost, NativeWindowParams};
use crate::{NativeDragDrop, PlatformBackend, PlatformError, PlatformResult};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return a microsecond timestamp (best-effort monotonic via `SystemTime`).
fn timestamp_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Per-window metadata
// ---------------------------------------------------------------------------

/// Metadata for a window managed by the macOS platform backend.
struct WindowInfo {
    /// The Cocoa `NSWindow` object.
    nswindow: ffi::id,
    /// Our platform handle for this window.
    handle: NativeWindowHandle,
}

// ---------------------------------------------------------------------------
// macOS display backend
// ---------------------------------------------------------------------------

/// Display backend that queries monitor information via Core Graphics.
struct MacOSDisplayBackend;

impl MacOSDisplayBackend {
    fn screen_size() -> (u32, u32) {
        unsafe {
            let display_id = ffi::CGMainDisplayID();
            let w = ffi::CGDisplayPixelsWide(display_id) as u32;
            let h = ffi::CGDisplayPixelsHigh(display_id) as u32;
            (w, h)
        }
    }
}

impl DisplayBackend for MacOSDisplayBackend {
    fn monitors(&self) -> Vec<MonitorInfo> {
        // Query the main screen via NSScreen.
        let (w, h) = Self::screen_size();
        let geometry = Rect::new(0.0, 0.0, w as f32, h as f32);

        // Attempt to get the visible frame (work area excluding menu bar / dock).
        let work_area = unsafe {
            let ns_screen_class = ffi::class(b"NSScreen\0");
            let main_screen = ffi::msg_send_id(ns_screen_class, ffi::sel(b"mainScreen\0"));
            if main_screen.is_null() {
                geometry
            } else {
                let frame = ffi::msg_send_nsrect(main_screen, ffi::sel(b"visibleFrame\0"));
                Rect::new(
                    frame.origin.x as f32,
                    frame.origin.y as f32,
                    frame.size.width as f32,
                    frame.size.height as f32,
                )
            }
        };

        // Get DPI scale (backing scale factor) from main screen.
        let dpi_scale = unsafe {
            let ns_screen_class = ffi::class(b"NSScreen\0");
            let main_screen = ffi::msg_send_id(ns_screen_class, ffi::sel(b"mainScreen\0"));
            if main_screen.is_null() {
                1.0f32
            } else {
                ffi::msg_send_cgfloat(main_screen, ffi::sel(b"backingScaleFactor\0")) as f32
            }
        };

        vec![MonitorInfo {
            id: 0,
            name: "Main Display".to_string(),
            geometry,
            work_area,
            dpi_scale,
            primary: true,
            refresh_rate_hz: 60,
        }]
    }

    fn primary_monitor(&self) -> Option<MonitorInfo> {
        self.monitors().into_iter().next()
    }

    fn virtual_screen_rect(&self) -> Rect {
        let (w, h) = Self::screen_size();
        Rect::new(0.0, 0.0, w as f32, h as f32)
    }
}

unsafe impl Send for MacOSDisplayBackend {}

// ---------------------------------------------------------------------------
// macOS window host
// ---------------------------------------------------------------------------

/// Window host implementation backed by Cocoa `NSWindow`.
struct MacOSWindowHost {
    /// Map from our `NativeWindowHandle` id to window metadata.
    windows: HashMap<u64, WindowInfo>,
    /// Monotonically increasing counter for handle generation.
    next_handle: u64,
}

unsafe impl Send for MacOSWindowHost {}

impl MacOSWindowHost {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            next_handle: 1,
        }
    }
}

impl NativeWindowHost for MacOSWindowHost {
    fn create_window(
        &mut self,
        params: NativeWindowParams,
    ) -> PlatformResult<NativeWindowHandle> {
        let handle = NativeWindowHandle(self.next_handle);
        self.next_handle += 1;

        let x = if params.geometry.x == 0.0 && params.geometry.y == 0.0 {
            100.0
        } else {
            params.geometry.x as ffi::CGFloat
        };
        let y = if params.geometry.x == 0.0 && params.geometry.y == 0.0 {
            100.0
        } else {
            params.geometry.y as ffi::CGFloat
        };
        let w = if params.geometry.width > 0.0 {
            params.geometry.width as ffi::CGFloat
        } else {
            800.0
        };
        let h = if params.geometry.height > 0.0 {
            params.geometry.height as ffi::CGFloat
        } else {
            600.0
        };

        let content_rect = ffi::NSRect::new(x, y, w, h);

        let style_mask = ffi::NSWindowStyleMaskDefault;

        unsafe {
            // Create an autorelease pool for this scope.
            let pool_class = ffi::class(b"NSAutoreleasePool\0");
            let pool = ffi::msg_send_id(pool_class, ffi::sel(b"new\0"));

            // Allocate and initialise the NSWindow.
            let ns_window_class = ffi::class(b"NSWindow\0");
            let alloc = ffi::msg_send_id(ns_window_class, ffi::sel(b"alloc\0"));

            let nswindow = ffi::msg_send_init_window(
                alloc,
                ffi::sel(b"initWithContentRect:styleMask:backing:defer:\0"),
                content_rect,
                style_mask,
                ffi::NSBackingStoreBuffered,
                ffi::NO,
            );

            if nswindow.is_null() {
                ffi::msg_send_void(pool, ffi::sel(b"drain\0"));
                return Err(PlatformError::Window(
                    "NSWindow initWithContentRect failed".into(),
                ));
            }

            // Set the window title.
            let title = ffi::nsstring(&params.title);
            ffi::msg_send_void_id(nswindow, ffi::sel(b"setTitle:\0"), title);

            // Make the window visible and key.
            ffi::msg_send_void_bool(
                nswindow,
                ffi::sel(b"setReleasedWhenClosed:\0"),
                ffi::NO,
            );
            ffi::msg_send_void(nswindow, ffi::sel(b"makeKeyAndOrderFront:\0"));

            ffi::msg_send_void(pool, ffi::sel(b"drain\0"));

            let info = WindowInfo { nswindow, handle };
            self.windows.insert(handle.0, info);
        }

        Ok(handle)
    }

    fn destroy_window(&mut self, handle: NativeWindowHandle) -> PlatformResult<()> {
        if let Some(info) = self.windows.remove(&handle.0) {
            unsafe {
                // Close the window. Because we set releasedWhenClosed to NO,
                // we must also release it explicitly.
                ffi::msg_send_void(info.nswindow, ffi::sel(b"close\0"));
                ffi::msg_send_void(info.nswindow, ffi::sel(b"release\0"));
            }
        }
        Ok(())
    }

    fn set_geometry(
        &mut self,
        handle: NativeWindowHandle,
        geometry: Rect,
    ) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            let frame = ffi::NSRect::new(
                geometry.x as ffi::CGFloat,
                geometry.y as ffi::CGFloat,
                geometry.width as ffi::CGFloat,
                geometry.height as ffi::CGFloat,
            );
            unsafe {
                // setFrame:display: with display=YES
                let f: unsafe extern "C" fn(ffi::id, ffi::SEL, ffi::NSRect, ffi::BOOL) =
                    std::mem::transmute(ffi::objc_msgSend as *const c_void);
                f(
                    info.nswindow,
                    ffi::sel(b"setFrame:display:\0"),
                    frame,
                    ffi::YES,
                );
            }
        }
        Ok(())
    }

    fn set_title(&mut self, handle: NativeWindowHandle, title: &str) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            unsafe {
                let pool_class = ffi::class(b"NSAutoreleasePool\0");
                let pool = ffi::msg_send_id(pool_class, ffi::sel(b"new\0"));

                let ns_title = ffi::nsstring(title);
                ffi::msg_send_void_id(info.nswindow, ffi::sel(b"setTitle:\0"), ns_title);

                ffi::msg_send_void(pool, ffi::sel(b"drain\0"));
            }
        }
        Ok(())
    }

    fn set_icon(
        &mut self,
        _handle: NativeWindowHandle,
        _icon_data: &[u8],
    ) -> PlatformResult<()> {
        // macOS does not support per-window icons in the title bar.
        // The application icon is set via the bundle. Accept and ignore.
        Ok(())
    }

    fn set_state(
        &mut self,
        handle: NativeWindowHandle,
        state: &str,
    ) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            unsafe {
                match state {
                    "maximized" => {
                        let is_zoomed = ffi::msg_send_bool(info.nswindow, ffi::sel(b"isZoomed\0"));
                        if is_zoomed == ffi::NO {
                            ffi::msg_send_void_id(
                                info.nswindow,
                                ffi::sel(b"zoom:\0"),
                                ffi::NIL,
                            );
                        }
                    }
                    "minimized" => {
                        ffi::msg_send_void_id(
                            info.nswindow,
                            ffi::sel(b"miniaturize:\0"),
                            ffi::NIL,
                        );
                    }
                    "restored" | "normal" => {
                        let is_miniaturized =
                            ffi::msg_send_bool(info.nswindow, ffi::sel(b"isMiniaturized\0"));
                        if is_miniaturized != ffi::NO {
                            ffi::msg_send_void_id(
                                info.nswindow,
                                ffi::sel(b"deminiaturize:\0"),
                                ffi::NIL,
                            );
                        }
                        let is_zoomed =
                            ffi::msg_send_bool(info.nswindow, ffi::sel(b"isZoomed\0"));
                        if is_zoomed != ffi::NO {
                            ffi::msg_send_void_id(
                                info.nswindow,
                                ffi::sel(b"zoom:\0"),
                                ffi::NIL,
                            );
                        }
                    }
                    "hidden" => {
                        ffi::msg_send_void_id(
                            info.nswindow,
                            ffi::sel(b"orderOut:\0"),
                            ffi::NIL,
                        );
                    }
                    _ => {
                        ffi::msg_send_void(
                            info.nswindow,
                            ffi::sel(b"makeKeyAndOrderFront:\0"),
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn set_z_order(
        &mut self,
        handle: NativeWindowHandle,
        z_order: i32,
    ) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            unsafe {
                if z_order > 0 {
                    // Set the window level to floating (above normal windows).
                    // NSFloatingWindowLevel = 3
                    let f: unsafe extern "C" fn(ffi::id, ffi::SEL, ffi::NSInteger) =
                        std::mem::transmute(ffi::objc_msgSend as *const c_void);
                    f(info.nswindow, ffi::sel(b"setLevel:\0"), 3);
                } else {
                    // NSNormalWindowLevel = 0
                    let f: unsafe extern "C" fn(ffi::id, ffi::SEL, ffi::NSInteger) =
                        std::mem::transmute(ffi::objc_msgSend as *const c_void);
                    f(info.nswindow, ffi::sel(b"setLevel:\0"), 0);
                }
            }
        }
        Ok(())
    }

    fn set_focus(&mut self, handle: NativeWindowHandle) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            unsafe {
                ffi::msg_send_void(info.nswindow, ffi::sel(b"makeKeyAndOrderFront:\0"));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// macOS taskbar (Dock) integration (minimal stub)
// ---------------------------------------------------------------------------

/// Minimal Dock integration.
///
/// macOS dock features (badge count, progress) require NSApp and
/// NSDockTile APIs. This implementation silently accepts all calls.
struct MacOSTaskbar;

impl TaskbarIntegration for MacOSTaskbar {
    fn set_progress(&mut self, _handle: u64, _progress: f64) -> PlatformResult<()> {
        // NSDockTile progress is not directly supported. Stubbed.
        Ok(())
    }

    fn set_overlay_icon(&mut self, _handle: u64, _icon_data: &[u8]) -> PlatformResult<()> {
        Ok(())
    }

    fn set_badge_count(&mut self, _count: u32) -> PlatformResult<()> {
        // Could use [[NSApp dockTile] setBadgeLabel:...], but stubbed for now.
        Ok(())
    }

    fn add_jump_list_item(&mut self, _item: JumpListItem) -> PlatformResult<()> {
        // macOS does not have jump lists. Dock menus use a different API.
        Ok(())
    }
}

unsafe impl Send for MacOSTaskbar {}

// ---------------------------------------------------------------------------
// macOS tray icon backend (minimal stub)
// ---------------------------------------------------------------------------

/// System tray (status bar item) backend.
///
/// Full implementation requires NSStatusBar / NSStatusItem. This stub
/// silently accepts all calls.
struct MacOSTray {
    next_id: u64,
    icons: HashMap<u64, ()>,
}

unsafe impl Send for MacOSTray {}

impl MacOSTray {
    fn new() -> Self {
        Self {
            next_id: 1,
            icons: HashMap::new(),
        }
    }
}

impl NativeTray for MacOSTray {
    fn add_icon(
        &mut self,
        _params: NativeTrayParams,
    ) -> PlatformResult<NativeTrayHandle> {
        let handle_id = self.next_id;
        self.next_id += 1;
        self.icons.insert(handle_id, ());
        Ok(NativeTrayHandle(handle_id))
    }

    fn update_icon(
        &mut self,
        _handle: NativeTrayHandle,
        _update: TrayUpdate,
    ) -> PlatformResult<()> {
        Ok(())
    }

    fn remove_icon(&mut self, handle: NativeTrayHandle) -> PlatformResult<()> {
        self.icons.remove(&handle.0);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// macOS notifications (minimal stub)
// ---------------------------------------------------------------------------

/// Desktop notification backend.
///
/// Full implementation requires `NSUserNotificationCenter` (deprecated) or
/// `UNUserNotificationCenter`. This stub returns unique IDs and silently
/// drops notification content.
struct MacOSNotifications {
    next_id: u32,
}

unsafe impl Send for MacOSNotifications {}

impl MacOSNotifications {
    fn new() -> Self {
        Self { next_id: 1 }
    }
}

impl NativeNotifications for MacOSNotifications {
    fn show(&mut self, _params: NativeNotificationParams) -> PlatformResult<u32> {
        let id = self.next_id;
        self.next_id += 1;
        Ok(id)
    }

    fn dismiss(&mut self, _id: u32) -> PlatformResult<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// macOS keymap translator
// ---------------------------------------------------------------------------

/// Keymap translator that delegates to `input::scancode_to_keycode`.
struct MacOSKeymap;

unsafe impl Send for MacOSKeymap {}

impl KeymapTranslator for MacOSKeymap {
    fn translate_scancode(&self, scancode: u32) -> Option<KeyCode> {
        input::scancode_to_keycode(scancode)
    }

    fn platform_name(&self) -> &str {
        "macos"
    }
}

// ---------------------------------------------------------------------------
// NSEvent helper — translate an NSEvent into PlatformEvent(s)
// ---------------------------------------------------------------------------

/// Translate an `NSEvent` (Objective-C object) into zero or more
/// `PlatformEvent`s, pushing them into the supplied vector.
///
/// # Safety
///
/// `ns_event` must be a valid `NSEvent *`. The function reads event type,
/// keyCode, modifierFlags, locationInWindow, and other properties via
/// Objective-C message sends.
unsafe fn translate_nsevent(
    ns_event: ffi::id,
    windows: &HashMap<u64, WindowInfo>,
    events: &mut Vec<PlatformEvent>,
) {
    let event_type = unsafe { ffi::msg_send_nsuinteger(ns_event, ffi::sel(b"type\0")) };

    // Find the window handle for this event.
    let ns_window = unsafe { ffi::msg_send_id(ns_event, ffi::sel(b"window\0")) };
    let handle = find_handle_for_nswindow(ns_window, windows);

    let ts = timestamp_us();

    match event_type {
        ffi::NSEventTypeKeyDown => {
            let handle = match handle {
                Some(h) => h,
                None => return,
            };
            let keycode = unsafe { ffi::msg_send_u16(ns_event, ffi::sel(b"keyCode\0")) };
            let flags = unsafe { ffi::msg_send_u64(ns_event, ffi::sel(b"modifierFlags\0")) };
            let is_repeat = unsafe { ffi::msg_send_bool(ns_event, ffi::sel(b"isARepeat\0")) };
            if let Some(key) = input::vk_to_keycode(keycode) {
                let state = if is_repeat != ffi::NO {
                    KeyState::Repeat
                } else {
                    KeyState::Pressed
                };
                let mods = input::modifiers_from_flags(flags);
                let event = KeyEvent::new(key, state, mods, keycode as u32, ts);
                events.push(PlatformEvent::KeyInput { handle, event });
            }
        }

        ffi::NSEventTypeKeyUp => {
            let handle = match handle {
                Some(h) => h,
                None => return,
            };
            let keycode = unsafe { ffi::msg_send_u16(ns_event, ffi::sel(b"keyCode\0")) };
            let flags = unsafe { ffi::msg_send_u64(ns_event, ffi::sel(b"modifierFlags\0")) };
            if let Some(key) = input::vk_to_keycode(keycode) {
                let mods = input::modifiers_from_flags(flags);
                let event = KeyEvent::new(key, KeyState::Released, mods, keycode as u32, ts);
                events.push(PlatformEvent::KeyInput { handle, event });
            }
        }

        ffi::NSEventTypeFlagsChanged => {
            let handle = match handle {
                Some(h) => h,
                None => return,
            };
            let keycode = unsafe { ffi::msg_send_u16(ns_event, ffi::sel(b"keyCode\0")) };
            let flags = unsafe { ffi::msg_send_u64(ns_event, ffi::sel(b"modifierFlags\0")) };
            if let Some(key) = input::vk_to_keycode(keycode) {
                // Determine if this is a press or release from the modifier
                // flags. If the corresponding flag is set, it's a press;
                // otherwise it's a release.
                let is_pressed = match keycode {
                    input::kVK_Shift | input::kVK_RightShift => {
                        flags & ffi::NSEventModifierFlagShift != 0
                    }
                    input::kVK_Control | input::kVK_RightControl => {
                        flags & ffi::NSEventModifierFlagControl != 0
                    }
                    input::kVK_Option | input::kVK_RightOption => {
                        flags & ffi::NSEventModifierFlagOption != 0
                    }
                    input::kVK_Command | input::kVK_RightCommand => {
                        flags & ffi::NSEventModifierFlagCommand != 0
                    }
                    input::kVK_CapsLock => flags & ffi::NSEventModifierFlagCapsLock != 0,
                    _ => true,
                };
                let state = if is_pressed {
                    KeyState::Pressed
                } else {
                    KeyState::Released
                };
                let mods = input::modifiers_from_flags(flags);
                let event = KeyEvent::new(key, state, mods, keycode as u32, ts);
                events.push(PlatformEvent::KeyInput { handle, event });
            }
        }

        ffi::NSEventTypeLeftMouseDown => {
            let handle = match handle {
                Some(h) => h,
                None => return,
            };
            let loc = unsafe { ffi::msg_send_nspoint(ns_event, ffi::sel(b"locationInWindow\0")) };
            events.push(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                    x: loc.x as f32,
                    y: loc.y as f32,
                },
            });
        }

        ffi::NSEventTypeLeftMouseUp => {
            let handle = match handle {
                Some(h) => h,
                None => return,
            };
            let loc = unsafe { ffi::msg_send_nspoint(ns_event, ffi::sel(b"locationInWindow\0")) };
            events.push(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Left,
                    state: ButtonState::Released,
                    x: loc.x as f32,
                    y: loc.y as f32,
                },
            });
        }

        ffi::NSEventTypeRightMouseDown => {
            let handle = match handle {
                Some(h) => h,
                None => return,
            };
            let loc = unsafe { ffi::msg_send_nspoint(ns_event, ffi::sel(b"locationInWindow\0")) };
            events.push(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Right,
                    state: ButtonState::Pressed,
                    x: loc.x as f32,
                    y: loc.y as f32,
                },
            });
        }

        ffi::NSEventTypeRightMouseUp => {
            let handle = match handle {
                Some(h) => h,
                None => return,
            };
            let loc = unsafe { ffi::msg_send_nspoint(ns_event, ffi::sel(b"locationInWindow\0")) };
            events.push(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Right,
                    state: ButtonState::Released,
                    x: loc.x as f32,
                    y: loc.y as f32,
                },
            });
        }

        ffi::NSEventTypeOtherMouseDown => {
            let handle = match handle {
                Some(h) => h,
                None => return,
            };
            let loc = unsafe { ffi::msg_send_nspoint(ns_event, ffi::sel(b"locationInWindow\0")) };
            events.push(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Middle,
                    state: ButtonState::Pressed,
                    x: loc.x as f32,
                    y: loc.y as f32,
                },
            });
        }

        ffi::NSEventTypeOtherMouseUp => {
            let handle = match handle {
                Some(h) => h,
                None => return,
            };
            let loc = unsafe { ffi::msg_send_nspoint(ns_event, ffi::sel(b"locationInWindow\0")) };
            events.push(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Middle,
                    state: ButtonState::Released,
                    x: loc.x as f32,
                    y: loc.y as f32,
                },
            });
        }

        ffi::NSEventTypeMouseMoved
        | ffi::NSEventTypeLeftMouseDragged
        | ffi::NSEventTypeRightMouseDragged
        | ffi::NSEventTypeOtherMouseDragged => {
            let handle = match handle {
                Some(h) => h,
                None => return,
            };
            let loc = unsafe { ffi::msg_send_nspoint(ns_event, ffi::sel(b"locationInWindow\0")) };
            events.push(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Move {
                    x: loc.x as f32,
                    y: loc.y as f32,
                },
            });
        }

        ffi::NSEventTypeScrollWheel => {
            let handle = match handle {
                Some(h) => h,
                None => return,
            };
            let loc = unsafe { ffi::msg_send_nspoint(ns_event, ffi::sel(b"locationInWindow\0")) };
            let delta_y = unsafe {
                ffi::msg_send_cgfloat(ns_event, ffi::sel(b"scrollingDeltaY\0"))
            };
            let delta_x = unsafe {
                ffi::msg_send_cgfloat(ns_event, ffi::sel(b"scrollingDeltaX\0"))
            };

            if delta_y.abs() > 0.0001 {
                events.push(PlatformEvent::MouseInput {
                    handle,
                    event: MouseEvent::Scroll {
                        axis: ScrollAxis::Vertical,
                        delta: delta_y as f32,
                        x: loc.x as f32,
                        y: loc.y as f32,
                    },
                });
            }
            if delta_x.abs() > 0.0001 {
                events.push(PlatformEvent::MouseInput {
                    handle,
                    event: MouseEvent::Scroll {
                        axis: ScrollAxis::Horizontal,
                        delta: delta_x as f32,
                        x: loc.x as f32,
                        y: loc.y as f32,
                    },
                });
            }
        }

        _ => {
            // Unhandled event types are ignored.
        }
    }
}

/// Find our `NativeWindowHandle` for a given `NSWindow` pointer by scanning
/// the window map.
fn find_handle_for_nswindow(
    nswindow: ffi::id,
    windows: &HashMap<u64, WindowInfo>,
) -> Option<NativeWindowHandle> {
    if nswindow.is_null() {
        return None;
    }
    for info in windows.values() {
        if info.nswindow == nswindow {
            return Some(info.handle);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// MacOSPlatform — the top-level backend
// ---------------------------------------------------------------------------

/// macOS platform backend.
///
/// Manages the `NSApplication` shared instance, window host, and all
/// sub-backends for the macOS desktop.
pub struct MacOSPlatform {
    // Sub-backends
    display: MacOSDisplayBackend,
    window_host: MacOSWindowHost,
    taskbar: MacOSTaskbar,
    tray: MacOSTray,
    notifications: MacOSNotifications,
    drag_drop: NullDragDrop,
    keymap: MacOSKeymap,

    /// The shared `NSApplication` instance.
    nsapp: ffi::id,

    /// Cached run loop mode string (`NSDefaultRunLoopMode`).
    run_loop_mode: ffi::id,
}

// Safety: MacOSPlatform owns all Cocoa objects and is designed to be used
// from the main thread. The `Send` bound on `PlatformBackend` is satisfied
// by structural guarantees (macOS requires all AppKit calls on the main
// thread, which is enforced by the caller).
unsafe impl Send for MacOSPlatform {}

impl MacOSPlatform {
    /// Create and initialise a new macOS platform backend.
    ///
    /// Initialises the shared `NSApplication`, sets the activation policy
    /// to `Regular` (so the app gets a Dock icon and menu bar), and
    /// activates the application.
    pub fn new() -> PlatformResult<Self> {
        unsafe {
            // Create an autorelease pool.
            let pool_class = ffi::class(b"NSAutoreleasePool\0");
            let pool = ffi::msg_send_id(pool_class, ffi::sel(b"new\0"));

            // Get or create the shared NSApplication.
            let ns_app_class = ffi::class(b"NSApplication\0");
            let nsapp = ffi::msg_send_id(ns_app_class, ffi::sel(b"sharedApplication\0"));
            if nsapp.is_null() {
                ffi::msg_send_void(pool, ffi::sel(b"drain\0"));
                return Err(PlatformError::Other(
                    "NSApplication sharedApplication returned nil".into(),
                ));
            }

            // Set activation policy to Regular (standard GUI app).
            ffi::msg_send_void_nsinteger(
                nsapp,
                ffi::sel(b"setActivationPolicy:\0"),
                ffi::NSApplicationActivationPolicyRegular,
            );

            // Activate the application (bring to front).
            ffi::msg_send_void_bool(
                nsapp,
                ffi::sel(b"activateIgnoringOtherApps:\0"),
                ffi::YES,
            );

            // Cache NSDefaultRunLoopMode string for event polling.
            let run_loop_mode = ffi::nsstring("kCFRunLoopDefaultMode");

            ffi::msg_send_void(pool, ffi::sel(b"drain\0"));

            Ok(Self {
                display: MacOSDisplayBackend,
                window_host: MacOSWindowHost::new(),
                taskbar: MacOSTaskbar,
                tray: MacOSTray::new(),
                notifications: MacOSNotifications::new(),
                drag_drop: NullDragDrop,
                keymap: MacOSKeymap,
                nsapp,
                run_loop_mode,
            })
        }
    }

    /// Fetch the next event from NSApp using `nextEventMatchingMask:...`.
    ///
    /// If `blocking` is `true`, waits until an event is available (using
    /// `[NSDate distantFuture]`). Otherwise returns `nil` immediately if
    /// no events are pending (using `nil` / zero-timeout date).
    fn next_nsevent(&self, blocking: bool) -> ffi::id {
        unsafe {
            let until_date = if blocking {
                let date_class = ffi::class(b"NSDate\0");
                ffi::msg_send_id(date_class, ffi::sel(b"distantFuture\0"))
            } else {
                ffi::NIL
            };

            ffi::msg_send_next_event(
                self.nsapp,
                ffi::sel(b"nextEventMatchingMask:untilDate:inMode:dequeue:\0"),
                ffi::NSEventMaskAny,
                until_date,
                self.run_loop_mode,
                ffi::YES,
            )
        }
    }

    /// Dispatch an NSEvent back to AppKit so that standard event handling
    /// (window dragging, resizing, menu shortcuts, etc.) still works.
    fn send_event(&self, ns_event: ffi::id) {
        unsafe {
            ffi::msg_send_void_id(self.nsapp, ffi::sel(b"sendEvent:\0"), ns_event);
        }
    }
}

impl Drop for MacOSPlatform {
    fn drop(&mut self) {
        // Destroy all tracked windows.
        let handles: Vec<u64> = self.window_host.windows.keys().copied().collect();
        for h in handles {
            if let Some(info) = self.window_host.windows.remove(&h) {
                unsafe {
                    ffi::msg_send_void(info.nswindow, ffi::sel(b"close\0"));
                    ffi::msg_send_void(info.nswindow, ffi::sel(b"release\0"));
                }
            }
        }
    }
}

impl PlatformBackend for MacOSPlatform {
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
        "macos"
    }

    // -- Event loop ---------------------------------------------------------

    fn poll_event(&mut self) -> Option<PlatformEvent> {
        unsafe {
            let pool_class = ffi::class(b"NSAutoreleasePool\0");
            let pool = ffi::msg_send_id(pool_class, ffi::sel(b"new\0"));

            let mut result = None;

            // Pull the next event (non-blocking).
            let ns_event = self.next_nsevent(false);
            if !ns_event.is_null() {
                let mut translated = Vec::new();
                translate_nsevent(ns_event, &self.window_host.windows, &mut translated);

                // Dispatch back to AppKit for standard handling.
                self.send_event(ns_event);

                result = translated.into_iter().next();
            }

            ffi::msg_send_void(pool, ffi::sel(b"drain\0"));
            result
        }
    }

    fn wait_event(&mut self) -> PlatformEvent {
        unsafe {
            let pool_class = ffi::class(b"NSAutoreleasePool\0");
            let pool = ffi::msg_send_id(pool_class, ffi::sel(b"new\0"));

            // Block until an event arrives.
            let ns_event = self.next_nsevent(true);
            if ns_event.is_null() {
                ffi::msg_send_void(pool, ffi::sel(b"drain\0"));
                return PlatformEvent::Quit;
            }

            let mut translated = Vec::new();
            translate_nsevent(ns_event, &self.window_host.windows, &mut translated);

            // Dispatch back to AppKit for standard handling.
            self.send_event(ns_event);

            ffi::msg_send_void(pool, ffi::sel(b"drain\0"));

            translated
                .into_iter()
                .next()
                .unwrap_or(PlatformEvent::Quit)
        }
    }

    // -- Frame presentation -------------------------------------------------

    fn present_frame(
        &mut self,
        handle: NativeWindowHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
        _stride: u32,
        format: PixelFormat,
    ) -> PlatformResult<()> {
        // We only support BGRA8.
        if format != PixelFormat::Bgra8 {
            return Err(PlatformError::Presentation(format!(
                "unsupported pixel format {:?}; only Bgra8 is supported",
                format
            )));
        }

        let info = self
            .window_host
            .windows
            .get(&handle.0)
            .ok_or_else(|| PlatformError::Presentation("unknown window handle".into()))?;

        let nswindow = info.nswindow;

        unsafe {
            let pool_class = ffi::class(b"NSAutoreleasePool\0");
            let pool = ffi::msg_send_id(pool_class, ffi::sel(b"new\0"));

            // Create a CGColorSpace (device RGB).
            let color_space = ffi::CGColorSpaceCreateDeviceRGB();
            if color_space.is_null() {
                ffi::msg_send_void(pool, ffi::sel(b"drain\0"));
                return Err(PlatformError::Presentation(
                    "CGColorSpaceCreateDeviceRGB failed".into(),
                ));
            }

            // Create a CGBitmapContext from the pixel data.
            let bytes_per_row = (width as usize) * 4;
            let context = ffi::CGBitmapContextCreate(
                pixels.as_ptr() as *mut c_void,
                width as usize,
                height as usize,
                8,             // bits per component
                bytes_per_row, // bytes per row
                color_space,
                ffi::kCGBitmapInfoBGRA8,
            );

            if context.is_null() {
                ffi::CGColorSpaceRelease(color_space);
                ffi::msg_send_void(pool, ffi::sel(b"drain\0"));
                return Err(PlatformError::Presentation(
                    "CGBitmapContextCreate failed".into(),
                ));
            }

            // Create a CGImage from the bitmap context.
            let cg_image = ffi::CGBitmapContextCreateImage(context);
            if cg_image.is_null() {
                ffi::CGContextRelease(context);
                ffi::CGColorSpaceRelease(color_space);
                ffi::msg_send_void(pool, ffi::sel(b"drain\0"));
                return Err(PlatformError::Presentation(
                    "CGBitmapContextCreateImage failed".into(),
                ));
            }

            // Get the window's content view and lock focus.
            let content_view =
                ffi::msg_send_id(nswindow, ffi::sel(b"contentView\0"));
            if !content_view.is_null() {
                ffi::msg_send_void(content_view, ffi::sel(b"lockFocus\0"));

                // Get the current graphics context (NSGraphicsContext).
                let ns_gctx_class = ffi::class(b"NSGraphicsContext\0");
                let ns_gctx = ffi::msg_send_id(
                    ns_gctx_class,
                    ffi::sel(b"currentContext\0"),
                );

                if !ns_gctx.is_null() {
                    // Get the CGContext from the graphics context.
                    let cg_ctx = ffi::msg_send_id(ns_gctx, ffi::sel(b"CGContext\0"));
                    if !cg_ctx.is_null() {
                        let draw_rect = ffi::CGRect::new(
                            0.0,
                            0.0,
                            width as ffi::CGFloat,
                            height as ffi::CGFloat,
                        );
                        ffi::CGContextDrawImage(cg_ctx, draw_rect, cg_image);
                        ffi::CGContextFlush(cg_ctx);
                    }
                }

                ffi::msg_send_void(content_view, ffi::sel(b"unlockFocus\0"));
            }

            // Flush the window.
            ffi::msg_send_void(nswindow, ffi::sel(b"flushWindow\0"));

            // Clean up CG objects.
            ffi::CGImageRelease(cg_image);
            ffi::CGContextRelease(context);
            ffi::CGColorSpaceRelease(color_space);

            ffi::msg_send_void(pool, ffi::sel(b"drain\0"));
        }

        Ok(())
    }

    fn request_redraw(&mut self, handle: NativeWindowHandle) {
        if let Some(info) = self.window_host.windows.get(&handle.0) {
            unsafe {
                // Get the content view and mark it as needing display.
                let content_view =
                    ffi::msg_send_id(info.nswindow, ffi::sel(b"contentView\0"));
                if !content_view.is_null() {
                    ffi::msg_send_void_bool(
                        content_view,
                        ffi::sel(b"setNeedsDisplay:\0"),
                        ffi::YES,
                    );
                }
            }
        }
    }
}
