//! Platform-specific audio management backends.
//!
//! Each platform module provides an `AudioManager` struct that implements
//! [`AudioBackend`](crate::AudioBackend).

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::AudioManager;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::AudioManager;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::AudioManager;

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod stub;
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub use stub::AudioManager;
