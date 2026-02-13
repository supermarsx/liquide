//! Win32 platform backend.
//!
//! Provides a complete `PlatformBackend` implementation using raw Win32 API
//! calls via FFI (no external crate dependencies). Links against user32.dll,
//! gdi32.dll, kernel32.dll, and shell32.dll at load time.

pub mod ffi;
pub mod input;

use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::ptr;
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
// UTF-16 helper
// ---------------------------------------------------------------------------

/// Convert a Rust `&str` to a null-terminated UTF-16 vector for Win32 wide APIs.
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Decode a null-terminated UTF-16 slice into a `String`.
fn from_wide(s: &[u16]) -> String {
    let len = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..len])
}

/// Return a microsecond timestamp (best-effort monotonic via `SystemTime`).
fn timestamp_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Per-window data stored via GWLP_USERDATA
// ---------------------------------------------------------------------------

/// Data attached to every window created by this backend.
///
/// A raw pointer to this struct is stored in the window's `GWLP_USERDATA`
/// slot so that the window procedure can push events into the shared queue.
struct WindowData {
    /// The `NativeWindowHandle` we assigned to this window.
    handle: NativeWindowHandle,
    /// Pointer back to the platform's event queue.
    /// The queue lives inside the `Win32Platform`, which owns all windows
    /// and outlives them.
    event_queue: *mut VecDeque<PlatformEvent>,
}

// ---------------------------------------------------------------------------
// Window procedure
// ---------------------------------------------------------------------------

/// The global window procedure registered with our WNDCLASSEXW.
///
/// # Safety
///
/// This function is called by the Win32 message dispatcher. It reads the
/// `WindowData` pointer stored in `GWLP_USERDATA`. That pointer is valid
/// for the lifetime of the window because `Win32Platform` owns both the
/// window data and the event queue, and `destroy_window` cleans up the
/// user-data before the window handle becomes invalid.
unsafe extern "system" fn wndproc(
    hwnd: ffi::HWND,
    msg: ffi::UINT,
    wp: ffi::WPARAM,
    lp: ffi::LPARAM,
) -> ffi::LRESULT {
    // During WM_CREATE the user-data is not yet set; fall through to default.
    let user_ptr = unsafe { ffi::GetWindowLongPtrW(hwnd, ffi::GWLP_USERDATA) };
    if user_ptr == 0 {
        return unsafe { ffi::DefWindowProcW(hwnd, msg, wp, lp) };
    }

    // Safety: the pointer was set in `create_window` and points to a
    // heap-allocated `WindowData` that is valid until `destroy_window`.
    let wd = unsafe { &*(user_ptr as *const WindowData) };
    let handle = wd.handle;
    let queue = unsafe { &mut *wd.event_queue };

    match msg {
        ffi::WM_CLOSE => {
            queue.push_back(PlatformEvent::WindowCloseRequested { handle });
            // Return 0 to prevent DefWindowProc from calling DestroyWindow.
            return 0;
        }

        ffi::WM_DESTROY => {
            queue.push_back(PlatformEvent::WindowDestroyed { handle });
        }

        ffi::WM_SIZE => {
            let width = ffi::loword(lp as usize) as u32;
            let height = ffi::hiword(lp as usize) as u32;
            match wp {
                ffi::SIZE_MINIMIZED => {
                    queue.push_back(PlatformEvent::WindowMinimized { handle });
                }
                ffi::SIZE_MAXIMIZED => {
                    queue.push_back(PlatformEvent::WindowMaximized { handle });
                    queue.push_back(PlatformEvent::WindowResized {
                        handle,
                        width,
                        height,
                    });
                }
                _ => {
                    queue.push_back(PlatformEvent::WindowResized {
                        handle,
                        width,
                        height,
                    });
                }
            }
        }

        ffi::WM_MOVE => {
            let x = ffi::get_x_lparam(lp);
            let y = ffi::get_y_lparam(lp);
            queue.push_back(PlatformEvent::WindowMoved { handle, x, y });
        }

        ffi::WM_PAINT => {
            // Must call BeginPaint/EndPaint to validate the update region.
            let mut ps = ffi::PAINTSTRUCT::default();
            unsafe {
                ffi::BeginPaint(hwnd, &mut ps);
                ffi::EndPaint(hwnd, &ps);
            }
            queue.push_back(PlatformEvent::WindowRedraw { handle });
            return 0;
        }

        ffi::WM_ERASEBKGND => {
            // Suppress background erase -- we paint the entire client area.
            return 1;
        }

        ffi::WM_SETCURSOR => {
            // Hide the hardware cursor over the client area — we render
            // a software cursor into the framebuffer instead.
            if (lp & 0xFFFF) as i32 == ffi::HTCLIENT {
                unsafe {
                    ffi::SetCursor(std::ptr::null_mut());
                }
                return 1; // Handled
            }
        }

        ffi::WM_SETFOCUS => {
            queue.push_back(PlatformEvent::FocusGained { handle });
        }

        ffi::WM_KILLFOCUS => {
            queue.push_back(PlatformEvent::FocusLost { handle });
        }

        ffi::WM_KEYDOWN | ffi::WM_SYSKEYDOWN => {
            let vk = wp as u32;
            let scancode = ((lp as u32) >> 16) & 0x1FF; // bits 16-24
            let repeat = (lp & 0x40000000) != 0;
            if let Some(key) = input::vk_to_keycode(vk) {
                let state = if repeat {
                    KeyState::Repeat
                } else {
                    KeyState::Pressed
                };
                let mods = input::modifiers_from_state();
                let event = KeyEvent::new(key, state, mods, scancode, timestamp_us());
                queue.push_back(PlatformEvent::KeyInput { handle, event });
            }
            // Let DefWindowProc handle Alt+F4 etc. for SYSKEYDOWN.
            if msg == ffi::WM_SYSKEYDOWN {
                return unsafe { ffi::DefWindowProcW(hwnd, msg, wp, lp) };
            }
            return 0;
        }

        ffi::WM_KEYUP | ffi::WM_SYSKEYUP => {
            let vk = wp as u32;
            let scancode = ((lp as u32) >> 16) & 0x1FF;
            if let Some(key) = input::vk_to_keycode(vk) {
                let mods = input::modifiers_from_state();
                let event = KeyEvent::new(key, KeyState::Released, mods, scancode, timestamp_us());
                queue.push_back(PlatformEvent::KeyInput { handle, event });
            }
            if msg == ffi::WM_SYSKEYUP {
                return unsafe { ffi::DefWindowProcW(hwnd, msg, wp, lp) };
            }
            return 0;
        }

        ffi::WM_MOUSEMOVE => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            queue.push_back(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Move { x, y },
            });
            return 0;
        }

        ffi::WM_LBUTTONDOWN => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            queue.push_back(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                    x,
                    y,
                },
            });
            unsafe {
                ffi::SetCapture(hwnd);
            }
            return 0;
        }

        ffi::WM_LBUTTONUP => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            queue.push_back(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Left,
                    state: ButtonState::Released,
                    x,
                    y,
                },
            });
            unsafe {
                ffi::ReleaseCapture();
            }
            return 0;
        }

        ffi::WM_RBUTTONDOWN => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            queue.push_back(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Right,
                    state: ButtonState::Pressed,
                    x,
                    y,
                },
            });
            return 0;
        }

        ffi::WM_RBUTTONUP => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            queue.push_back(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Right,
                    state: ButtonState::Released,
                    x,
                    y,
                },
            });
            return 0;
        }

        ffi::WM_MBUTTONDOWN => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            queue.push_back(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Middle,
                    state: ButtonState::Pressed,
                    x,
                    y,
                },
            });
            return 0;
        }

        ffi::WM_MBUTTONUP => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            queue.push_back(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Middle,
                    state: ButtonState::Released,
                    x,
                    y,
                },
            });
            return 0;
        }

        ffi::WM_MOUSEWHEEL => {
            let delta = ffi::get_wheel_delta_wparam(wp) as f32 / ffi::WHEEL_DELTA as f32;
            // WM_MOUSEWHEEL coordinates are in screen space; convert to client.
            let mut pt = ffi::POINT {
                x: ffi::get_x_lparam(lp),
                y: ffi::get_y_lparam(lp),
            };
            unsafe {
                ffi::ScreenToClient(hwnd, &mut pt);
            }
            queue.push_back(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Scroll {
                    axis: ScrollAxis::Vertical,
                    delta,
                    x: pt.x as f32,
                    y: pt.y as f32,
                },
            });
            return 0;
        }

        ffi::WM_MOUSEHWHEEL => {
            let delta = ffi::get_wheel_delta_wparam(wp) as f32 / ffi::WHEEL_DELTA as f32;
            let mut pt = ffi::POINT {
                x: ffi::get_x_lparam(lp),
                y: ffi::get_y_lparam(lp),
            };
            unsafe {
                ffi::ScreenToClient(hwnd, &mut pt);
            }
            queue.push_back(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Scroll {
                    axis: ScrollAxis::Horizontal,
                    delta,
                    x: pt.x as f32,
                    y: pt.y as f32,
                },
            });
            return 0;
        }

        ffi::WM_DPICHANGED => {
            let new_dpi = ffi::loword(wp) as f32;
            let dpi_scale = new_dpi / 96.0;
            queue.push_back(PlatformEvent::DpiChanged { handle, dpi_scale });
            // Move / resize window to the suggested rectangle.
            if lp != 0 {
                let suggested = unsafe { &*(lp as *const ffi::RECT) };
                unsafe {
                    ffi::SetWindowPos(
                        hwnd,
                        ffi::HWND_TOP,
                        suggested.left,
                        suggested.top,
                        suggested.right - suggested.left,
                        suggested.bottom - suggested.top,
                        ffi::SWP_NOZORDER,
                    );
                }
            }
            return 0;
        }

        _ => {}
    }

    unsafe { ffi::DefWindowProcW(hwnd, msg, wp, lp) }
}

// ---------------------------------------------------------------------------
// Window info stored on the Rust side
// ---------------------------------------------------------------------------

/// Metadata for a window managed by the platform backend.
#[allow(dead_code)]
struct WindowInfo {
    hwnd: ffi::HWND,
    handle: NativeWindowHandle,
    _data: Box<WindowData>,
}

// ---------------------------------------------------------------------------
// Win32 display backend
// ---------------------------------------------------------------------------

/// Display backend that queries monitor information via Win32 APIs.
struct Win32DisplayBackend;

impl Win32DisplayBackend {
    fn enumerate_monitors(&self) -> Vec<MonitorInfo> {
        let mut result: Vec<MonitorInfo> = Vec::new();

        // Safety: EnumDisplayMonitors calls our callback for each monitor.
        // The callback pushes a `MonitorInfo` for each monitor into the Vec
        // via the LPARAM pointer.
        unsafe {
            ffi::EnumDisplayMonitors(
                ptr::null_mut(),
                ptr::null(),
                Some(monitor_enum_callback),
                &mut result as *mut Vec<MonitorInfo> as ffi::LPARAM,
            );
        }
        result
    }
}

/// Callback invoked by `EnumDisplayMonitors` for each connected monitor.
///
/// # Safety
///
/// `lparam` must point to a valid `Vec<MonitorInfo>` whose lifetime spans the
/// entire `EnumDisplayMonitors` call. This is guaranteed by `enumerate_monitors`.
unsafe extern "system" fn monitor_enum_callback(
    hmonitor: ffi::HMONITOR,
    _hdc: ffi::HDC,
    _lprc: *mut ffi::RECT,
    lparam: ffi::LPARAM,
) -> ffi::BOOL {
    let monitors = unsafe { &mut *(lparam as *mut Vec<MonitorInfo>) };

    let mut mi = ffi::MONITORINFOEXW::default();
    mi.base.cbSize = std::mem::size_of::<ffi::MONITORINFOEXW>() as ffi::DWORD;

    if unsafe { ffi::GetMonitorInfoW(hmonitor, &mut mi) } == 0 {
        return ffi::TRUE;
    }

    let id = monitors.len() as u32;
    let name = from_wide(&mi.szDevice);
    let primary = (mi.base.dwFlags & ffi::MONITORINFOF_PRIMARY) != 0;

    let geometry = Rect::new(
        mi.base.rcMonitor.left as f32,
        mi.base.rcMonitor.top as f32,
        (mi.base.rcMonitor.right - mi.base.rcMonitor.left) as f32,
        (mi.base.rcMonitor.bottom - mi.base.rcMonitor.top) as f32,
    );
    let work_area = Rect::new(
        mi.base.rcWork.left as f32,
        mi.base.rcWork.top as f32,
        (mi.base.rcWork.right - mi.base.rcWork.left) as f32,
        (mi.base.rcWork.bottom - mi.base.rcWork.top) as f32,
    );

    monitors.push(MonitorInfo {
        id,
        name,
        geometry,
        work_area,
        dpi_scale: 1.0, // proper DPI requires shcore; default to 1.0
        primary,
        refresh_rate_hz: 60, // best-effort default
    });

    ffi::TRUE
}

impl DisplayBackend for Win32DisplayBackend {
    fn monitors(&self) -> Vec<MonitorInfo> {
        self.enumerate_monitors()
    }

    fn primary_monitor(&self) -> Option<MonitorInfo> {
        self.enumerate_monitors().into_iter().find(|m| m.primary)
    }

    fn virtual_screen_rect(&self) -> Rect {
        // Safety: GetSystemMetrics is always safe to call.
        let w = unsafe { ffi::GetSystemMetrics(ffi::SM_CXSCREEN) };
        let h = unsafe { ffi::GetSystemMetrics(ffi::SM_CYSCREEN) };
        Rect::new(0.0, 0.0, w as f32, h as f32)
    }
}

// Safety: Win32DisplayBackend has no mutable state; all queries call
// thread-safe Win32 APIs.
unsafe impl Send for Win32DisplayBackend {}

// ---------------------------------------------------------------------------
// Win32 window host
// ---------------------------------------------------------------------------

/// Window host implementation backed by `CreateWindowExW`.
struct Win32WindowHost {
    /// Map from our `NativeWindowHandle` to the Win32 HWND and metadata.
    windows: HashMap<u64, WindowInfo>,
    /// Monotonically increasing counter for handle generation.
    next_handle: u64,
    /// HINSTANCE used for window creation.
    hinstance: ffi::HINSTANCE,
    /// Class name (wide) kept alive for `CreateWindowExW`.
    class_name: Vec<u16>,
    /// Shared event queue (raw pointer because `WindowData` needs it).
    event_queue: *mut VecDeque<PlatformEvent>,
}

// Safety: Win32WindowHost is !Send by default due to raw pointers. However,
// the struct (and the platform that owns it) is only accessed from the thread
// that created the message loop, which is the same thread that created the
// windows. We enforce this structurally.
unsafe impl Send for Win32WindowHost {}

impl NativeWindowHost for Win32WindowHost {
    fn create_window(&mut self, params: NativeWindowParams) -> PlatformResult<NativeWindowHandle> {
        let handle = NativeWindowHandle(self.next_handle);
        self.next_handle += 1;

        let title = to_wide(&params.title);

        // Choose window style based on window_type:
        //   "desktop" → borderless popup covering the full screen
        //   anything else → normal overlapped window
        let is_desktop = params.window_type == "desktop";

        let (style, ex_style) = if is_desktop {
            (ffi::WS_POPUP | ffi::WS_VISIBLE, ffi::WS_EX_APPWINDOW)
        } else {
            (ffi::WS_OVERLAPPEDWINDOW, ffi::WS_EX_APPWINDOW)
        };

        let (x, y, w, h) = if is_desktop {
            // Full primary screen dimensions.
            let sw = unsafe { ffi::GetSystemMetrics(ffi::SM_CXSCREEN) };
            let sh = unsafe { ffi::GetSystemMetrics(ffi::SM_CYSCREEN) };
            (0, 0, sw, sh)
        } else {
            let x = if params.geometry.x == 0.0 && params.geometry.y == 0.0 {
                ffi::CW_USEDEFAULT
            } else {
                params.geometry.x as i32
            };
            let y = if params.geometry.x == 0.0 && params.geometry.y == 0.0 {
                ffi::CW_USEDEFAULT
            } else {
                params.geometry.y as i32
            };
            let w = if params.geometry.width > 0.0 {
                params.geometry.width as i32
            } else {
                ffi::CW_USEDEFAULT
            };
            let h = if params.geometry.height > 0.0 {
                params.geometry.height as i32
            } else {
                ffi::CW_USEDEFAULT
            };
            (x, y, w, h)
        };

        let parent = match params.parent {
            Some(p) => {
                // Look up the HWND of the parent.
                self.windows
                    .get(&p.0)
                    .map(|info| info.hwnd)
                    .unwrap_or(ptr::null_mut())
            }
            None => ptr::null_mut(),
        };

        // Allocate per-window data on the heap.
        let data = Box::new(WindowData {
            handle,
            event_queue: self.event_queue,
        });

        // Safety: CreateWindowExW creates a native Win32 window. All
        // parameters are valid: the class was registered in `new()`, the
        // hinstance is the process module handle, and the title is a
        // null-terminated wide string.
        let hwnd = unsafe {
            ffi::CreateWindowExW(
                ex_style,
                self.class_name.as_ptr(),
                title.as_ptr(),
                style,
                x,
                y,
                w,
                h,
                parent,
                ptr::null_mut(),
                self.hinstance,
                ptr::null_mut(),
            )
        };

        if hwnd.is_null() {
            return Err(PlatformError::Window(format!(
                "CreateWindowExW failed (error {})",
                unsafe { ffi::GetLastError() }
            )));
        }

        // Store the WindowData pointer in the window's user-data slot so that
        // wndproc can access the event queue.
        // Safety: `data` is heap-allocated and will remain valid until
        // `destroy_window` deallocates it.
        let data_ptr = &*data as *const WindowData;
        unsafe {
            ffi::SetWindowLongPtrW(hwnd, ffi::GWLP_USERDATA, data_ptr as ffi::LONG_PTR);
        }

        // Show the window.
        unsafe {
            ffi::ShowWindow(hwnd, ffi::SW_SHOW);
            ffi::UpdateWindow(hwnd);
        }

        let info = WindowInfo {
            hwnd,
            handle,
            _data: data,
        };
        self.windows.insert(handle.0, info);

        // Push a WindowCreated event.
        let mut rc = ffi::RECT::default();
        unsafe {
            ffi::GetClientRect(hwnd, &mut rc);
        }
        let width = (rc.right - rc.left) as u32;
        let height = (rc.bottom - rc.top) as u32;
        // Safety: the event_queue pointer is valid for the lifetime of
        // the platform.
        unsafe {
            (*self.event_queue).push_back(PlatformEvent::WindowCreated {
                handle,
                width,
                height,
            });
        }

        Ok(handle)
    }

    fn destroy_window(&mut self, handle: NativeWindowHandle) -> PlatformResult<()> {
        if let Some(info) = self.windows.remove(&handle.0) {
            // Clear user-data before destroying so wndproc won't
            // dereference a dangling pointer during teardown messages.
            unsafe {
                ffi::SetWindowLongPtrW(info.hwnd, ffi::GWLP_USERDATA, 0);
                ffi::DestroyWindow(info.hwnd);
            }
            // `info._data` (the Box<WindowData>) is dropped here.
        }
        Ok(())
    }

    fn set_geometry(&mut self, handle: NativeWindowHandle, geometry: Rect) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            // Safety: MoveWindow is safe to call on a valid HWND.
            unsafe {
                ffi::MoveWindow(
                    info.hwnd,
                    geometry.x as i32,
                    geometry.y as i32,
                    geometry.width as i32,
                    geometry.height as i32,
                    ffi::TRUE,
                );
            }
        }
        Ok(())
    }

    fn set_title(&mut self, handle: NativeWindowHandle, title: &str) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            let wide = to_wide(title);
            // Safety: SetWindowTextW is safe on a valid HWND with a
            // null-terminated wide string.
            unsafe {
                ffi::SetWindowTextW(info.hwnd, wide.as_ptr());
            }
        }
        Ok(())
    }

    fn set_icon(&mut self, _handle: NativeWindowHandle, _icon_data: &[u8]) -> PlatformResult<()> {
        // Setting a window icon from raw pixel data requires creating an
        // HICON via CreateIconIndirect, which is non-trivial. For now we
        // accept the call but do nothing.
        Ok(())
    }

    fn set_state(&mut self, handle: NativeWindowHandle, state: &str) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            let cmd = match state {
                "maximized" => ffi::SW_MAXIMIZE,
                "minimized" => ffi::SW_MINIMIZE,
                "restored" | "normal" => ffi::SW_RESTORE,
                "hidden" => ffi::SW_HIDE,
                _ => ffi::SW_SHOW,
            };
            unsafe {
                ffi::ShowWindow(info.hwnd, cmd);
            }
        }
        Ok(())
    }

    fn set_z_order(&mut self, handle: NativeWindowHandle, z_order: i32) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            let insert_after = if z_order > 0 {
                ffi::HWND_TOPMOST
            } else {
                ffi::HWND_TOP
            };
            unsafe {
                ffi::SetWindowPos(
                    info.hwnd,
                    insert_after,
                    0,
                    0,
                    0,
                    0,
                    ffi::SWP_NOMOVE | ffi::SWP_NOSIZE,
                );
            }
        }
        Ok(())
    }

    fn set_focus(&mut self, handle: NativeWindowHandle) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            unsafe {
                ffi::SetForegroundWindow(info.hwnd);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Win32 taskbar integration (minimal stub)
// ---------------------------------------------------------------------------

/// Minimal taskbar integration.
///
/// Full ITaskbarList3 integration requires COM; this implementation
/// silently accepts all calls.
struct Win32Taskbar;

impl TaskbarIntegration for Win32Taskbar {
    fn set_progress(&mut self, _handle: u64, _progress: f64) -> PlatformResult<()> {
        // Requires ITaskbarList3 (COM). Stubbed.
        Ok(())
    }

    fn set_overlay_icon(&mut self, _handle: u64, _icon_data: &[u8]) -> PlatformResult<()> {
        Ok(())
    }

    fn set_badge_count(&mut self, _count: u32) -> PlatformResult<()> {
        Ok(())
    }

    fn add_jump_list_item(&mut self, _item: JumpListItem) -> PlatformResult<()> {
        Ok(())
    }
}

unsafe impl Send for Win32Taskbar {}

// ---------------------------------------------------------------------------
// Win32 tray icon backend
// ---------------------------------------------------------------------------

/// System tray backend using `Shell_NotifyIconW`.
struct Win32Tray {
    /// Monotonically increasing tray icon ID.
    next_id: u64,
    /// Active tray icons keyed by our handle ID.
    icons: HashMap<u64, TrayIconInfo>,
    /// Hidden message-only window that receives tray callbacks.
    msg_hwnd: ffi::HWND,
}

struct TrayIconInfo {
    uid: u32,
}

// Safety: the struct is only used from the platform's owning thread.
unsafe impl Send for Win32Tray {}

impl Win32Tray {
    fn new(msg_hwnd: ffi::HWND) -> Self {
        Self {
            next_id: 1,
            icons: HashMap::new(),
            msg_hwnd,
        }
    }
}

impl NativeTray for Win32Tray {
    fn add_icon(&mut self, params: NativeTrayParams) -> PlatformResult<NativeTrayHandle> {
        let handle_id = self.next_id;
        self.next_id += 1;
        let uid = handle_id as u32;

        let mut nid = ffi::NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<ffi::NOTIFYICONDATAW>() as ffi::DWORD;
        nid.hWnd = self.msg_hwnd;
        nid.uID = uid;
        nid.uFlags = ffi::NIF_MESSAGE | ffi::NIF_TIP;
        nid.uCallbackMessage = ffi::WM_USER + 1;

        // Copy tooltip (up to 127 chars).
        let tip = to_wide(&params.tooltip);
        let copy_len = tip.len().min(127);
        nid.szTip[..copy_len].copy_from_slice(&tip[..copy_len]);

        // Safety: Shell_NotifyIconW with NIM_ADD adds a tray icon.
        let ok = unsafe { ffi::Shell_NotifyIconW(ffi::NIM_ADD, &mut nid) };
        if ok == 0 {
            return Err(PlatformError::Tray(
                "Shell_NotifyIconW NIM_ADD failed".into(),
            ));
        }

        self.icons.insert(handle_id, TrayIconInfo { uid });
        Ok(NativeTrayHandle(handle_id))
    }

    fn update_icon(&mut self, handle: NativeTrayHandle, update: TrayUpdate) -> PlatformResult<()> {
        let info = self
            .icons
            .get(&handle.0)
            .ok_or_else(|| PlatformError::Tray("unknown tray handle".into()))?;

        let mut nid = ffi::NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<ffi::NOTIFYICONDATAW>() as ffi::DWORD;
        nid.hWnd = self.msg_hwnd;
        nid.uID = info.uid;

        if let Some(ref tooltip) = update.tooltip {
            nid.uFlags |= ffi::NIF_TIP;
            let tip = to_wide(tooltip);
            let copy_len = tip.len().min(127);
            nid.szTip[..copy_len].copy_from_slice(&tip[..copy_len]);
        }

        unsafe {
            ffi::Shell_NotifyIconW(ffi::NIM_MODIFY, &mut nid);
        }
        Ok(())
    }

    fn remove_icon(&mut self, handle: NativeTrayHandle) -> PlatformResult<()> {
        if let Some(info) = self.icons.remove(&handle.0) {
            let mut nid = ffi::NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<ffi::NOTIFYICONDATAW>() as ffi::DWORD;
            nid.hWnd = self.msg_hwnd;
            nid.uID = info.uid;
            unsafe {
                ffi::Shell_NotifyIconW(ffi::NIM_DELETE, &mut nid);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Win32 notifications (basic implementation)
// ---------------------------------------------------------------------------

/// Desktop notification backend.
///
/// Uses the balloon tip feature of `Shell_NotifyIconW` for a minimal
/// implementation. Full toast notifications require COM (Windows Runtime).
struct Win32Notifications {
    next_id: u32,
}

unsafe impl Send for Win32Notifications {}

impl Win32Notifications {
    fn new() -> Self {
        Self { next_id: 1 }
    }
}

impl NativeNotifications for Win32Notifications {
    fn show(&mut self, _params: NativeNotificationParams) -> PlatformResult<u32> {
        let id = self.next_id;
        self.next_id += 1;
        // Full toast notification requires WinRT / COM; return a unique ID
        // and silently drop the notification content.
        Ok(id)
    }

    fn dismiss(&mut self, _id: u32) -> PlatformResult<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Win32 keymap translator
// ---------------------------------------------------------------------------

/// Keymap translator that delegates to `input::scancode_to_keycode`.
struct Win32Keymap;

unsafe impl Send for Win32Keymap {}

impl KeymapTranslator for Win32Keymap {
    fn translate_scancode(&self, scancode: u32) -> Option<KeyCode> {
        input::scancode_to_keycode(scancode)
    }

    fn platform_name(&self) -> &str {
        "win32"
    }
}

// ---------------------------------------------------------------------------
// Win32Platform — the top-level backend
// ---------------------------------------------------------------------------

/// Win32 platform backend.
///
/// Manages the window class, event queue, and all sub-backends for the
/// Windows desktop.
pub struct Win32Platform {
    // Sub-backends
    display: Win32DisplayBackend,
    window_host: Win32WindowHost,
    taskbar: Win32Taskbar,
    tray: Win32Tray,
    notifications: Win32Notifications,
    drag_drop: NullDragDrop,
    keymap: Win32Keymap,

    /// Shared event queue. Kept in a `Box` so that its heap address is
    /// stable for `WindowData` pointers.
    event_queue: Box<VecDeque<PlatformEvent>>,

    /// ATOM from `RegisterClassExW` (needed for unregistration).
    class_atom: ffi::ATOM,

    /// Process instance handle.
    hinstance: ffi::HINSTANCE,

    /// Wide class name (kept alive so the ATOM remains valid).
    class_name_wide: Vec<u16>,

    /// Hidden message-only window for tray callbacks, etc.
    msg_hwnd: ffi::HWND,
}

// Safety: Win32Platform owns all raw handles and is designed to be used
// from a single thread (the one that runs the message loop). The `Send`
// bound on `PlatformBackend` is satisfied by structural guarantees.
unsafe impl Send for Win32Platform {}

impl Win32Platform {
    /// Create and initialise a new Win32 platform backend.
    ///
    /// Registers a window class and creates a hidden message-only window for
    /// tray icon callbacks. Fails if `RegisterClassExW` fails.
    pub fn new() -> PlatformResult<Self> {
        // Safety: GetModuleHandleW(null) returns the HINSTANCE of the
        // running executable, which is always valid.
        let hinstance = unsafe { ffi::GetModuleHandleW(ptr::null()) };
        if hinstance.is_null() {
            return Err(PlatformError::Other(
                "GetModuleHandleW returned null".into(),
            ));
        }

        let class_name = "LiquiDE_Win32_Window";
        let class_name_wide = to_wide(class_name);

        let wc = ffi::WNDCLASSEXW {
            cbSize: std::mem::size_of::<ffi::WNDCLASSEXW>() as ffi::UINT,
            style: ffi::CS_HREDRAW | ffi::CS_VREDRAW | ffi::CS_OWNDC,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: ptr::null_mut(),
            hCursor: unsafe { ffi::LoadCursorW(ptr::null_mut(), ffi::IDC_ARROW) },
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name_wide.as_ptr(),
            hIconSm: ptr::null_mut(),
        };

        // Safety: RegisterClassExW registers the window class. The struct is
        // fully initialised above.
        let class_atom = unsafe { ffi::RegisterClassExW(&wc) };
        if class_atom == 0 {
            return Err(PlatformError::Other(format!(
                "RegisterClassExW failed (error {})",
                unsafe { ffi::GetLastError() }
            )));
        }

        // Create a hidden message-only window for tray icon callbacks.
        let msg_class = "LiquiDE_MsgOnly";
        let msg_class_wide = to_wide(msg_class);

        let msg_wc = ffi::WNDCLASSEXW {
            cbSize: std::mem::size_of::<ffi::WNDCLASSEXW>() as ffi::UINT,
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: msg_class_wide.as_ptr(),
            hIconSm: ptr::null_mut(),
        };
        unsafe {
            ffi::RegisterClassExW(&msg_wc);
        }

        let msg_title = to_wide("");
        let msg_hwnd = unsafe {
            ffi::CreateWindowExW(
                0,
                msg_class_wide.as_ptr(),
                msg_title.as_ptr(),
                0, // not visible
                0,
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                hinstance,
                ptr::null_mut(),
            )
        };

        // Allocate a stable event queue on the heap.  The Box provides a
        // stable heap address that survives moves of the containing struct.
        let mut event_queue = Box::new(VecDeque::<PlatformEvent>::with_capacity(256));

        // Safety: obtain a raw mutable pointer with write provenance from the
        // mutable reference.  The temporary `&mut` borrow ends at the
        // semicolon so it does not conflict with later accesses.
        let eq_ptr: *mut VecDeque<PlatformEvent> = &mut *event_queue;

        let window_host = Win32WindowHost {
            windows: HashMap::new(),
            next_handle: 1,
            hinstance,
            class_name: class_name_wide.clone(),
            event_queue: eq_ptr,
        };

        let tray = Win32Tray::new(msg_hwnd);

        Ok(Self {
            display: Win32DisplayBackend,
            window_host,
            taskbar: Win32Taskbar,
            tray,
            notifications: Win32Notifications::new(),
            drag_drop: NullDragDrop,
            keymap: Win32Keymap,
            event_queue,
            class_atom,
            hinstance,
            class_name_wide,
            msg_hwnd,
        })
    }

    /// Pump all pending Win32 messages and dispatch them through the wndproc,
    /// which pushes `PlatformEvent`s into `self.event_queue`.
    fn pump_messages(&mut self) {
        let mut msg = ffi::MSG::default();
        // Safety: PeekMessageW retrieves messages from the calling thread's
        // queue. The MSG struct is valid.
        while unsafe { ffi::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, ffi::PM_REMOVE) != 0 } {
            if msg.message == ffi::WM_QUIT {
                self.event_queue.push_back(PlatformEvent::Quit);
                return;
            }
            // Safety: TranslateMessage and DispatchMessageW are standard
            // message loop calls and are safe with a valid MSG.
            unsafe {
                ffi::TranslateMessage(&msg);
                ffi::DispatchMessageW(&msg);
            }
        }
    }
}

impl Drop for Win32Platform {
    fn drop(&mut self) {
        // Destroy all tracked windows.
        let handles: Vec<u64> = self.window_host.windows.keys().copied().collect();
        for h in handles {
            if let Some(info) = self.window_host.windows.remove(&h) {
                unsafe {
                    ffi::SetWindowLongPtrW(info.hwnd, ffi::GWLP_USERDATA, 0);
                    ffi::DestroyWindow(info.hwnd);
                }
            }
        }

        // Remove all tray icons.
        let tray_ids: Vec<u64> = self.tray.icons.keys().copied().collect();
        for id in tray_ids {
            let _ = self.tray.remove_icon(NativeTrayHandle(id));
        }

        // Destroy the hidden message window.
        if !self.msg_hwnd.is_null() {
            unsafe {
                ffi::DestroyWindow(self.msg_hwnd);
            }
        }

        // Unregister the window class.
        if self.class_atom != 0 {
            unsafe {
                ffi::UnregisterClassW(self.class_name_wide.as_ptr(), self.hinstance);
            }
        }
    }
}

impl PlatformBackend for Win32Platform {
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
        "win32"
    }

    // ── Event loop ───────────────────────────────────────────────────

    fn poll_event(&mut self) -> Option<PlatformEvent> {
        // First drain any already-queued events.
        if let Some(ev) = self.event_queue.pop_front() {
            return Some(ev);
        }
        // Pump the Win32 message queue.
        self.pump_messages();
        // Return the next event, if any.
        self.event_queue.pop_front()
    }

    fn wait_event(&mut self) -> PlatformEvent {
        // If we already have queued events, return immediately.
        if let Some(ev) = self.event_queue.pop_front() {
            return ev;
        }

        // Block until a message arrives.
        let mut msg = ffi::MSG::default();
        // Safety: GetMessageW blocks until a message is available. A return
        // value of 0 means WM_QUIT, and -1 is an error (treated as Quit).
        let ret = unsafe { ffi::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) };

        if ret <= 0 {
            return PlatformEvent::Quit;
        }

        unsafe {
            ffi::TranslateMessage(&msg);
            ffi::DispatchMessageW(&msg);
        }

        // Drain any additional pending messages.
        self.pump_messages();

        // Return the first queued event, or Quit as fallback.
        self.event_queue.pop_front().unwrap_or(PlatformEvent::Quit)
    }

    // ── Frame presentation ───────────────────────────────────────────

    fn present_frame(
        &mut self,
        handle: NativeWindowHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
        _stride: u32,
        format: PixelFormat,
    ) -> PlatformResult<()> {
        // We only support BGRA8, which maps directly to Win32's 32-bit
        // BI_RGB (which is actually BGRA in memory).
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

        let hwnd = info.hwnd;

        let mut bmi = ffi::BITMAPINFO {
            bmiHeader: ffi::BITMAPINFOHEADER {
                biSize: std::mem::size_of::<ffi::BITMAPINFOHEADER>() as ffi::DWORD,
                biWidth: width as ffi::LONG,
                // Negative height → top-down bitmap (row 0 is the top row).
                biHeight: -(height as ffi::LONG),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: ffi::BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [ffi::RGBQUAD::default()],
        };

        // Safety: GetDC / SetDIBitsToDevice / ReleaseDC are standard GDI
        // calls. The pixel buffer must be valid for `width * height * 4`
        // bytes, which is checked by the caller.
        unsafe {
            let hdc = ffi::GetDC(hwnd);
            if hdc.is_null() {
                return Err(PlatformError::Presentation("GetDC returned null".into()));
            }

            ffi::SetDIBitsToDevice(
                hdc,
                0,      // xDest
                0,      // yDest
                width,  // dwWidth
                height, // dwHeight
                0,      // xSrc
                0,      // ySrc
                0,      // uStartScan
                height, // cScanLines
                pixels.as_ptr() as *const c_void,
                &mut bmi,
                ffi::DIB_RGB_COLORS,
            );

            ffi::ReleaseDC(hwnd, hdc);
        }

        Ok(())
    }

    fn request_redraw(&mut self, handle: NativeWindowHandle) {
        if let Some(info) = self.window_host.windows.get(&handle.0) {
            // Safety: InvalidateRect with a null RECT invalidates the entire
            // client area, causing a WM_PAINT message to be posted.
            unsafe {
                ffi::InvalidateRect(info.hwnd, ptr::null(), ffi::FALSE);
            }
        }
    }
}
