//! Types for Win32 window and app information.

use serde::{Deserialize, Serialize};

/// Information about a running Win32 application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Win32AppInfo {
    /// Window handle (HWND as u64).
    pub hwnd: u64,
    /// Window title.
    pub title: String,
    /// Window class name.
    pub class_name: String,
    /// Process ID.
    pub pid: u32,
    /// Executable path.
    pub exe_path: String,
    /// Application name (derived from exe or title).
    pub app_name: String,
    /// Whether the window is visible.
    pub visible: bool,
    /// Whether the window is minimized.
    pub minimized: bool,
    /// Whether the window has focus.
    pub focused: bool,
    /// Current window state.
    pub state: Win32WindowState,
    /// Window bounds (x, y, width, height).
    pub bounds: (i32, i32, u32, u32),
}

/// Window state for dock display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Win32WindowState {
    /// Normal visible window.
    Normal,
    /// Window is minimized to taskbar.
    Minimized,
    /// Window is maximized.
    Maximized,
    /// Window is not responding (ghost window).
    NotResponding,
    /// Window is hidden.
    Hidden,
}

/// Events emitted by the window enumerator when windows change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Win32AppEvent {
    /// A new window was created / discovered.
    WindowOpened(Win32AppInfo),
    /// A window was closed.
    WindowClosed {
        hwnd: u64,
        pid: u32,
    },
    /// A window changed state (title, focus, minimize, etc.).
    WindowChanged(Win32AppInfo),
    /// A window gained focus.
    WindowFocused {
        hwnd: u64,
    },
    /// A window lost focus.
    WindowUnfocused {
        hwnd: u64,
    },
}
