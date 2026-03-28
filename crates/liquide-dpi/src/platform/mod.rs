#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::PlatformDpi;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::PlatformDpi;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::PlatformDpi;

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod stub;
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub use stub::PlatformDpi;
