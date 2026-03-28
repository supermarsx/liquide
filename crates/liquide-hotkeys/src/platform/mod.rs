/// Stub backend — stores bindings but never fires. Used for testing and
/// unsupported platforms.
pub mod stub;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

// Platform-specific GlobalHotkeyManager re-export
#[cfg(target_os = "windows")]
pub use windows::GlobalHotkeyManager;

#[cfg(target_os = "linux")]
pub use linux::GlobalHotkeyManager;

#[cfg(target_os = "macos")]
pub use macos::GlobalHotkeyManager;

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub use stub::GlobalHotkeyManager;
