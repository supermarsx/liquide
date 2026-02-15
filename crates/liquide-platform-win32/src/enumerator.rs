//! Win32 window enumerator — discovers and tracks running applications.
#![allow(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;

use tracing::info;

use crate::types::{Win32AppEvent, Win32AppInfo, Win32WindowState};
use crate::Result;

/// Enumerates visible Win32 top-level windows and tracks changes.
pub struct Win32WindowEnumerator {
    /// Previously known windows, keyed by HWND.
    known_windows: HashMap<u64, Win32AppInfo>,
    /// Pending events since last poll.
    pending_events: Vec<Win32AppEvent>,
    /// Whether enumeration has been run at least once.
    initialized: bool,
    /// Window classes to ignore (system chrome, tooltips, etc.).
    ignored_classes: Vec<String>,
}

impl Win32WindowEnumerator {
    /// Create a new enumerator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            known_windows: HashMap::new(),
            pending_events: Vec::new(),
            initialized: false,
            ignored_classes: vec![
                "Shell_TrayWnd".into(),
                "Shell_SecondaryTrayWnd".into(),
                "Progman".into(),
                "WorkerW".into(),
                "Windows.UI.Core.CoreWindow".into(),
                "ApplicationFrameInputSinkWindow".into(),
                "tooltips_class32".into(),
                "TaskListThumbnailWnd".into(),
                "NotifyIconOverflowWindow".into(),
            ],
        }
    }

    /// Perform a full enumeration of visible windows.
    ///
    /// Returns the complete list of discovered apps.
    pub fn discover(&mut self) -> Result<Vec<Win32AppInfo>> {
        let current = self.enumerate_windows()?;
        let current_map: HashMap<u64, Win32AppInfo> =
            current.iter().map(|w| (w.hwnd, w.clone())).collect();

        // Generate events.
        if self.initialized {
            // Find new windows.
            for (hwnd, info) in &current_map {
                if !self.known_windows.contains_key(hwnd) {
                    self.pending_events
                        .push(Win32AppEvent::WindowOpened(info.clone()));
                }
            }

            // Find closed windows.
            for (hwnd, info) in &self.known_windows {
                if !current_map.contains_key(hwnd) {
                    self.pending_events.push(Win32AppEvent::WindowClosed {
                        hwnd: *hwnd,
                        pid: info.pid,
                    });
                }
            }

            // Find changed windows.
            for (hwnd, new_info) in &current_map {
                if let Some(old_info) = self.known_windows.get(hwnd) {
                    if old_info.title != new_info.title
                        || old_info.state != new_info.state
                        || old_info.focused != new_info.focused
                        || old_info.bounds != new_info.bounds
                    {
                        self.pending_events
                            .push(Win32AppEvent::WindowChanged(new_info.clone()));

                        if !old_info.focused && new_info.focused {
                            self.pending_events
                                .push(Win32AppEvent::WindowFocused { hwnd: *hwnd });
                        } else if old_info.focused && !new_info.focused {
                            self.pending_events
                                .push(Win32AppEvent::WindowUnfocused { hwnd: *hwnd });
                        }
                    }
                }
            }
        }

        self.known_windows = current_map;
        self.initialized = true;

        Ok(current)
    }

    /// Drain pending change events since the last call.
    pub fn poll_events(&mut self) -> Vec<Win32AppEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Get currently known windows.
    #[must_use]
    pub fn known_windows(&self) -> &HashMap<u64, Win32AppInfo> {
        &self.known_windows
    }

    /// Get window info by HWND.
    #[must_use]
    pub fn get_window(&self, hwnd: u64) -> Option<&Win32AppInfo> {
        self.known_windows.get(&hwnd)
    }

    /// Check if a window class should be ignored.
    #[allow(dead_code)]
    fn is_ignored_class(&self, class_name: &str) -> bool {
        self.ignored_classes
            .iter()
            .any(|c| c.eq_ignore_ascii_case(class_name))
    }

    /// Enumerate all visible top-level windows using Win32 API.
    #[cfg(windows)]
    fn enumerate_windows(&self) -> Result<Vec<Win32AppInfo>> {
        use std::ffi::OsString;
        use std::mem;
        use std::os::windows::ffi::OsStringExt;

        use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowRect,
            GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
            IsWindowVisible, IsZoomed, GWL_EXSTYLE, WS_EX_APPWINDOW,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        };

        struct EnumContext {
            windows: Vec<Win32AppInfo>,
            foreground_hwnd: u64,
            ignored_classes: Vec<String>,
        }

        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let ctx = &mut *(lparam as *mut EnumContext);

            // Skip invisible windows.
            if IsWindowVisible(hwnd) == 0 {
                return TRUE;
            }

            // Check extended style — skip tool windows, include app windows.
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            if (ex_style & WS_EX_TOOLWINDOW) != 0 && (ex_style & WS_EX_APPWINDOW) == 0 {
                return TRUE;
            }
            if (ex_style & WS_EX_NOACTIVATE) != 0 {
                return TRUE;
            }

            // Get class name.
            let mut class_buf = [0u16; 256];
            let class_len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), 256);
            let class_name = if class_len > 0 {
                OsString::from_wide(&class_buf[..class_len as usize])
                    .to_string_lossy()
                    .to_string()
            } else {
                String::new()
            };

            // Skip ignored classes.
            if ctx
                .ignored_classes
                .iter()
                .any(|c| c.eq_ignore_ascii_case(&class_name))
            {
                return TRUE;
            }

            // Get window title.
            let title_len = GetWindowTextLengthW(hwnd);
            let title = if title_len > 0 {
                let mut title_buf = vec![0u16; (title_len + 1) as usize];
                let actual = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_len + 1);
                OsString::from_wide(&title_buf[..actual as usize])
                    .to_string_lossy()
                    .to_string()
            } else {
                String::new()
            };

            // Skip windows with no title.
            if title.is_empty() {
                return TRUE;
            }

            // Get process ID.
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);

            // Get window rect.
            let mut rect = mem::zeroed::<windows_sys::Win32::Foundation::RECT>();
            GetWindowRect(hwnd, &mut rect);
            let bounds = (
                rect.left,
                rect.top,
                (rect.right - rect.left).max(0) as u32,
                (rect.bottom - rect.top).max(0) as u32,
            );

            // Determine state.
            let minimized = IsIconic(hwnd) != 0;
            let maximized = IsZoomed(hwnd) != 0;
            let state = if minimized {
                Win32WindowState::Minimized
            } else if maximized {
                Win32WindowState::Maximized
            } else {
                Win32WindowState::Normal
            };

            let focused = (hwnd as u64) == ctx.foreground_hwnd;

            // Derive app name from title (simplified — in production, query process path).
            let app_name = title
                .split(" - ")
                .last()
                .unwrap_or(&title)
                .trim()
                .to_string();

            ctx.windows.push(Win32AppInfo {
                hwnd: hwnd as u64,
                title,
                class_name,
                pid,
                exe_path: String::new(), // Would use QueryFullProcessImageNameW
                app_name,
                visible: true,
                minimized,
                focused,
                state,
                bounds,
            });

            TRUE
        }

        let mut ctx = EnumContext {
            windows: Vec::new(),
            foreground_hwnd: unsafe { GetForegroundWindow() as u64 },
            ignored_classes: self.ignored_classes.clone(),
        };

        unsafe {
            EnumWindows(
                Some(enum_callback),
                &mut ctx as *mut EnumContext as LPARAM,
            );
        }

        info!(count = ctx.windows.len(), "enumerated Win32 windows");
        Ok(ctx.windows)
    }

    #[cfg(not(windows))]
    fn enumerate_windows(&self) -> Result<Vec<Win32AppInfo>> {
        // Non-Windows: return empty list.
        Ok(Vec::new())
    }
}

impl Default for Win32WindowEnumerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_enumerator() {
        let enumerator = Win32WindowEnumerator::new();
        assert!(enumerator.known_windows().is_empty());
        assert!(!enumerator.initialized);
    }

    #[test]
    fn test_discover() {
        let mut enumerator = Win32WindowEnumerator::new();
        let windows = enumerator.discover().unwrap();
        // On Windows CI, this will find real windows.
        // On non-Windows, returns empty.
        #[cfg(not(windows))]
        assert!(windows.is_empty());
        let _ = windows; // Use the variable.
    }

    #[test]
    fn test_ignored_classes() {
        let enumerator = Win32WindowEnumerator::new();
        assert!(enumerator.is_ignored_class("Shell_TrayWnd"));
        assert!(enumerator.is_ignored_class("shell_traywnd")); // Case insensitive.
        assert!(!enumerator.is_ignored_class("MyAppClass"));
    }

    #[test]
    fn test_poll_events() {
        let mut enumerator = Win32WindowEnumerator::new();
        let events = enumerator.poll_events();
        assert!(events.is_empty());
    }
}
