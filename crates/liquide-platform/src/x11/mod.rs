//! X11 platform backend — full Xlib implementation via raw FFI.
//!
//! This module implements [`PlatformBackend`] for the X Window System using
//! direct calls to `libX11` and `libXrandr`.  It is only compiled on Linux.

pub mod ffi;
pub mod input;

use std::collections::{HashMap, VecDeque};
use std::os::raw::{c_char, c_int, c_long, c_uint};
use std::ptr;

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;
use liquide_input::keyboard::{KeyEvent, KeyState};
use liquide_input::mouse::{ButtonState, MouseEvent, ScrollAxis};

use crate::display::{DisplayBackend, MonitorInfo};
use crate::dnd::{NativeDragDrop, NullDragDrop};
use crate::event_loop::PlatformEvent;
use crate::keymap::KeymapTranslator;
use crate::notifications::{NativeNotifications, NullNativeNotifications};
use crate::taskbar::{NullTaskbar, TaskbarIntegration};
use crate::tray::{NativeTray, NullNativeTray};
use crate::window_host::{NativeWindowHandle, NativeWindowHost, NativeWindowParams};
use crate::{PlatformBackend, PlatformError, PlatformResult};

// ── Helpers ─────────────────────────────────────────────────────────

/// Convert a Rust `&str` to a null-terminated C string in a `Vec<u8>`.
fn to_c_string(s: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(s.len() + 1);
    v.extend_from_slice(s.as_bytes());
    v.push(0);
    v
}

/// Intern an X atom from a Rust string slice.
unsafe fn intern_atom(display: *mut ffi::Display, name: &str) -> ffi::Atom {
    let cname = to_c_string(name);
    ffi::XInternAtom(display, cname.as_ptr() as *const c_char, 0)
}

// ── Per-window state ────────────────────────────────────────────────

/// State tracked for each window created by the backend.
struct WindowState {
    /// The X11 window ID.
    xwindow: ffi::Window,
    /// Most recently known width.
    width: u32,
    /// Most recently known height.
    height: u32,
}

// ── Interned atoms ──────────────────────────────────────────────────

/// Collection of interned X atoms used throughout the backend.
struct X11Atoms {
    wm_delete_window: ffi::Atom,
    net_wm_name: ffi::Atom,
    utf8_string: ffi::Atom,
    net_wm_state: ffi::Atom,
    net_wm_state_maximized_vert: ffi::Atom,
    net_wm_state_maximized_horz: ffi::Atom,
    net_wm_state_fullscreen: ffi::Atom,
    net_wm_state_hidden: ffi::Atom,
    net_wm_window_type: ffi::Atom,
    net_wm_window_type_normal: ffi::Atom,
    net_wm_window_type_dialog: ffi::Atom,
    net_wm_window_type_splash: ffi::Atom,
    net_wm_icon: ffi::Atom,
}

impl X11Atoms {
    unsafe fn new(display: *mut ffi::Display) -> Self {
        Self {
            wm_delete_window: intern_atom(display, "WM_DELETE_WINDOW"),
            net_wm_name: intern_atom(display, "_NET_WM_NAME"),
            utf8_string: intern_atom(display, "UTF8_STRING"),
            net_wm_state: intern_atom(display, "_NET_WM_STATE"),
            net_wm_state_maximized_vert: intern_atom(display, "_NET_WM_STATE_MAXIMIZED_VERT"),
            net_wm_state_maximized_horz: intern_atom(display, "_NET_WM_STATE_MAXIMIZED_HORZ"),
            net_wm_state_fullscreen: intern_atom(display, "_NET_WM_STATE_FULLSCREEN"),
            net_wm_state_hidden: intern_atom(display, "_NET_WM_STATE_HIDDEN"),
            net_wm_window_type: intern_atom(display, "_NET_WM_WINDOW_TYPE"),
            net_wm_window_type_normal: intern_atom(display, "_NET_WM_WINDOW_TYPE_NORMAL"),
            net_wm_window_type_dialog: intern_atom(display, "_NET_WM_WINDOW_TYPE_DIALOG"),
            net_wm_window_type_splash: intern_atom(display, "_NET_WM_WINDOW_TYPE_SPLASH"),
            net_wm_icon: intern_atom(display, "_NET_WM_ICON"),
        }
    }
}

// ── X11 Display backend (XRandR) ────────────────────────────────────

/// Display/monitor enumeration backed by XRandR.
struct X11DisplayBackend {
    display: *mut ffi::Display,
    root: ffi::Window,
}

// SAFETY: only used from the single event-loop thread; the X display
// connection is never shared across threads.
unsafe impl Send for X11DisplayBackend {}

impl DisplayBackend for X11DisplayBackend {
    fn monitors(&self) -> Vec<MonitorInfo> {
        self.enumerate_monitors()
    }

    fn primary_monitor(&self) -> Option<MonitorInfo> {
        self.monitors().into_iter().find(|m| m.primary)
    }

    fn virtual_screen_rect(&self) -> Rect {
        let monitors = self.monitors();
        if monitors.is_empty() {
            return Rect::ZERO;
        }
        let mut min_x: f32 = f32::MAX;
        let mut min_y: f32 = f32::MAX;
        let mut max_x: f32 = f32::MIN;
        let mut max_y: f32 = f32::MIN;
        for m in &monitors {
            min_x = min_x.min(m.geometry.x);
            min_y = min_y.min(m.geometry.y);
            max_x = max_x.max(m.geometry.x + m.geometry.width);
            max_y = max_y.max(m.geometry.y + m.geometry.height);
        }
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }
}

impl X11DisplayBackend {
    fn enumerate_monitors(&self) -> Vec<MonitorInfo> {
        let mut monitors = Vec::new();

        // SAFETY: All XRandR calls below use a valid display pointer and
        // properly allocated resources. Output/crtc info pointers are freed
        // after use. The display was successfully opened in X11Platform::new().
        unsafe {
            let resources = ffi::XRRGetScreenResourcesCurrent(self.display, self.root);
            if resources.is_null() {
                return self.fallback_monitors();
            }

            let noutput = (*resources).noutput;
            for i in 0..noutput {
                let output_id = *(*resources).outputs.add(i as usize);
                let output_info = ffi::XRRGetOutputInfo(self.display, resources, output_id);
                if output_info.is_null() {
                    continue;
                }

                // Skip disconnected outputs.
                if (*output_info).connection != ffi::RR_Connected {
                    ffi::XRRFreeOutputInfo(output_info);
                    continue;
                }

                let crtc_id = (*output_info).crtc;
                if crtc_id == 0 {
                    ffi::XRRFreeOutputInfo(output_info);
                    continue;
                }

                let crtc_info = ffi::XRRGetCrtcInfo(self.display, resources, crtc_id);
                if crtc_info.is_null() {
                    ffi::XRRFreeOutputInfo(output_info);
                    continue;
                }

                // Build the monitor name from the output name bytes.
                let name = if (*output_info).name_len > 0 {
                    let name_slice = std::slice::from_raw_parts(
                        (*output_info).name as *const u8,
                        (*output_info).name_len as usize,
                    );
                    String::from_utf8_lossy(name_slice).into_owned()
                } else {
                    format!("output-{i}")
                };

                let x = (*crtc_info).x as f32;
                let y = (*crtc_info).y as f32;
                let width = (*crtc_info).width as f32;
                let height = (*crtc_info).height as f32;

                // Compute DPI scale from physical mm dimensions.
                let dpi_scale = if (*output_info).mm_width > 0 {
                    let dpi_x = ((*crtc_info).width as f64 * 25.4) / (*output_info).mm_width as f64;
                    (dpi_x / 96.0) as f32
                } else {
                    1.0
                };

                // Compute refresh rate from the CRTC mode.
                let refresh_rate_hz = self.mode_refresh_rate(resources, (*crtc_info).mode);

                // First connected output is treated as primary.
                let primary = i == 0;
                let geometry = Rect::new(x, y, width, height);

                monitors.push(MonitorInfo {
                    id: i as u32,
                    name,
                    geometry,
                    work_area: geometry,
                    dpi_scale,
                    primary,
                    refresh_rate_hz,
                });

                ffi::XRRFreeCrtcInfo(crtc_info);
                ffi::XRRFreeOutputInfo(output_info);
            }

            ffi::XRRFreeScreenResources(resources);
        }

        if monitors.is_empty() {
            return self.fallback_monitors();
        }
        monitors
    }

    /// Fallback when XRandR returns no outputs: use basic Xlib screen info.
    fn fallback_monitors(&self) -> Vec<MonitorInfo> {
        let mut monitors = Vec::new();
        // SAFETY: XScreenCount, XScreenOfDisplay, XWidthOfScreen, and
        // XHeightOfScreen are safe with a valid display pointer.
        unsafe {
            let screen_count = ffi::XScreenCount(self.display);
            for s in 0..screen_count {
                let screen = ffi::XScreenOfDisplay(self.display, s);
                if screen.is_null() {
                    continue;
                }
                let w = ffi::XWidthOfScreen(screen) as f32;
                let h = ffi::XHeightOfScreen(screen) as f32;
                let geometry = Rect::new(0.0, 0.0, w, h);
                monitors.push(MonitorInfo {
                    id: s as u32,
                    name: format!("screen-{s}"),
                    geometry,
                    work_area: geometry,
                    dpi_scale: 1.0,
                    primary: s == 0,
                    refresh_rate_hz: 60,
                });
            }
        }
        monitors
    }

    /// Look up the refresh rate for a given RandR mode ID.
    unsafe fn mode_refresh_rate(
        &self,
        resources: *const ffi::XRRScreenResources,
        mode_id: ffi::XID,
    ) -> u32 {
        if mode_id == 0 {
            return 60;
        }
        let nmode = (*resources).nmode;
        for m in 0..nmode {
            let mode = &*(*resources).modes.add(m as usize);
            if mode.id == mode_id && mode.h_total > 0 && mode.v_total > 0 {
                let total = mode.h_total as u64 * mode.v_total as u64;
                if total > 0 {
                    return ((mode.dot_clock as u64 + total / 2) / total) as u32;
                }
            }
        }
        60
    }
}

// ── X11 Window Manager ──────────────────────────────────────────────
//
// Owns all window-related state and implements `NativeWindowHost`.
// The parent `X11Platform` accesses internal fields directly for
// event translation.

struct X11WindowManager {
    display: *mut ffi::Display,
    screen: c_int,
    root: ffi::Window,
    atoms: X11Atoms,

    /// Map from our opaque handle id to per-window state.
    windows: HashMap<u64, WindowState>,
    /// Reverse map: X11 Window -> our handle id.
    xwindow_to_handle: HashMap<ffi::Window, u64>,
    /// Next handle value to assign.
    next_handle: u64,
    /// Events generated during window operations (e.g. WindowCreated).
    pending_events: VecDeque<PlatformEvent>,
}

// SAFETY: only used from the single event-loop thread; the X display
// connection is never shared across threads.
unsafe impl Send for X11WindowManager {}

impl X11WindowManager {
    /// Set the title on an X window using both `XStoreName` (ICCCM) and
    /// `_NET_WM_NAME` (EWMH, UTF-8).
    unsafe fn set_title_raw(&self, xwindow: ffi::Window, title: &str) {
        let c_title = to_c_string(title);
        ffi::XStoreName(self.display, xwindow, c_title.as_ptr() as *const c_char);
        ffi::XChangeProperty(
            self.display,
            xwindow,
            self.atoms.net_wm_name,
            self.atoms.utf8_string,
            8,
            ffi::PropModeReplace,
            title.as_bytes().as_ptr(),
            title.len() as c_int,
        );
    }
}

impl NativeWindowHost for X11WindowManager {
    fn create_window(&mut self, params: NativeWindowParams) -> PlatformResult<NativeWindowHandle> {
        // SAFETY: All Xlib calls use the valid display connection.
        // XCreateWindow parameters are validated (width/height clamped to >= 1).
        // The root window and visual are obtained from the default screen.
        unsafe {
            let visual = ffi::XDefaultVisual(self.display, self.screen);
            let depth = ffi::XDefaultDepth(self.display, self.screen);
            let colormap = ffi::XDefaultColormap(self.display, self.screen);

            let x = params.geometry.x as c_int;
            let y = params.geometry.y as c_int;
            let width = (params.geometry.width as c_uint).max(1);
            let height = (params.geometry.height as c_uint).max(1);

            let event_mask: c_long = ffi::KeyPressMask
                | ffi::KeyReleaseMask
                | ffi::ButtonPressMask
                | ffi::ButtonReleaseMask
                | ffi::PointerMotionMask
                | ffi::EnterWindowMask
                | ffi::LeaveWindowMask
                | ffi::ExposureMask
                | ffi::StructureNotifyMask
                | ffi::FocusChangeMask;

            let mut attrs: ffi::XSetWindowAttributes = std::mem::zeroed();
            attrs.event_mask = event_mask;
            attrs.colormap = colormap;
            attrs.border_pixel = 0;
            attrs.background_pixel = 0;

            let value_mask =
                ffi::CWEventMask | ffi::CWColormap | ffi::CWBorderPixel | ffi::CWBackPixel;

            let xwindow = ffi::XCreateWindow(
                self.display,
                self.root,
                x,
                y,
                width,
                height,
                0, // border width
                depth,
                ffi::InputOutput,
                visual,
                value_mask,
                &mut attrs,
            );

            if xwindow == 0 {
                return Err(PlatformError::Window("XCreateWindow returned 0".into()));
            }

            // Register WM_DELETE_WINDOW so we receive close requests.
            let mut wm_delete = self.atoms.wm_delete_window;
            ffi::XSetWMProtocols(self.display, xwindow, &mut wm_delete, 1);

            // Set window title (ICCCM + EWMH).
            self.set_title_raw(xwindow, &params.title);

            // Set _NET_WM_WINDOW_TYPE hint.
            let type_atom = match params.window_type.as_str() {
                "dialog" => self.atoms.net_wm_window_type_dialog,
                "splash" => self.atoms.net_wm_window_type_splash,
                _ => self.atoms.net_wm_window_type_normal,
            };
            ffi::XChangeProperty(
                self.display,
                xwindow,
                self.atoms.net_wm_window_type,
                ffi::XA_ATOM,
                32,
                ffi::PropModeReplace,
                &type_atom as *const ffi::Atom as *const u8,
                1,
            );

            // Show the window.
            ffi::XMapWindow(self.display, xwindow);
            ffi::XFlush(self.display);

            // Assign an opaque handle.
            let handle_val = self.next_handle;
            self.next_handle += 1;
            let handle = NativeWindowHandle(handle_val);

            self.windows.insert(
                handle_val,
                WindowState {
                    xwindow,
                    width: width as u32,
                    height: height as u32,
                },
            );
            self.xwindow_to_handle.insert(xwindow, handle_val);

            // Queue a WindowCreated event so the caller sees it.
            self.pending_events.push_back(PlatformEvent::WindowCreated {
                handle,
                width: width as u32,
                height: height as u32,
            });

            Ok(handle)
        }
    }

    fn destroy_window(&mut self, handle: NativeWindowHandle) -> PlatformResult<()> {
        if let Some(state) = self.windows.remove(&handle.0) {
            self.xwindow_to_handle.remove(&state.xwindow);
            // SAFETY: XDestroyWindow and XFlush are safe on a valid display/window.
            unsafe {
                ffi::XDestroyWindow(self.display, state.xwindow);
                ffi::XFlush(self.display);
            }
        }
        Ok(())
    }

    fn set_geometry(&mut self, handle: NativeWindowHandle, geometry: Rect) -> PlatformResult<()> {
        if let Some(state) = self.windows.get_mut(&handle.0) {
            let w = (geometry.width as c_uint).max(1);
            let h = (geometry.height as c_uint).max(1);
            // SAFETY: XMoveResizeWindow and XFlush are safe on valid display/window.
            unsafe {
                ffi::XMoveResizeWindow(
                    self.display,
                    state.xwindow,
                    geometry.x as c_int,
                    geometry.y as c_int,
                    w,
                    h,
                );
                ffi::XFlush(self.display);
            }
            state.width = w as u32;
            state.height = h as u32;
        }
        Ok(())
    }

    fn set_title(&mut self, handle: NativeWindowHandle, title: &str) -> PlatformResult<()> {
        if let Some(state) = self.windows.get(&handle.0) {
            // SAFETY: set_title_raw and XFlush use valid display/window pointers.
            unsafe {
                self.set_title_raw(state.xwindow, title);
                ffi::XFlush(self.display);
            }
        }
        Ok(())
    }

    fn set_icon(&mut self, handle: NativeWindowHandle, icon_data: &[u8]) -> PlatformResult<()> {
        if let Some(state) = self.windows.get(&handle.0) {
            // _NET_WM_ICON expects: [width, height, pixel0, pixel1, ...]
            // each pixel is ARGB packed into a c_long.
            // Assumes icon_data is raw BGRA8 for a square image.
            if icon_data.len() >= 4 {
                let pixel_count = icon_data.len() / 4;
                let side = (pixel_count as f64).sqrt() as usize;
                if side * side == pixel_count && side > 0 {
                    let mut data: Vec<c_long> = Vec::with_capacity(2 + pixel_count);
                    data.push(side as c_long);
                    data.push(side as c_long);
                    for p in 0..pixel_count {
                        let b = icon_data[p * 4] as c_long;
                        let g = icon_data[p * 4 + 1] as c_long;
                        let r = icon_data[p * 4 + 2] as c_long;
                        let a = icon_data[p * 4 + 3] as c_long;
                        data.push((a << 24) | (r << 16) | (g << 8) | b);
                    }
                    // SAFETY: XChangeProperty and XFlush are safe with valid
                    // display/window. The data pointer and length are correct.
                    unsafe {
                        ffi::XChangeProperty(
                            self.display,
                            state.xwindow,
                            self.atoms.net_wm_icon,
                            ffi::XA_CARDINAL,
                            32,
                            ffi::PropModeReplace,
                            data.as_ptr() as *const u8,
                            data.len() as c_int,
                        );
                        ffi::XFlush(self.display);
                    }
                }
            }
        }
        Ok(())
    }

    fn set_state(&mut self, handle: NativeWindowHandle, state_name: &str) -> PlatformResult<()> {
        if let Some(state) = self.windows.get(&handle.0) {
            let xw = state.xwindow;
            // SAFETY: send_ewmh_state, XUnmapWindow, XMapWindow, and XFlush
            // are safe with valid display and window handles.
            unsafe {
                match state_name {
                    "maximized" => {
                        send_ewmh_state(
                            self.display,
                            xw,
                            self.root,
                            self.atoms.net_wm_state,
                            1,
                            self.atoms.net_wm_state_maximized_vert,
                            self.atoms.net_wm_state_maximized_horz,
                        );
                    }
                    "minimized" => {
                        send_ewmh_state(
                            self.display,
                            xw,
                            self.root,
                            self.atoms.net_wm_state,
                            1,
                            self.atoms.net_wm_state_hidden,
                            0,
                        );
                        ffi::XUnmapWindow(self.display, xw);
                    }
                    "fullscreen" => {
                        send_ewmh_state(
                            self.display,
                            xw,
                            self.root,
                            self.atoms.net_wm_state,
                            1,
                            self.atoms.net_wm_state_fullscreen,
                            0,
                        );
                    }
                    _ => {
                        // "normal" — remove maximized / fullscreen / hidden.
                        send_ewmh_state(
                            self.display,
                            xw,
                            self.root,
                            self.atoms.net_wm_state,
                            0,
                            self.atoms.net_wm_state_maximized_vert,
                            self.atoms.net_wm_state_maximized_horz,
                        );
                        send_ewmh_state(
                            self.display,
                            xw,
                            self.root,
                            self.atoms.net_wm_state,
                            0,
                            self.atoms.net_wm_state_fullscreen,
                            0,
                        );
                        ffi::XMapWindow(self.display, xw);
                    }
                }
                ffi::XFlush(self.display);
            }
        }
        Ok(())
    }

    fn set_z_order(&mut self, handle: NativeWindowHandle, z_order: i32) -> PlatformResult<()> {
        if let Some(state) = self.windows.get(&handle.0) {
            // SAFETY: XRaiseWindow / XLowerWindow and XFlush are safe
            // with a valid display and window.
            unsafe {
                if z_order >= 0 {
                    ffi::XRaiseWindow(self.display, state.xwindow);
                } else {
                    ffi::XLowerWindow(self.display, state.xwindow);
                }
                ffi::XFlush(self.display);
            }
        }
        Ok(())
    }

    fn set_focus(&mut self, handle: NativeWindowHandle) -> PlatformResult<()> {
        if let Some(state) = self.windows.get(&handle.0) {
            // SAFETY: XSetInputFocus and XFlush are safe with valid display/window.
            unsafe {
                ffi::XSetInputFocus(
                    self.display,
                    state.xwindow,
                    ffi::RevertToParent,
                    ffi::CurrentTime,
                );
                ffi::XFlush(self.display);
            }
        }
        Ok(())
    }
}

/// Send an EWMH `_NET_WM_STATE` client message to the root window.
unsafe fn send_ewmh_state(
    display: *mut ffi::Display,
    window: ffi::Window,
    root: ffi::Window,
    net_wm_state: ffi::Atom,
    action: c_long,
    prop1: ffi::Atom,
    prop2: ffi::Atom,
) {
    let mut event: ffi::XEvent = std::mem::zeroed();
    let cm = &mut *(event.pad.as_mut_ptr() as *mut ffi::XClientMessageEvent);
    cm.type_ = ffi::ClientMessage;
    cm.window = window;
    cm.message_type = net_wm_state;
    cm.format = 32;
    cm.data.l[0] = action;
    cm.data.l[1] = prop1 as c_long;
    cm.data.l[2] = prop2 as c_long;
    cm.data.l[3] = 1; // source indication: application
    let mask = ffi::SubstructureRedirectMask | ffi::SubstructureNotifyMask;
    ffi::XSendEvent(display, root, 0, mask, &mut event);
}

// ── X11 Keymap translator ───────────────────────────────────────────

/// Keymap translator that converts X11 keysyms via [`input::keysym_to_keycode`].
struct X11Keymap;

// SAFETY: X11Keymap is stateless — safe to send between threads.
unsafe impl Send for X11Keymap {}

impl KeymapTranslator for X11Keymap {
    fn translate_scancode(&self, scancode: u32) -> Option<liquide_input::KeyCode> {
        input::keysym_to_keycode(scancode as u64)
    }

    fn platform_name(&self) -> &str {
        "x11"
    }
}

// ── X11Platform ─────────────────────────────────────────────────────

/// The full X11 platform backend.
///
/// Wraps a connection to the X server and manages windows, events, and
/// frame presentation through raw Xlib FFI.
pub struct X11Platform {
    /// Raw X display connection.
    display: *mut ffi::Display,
    /// Default screen number.
    screen: c_int,
    /// Root window of the default screen.
    root: ffi::Window,
    /// Default graphics context for blitting.
    gc: ffi::GC,

    /// Window management (creates, tracks, and manipulates X11 windows).
    wm: X11WindowManager,
    /// Internal event buffer for translated platform events.
    event_queue: VecDeque<PlatformEvent>,

    // Sub-backends
    display_backend: X11DisplayBackend,
    keymap: X11Keymap,
    taskbar: NullTaskbar,
    tray: NullNativeTray,
    notifications: NullNativeNotifications,
    drag_drop: NullDragDrop,
}

// The X11 display connection is NOT thread-safe, but our trait requires
// `Send`.  This is safe because the platform is always used from a
// single event-loop thread.
// SAFETY: X11Platform is only used from a single event-loop thread.
// The X display connection is not shared across threads.
unsafe impl Send for X11Platform {}

impl X11Platform {
    /// Open an X display connection and initialise the platform backend.
    ///
    /// Connects to the X server specified by `$DISPLAY` (or the default
    /// server if the environment variable is not set).
    pub fn new() -> PlatformResult<Self> {
        // SAFETY: XOpenDisplay, XDefaultScreen, XDefaultRootWindow,
        // X11Atoms::new, and XCreateGC all operate on the display
        // connection. XOpenDisplay returns null on failure (checked below).
        unsafe {
            let display = ffi::XOpenDisplay(ptr::null());
            if display.is_null() {
                return Err(PlatformError::Display(
                    "XOpenDisplay returned null -- is $DISPLAY set?".into(),
                ));
            }

            let screen = ffi::XDefaultScreen(display);
            let root = ffi::XDefaultRootWindow(display);
            let atoms = X11Atoms::new(display);
            let gc = ffi::XCreateGC(display, root, 0, ptr::null_mut());

            let display_backend = X11DisplayBackend { display, root };

            let wm = X11WindowManager {
                display,
                screen,
                root,
                atoms,
                windows: HashMap::new(),
                xwindow_to_handle: HashMap::new(),
                next_handle: 1,
                pending_events: VecDeque::new(),
            };

            Ok(Self {
                display,
                screen,
                root,
                gc,
                wm,
                event_queue: VecDeque::new(),
                display_backend,
                keymap: X11Keymap,
                taskbar: NullTaskbar,
                tray: NullNativeTray::new(),
                notifications: NullNativeNotifications::new(),
                drag_drop: NullDragDrop,
            })
        }
    }

    // ── Event translation ───────────────────────────────────────────

    /// Drain any events generated by the window manager (e.g.
    /// WindowCreated from `create_window`) into the main event queue.
    fn drain_wm_events(&mut self) {
        while let Some(ev) = self.wm.pending_events.pop_front() {
            self.event_queue.push_back(ev);
        }
    }

    /// Translate a raw XEvent into zero or more `PlatformEvent` entries
    /// pushed onto `self.event_queue`.
    fn translate_event(&mut self, event: &ffi::XEvent) {
        let etype = event.event_type();

        match etype {
            // ── Keyboard ────────────────────────────────────────────
            ffi::KeyPress | ffi::KeyRelease => {
                let xkey = event.as_key();
                // SAFETY: XLookupKeysym is safe with a valid XKeyEvent pointer.
                let keysym = unsafe { ffi::XLookupKeysym(xkey, 0) };
                if let Some(key) = input::keysym_to_keycode(keysym as u64) {
                    let state = if etype == ffi::KeyPress {
                        KeyState::Pressed
                    } else {
                        KeyState::Released
                    };
                    let modifiers = input::x11_modifiers_to_modifiers(xkey.state);
                    let scancode = keysym as u32;
                    let timestamp_us = (xkey.time as u64) * 1000;

                    if let Some(&hv) = self.wm.xwindow_to_handle.get(&xkey.window) {
                        self.event_queue.push_back(PlatformEvent::KeyInput {
                            handle: NativeWindowHandle(hv),
                            event: KeyEvent::new(key, state, modifiers, scancode, timestamp_us),
                        });
                    }
                }
            }

            // ── Mouse buttons / scroll ──────────────────────────────
            ffi::ButtonPress | ffi::ButtonRelease => {
                let xb = event.as_button();
                if let Some(&hv) = self.wm.xwindow_to_handle.get(&xb.window) {
                    let handle = NativeWindowHandle(hv);
                    let x = xb.x as f32;
                    let y = xb.y as f32;

                    match xb.button {
                        // Vertical scroll (up/down).
                        ffi::Button4 if etype == ffi::ButtonPress => {
                            self.event_queue.push_back(PlatformEvent::MouseInput {
                                handle,
                                event: MouseEvent::Scroll {
                                    axis: ScrollAxis::Vertical,
                                    delta: 3.0,
                                    x,
                                    y,
                                },
                            });
                        }
                        ffi::Button5 if etype == ffi::ButtonPress => {
                            self.event_queue.push_back(PlatformEvent::MouseInput {
                                handle,
                                event: MouseEvent::Scroll {
                                    axis: ScrollAxis::Vertical,
                                    delta: -3.0,
                                    x,
                                    y,
                                },
                            });
                        }
                        // Horizontal scroll (left/right) on some setups.
                        6 if etype == ffi::ButtonPress => {
                            self.event_queue.push_back(PlatformEvent::MouseInput {
                                handle,
                                event: MouseEvent::Scroll {
                                    axis: ScrollAxis::Horizontal,
                                    delta: -3.0,
                                    x,
                                    y,
                                },
                            });
                        }
                        7 if etype == ffi::ButtonPress => {
                            self.event_queue.push_back(PlatformEvent::MouseInput {
                                handle,
                                event: MouseEvent::Scroll {
                                    axis: ScrollAxis::Horizontal,
                                    delta: 3.0,
                                    x,
                                    y,
                                },
                            });
                        }
                        btn => {
                            if let Some(mb) = input::x11_button_to_mouse_button(btn) {
                                let bs = if etype == ffi::ButtonPress {
                                    ButtonState::Pressed
                                } else {
                                    ButtonState::Released
                                };
                                self.event_queue.push_back(PlatformEvent::MouseInput {
                                    handle,
                                    event: MouseEvent::Button {
                                        button: mb,
                                        state: bs,
                                        x,
                                        y,
                                    },
                                });
                            }
                        }
                    }
                }
            }

            // ── Pointer motion ──────────────────────────────────────
            ffi::MotionNotify => {
                let xm = event.as_motion();
                if let Some(&hv) = self.wm.xwindow_to_handle.get(&xm.window) {
                    self.event_queue.push_back(PlatformEvent::MouseInput {
                        handle: NativeWindowHandle(hv),
                        event: MouseEvent::Move {
                            x: xm.x as f32,
                            y: xm.y as f32,
                        },
                    });
                }
            }

            // ── Enter / Leave ───────────────────────────────────────
            ffi::EnterNotify => {
                let xc = event.as_crossing();
                if let Some(&hv) = self.wm.xwindow_to_handle.get(&xc.window) {
                    self.event_queue.push_back(PlatformEvent::MouseInput {
                        handle: NativeWindowHandle(hv),
                        event: MouseEvent::Enter {
                            x: xc.x as f32,
                            y: xc.y as f32,
                        },
                    });
                }
            }
            ffi::LeaveNotify => {
                let xc = event.as_crossing();
                if let Some(&hv) = self.wm.xwindow_to_handle.get(&xc.window) {
                    self.event_queue.push_back(PlatformEvent::MouseInput {
                        handle: NativeWindowHandle(hv),
                        event: MouseEvent::Leave,
                    });
                }
            }

            // ── Focus ───────────────────────────────────────────────
            ffi::FocusIn => {
                let xf = event.as_focus_change();
                if let Some(&hv) = self.wm.xwindow_to_handle.get(&xf.window) {
                    self.event_queue.push_back(PlatformEvent::FocusGained {
                        handle: NativeWindowHandle(hv),
                    });
                }
            }
            ffi::FocusOut => {
                let xf = event.as_focus_change();
                if let Some(&hv) = self.wm.xwindow_to_handle.get(&xf.window) {
                    self.event_queue.push_back(PlatformEvent::FocusLost {
                        handle: NativeWindowHandle(hv),
                    });
                }
            }

            // ── Expose (repaint) ────────────────────────────────────
            ffi::Expose => {
                let xe = event.as_expose();
                if xe.count == 0 {
                    if let Some(&hv) = self.wm.xwindow_to_handle.get(&xe.window) {
                        self.event_queue.push_back(PlatformEvent::WindowRedraw {
                            handle: NativeWindowHandle(hv),
                        });
                    }
                }
            }

            // ── Configure (move / resize) ───────────────────────────
            ffi::ConfigureNotify => {
                let xc = event.as_configure();
                if let Some(&hv) = self.wm.xwindow_to_handle.get(&xc.window) {
                    let handle = NativeWindowHandle(hv);
                    let new_w = xc.width as u32;
                    let new_h = xc.height as u32;

                    self.event_queue.push_back(PlatformEvent::WindowMoved {
                        handle,
                        x: xc.x,
                        y: xc.y,
                    });

                    let resized = if let Some(ws) = self.wm.windows.get_mut(&hv) {
                        if ws.width != new_w || ws.height != new_h {
                            ws.width = new_w;
                            ws.height = new_h;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if resized {
                        self.event_queue.push_back(PlatformEvent::WindowResized {
                            handle,
                            width: new_w,
                            height: new_h,
                        });
                    }
                }
            }

            // ── Map / Unmap ─────────────────────────────────────────
            ffi::MapNotify => {
                // Mapped (made visible) — no distinct event needed.
            }
            ffi::UnmapNotify => {
                let xu = event.as_unmap();
                if let Some(&hv) = self.wm.xwindow_to_handle.get(&xu.window) {
                    self.event_queue.push_back(PlatformEvent::WindowMinimized {
                        handle: NativeWindowHandle(hv),
                    });
                }
            }

            // ── Destroy ─────────────────────────────────────────────
            ffi::DestroyNotify => {
                let xd = event.as_destroy();
                if let Some(&hv) = self.wm.xwindow_to_handle.get(&xd.window) {
                    self.event_queue.push_back(PlatformEvent::WindowDestroyed {
                        handle: NativeWindowHandle(hv),
                    });
                    self.wm.windows.remove(&hv);
                    self.wm.xwindow_to_handle.remove(&xd.window);
                }
            }

            // ── Client message (WM_DELETE_WINDOW) ───────────────────
            ffi::ClientMessage => {
                let xcm = event.as_client_message();
                // SAFETY: Accessing the l[0] field of a ClientMessage data union.
                // The XEvent was received from XNextEvent and is valid.
                let data_l0 = unsafe { xcm.data.l[0] } as ffi::Atom;
                if data_l0 == self.wm.atoms.wm_delete_window {
                    if let Some(&hv) = self.wm.xwindow_to_handle.get(&xcm.window) {
                        self.event_queue
                            .push_back(PlatformEvent::WindowCloseRequested {
                                handle: NativeWindowHandle(hv),
                            });
                    }
                }
            }

            // ── Property change (unused for now) ────────────────────
            ffi::PropertyNotify => {}

            // ── Everything else ─────────────────────────────────────
            _ => {}
        }
    }
}

impl Drop for X11Platform {
    fn drop(&mut self) {
        // SAFETY: All Xlib cleanup calls use the valid display connection.
        // Windows are destroyed before freeing the GC and closing the display.
        unsafe {
            // Destroy all windows we still own.
            let xwindows: Vec<ffi::Window> = self.wm.windows.values().map(|s| s.xwindow).collect();
            for xw in xwindows {
                ffi::XDestroyWindow(self.display, xw);
            }
            self.wm.windows.clear();
            self.wm.xwindow_to_handle.clear();

            if !self.gc.is_null() {
                ffi::XFreeGC(self.display, self.gc);
            }
            ffi::XCloseDisplay(self.display);
        }
    }
}

// ── PlatformBackend implementation ──────────────────────────────────

impl PlatformBackend for X11Platform {
    fn display(&self) -> &dyn DisplayBackend {
        &self.display_backend
    }

    fn window_host(&mut self) -> &mut dyn NativeWindowHost {
        &mut self.wm
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
        "x11"
    }

    // ── Event loop ──────────────────────────────────────────────────

    fn poll_event(&mut self) -> Option<PlatformEvent> {
        // First, drain events generated by window manager operations.
        self.drain_wm_events();

        if let Some(ev) = self.event_queue.pop_front() {
            return Some(ev);
        }

        // Process all pending X events.
        // SAFETY: XPending and XNextEvent are safe with a valid display pointer.
        // The XEvent is stack-allocated and properly zeroed before use.
        unsafe {
            while ffi::XPending(self.display) > 0 {
                let mut xevent: ffi::XEvent = std::mem::zeroed();
                ffi::XNextEvent(self.display, &mut xevent);
                self.translate_event(&xevent);
            }
        }

        self.event_queue.pop_front()
    }

    fn wait_event(&mut self) -> PlatformEvent {
        // Drain buffered events first.
        self.drain_wm_events();

        if let Some(ev) = self.event_queue.pop_front() {
            return ev;
        }

        // Block until an X event arrives.
        // SAFETY: XNextEvent blocks until an event is available.
        // The display pointer is valid.
        unsafe {
            let mut xevent: ffi::XEvent = std::mem::zeroed();
            ffi::XNextEvent(self.display, &mut xevent);
            self.translate_event(&xevent);
        }

        // Drain any additional pending events into the queue.
        // SAFETY: XPending and XNextEvent are safe with a valid display.
        unsafe {
            while ffi::XPending(self.display) > 0 {
                let mut xevent: ffi::XEvent = std::mem::zeroed();
                ffi::XNextEvent(self.display, &mut xevent);
                self.translate_event(&xevent);
            }
        }

        self.event_queue.pop_front().unwrap_or(PlatformEvent::Quit)
    }

    // ── Frame presentation ──────────────────────────────────────────

    fn present_frame(
        &mut self,
        handle: NativeWindowHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        _format: PixelFormat,
    ) -> PlatformResult<()> {
        let xwindow = match self.wm.windows.get(&handle.0) {
            Some(s) => s.xwindow,
            None => {
                return Err(PlatformError::Presentation("unknown window handle".into()));
            }
        };

        // SAFETY: All Xlib calls below use valid display, window, and GC.
        // The pixel buffer is copied into a Rust-owned Vec before being
        // passed to XCreateImage. We null out image->data before
        // XDestroyImage to prevent double-free of Rust-owned memory.
        unsafe {
            let visual = ffi::XDefaultVisual(self.display, self.screen);
            let depth = ffi::XDefaultDepth(self.display, self.screen);

            let bpp: u32 = 4; // BGRA8
            let row_bytes = width * bpp;

            // Build a contiguous pixel buffer that XCreateImage can use.
            let data: Vec<u8> = if stride == row_bytes {
                let end = ((height * stride) as usize).min(pixels.len());
                pixels[..end].to_vec()
            } else {
                let mut buf = Vec::with_capacity((height * row_bytes) as usize);
                for row in 0..height {
                    let src_start = (row * stride) as usize;
                    let src_end = (src_start + row_bytes as usize).min(pixels.len());
                    if src_start < pixels.len() {
                        buf.extend_from_slice(&pixels[src_start..src_end]);
                        let written = src_end - src_start;
                        if written < row_bytes as usize {
                            buf.resize(buf.len() + row_bytes as usize - written, 0);
                        }
                    } else {
                        buf.resize(buf.len() + row_bytes as usize, 0);
                    }
                }
                buf
            };

            let image = ffi::XCreateImage(
                self.display,
                visual,
                depth as c_uint,
                ffi::ZPixmap,
                0,
                data.as_ptr() as *mut c_char,
                width,
                height,
                32,
                row_bytes as c_int,
            );

            if image.is_null() {
                return Err(PlatformError::Presentation(
                    "XCreateImage returned null".into(),
                ));
            }

            // On little-endian x86_64, ZPixmap with 32-bit depth stores
            // pixels in BGRA byte order — matching our pixel format.
            (*image).byte_order = ffi::LSBFirst;

            ffi::XPutImage(
                self.display,
                xwindow,
                self.gc,
                image,
                0,
                0,
                0,
                0,
                width,
                height,
            );

            // Prevent XDestroyImage from freeing our Rust-owned buffer.
            (*image).data = ptr::null_mut();
            ffi::XDestroyImage(image);

            ffi::XFlush(self.display);
        }

        Ok(())
    }

    fn request_redraw(&mut self, handle: NativeWindowHandle) {
        if let Some(state) = self.wm.windows.get(&handle.0) {
            // SAFETY: We construct a synthetic Expose XEvent and send it to the
            // window via XSendEvent. All parameters are valid. XFlush ensures
            // the event is dispatched.
            unsafe {
                let expose = &mut *(event.pad.as_mut_ptr() as *mut ffi::XExposeEvent);
                expose.type_ = ffi::Expose;
                expose.window = state.xwindow;
                expose.x = 0;
                expose.y = 0;
                expose.width = state.width as c_int;
                expose.height = state.height as c_int;
                expose.count = 0;

                ffi::XSendEvent(
                    self.display,
                    state.xwindow,
                    0,
                    ffi::ExposureMask,
                    &mut event,
                );
                ffi::XFlush(self.display);
            }
        }
    }
}
