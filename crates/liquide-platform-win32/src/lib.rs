//! Win32 platform integration — enumerates native Windows applications
//! for the Liquide desktop dock and provides window management hooks.
//!
//! # Architecture
//!
//! On Windows, the Liquide desktop needs to show native Win32 apps in its
//! dock. This crate uses `EnumWindows` and related Win32 APIs to discover
//! running applications, extract their icons, and track their state.
//!
//! ```text
//! Win32 API
//!   ├── EnumWindows → list visible top-level windows
//!   ├── GetWindowText → window titles
//!   ├── GetWindowThreadProcessId → process info
//!   ├── GetClassNameW → window class
//!   └── ExtractIconExW → app icons
//!       │
//!       ▼
//! Win32WindowEnumerator
//!   ├── discover() → Vec<Win32AppInfo>
//!   ├── poll_changes() → Vec<Win32AppEvent>
//!   └── get_icon(hwnd) → Option<IconData>
//! ```

pub mod enumerator;
pub mod icon;
pub mod types;

pub use enumerator::Win32WindowEnumerator;
pub use icon::IconExtractor;
pub use types::{Win32AppEvent, Win32AppInfo, Win32WindowState};

use thiserror::Error;

/// Errors from Win32 platform operations.
#[derive(Debug, Error)]
pub enum Win32Error {
    /// Win32 API call failed.
    #[error("Win32 API error: {function} failed with code {code}")]
    ApiError { function: String, code: u32 },

    /// Window handle is invalid or stale.
    #[error("invalid window handle: {0}")]
    InvalidHandle(u64),

    /// Not running on Windows.
    #[error("Win32 integration requires Windows OS")]
    NotWindows,
}

pub type Result<T> = std::result::Result<T, Win32Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Win32Error::ApiError {
            function: "EnumWindows".into(),
            code: 5,
        };
        assert!(err.to_string().contains("EnumWindows"));
    }
}
