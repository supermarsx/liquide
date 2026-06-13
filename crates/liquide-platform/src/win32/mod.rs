//! Win32 platform backend.
//!
//! Provides a complete `PlatformBackend` implementation using raw Win32 API
//! calls via FFI (no external crate dependencies). Links against user32.dll,
//! gdi32.dll, kernel32.dll, and shell32.dll at load time.

pub mod dxgi;
pub mod ffi;
pub mod input;

pub use dxgi::{DxgiPresentCapabilities, DxgiPresentMode};

use std::cell::UnsafeCell;
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
use crate::{NativeDragDrop, PlatformBackend, PlatformError, PlatformResult, PresentFeedback};

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

fn timestamp_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
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
    /// Wrapped in `UnsafeCell` to avoid creating `&mut` references in the
    /// reentrant wndproc (which would alias with other borrows).
    event_queue: *const UnsafeCell<VecDeque<PlatformEvent>>,
    /// Current hardware cursor handle. When non-null, the wndproc uses
    /// this for `WM_SETCURSOR` so the OS renders the cursor shape.
    cursor: std::sync::atomic::AtomicIsize,
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
    // SAFETY: GetWindowLongPtrW is safe on any valid HWND; returns 0 if not yet set.
    let user_ptr = unsafe { ffi::GetWindowLongPtrW(hwnd, ffi::GWLP_USERDATA) };
    if user_ptr == 0 {
        // SAFETY: DefWindowProcW is safe to call with any valid window message.
        return unsafe { ffi::DefWindowProcW(hwnd, msg, wp, lp) };
    }

    // Safety: the pointer was set in `create_window` and points to a
    // heap-allocated `WindowData` that is valid until `destroy_window`.
    let wd = unsafe { &*(user_ptr as *const WindowData) };
    let handle = wd.handle;

    // Use a macro to push events via raw pointer operations to avoid
    // creating a `&mut` reference, which would be UB due to reentrancy.
    macro_rules! push_event {
        ($event:expr) => {
            // SAFETY: The event_queue pointer is valid for the window's lifetime.
            // We use raw pointer ops to avoid creating `&mut` references (UB due to reentrancy).
            unsafe { (*(*wd.event_queue).get()).push_back($event) }
        };
    }

    match msg {
        ffi::WM_CLOSE => {
            push_event!(PlatformEvent::WindowCloseRequested { handle });
            // Return 0 to prevent DefWindowProc from calling DestroyWindow.
            return 0;
        }

        ffi::WM_DESTROY => {
            push_event!(PlatformEvent::WindowDestroyed { handle });
        }

        ffi::WM_SIZE => {
            let width = ffi::loword(lp as usize) as u32;
            let height = ffi::hiword(lp as usize) as u32;
            match wp {
                ffi::SIZE_MINIMIZED => {
                    push_event!(PlatformEvent::WindowMinimized { handle });
                }
                ffi::SIZE_MAXIMIZED => {
                    push_event!(PlatformEvent::WindowMaximized { handle });
                    push_event!(PlatformEvent::WindowResized {
                        handle,
                        width,
                        height,
                    });
                }
                _ => {
                    push_event!(PlatformEvent::WindowResized {
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
            push_event!(PlatformEvent::WindowMoved { handle, x, y });
        }

        ffi::WM_PAINT => {
            // Must call BeginPaint/EndPaint to validate the update region.
            let mut ps = ffi::PAINTSTRUCT::default();
            // SAFETY: BeginPaint/EndPaint are safe on a valid HWND during WM_PAINT.
            // The PAINTSTRUCT is stack-allocated and properly initialized.
            unsafe {
                ffi::BeginPaint(hwnd, &mut ps);
                ffi::EndPaint(hwnd, &ps);
            }
            push_event!(PlatformEvent::WindowRedraw { handle });
            return 0;
        }

        ffi::WM_ERASEBKGND => {
            // Suppress background erase -- we paint the entire client area.
            return 1;
        }

        ffi::WM_SETCURSOR => {
            // Over the client area, use our stored cursor handle.
            // If the handle is non-null, show the hardware cursor.
            // If null, hide it (software cursor mode).
            if (lp & 0xFFFF) as i32 == ffi::HTCLIENT {
                let cursor_val = wd.cursor.load(std::sync::atomic::Ordering::Relaxed);
                // SAFETY: SetCursor accepts any cursor handle (or null to hide).
                unsafe {
                    ffi::SetCursor(cursor_val as ffi::HCURSOR);
                }
                return 1; // Handled
            }
        }

        ffi::WM_SETFOCUS => {
            push_event!(PlatformEvent::FocusGained { handle });
        }

        ffi::WM_KILLFOCUS => {
            push_event!(PlatformEvent::FocusLost { handle });
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
                push_event!(PlatformEvent::KeyInput { handle, event });
            }
            // Let DefWindowProc handle Alt+F4 etc. for SYSKEYDOWN.
            if msg == ffi::WM_SYSKEYDOWN {
                // SAFETY: DefWindowProcW is safe for default handling of system key messages.
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
                push_event!(PlatformEvent::KeyInput { handle, event });
            }
            if msg == ffi::WM_SYSKEYUP {
                // SAFETY: DefWindowProcW is safe for default handling of system key messages.
                return unsafe { ffi::DefWindowProcW(hwnd, msg, wp, lp) };
            }
            return 0;
        }

        ffi::WM_MOUSEMOVE => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            push_event!(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Move { x, y },
            });
            return 0;
        }

        ffi::WM_LBUTTONDOWN => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            push_event!(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                    x,
                    y,
                },
            });
            // SAFETY: SetCapture is safe on a valid HWND to capture mouse input.
            unsafe {
                ffi::SetCapture(hwnd);
            }
            return 0;
        }

        ffi::WM_LBUTTONUP => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            push_event!(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Left,
                    state: ButtonState::Released,
                    x,
                    y,
                },
            });
            // SAFETY: ReleaseCapture is always safe to call.
            unsafe {
                ffi::ReleaseCapture();
            }
            return 0;
        }

        ffi::WM_RBUTTONDOWN => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            push_event!(PlatformEvent::MouseInput {
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
            push_event!(PlatformEvent::MouseInput {
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
            push_event!(PlatformEvent::MouseInput {
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
            push_event!(PlatformEvent::MouseInput {
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
            // SAFETY: ScreenToClient is safe on a valid HWND with a valid POINT pointer.
            unsafe {
                ffi::ScreenToClient(hwnd, &mut pt);
            }
            push_event!(PlatformEvent::MouseInput {
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
            // SAFETY: ScreenToClient is safe on a valid HWND with a valid POINT pointer.
            unsafe {
                ffi::ScreenToClient(hwnd, &mut pt);
            }
            push_event!(PlatformEvent::MouseInput {
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
            push_event!(PlatformEvent::DpiChanged { handle, dpi_scale });
            // Move / resize window to the suggested rectangle.
            if lp != 0 {
                // SAFETY: lp is non-zero, so it points to the suggested RECT
                // provided by the OS in the WM_DPICHANGED message.
                let suggested = unsafe { &*(lp as *const ffi::RECT) };
                // SAFETY: SetWindowPos is safe on a valid HWND with valid parameters.
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

    // SAFETY: DefWindowProcW handles any unprocessed message on a valid HWND.
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
    /// DXGI swap-chain presenter (lazily initialized on first present).
    dxgi: Option<dxgi::DxgiPresenter>,
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
    // SAFETY: lparam is a valid pointer to Vec<MonitorInfo>, guaranteed by the caller.
    let monitors = unsafe { &mut *(lparam as *mut Vec<MonitorInfo>) };

    let mut mi = ffi::MONITORINFOEXW::default();
    mi.base.cbSize = std::mem::size_of::<ffi::MONITORINFOEXW>() as ffi::DWORD;

    // SAFETY: GetMonitorInfoW is safe with a valid HMONITOR and properly sized struct.
    if unsafe { ffi::GetMonitorInfoW(hmonitor, &mut mi) } == 0 {
        return ffi::TRUE;
    }

    let id = monitors.len() as u32;
    let name = from_wide(&mi.szDevice);
    let primary = (mi.base.dwFlags & ffi::MONITORINFOF_PRIMARY) != 0;
    let refresh_rate_hz = query_refresh_rate_hz(&mi.szDevice).unwrap_or(60);

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
        refresh_rate_hz,
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

pub fn refresh_rate_hz_from_devmode_frequency(frequency: u32) -> Option<u32> {
    (frequency > 1).then_some(frequency)
}

fn query_refresh_rate_hz(device_name: &[u16; ffi::CCHDEVICENAME]) -> Option<u32> {
    let mut dev_mode = ffi::DEVMODEW::default();
    dev_mode.dmSize = std::mem::size_of::<ffi::DEVMODEW>() as ffi::WORD;

    // SAFETY: device_name is a valid monitor device string returned by
    // GetMonitorInfoW and dev_mode is a properly sized output buffer.
    let result = unsafe {
        ffi::EnumDisplaySettingsW(
            device_name.as_ptr(),
            ffi::ENUM_CURRENT_SETTINGS,
            &mut dev_mode,
        )
    };

    (result != 0)
        .then_some(dev_mode.dmDisplayFrequency)
        .and_then(refresh_rate_hz_from_devmode_frequency)
}

#[derive(Debug, Default)]
struct Win32PresentFeedbackState {
    submitted_present_count: u64,
    feedback_queue: VecDeque<PresentFeedback>,
}

impl Win32PresentFeedbackState {
    fn record_accepted_present(&mut self, timestamp_ns: u64) {
        self.submitted_present_count = self.submitted_present_count.saturating_add(1);
        let acknowledged_present_count = self.submitted_present_count;

        self.feedback_queue.push_back(PresentFeedback {
            acknowledged_present_count,
            sequence: (acknowledged_present_count <= u32::MAX as u64)
                .then_some(acknowledged_present_count as u32),
            timestamp_ns: Some(timestamp_ns),
            crtc_id: None,
        });
    }

    fn take_feedback(&mut self) -> Option<PresentFeedback> {
        self.feedback_queue.pop_front()
    }
}

// ---------------------------------------------------------------------------
// Win32 window host
// ---------------------------------------------------------------------------

/// Convert a desired CLIENT-area size into the OUTER window size that
/// `CreateWindowExW` needs so that the resulting client area matches the
/// request, accounting for the window's title bar / borders.
///
/// Uses `AdjustWindowRectEx`; on failure (it should not fail for valid styles)
/// it falls back to the requested size unchanged. Inputs are clamped to be at
/// least 1×1. Returns `(window_width, window_height)`.
fn client_size_to_window_size(
    client_width: i32,
    client_height: i32,
    style: ffi::DWORD,
    ex_style: ffi::DWORD,
) -> (i32, i32) {
    let cw = client_width.max(1);
    let ch = client_height.max(1);
    let mut rect = ffi::RECT {
        left: 0,
        top: 0,
        right: cw,
        bottom: ch,
    };
    // SAFETY: AdjustWindowRectEx only reads/writes the provided RECT and takes
    // the style flags by value. `rect` is a valid stack-allocated RECT.
    let ok = unsafe { ffi::AdjustWindowRectEx(&mut rect, style, ffi::FALSE, ex_style) };
    if ok == 0 {
        // Should not happen for valid styles; keep the requested size so the
        // window is at least created.
        return (cw, ch);
    }
    let w = (rect.right - rect.left).max(1);
    let h = (rect.bottom - rect.top).max(1);
    (w, h)
}

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
    event_queue: *const UnsafeCell<VecDeque<PlatformEvent>>,
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
            // SAFETY: GetSystemMetrics is always safe to call.
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
            // The geometry width/height the caller requests is the desired
            // CLIENT area (the surface the compositor renders into). Win32's
            // CreateWindowExW takes the OUTER window size, so for a normal
            // overlapped window the title bar + borders would eat into the
            // client area, leaving it smaller than requested. DXGI would then
            // stretch the rendered frame into the smaller client rect, which
            // shimmers on every present (t55 flicker fix, H2). Expand the
            // requested client size to the matching outer window size via
            // AdjustWindowRectEx so the client area == the requested size.
            let (w, h) = if params.geometry.width > 0.0 && params.geometry.height > 0.0 {
                client_size_to_window_size(
                    params.geometry.width as i32,
                    params.geometry.height as i32,
                    style,
                    ex_style,
                )
            } else {
                (ffi::CW_USEDEFAULT, ffi::CW_USEDEFAULT)
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
        // Load the default arrow cursor for the hardware cursor.
        // SAFETY: LoadCursorW with null hInstance loads a system cursor.
        let default_cursor = unsafe { ffi::LoadCursorW(ptr::null_mut(), ffi::IDC_ARROW) };
        let data = Box::new(WindowData {
            handle,
            event_queue: self.event_queue,
            cursor: std::sync::atomic::AtomicIsize::new(default_cursor as isize),
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
        // SAFETY: ShowWindow and UpdateWindow are safe on a valid HWND.
        unsafe {
            ffi::ShowWindow(hwnd, ffi::SW_SHOW);
            ffi::UpdateWindow(hwnd);
        }

        let info = WindowInfo {
            hwnd,
            handle,
            _data: data,
            dxgi: None,
        };
        self.windows.insert(handle.0, info);

        // Push a WindowCreated event.
        let mut rc = ffi::RECT::default();
        // SAFETY: GetClientRect is safe on a valid HWND with a valid RECT pointer.
        unsafe {
            ffi::GetClientRect(hwnd, &mut rc);
        }
        let width = (rc.right - rc.left) as u32;
        let height = (rc.bottom - rc.top) as u32;
        // Safety: the event_queue pointer is valid for the lifetime of
        // the platform.
        unsafe {
            (*(*self.event_queue).get()).push_back(PlatformEvent::WindowCreated {
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
            // SAFETY: SetWindowLongPtrW and DestroyWindow are safe on a valid HWND.
            // We clear user-data first to prevent the wndproc from using stale pointers.
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
            // SAFETY: ShowWindow is safe on a valid HWND.
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
            // SAFETY: SetWindowPos is safe on a valid HWND. SWP_NOMOVE | SWP_NOSIZE
            // means only the Z-order is changed.
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
            // SAFETY: SetForegroundWindow is safe on a valid HWND.
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

// SAFETY: Win32Taskbar is stateless and all methods are no-ops.
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

        // SAFETY: Shell_NotifyIconW with NIM_MODIFY updates an existing tray icon.
        // The NOTIFYICONDATAW struct is properly initialized.
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
            // SAFETY: Shell_NotifyIconW with NIM_DELETE removes a tray icon.
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

// SAFETY: Win32Notifications is only used from the platform's owning thread.
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

// SAFETY: Win32Keymap is stateless — safe to send between threads.
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
    /// stable for `WindowData` pointers. Wrapped in `UnsafeCell` to
    /// allow the wndproc to push events without creating `&mut` references.
    event_queue: Box<UnsafeCell<VecDeque<PlatformEvent>>>,

    /// ATOM from `RegisterClassExW` (needed for unregistration).
    class_atom: ffi::ATOM,

    /// Process instance handle.
    hinstance: ffi::HINSTANCE,

    /// Wide class name (kept alive so the ATOM remains valid).
    class_name_wide: Vec<u16>,

    /// Hidden message-only window for tray callbacks, etc.
    msg_hwnd: ffi::HWND,

    /// Requested presentation behavior for lazily-created DXGI presenters.
    present_mode: dxgi::DxgiPresentMode,

    /// Accepted-present metadata surfaced through PlatformBackend feedback.
    present_feedback: Win32PresentFeedbackState,
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
            hCursor: // SAFETY: LoadCursorW with null hInstance loads systems cursors.
                     unsafe { ffi::LoadCursorW(ptr::null_mut(), ffi::IDC_ARROW) },
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name_wide.as_ptr(),
            hIconSm: ptr::null_mut(),
        };

        // Safety: RegisterClassExW registers the window class. The struct is
        // fully initialised above.
        let class_atom = unsafe { ffi::RegisterClassExW(&wc) };
        if class_atom == 0 {
            // ERROR_CLASS_ALREADY_EXISTS (1410) means a previous Win32Platform
            // in this process already registered the (process-global) class.
            // That registration is still valid for CreateWindowExW, so tolerate
            // it rather than failing — this lets a second backend (and the unit
            // tests) construct without error.
            const ERROR_CLASS_ALREADY_EXISTS: ffi::DWORD = 1410;
            let err = unsafe { ffi::GetLastError() };
            if err != ERROR_CLASS_ALREADY_EXISTS {
                return Err(PlatformError::Other(format!(
                    "RegisterClassExW failed (error {err})"
                )));
            }
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
        // SAFETY: RegisterClassExW registers the message-only window class.
        // The struct is fully initialised above.
        unsafe {
            ffi::RegisterClassExW(&msg_wc);
        }

        let msg_title = to_wide("");
        // SAFETY: CreateWindowExW creates a hidden message-only window.
        // All parameters are valid and the class was just registered.
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
        // Wrapped in UnsafeCell so the wndproc can push events without
        // creating &mut references that would alias.
        let event_queue = Box::new(UnsafeCell::new(VecDeque::<PlatformEvent>::with_capacity(
            256,
        )));

        // Safety: obtain a raw pointer to the UnsafeCell. The temporary borrow
        // ends at the semicolon so it does not conflict with later accesses.
        let eq_ptr: *const UnsafeCell<VecDeque<PlatformEvent>> = &*event_queue;

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
            // Default to vsync (RefreshSync) for the windowed/dev CPU present
            // path. The CPU renderer uploads a full BGRA frame each present via
            // UpdateSubresource; presenting that with no vsync + ALLOW_TEARING
            // races DWM composition and reads as flicker (t55 flicker fix, H1).
            // RefreshSync presents with sync_interval = 1 and never combines
            // with ALLOW_TEARING, so DWM composites whole frames. Callers that
            // genuinely need no-vsync immediate presentation (e.g. specialized
            // non-windowed paths) can opt in via `new_with_present_mode`.
            present_mode: dxgi::DxgiPresentMode::RefreshSync,
            present_feedback: Win32PresentFeedbackState::default(),
        })
    }

    /// Create a Win32 backend with an explicit DXGI present mode.
    pub fn new_with_present_mode(present_mode: dxgi::DxgiPresentMode) -> PlatformResult<Self> {
        let mut platform = Self::new()?;
        platform.present_mode = present_mode;
        Ok(platform)
    }

    /// Return the requested DXGI present mode for newly-created presenters.
    pub fn present_mode(&self) -> dxgi::DxgiPresentMode {
        self.present_mode
    }

    /// Pump all pending Win32 messages and dispatch them through the wndproc,
    /// which pushes `PlatformEvent`s into `self.event_queue`.
    fn pump_messages(&mut self) {
        let mut msg = ffi::MSG::default();
        // Safety: PeekMessageW retrieves messages from the calling thread's
        // queue. The MSG struct is valid.
        while unsafe { ffi::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, ffi::PM_REMOVE) != 0 } {
            if msg.message == ffi::WM_QUIT {
                // SAFETY: The event_queue UnsafeCell is only accessed from
                // the message-loop thread.  No concurrent access is possible.
                unsafe { (*self.event_queue.get()).push_back(PlatformEvent::Quit) };
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
                // SAFETY: Clearing user-data and destroying windows during teardown.
                // All HWNDs are valid because they were created by this platform.
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
            // SAFETY: msg_hwnd is a valid HWND created in `new()`.
            unsafe {
                ffi::DestroyWindow(self.msg_hwnd);
            }
        }

        // Unregister the window class.
        if self.class_atom != 0 {
            // SAFETY: UnregisterClassW is safe with the class name and hinstance
            // that were used to register the class.
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
        // SAFETY: we have &mut self, so no other Rust code can access
        // the event_queue concurrently. The wndproc only runs during
        // pump_messages below (same thread).
        let queue = unsafe { &mut *self.event_queue.get() };
        if let Some(ev) = queue.pop_front() {
            return Some(ev);
        }
        // Pump the Win32 message queue.
        self.pump_messages();
        // Return the next event, if any.
        // SAFETY: same reasoning as above — exclusive &mut self access.
        let queue = unsafe { &mut *self.event_queue.get() };
        queue.pop_front()
    }

    fn wait_event(&mut self) -> PlatformEvent {
        // Block until a real platform event is available.
        // Some Win32 messages (WM_TIMER, WM_SETFOCUS, internal painting
        // messages, etc.) don't produce a PlatformEvent, so we loop until
        // GetMessageW delivers one that does — matching the Wayland
        // backend's looping behaviour.
        loop {
            // Drain any already-queued events first.
            // SAFETY: exclusive &mut self access; no concurrent queue mutation.
            let queue = unsafe { &mut *self.event_queue.get() };
            if let Some(ev) = queue.pop_front() {
                return ev;
            }

            // Block until a message arrives.
            let mut msg = ffi::MSG::default();
            // SAFETY: GetMessageW blocks until a message is available.
            // Return value 0 = WM_QUIT, -1 = error.
            let ret = unsafe { ffi::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) };

            if ret <= 0 {
                return PlatformEvent::Quit;
            }

            // SAFETY: TranslateMessage and DispatchMessageW are standard
            // message-loop calls; the MSG struct is valid.
            unsafe {
                ffi::TranslateMessage(&msg);
                ffi::DispatchMessageW(&msg);
            }

            // Drain any additional pending messages.
            self.pump_messages();
            // Loop back to check if any PlatformEvents were produced.
        }
    }

    fn take_present_feedback(&mut self) -> Option<PresentFeedback> {
        self.present_feedback.take_feedback()
    }

    // ── Frame presentation ───────────────────────────────────────────

    fn present_frame(
        &mut self,
        handle: NativeWindowHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
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

        let present_mode = self.present_mode;
        let info = self
            .window_host
            .windows
            .get_mut(&handle.0)
            .ok_or_else(|| PlatformError::Presentation("unknown window handle".into()))?;

        let hwnd = info.hwnd;

        // Try DXGI presentation first.
        // Lazily initialize the DXGI presenter on first use.
        if info.dxgi.is_none() {
            match dxgi::DxgiPresenter::new_with_present_mode(hwnd, width, height, present_mode) {
                Ok(presenter) => {
                    info.dxgi = Some(presenter);
                }
                Err(_) => {
                    // DXGI unavailable; will fall through to GDI below.
                }
            }
        }

        if let Some(ref mut presenter) = info.dxgi {
            match presenter.present(pixels, width, height, stride) {
                Ok(()) => {
                    self.present_feedback
                        .record_accepted_present(timestamp_ns());
                    return Ok(());
                }
                Err(_) => {
                    // DXGI present failed (device lost, etc.); drop the
                    // presenter and fall through to GDI for this frame.
                    info.dxgi = None;
                }
            }
        }

        // GDI fallback: SetDIBitsToDevice.
        //
        // SetDIBitsToDevice expects the source DIB rows to be packed at the
        // bitmap's natural stride (width * 4 for 32bpp, which is already
        // DWORD-aligned). It has no way to express a custom source stride, so a
        // padded framebuffer (stride > width * 4) would be read incorrectly and
        // produce a sheared/garbage image. Guard that here rather than present
        // corruption (t55 flicker fix, H3 hardening).
        let packed_stride = width.saturating_mul(4);
        if stride != packed_stride {
            return Err(PlatformError::Presentation(format!(
                "GDI fallback requires packed BGRA rows (stride {stride} != {packed_stride} for width {width})"
            )));
        }
        // Validate the buffer is large enough for the full frame before handing
        // a raw pointer to GDI.
        let required = (packed_stride as usize).saturating_mul(height as usize);
        if pixels.len() < required {
            return Err(PlatformError::Presentation(format!(
                "GDI fallback pixel buffer too small: {} < {} for {}x{}",
                pixels.len(),
                required,
                width,
                height
            )));
        }
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

        // SAFETY: GetDC / SetDIBitsToDevice / ReleaseDC are standard GDI
        // calls. The pixel buffer is valid for `width * height * 4` bytes.
        // The HWND and bitmap info are valid for the duration of this call.
        unsafe {
            let hdc = ffi::GetDC(hwnd);
            if hdc.is_null() {
                return Err(PlatformError::Presentation("GetDC returned null".into()));
            }

            let scan_lines = ffi::SetDIBitsToDevice(
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

            if scan_lines == 0 {
                return Err(PlatformError::Presentation(
                    "SetDIBitsToDevice set 0 scan lines (GDI present failed)".into(),
                ));
            }
        }

        self.present_feedback
            .record_accepted_present(timestamp_ns());
        Ok(())
    }

    fn request_redraw(&mut self, handle: NativeWindowHandle) {
        if let Some(info) = self.window_host.windows.get(&handle.0) {
            // SAFETY: InvalidateRect with a null RECT invalidates the entire
            // client area, causing a WM_PAINT message to be posted.
            // The HWND is valid because it's in our window map.
            unsafe {
                ffi::InvalidateRect(info.hwnd, ptr::null(), ffi::FALSE);
            }
        }
    }

    fn set_cursor_shape(&mut self, handle: NativeWindowHandle, shape: &str) -> bool {
        let cursor_id = match shape {
            "default" | "arrow" => ffi::IDC_ARROW,
            "pointer" | "hand" => ffi::IDC_HAND,
            "text" | "ibeam" => ffi::IDC_IBEAM,
            "crosshair" => ffi::IDC_CROSS,
            "move" | "all-scroll" => ffi::IDC_SIZEALL,
            "not-allowed" | "no-drop" => ffi::IDC_NO,
            "wait" => ffi::IDC_WAIT,
            "progress" => ffi::IDC_APPSTARTING,
            "help" => ffi::IDC_HELP,
            "ns-resize" | "row-resize" => ffi::IDC_SIZENS,
            "ew-resize" | "col-resize" => ffi::IDC_SIZEWE,
            "nwse-resize" => ffi::IDC_SIZENWSE,
            "nesw-resize" => ffi::IDC_SIZENESW,
            "none" | "hidden" => ptr::null(),
            _ => ffi::IDC_ARROW,
        };

        if let Some(info) = self.window_host.windows.get(&handle.0) {
            // SAFETY: LoadCursorW with null hInstance loads a system cursor.
            let hcursor = unsafe { ffi::LoadCursorW(ptr::null_mut(), cursor_id) };
            // Store the cursor handle in the WindowData so the wndproc
            // uses it on WM_SETCURSOR.
            info._data
                .cursor
                .store(hcursor as isize, std::sync::atomic::Ordering::Relaxed);
            // Also set it immediately (in case we're already in the client area).
            // SAFETY: SetCursor is safe with any cursor handle.
            unsafe {
                ffi::SetCursor(hcursor);
            }
            true
        } else {
            false
        }
    }

    fn hide_cursor(&mut self, handle: NativeWindowHandle) {
        if let Some(info) = self.window_host.windows.get(&handle.0) {
            info._data
                .cursor
                .store(0, std::sync::atomic::Ordering::Relaxed);
            // SAFETY: SetCursor(null) hides the cursor. Always safe to call.
            unsafe {
                ffi::SetCursor(ptr::null_mut());
            }
        }
    }

    fn show_cursor(&mut self, handle: NativeWindowHandle) {
        if let Some(info) = self.window_host.windows.get(&handle.0) {
            // SAFETY: LoadCursorW with null hInstance loads a system cursor.
            let hcursor = unsafe { ffi::LoadCursorW(ptr::null_mut(), ffi::IDC_ARROW) };
            info._data
                .cursor
                .store(hcursor as isize, std::sync::atomic::Ordering::Relaxed);
            // SAFETY: SetCursor is safe with any cursor handle.
            unsafe {
                ffi::SetCursor(hcursor);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_wide_ascii_string() {
        let wide = to_wide("ABC");
        // 'A'=0x41, 'B'=0x42, 'C'=0x43, null=0x00
        assert_eq!(wide, vec![0x41, 0x42, 0x43, 0x00]);
    }

    #[test]
    fn to_wide_empty_string() {
        let wide = to_wide("");
        assert_eq!(wide, vec![0x00]);
    }

    #[test]
    fn to_wide_unicode_string() {
        // '€' is U+20AC → single UTF-16 code unit 0x20AC
        let wide = to_wide("€");
        assert_eq!(wide, vec![0x20AC, 0x00]);
    }

    #[test]
    fn from_wide_ascii() {
        let wide = [0x48u16, 0x69, 0x00]; // "Hi\0"
        assert_eq!(from_wide(&wide), "Hi");
    }

    #[test]
    fn from_wide_stops_at_null() {
        let wide = [0x41u16, 0x42, 0x00, 0x43, 0x44];
        assert_eq!(from_wide(&wide), "AB");
    }

    #[test]
    fn from_wide_no_null_terminator() {
        let wide = [0x58u16, 0x59, 0x5A]; // "XYZ" with no null
        assert_eq!(from_wide(&wide), "XYZ");
    }

    #[test]
    fn to_wide_roundtrip() {
        let original = "Hello Platform";
        let wide = to_wide(original);
        let back = from_wide(&wide);
        assert_eq!(back, original);
    }

    #[test]
    fn timestamp_us_is_nonzero() {
        let ts = timestamp_us();
        assert!(ts > 0);
    }

    #[test]
    fn refresh_rate_metadata_filters_windows_default_values() {
        assert_eq!(refresh_rate_hz_from_devmode_frequency(0), None);
        assert_eq!(refresh_rate_hz_from_devmode_frequency(1), None);
        assert_eq!(refresh_rate_hz_from_devmode_frequency(60), Some(60));
        assert_eq!(refresh_rate_hz_from_devmode_frequency(144), Some(144));
    }

    /// The default Win32 present mode must be vsync (RefreshSync), so the
    /// windowed/dev CPU present path does not tear/flicker (t55 flicker fix,
    /// H1). This guards the wiring decision point without rendering.
    #[test]
    fn default_present_mode_is_refresh_sync() {
        let platform = Win32Platform::new().expect("create Win32 platform");
        assert_eq!(platform.present_mode(), dxgi::DxgiPresentMode::RefreshSync);
    }

    /// An explicit present mode is still honored (e.g. Immediate for
    /// non-windowed paths), so the default change does not lock out callers.
    #[test]
    fn explicit_present_mode_overrides_default() {
        let platform = Win32Platform::new_with_present_mode(dxgi::DxgiPresentMode::Immediate)
            .expect("create Win32 platform");
        assert_eq!(platform.present_mode(), dxgi::DxgiPresentMode::Immediate);
    }

    /// For a normal overlapped window, AdjustWindowRectEx must grow the
    /// requested client size to a strictly larger outer window size (title bar
    /// + borders), so the client area ends up == the requested size and DXGI
    /// does not stretch the frame (t55 flicker fix, H2).
    #[test]
    fn client_sizing_grows_overlapped_window() {
        let (w, h) =
            client_size_to_window_size(1270, 768, ffi::WS_OVERLAPPEDWINDOW, ffi::WS_EX_APPWINDOW);
        assert!(
            w >= 1270 && h >= 768,
            "outer window must be at least the requested client size, got {w}x{h}"
        );
        assert!(
            w > 1270 || h > 768,
            "overlapped window has borders/title bar, so outer must exceed client; got {w}x{h}"
        );
    }

    /// A borderless popup ("desktop") has no non-client area, so the outer
    /// window size equals the requested client size.
    #[test]
    fn client_sizing_borderless_popup_is_identity() {
        let (w, h) = client_size_to_window_size(800, 600, ffi::WS_POPUP, ffi::WS_EX_APPWINDOW);
        assert_eq!((w, h), (800, 600));
    }

    /// Degenerate (zero/negative) client sizes are clamped to at least 1×1.
    #[test]
    fn client_sizing_clamps_degenerate_sizes() {
        let (w, h) = client_size_to_window_size(0, -5, ffi::WS_POPUP, ffi::WS_EX_APPWINDOW);
        assert!(w >= 1 && h >= 1, "must clamp to >= 1x1, got {w}x{h}");
    }

    #[test]
    fn present_feedback_state_records_accepted_presents_in_order() {
        let mut state = Win32PresentFeedbackState::default();
        state.record_accepted_present(10);
        state.record_accepted_present(20);

        let first = state.take_feedback().unwrap();
        let second = state.take_feedback().unwrap();

        assert_eq!(first.acknowledged_present_count, 1);
        assert_eq!(first.sequence, Some(1));
        assert_eq!(first.timestamp_ns, Some(10));
        assert_eq!(second.acknowledged_present_count, 2);
        assert_eq!(second.sequence, Some(2));
        assert_eq!(second.timestamp_ns, Some(20));
        assert!(state.take_feedback().is_none());
    }
}
