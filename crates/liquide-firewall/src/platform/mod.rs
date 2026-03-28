#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::PlatformFirewall;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::PlatformFirewall;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::PlatformFirewall;

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub use stub::StubFirewall as PlatformFirewall;

// Stub is always compiled so tests can use it on any platform.
pub(crate) mod stub;
