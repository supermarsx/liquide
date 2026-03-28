//! Windows DPI detection via Win32 API.
//!
//! Uses `GetDpiForMonitor` (per-monitor DPI aware, Windows 8.1+) and
//! `GetDpiForWindow` (per-monitor v2, Windows 10 1607+), with fallback
//! to `GetDpiForSystem` / `GetDeviceCaps(LOGPIXELSX)`.

use crate::monitor::MonitorId;
use crate::scale::DpiScale;

// ── Win32 FFI ──────────────────────────────────────────────────────────

type HMONITOR = isize;
type HWND = isize;
type HDC = isize;
type BOOL = i32;

const MONITOR_DEFAULTTOPRIMARY: u32 = 1;

/// MDT_EFFECTIVE_DPI = 0
const MDT_EFFECTIVE_DPI: u32 = 0;

/// LOGPIXELSX = 88
const LOGPIXELSX: i32 = 88;

#[repr(C)]
#[derive(Default)]
struct Point {
    x: i32,
    y: i32,
}

type MonitorEnumProc =
    unsafe extern "system" fn(hmonitor: HMONITOR, hdc: HDC, lprect: *mut i32, lparam: isize)
        -> BOOL;

#[link(name = "user32")]
unsafe extern "system" {
    fn MonitorFromPoint(pt: Point, flags: u32) -> HMONITOR;
    fn GetDpiForWindow(hwnd: HWND) -> u32;
    fn GetDpiForSystem() -> u32;
    fn EnumDisplayMonitors(
        hdc: HDC,
        lprc_clip: *const i32,
        lpfn_enum: MonitorEnumProc,
        dw_data: isize,
    ) -> BOOL;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn GetDeviceCaps(hdc: HDC, index: i32) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetDC(hwnd: HWND) -> HDC;
    fn ReleaseDC(hwnd: HWND, hdc: HDC) -> i32;
}

// Shcore.dll — per-monitor DPI APIs (Windows 8.1+).
mod shcore {
    use super::*;

    #[link(name = "shcore")]
    unsafe extern "system" {
        pub fn GetDpiForMonitor(
            hmonitor: HMONITOR,
            dpi_type: u32,
            dpi_x: *mut u32,
            dpi_y: *mut u32,
        ) -> i32;
    }
}

// ── Platform DPI implementation ───────────────────────────────────────

/// Windows platform DPI detector.
pub struct PlatformDpi;

impl PlatformDpi {
    /// Create a new platform DPI detector.
    pub fn new() -> Self {
        Self
    }

    /// Get the system DPI (primary monitor).
    ///
    /// Uses `GetDpiForSystem` (available since Windows 10 1607).
    /// Falls back to `GetDeviceCaps(LOGPIXELSX)` on older systems.
    pub fn system_dpi(&self) -> DpiScale {
        let dpi = unsafe { GetDpiForSystem() };
        if dpi > 0 {
            return DpiScale::from_dpi(dpi as f32);
        }
        // Fallback: GetDeviceCaps on the screen DC.
        unsafe {
            let hdc = GetDC(0);
            if hdc != 0 {
                let dpi = GetDeviceCaps(hdc, LOGPIXELSX);
                ReleaseDC(0, hdc);
                if dpi > 0 {
                    return DpiScale::from_dpi(dpi as f32);
                }
            }
        }
        DpiScale::identity()
    }

    /// Get the DPI for a specific window handle.
    ///
    /// Uses `GetDpiForWindow` (per-monitor v2).
    pub fn dpi_for_window(&self, hwnd: isize) -> DpiScale {
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        if dpi > 0 {
            DpiScale::from_dpi(dpi as f32)
        } else {
            self.system_dpi()
        }
    }

    /// Get the DPI for a specific monitor handle.
    ///
    /// Uses `GetDpiForMonitor` from shcore.dll.
    pub fn dpi_for_monitor_handle(&self, hmonitor: isize) -> DpiScale {
        let mut dpi_x: u32 = 0;
        let mut dpi_y: u32 = 0;
        let hr = unsafe {
            shcore::GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y)
        };
        if hr == 0 && dpi_x > 0 {
            DpiScale::from_dpi(dpi_x as f32)
        } else {
            self.system_dpi()
        }
    }

    /// Get the DPI for the primary monitor.
    pub fn primary_monitor_dpi(&self) -> DpiScale {
        let hmon = unsafe { MonitorFromPoint(Point::default(), MONITOR_DEFAULTTOPRIMARY) };
        if hmon != 0 {
            self.dpi_for_monitor_handle(hmon)
        } else {
            self.system_dpi()
        }
    }

    /// Enumerate all monitors and return their DPI values.
    ///
    /// Returns a `Vec` of `(monitor_index, DpiScale)` pairs. The monitor
    /// indices are assigned sequentially during enumeration (not HMONITOR values).
    pub fn enumerate_monitor_dpis(&self) -> Vec<(MonitorId, DpiScale)> {
        struct EnumData {
            results: Vec<(MonitorId, DpiScale)>,
            next_id: MonitorId,
        }

        unsafe extern "system" fn enum_proc(
            hmonitor: HMONITOR,
            _hdc: HDC,
            _lprect: *mut i32,
            lparam: isize,
        ) -> BOOL {
            unsafe {
                let data = &mut *(lparam as *mut EnumData);

                let mut dpi_x: u32 = 0;
                let mut dpi_y: u32 = 0;
                let hr = shcore::GetDpiForMonitor(
                    hmonitor,
                    MDT_EFFECTIVE_DPI,
                    &mut dpi_x,
                    &mut dpi_y,
                );

                let scale = if hr == 0 && dpi_x > 0 {
                    DpiScale::from_dpi(dpi_x as f32)
                } else {
                    DpiScale::identity()
                };

                let id = data.next_id;
                data.next_id += 1;
                data.results.push((id, scale));
                1 // TRUE = continue enumeration
            }
        }

        let mut data = EnumData {
            results: Vec::new(),
            next_id: 0,
        };

        unsafe {
            EnumDisplayMonitors(
                0,
                std::ptr::null(),
                enum_proc,
                &mut data as *mut EnumData as isize,
            );
        }

        if data.results.is_empty() {
            // Fallback: return system DPI as monitor 0.
            vec![(0, self.system_dpi())]
        } else {
            data.results
        }
    }
}

impl Default for PlatformDpi {
    fn default() -> Self {
        Self::new()
    }
}
