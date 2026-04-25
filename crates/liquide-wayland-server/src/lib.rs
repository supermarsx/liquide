//! Wayland compositor server for the LiquiDE desktop environment.
//!
//! When running as a standalone compositor (launched from TTY), this crate
//! accepts client application connections via the Wayland protocol, managing
//! surface lifecycle, buffer import, input dispatch, and client communication.
//!
//! This crate builds on the protocol type definitions in `liquide-wayland`
//! and integrates with the existing `liquide-compositor` scene graph.

pub mod buffer;
pub mod client;
pub mod display;
pub mod error;
pub mod global;
pub mod registry;
pub mod seat_manager;
pub mod shell_manager;
pub mod shm;
pub mod surface_manager;

pub use buffer::{BufferRef, BufferSource};
pub use client::{ClientConnection, ClientId, ClientState};
pub use display::WaylandDisplay;
pub use error::{Result, WaylandServerError};
pub use global::{Global, GlobalId};
pub use registry::GlobalRegistry;
pub use seat_manager::SeatManager;
pub use shell_manager::ShellManager;
pub use shm::ShmPool;
pub use surface_manager::SurfaceManager;

#[cfg(test)]
mod tests;
