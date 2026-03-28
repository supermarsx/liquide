#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::NetworkManager;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::NetworkManager;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::NetworkManager;

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub use stub::NetworkManager;

// Stub is always compiled so tests can use it on any platform.
pub(crate) mod stub;
