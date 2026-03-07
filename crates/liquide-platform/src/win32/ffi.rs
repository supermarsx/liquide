//! Raw Win32 FFI type definitions and extern declarations.
//!
//! This module contains all the Win32 types, constants, and extern function
//! declarations needed by the Win32 platform backend. No external crate
//! dependencies are used -- we link directly to system DLLs (user32, gdi32,
//! kernel32, shell32).

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(clippy::upper_case_acronyms)]
#![allow(dead_code)]

use std::ffi::c_void;

// ---------------------------------------------------------------------------
// Fundamental Win32 handle and scalar types
// ---------------------------------------------------------------------------

pub type HWND = *mut c_void;
pub type HDC = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type HMODULE = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HCURSOR = *mut c_void;
pub type HICON = *mut c_void;
pub type HMENU = *mut c_void;
pub type HBITMAP = *mut c_void;
pub type HGDIOBJ = *mut c_void;
pub type HMONITOR = *mut c_void;
pub type ATOM = u16;
pub type BOOL = i32;
pub type DWORD = u32;
pub type UINT = u32;
pub type LONG = i32;
pub type LONG_PTR = isize;
pub type UINT_PTR = usize;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type WORD = u16;
pub type BYTE = u8;
pub type WCHAR = u16;
pub type LPCWSTR = *const u16;
pub type LPWSTR = *mut u16;

pub const TRUE: BOOL = 1;
pub const FALSE: BOOL = 0;

/// Window procedure callback type.
pub type WNDPROC =
    Option<unsafe extern "system" fn(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT>;

/// Monitor enum callback type.
pub type MONITORENUMPROC = Option<
    unsafe extern "system" fn(
        hmonitor: HMONITOR,
        hdc: HDC,
        lprc: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL,
>;

// ---------------------------------------------------------------------------
// Structures
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct POINT {
    pub x: LONG,
    pub y: LONG,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RECT {
    pub left: LONG,
    pub top: LONG,
    pub right: LONG,
    pub bottom: LONG,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: UINT,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: DWORD,
    pub pt: POINT,
}

impl Default for MSG {
    fn default() -> Self {
        // Safety: MSG is a plain-old-data struct; zeroed memory is valid.
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: UINT,
    pub style: UINT,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: LPCWSTR,
    pub lpszClassName: LPCWSTR,
    pub hIconSm: HICON,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct BITMAPINFOHEADER {
    pub biSize: DWORD,
    pub biWidth: LONG,
    pub biHeight: LONG,
    pub biPlanes: WORD,
    pub biBitCount: WORD,
    pub biCompression: DWORD,
    pub biSizeImage: DWORD,
    pub biXPelsPerMeter: LONG,
    pub biYPelsPerMeter: LONG,
    pub biClrUsed: DWORD,
    pub biClrImportant: DWORD,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RGBQUAD {
    pub rgbBlue: BYTE,
    pub rgbGreen: BYTE,
    pub rgbRed: BYTE,
    pub rgbReserved: BYTE,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct BITMAPINFO {
    pub bmiHeader: BITMAPINFOHEADER,
    pub bmiColors: [RGBQUAD; 1],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MONITORINFO {
    pub cbSize: DWORD,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: DWORD,
}

pub const CCHDEVICENAME: usize = 32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MONITORINFOEXW {
    pub base: MONITORINFO,
    pub szDevice: [WCHAR; CCHDEVICENAME],
}

impl Default for MONITORINFOEXW {
    fn default() -> Self {
        // Safety: plain-old-data struct; zeroed memory is valid.
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [BYTE; 32],
}

impl Default for PAINTSTRUCT {
    fn default() -> Self {
        // Safety: plain-old-data struct; zeroed memory is valid.
        unsafe { std::mem::zeroed() }
    }
}

/// NOTIFYICONDATAW for Shell_NotifyIconW (V2 size -- sufficient for tooltip).
#[repr(C)]
pub struct NOTIFYICONDATAW {
    pub cbSize: DWORD,
    pub hWnd: HWND,
    pub uID: UINT,
    pub uFlags: UINT,
    pub uCallbackMessage: UINT,
    pub hIcon: HICON,
    pub szTip: [WCHAR; 128],
    pub dwState: DWORD,
    pub dwStateMask: DWORD,
    pub szInfo: [WCHAR; 256],
    pub uVersion_or_uTimeout: UINT,
    pub szInfoTitle: [WCHAR; 64],
    pub dwInfoFlags: DWORD,
}

impl Default for NOTIFYICONDATAW {
    fn default() -> Self {
        // Safety: plain-old-data struct; zeroed memory is valid.
        unsafe { std::mem::zeroed() }
    }
}

// ---------------------------------------------------------------------------
// Window message constants
// ---------------------------------------------------------------------------

pub const WM_CREATE: UINT = 0x0001;
pub const WM_DESTROY: UINT = 0x0002;
pub const WM_MOVE: UINT = 0x0003;
pub const WM_SIZE: UINT = 0x0005;
pub const WM_SETFOCUS: UINT = 0x0007;
pub const WM_KILLFOCUS: UINT = 0x0008;
pub const WM_PAINT: UINT = 0x000F;
pub const WM_CLOSE: UINT = 0x0010;
pub const WM_QUIT: UINT = 0x0012;
pub const WM_ERASEBKGND: UINT = 0x0014;
pub const WM_SETCURSOR: UINT = 0x0020;
pub const WM_SIZING: UINT = 0x0214;
pub const WM_KEYDOWN: UINT = 0x0100;
pub const WM_KEYUP: UINT = 0x0101;
pub const WM_CHAR: UINT = 0x0102;
pub const WM_SYSKEYDOWN: UINT = 0x0104;
pub const WM_SYSKEYUP: UINT = 0x0105;
pub const WM_MOUSEMOVE: UINT = 0x0200;
pub const WM_LBUTTONDOWN: UINT = 0x0201;
pub const WM_LBUTTONUP: UINT = 0x0202;
pub const WM_RBUTTONDOWN: UINT = 0x0204;
pub const WM_RBUTTONUP: UINT = 0x0205;
pub const WM_MBUTTONDOWN: UINT = 0x0207;
pub const WM_MBUTTONUP: UINT = 0x0208;
pub const WM_MOUSEWHEEL: UINT = 0x020A;
pub const WM_MOUSEHWHEEL: UINT = 0x020E;
pub const WM_DPICHANGED: UINT = 0x02E0;
pub const WM_TIMER: UINT = 0x0113;
pub const WM_USER: UINT = 0x0400;

// ---------------------------------------------------------------------------
// Window class / style constants
// ---------------------------------------------------------------------------

pub const CS_HREDRAW: UINT = 0x0002;
pub const CS_VREDRAW: UINT = 0x0001;
pub const CS_OWNDC: UINT = 0x0020;

pub const WS_OVERLAPPEDWINDOW: DWORD = 0x00CF0000;
pub const WS_POPUP: DWORD = 0x80000000;
pub const WS_VISIBLE: DWORD = 0x10000000;
pub const WS_MAXIMIZE: DWORD = 0x01000000;
pub const WS_MINIMIZE: DWORD = 0x20000000;

pub const WS_EX_APPWINDOW: DWORD = 0x00040000;
pub const WS_EX_TOPMOST: DWORD = 0x00000008;
pub const WS_EX_TOOLWINDOW: DWORD = 0x00000080;

pub const SW_SHOW: i32 = 5;
pub const SW_HIDE: i32 = 0;
pub const SW_MAXIMIZE: i32 = 3;
pub const SW_MINIMIZE: i32 = 6;
pub const SW_RESTORE: i32 = 9;

pub const HTCLIENT: i32 = 1;

pub const PM_REMOVE: UINT = 0x0001;
pub const PM_NOREMOVE: UINT = 0x0000;

// ---------------------------------------------------------------------------
// GDI constants
// ---------------------------------------------------------------------------

pub const DIB_RGB_COLORS: UINT = 0;
pub const SRCCOPY: DWORD = 0x00CC0020;
pub const BI_RGB: DWORD = 0;

// ---------------------------------------------------------------------------
// Cursor constants
// ---------------------------------------------------------------------------

pub const IDC_ARROW: LPCWSTR = 32512 as LPCWSTR;
pub const IDC_IBEAM: LPCWSTR = 32513 as LPCWSTR;
pub const IDC_WAIT: LPCWSTR = 32514 as LPCWSTR;
pub const IDC_CROSS: LPCWSTR = 32515 as LPCWSTR;
pub const IDC_SIZEALL: LPCWSTR = 32646 as LPCWSTR;
pub const IDC_SIZENWSE: LPCWSTR = 32642 as LPCWSTR;
pub const IDC_SIZENESW: LPCWSTR = 32643 as LPCWSTR;
pub const IDC_SIZEWE: LPCWSTR = 32644 as LPCWSTR;
pub const IDC_SIZENS: LPCWSTR = 32645 as LPCWSTR;
pub const IDC_HAND: LPCWSTR = 32649 as LPCWSTR;
pub const IDC_HELP: LPCWSTR = 32651 as LPCWSTR;
pub const IDC_NO: LPCWSTR = 32648 as LPCWSTR;
pub const IDC_APPSTARTING: LPCWSTR = 32650 as LPCWSTR;

// ---------------------------------------------------------------------------
// SetWindowPos constants
// ---------------------------------------------------------------------------

pub const HWND_TOP: HWND = 0 as HWND;
pub const HWND_TOPMOST: HWND = -1isize as HWND;

pub const SWP_NOMOVE: UINT = 0x0002;
pub const SWP_NOSIZE: UINT = 0x0001;
pub const SWP_NOZORDER: UINT = 0x0004;

// ---------------------------------------------------------------------------
// GetSystemMetrics constants
// ---------------------------------------------------------------------------

pub const SM_CXSCREEN: i32 = 0;
pub const SM_CYSCREEN: i32 = 1;

// ---------------------------------------------------------------------------
// Monitor constants
// ---------------------------------------------------------------------------

pub const MONITOR_DEFAULTTOPRIMARY: DWORD = 0x00000001;
pub const MONITOR_DEFAULTTONEAREST: DWORD = 0x00000002;
pub const MONITORINFOF_PRIMARY: DWORD = 0x00000001;

// ---------------------------------------------------------------------------
// Shell notification icon constants
// ---------------------------------------------------------------------------

pub const NIF_MESSAGE: UINT = 0x00000001;
pub const NIF_ICON: UINT = 0x00000002;
pub const NIF_TIP: UINT = 0x00000004;

pub const NIM_ADD: DWORD = 0x00000000;
pub const NIM_MODIFY: DWORD = 0x00000001;
pub const NIM_DELETE: DWORD = 0x00000002;

// ---------------------------------------------------------------------------
// SetWindowLongPtr index constants
// ---------------------------------------------------------------------------

pub const GWL_STYLE: i32 = -16;
pub const GWLP_USERDATA: i32 = -21;

// ---------------------------------------------------------------------------
// Mouse / wheel constants
// ---------------------------------------------------------------------------

pub const WHEEL_DELTA: i16 = 120;
pub const MK_LBUTTON: WPARAM = 0x0001;
pub const MK_RBUTTON: WPARAM = 0x0002;
pub const MK_MBUTTON: WPARAM = 0x0010;

// ---------------------------------------------------------------------------
// SIZE_* constants (wParam of WM_SIZE)
// ---------------------------------------------------------------------------

pub const SIZE_MINIMIZED: usize = 1;
pub const SIZE_MAXIMIZED: usize = 2;
pub const SIZE_RESTORED: usize = 0;

// ---------------------------------------------------------------------------
// CW_USEDEFAULT
// ---------------------------------------------------------------------------

pub const CW_USEDEFAULT: i32 = 0x80000000u32 as i32;

// ---------------------------------------------------------------------------
// Virtual key codes
// ---------------------------------------------------------------------------

pub const VK_BACK: u32 = 0x08;
pub const VK_TAB: u32 = 0x09;
pub const VK_CLEAR: u32 = 0x0C;
pub const VK_RETURN: u32 = 0x0D;
pub const VK_SHIFT: u32 = 0x10;
pub const VK_CONTROL: u32 = 0x11;
pub const VK_MENU: u32 = 0x12; // Alt
pub const VK_PAUSE: u32 = 0x13;
pub const VK_CAPITAL: u32 = 0x14; // Caps Lock
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_SPACE: u32 = 0x20;
pub const VK_PRIOR: u32 = 0x21; // Page Up
pub const VK_NEXT: u32 = 0x22; // Page Down
pub const VK_END: u32 = 0x23;
pub const VK_HOME: u32 = 0x24;
pub const VK_LEFT: u32 = 0x25;
pub const VK_UP: u32 = 0x26;
pub const VK_RIGHT: u32 = 0x27;
pub const VK_DOWN: u32 = 0x28;
pub const VK_SNAPSHOT: u32 = 0x2C; // Print Screen
pub const VK_INSERT: u32 = 0x2D;
pub const VK_DELETE: u32 = 0x2E;

// 0-9 are ASCII '0'..'9'
pub const VK_0: u32 = 0x30;
pub const VK_1: u32 = 0x31;
pub const VK_2: u32 = 0x32;
pub const VK_3: u32 = 0x33;
pub const VK_4: u32 = 0x34;
pub const VK_5: u32 = 0x35;
pub const VK_6: u32 = 0x36;
pub const VK_7: u32 = 0x37;
pub const VK_8: u32 = 0x38;
pub const VK_9: u32 = 0x39;

// A-Z are ASCII 'A'..'Z'
pub const VK_A: u32 = 0x41;
pub const VK_B: u32 = 0x42;
pub const VK_C: u32 = 0x43;
pub const VK_D: u32 = 0x44;
pub const VK_E: u32 = 0x45;
pub const VK_F: u32 = 0x46;
pub const VK_G: u32 = 0x47;
pub const VK_H: u32 = 0x48;
pub const VK_I: u32 = 0x49;
pub const VK_J: u32 = 0x4A;
pub const VK_K: u32 = 0x4B;
pub const VK_L: u32 = 0x4C;
pub const VK_M: u32 = 0x4D;
pub const VK_N: u32 = 0x4E;
pub const VK_O: u32 = 0x4F;
pub const VK_P: u32 = 0x50;
pub const VK_Q: u32 = 0x51;
pub const VK_R: u32 = 0x52;
pub const VK_S: u32 = 0x53;
pub const VK_T: u32 = 0x54;
pub const VK_U: u32 = 0x55;
pub const VK_V: u32 = 0x56;
pub const VK_W: u32 = 0x57;
pub const VK_X: u32 = 0x58;
pub const VK_Y: u32 = 0x59;
pub const VK_Z: u32 = 0x5A;

pub const VK_LWIN: u32 = 0x5B;
pub const VK_RWIN: u32 = 0x5C;
pub const VK_APPS: u32 = 0x5D; // Context Menu

pub const VK_NUMPAD0: u32 = 0x60;
pub const VK_NUMPAD1: u32 = 0x61;
pub const VK_NUMPAD2: u32 = 0x62;
pub const VK_NUMPAD3: u32 = 0x63;
pub const VK_NUMPAD4: u32 = 0x64;
pub const VK_NUMPAD5: u32 = 0x65;
pub const VK_NUMPAD6: u32 = 0x66;
pub const VK_NUMPAD7: u32 = 0x67;
pub const VK_NUMPAD8: u32 = 0x68;
pub const VK_NUMPAD9: u32 = 0x69;
pub const VK_MULTIPLY: u32 = 0x6A;
pub const VK_ADD: u32 = 0x6B;
pub const VK_SUBTRACT: u32 = 0x6D;
pub const VK_DECIMAL: u32 = 0x6E;
pub const VK_DIVIDE: u32 = 0x6F;

pub const VK_F1: u32 = 0x70;
pub const VK_F2: u32 = 0x71;
pub const VK_F3: u32 = 0x72;
pub const VK_F4: u32 = 0x73;
pub const VK_F5: u32 = 0x74;
pub const VK_F6: u32 = 0x75;
pub const VK_F7: u32 = 0x76;
pub const VK_F8: u32 = 0x77;
pub const VK_F9: u32 = 0x78;
pub const VK_F10: u32 = 0x79;
pub const VK_F11: u32 = 0x7A;
pub const VK_F12: u32 = 0x7B;

pub const VK_NUMLOCK: u32 = 0x90;
pub const VK_SCROLL: u32 = 0x91;

pub const VK_LSHIFT: u32 = 0xA0;
pub const VK_RSHIFT: u32 = 0xA1;
pub const VK_LCONTROL: u32 = 0xA2;
pub const VK_RCONTROL: u32 = 0xA3;
pub const VK_LMENU: u32 = 0xA4; // Left Alt
pub const VK_RMENU: u32 = 0xA5; // Right Alt

pub const VK_OEM_1: u32 = 0xBA; // ;:
pub const VK_OEM_PLUS: u32 = 0xBB; // =+
pub const VK_OEM_COMMA: u32 = 0xBC; // ,<
pub const VK_OEM_MINUS: u32 = 0xBD; // -_
pub const VK_OEM_PERIOD: u32 = 0xBE; // .>
pub const VK_OEM_2: u32 = 0xBF; // /?
pub const VK_OEM_3: u32 = 0xC0; // `~
pub const VK_OEM_4: u32 = 0xDB; // [{
pub const VK_OEM_5: u32 = 0xDC; // \|
pub const VK_OEM_6: u32 = 0xDD; // ]}
pub const VK_OEM_7: u32 = 0xDE; // '"

// ---------------------------------------------------------------------------
// Helper macros / inline functions
// ---------------------------------------------------------------------------

/// Extract the low word from an LPARAM or WPARAM.
#[inline]
pub fn loword(l: usize) -> u16 {
    (l & 0xFFFF) as u16
}

/// Extract the high word from an LPARAM or WPARAM.
#[inline]
pub fn hiword(l: usize) -> u16 {
    ((l >> 16) & 0xFFFF) as u16
}

/// Extract the signed low word from an LPARAM (equivalent to `GET_X_LPARAM`).
#[inline]
pub fn get_x_lparam(lp: LPARAM) -> i32 {
    (lp & 0xFFFF) as i16 as i32
}

/// Extract the signed high word from an LPARAM (equivalent to `GET_Y_LPARAM`).
#[inline]
pub fn get_y_lparam(lp: LPARAM) -> i32 {
    ((lp >> 16) & 0xFFFF) as i16 as i32
}

/// Extract the signed high word from WPARAM (used for mouse wheel delta).
#[inline]
pub fn get_wheel_delta_wparam(wp: WPARAM) -> i16 {
    hiword(wp) as i16
}

// ---------------------------------------------------------------------------
// Extern function declarations -- user32.dll
// ---------------------------------------------------------------------------

#[link(name = "user32")]
unsafe extern "system" {
    pub fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> ATOM;
    pub fn UnregisterClassW(lpClassName: LPCWSTR, hInstance: HINSTANCE) -> BOOL;

    pub fn CreateWindowExW(
        dwExStyle: DWORD,
        lpClassName: LPCWSTR,
        lpWindowName: LPCWSTR,
        dwStyle: DWORD,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: *mut c_void,
    ) -> HWND;

    pub fn DestroyWindow(hWnd: HWND) -> BOOL;
    pub fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
    pub fn UpdateWindow(hWnd: HWND) -> BOOL;

    pub fn DefWindowProcW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;

    pub fn GetMessageW(
        lpMsg: *mut MSG,
        hWnd: HWND,
        wMsgFilterMin: UINT,
        wMsgFilterMax: UINT,
    ) -> BOOL;

    pub fn PeekMessageW(
        lpMsg: *mut MSG,
        hWnd: HWND,
        wMsgFilterMin: UINT,
        wMsgFilterMax: UINT,
        wRemoveMsg: UINT,
    ) -> BOOL;

    pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    pub fn PostQuitMessage(nExitCode: i32);
    pub fn PostMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> BOOL;

    pub fn GetDC(hWnd: HWND) -> HDC;
    pub fn ReleaseDC(hWnd: HWND, hDC: HDC) -> i32;
    pub fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> BOOL;
    pub fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: BOOL) -> BOOL;

    pub fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: LPCWSTR) -> HCURSOR;
    pub fn SetCursor(hCursor: HCURSOR) -> HCURSOR;

    pub fn SetWindowTextW(hWnd: HWND, lpString: LPCWSTR) -> BOOL;
    pub fn SetForegroundWindow(hWnd: HWND) -> BOOL;

    pub fn SetWindowPos(
        hWnd: HWND,
        hWndInsertAfter: HWND,
        X: i32,
        Y: i32,
        cx: i32,
        cy: i32,
        uFlags: UINT,
    ) -> BOOL;

    pub fn MoveWindow(
        hWnd: HWND,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        bRepaint: BOOL,
    ) -> BOOL;

    pub fn GetWindowRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;

    pub fn SetTimer(hWnd: HWND, nIDEvent: UINT_PTR, uElapse: UINT, lpTimerFunc: *const c_void) -> UINT_PTR;
    pub fn KillTimer(hWnd: HWND, uIDEvent: UINT_PTR) -> BOOL;

    pub fn GetSystemMetrics(nIndex: i32) -> i32;

    pub fn EnumDisplayMonitors(
        hdc: HDC,
        lprcClip: *const RECT,
        lpfnEnum: MONITORENUMPROC,
        dwData: LPARAM,
    ) -> BOOL;

    pub fn GetMonitorInfoW(hMonitor: HMONITOR, lpmi: *mut MONITORINFOEXW) -> BOOL;
    pub fn MonitorFromWindow(hwnd: HWND, dwFlags: DWORD) -> HMONITOR;

    pub fn SetCapture(hWnd: HWND) -> HWND;
    pub fn ReleaseCapture() -> BOOL;
    pub fn GetCursorPos(lpPoint: *mut POINT) -> BOOL;
    pub fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> BOOL;

    pub fn SetWindowLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: LONG_PTR) -> LONG_PTR;
    pub fn GetWindowLongPtrW(hWnd: HWND, nIndex: i32) -> LONG_PTR;

    pub fn GetKeyState(nVirtKey: i32) -> i16;
}

// ---------------------------------------------------------------------------
// Extern function declarations -- gdi32.dll
// ---------------------------------------------------------------------------

#[link(name = "gdi32")]
unsafe extern "system" {
    pub fn SetDIBitsToDevice(
        hdc: HDC,
        xDest: i32,
        yDest: i32,
        w: DWORD,
        h: DWORD,
        xSrc: i32,
        ySrc: i32,
        StartScan: UINT,
        cLines: UINT,
        lpvBits: *const c_void,
        lpbmi: *const BITMAPINFO,
        ColorUse: UINT,
    ) -> i32;

    pub fn StretchDIBits(
        hdc: HDC,
        xDest: i32,
        yDest: i32,
        DestWidth: i32,
        DestHeight: i32,
        xSrc: i32,
        ySrc: i32,
        SrcWidth: i32,
        SrcHeight: i32,
        lpBits: *const c_void,
        lpbmi: *const BITMAPINFO,
        iUsage: UINT,
        rop: DWORD,
    ) -> i32;

    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn CreateDIBSection(
        hdc: HDC,
        pbmi: *const BITMAPINFO,
        usage: UINT,
        ppvBits: *mut *mut c_void,
        hSection: *mut c_void,
        offset: DWORD,
    ) -> HBITMAP;
    pub fn DeleteDC(hdc: HDC) -> BOOL;
    pub fn DeleteObject(ho: HGDIOBJ) -> BOOL;
    pub fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
    pub fn BitBlt(
        hdc: HDC,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        hdcSrc: HDC,
        x1: i32,
        y1: i32,
        rop: DWORD,
    ) -> BOOL;
}

// ---------------------------------------------------------------------------
// Extern function declarations -- kernel32.dll
// ---------------------------------------------------------------------------

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn GetModuleHandleW(lpModuleName: LPCWSTR) -> HMODULE;
    pub fn GetLastError() -> DWORD;
}

// ---------------------------------------------------------------------------
// Extern function declarations -- shell32.dll
// ---------------------------------------------------------------------------

#[link(name = "shell32")]
unsafe extern "system" {
    pub fn Shell_NotifyIconW(dwMessage: DWORD, lpData: *mut NOTIFYICONDATAW) -> BOOL;
}
