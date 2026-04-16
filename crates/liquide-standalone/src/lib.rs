//! LiquiDE standalone compositor library.
//!
//! Coordinates DRM/KMS output, raw input, Wayland server, and XWayland
//! to provide a complete standalone compositor experience from TTY.
//!
//! The existing remote desktop path (liquid-session → encoder → transport
//! → liquidclient) remains fully functional and unchanged. This module
//! provides an alternative LOCAL output path (Path B) alongside the
//! existing remote path (Path A).

pub mod config;
pub mod launcher;
pub mod display;
pub mod input;
pub mod wayland;
pub mod xwayland_bridge;

pub use config::StandaloneConfig;
pub use launcher::StandaloneLauncher;

#[cfg(test)]
mod tests;
