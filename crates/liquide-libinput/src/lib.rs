//! Input device enumeration and management for LiquiDE standalone compositor.
//!
//! Extends `liquide-input` with automatic `/dev/input/event*` device discovery,
//! device type classification (keyboard, mouse, touchpad, touch screen), and
//! hotplug monitoring via udev or inotify.
//!
//! When running as a standalone compositor from TTY, this crate provides
//! the raw input pipeline that feeds into the existing `InputState` and
//! `InputRouter` infrastructure.

pub mod classify;
pub mod enumerate;
pub mod error;
pub mod hotplug;
pub mod seat;

pub use classify::{DeviceCapability, DeviceClass, DeviceInfo};
pub use enumerate::EvdevEnumerator;
pub use error::{LibinputError, Result};
pub use hotplug::HotplugMonitor;
pub use seat::{InputSeat, SeatId};

#[cfg(test)]
mod tests;
