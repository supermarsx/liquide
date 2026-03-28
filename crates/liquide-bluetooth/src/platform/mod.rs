#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::BluetoothManager;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::BluetoothManager;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::BluetoothManager;

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub use stub::BluetoothManager;

// Stub is always compiled so tests can use it on any platform.
pub(crate) mod stub;
