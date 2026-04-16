//! XWayland integration for the LiquiDE standalone compositor.
//!
//! Manages the XWayland process lifecycle, enabling X11 applications to
//! run inside the LiquiDE Wayland compositor. X11 client windows are
//! mapped to Wayland surfaces and integrated into the compositor's
//! scene graph.
//!
//! # Architecture
//!
//! ```text
//! X11 App (e.g. Firefox, GIMP)
//!     ↓ X11 protocol
//! XWayland process
//!     ↓ Wayland protocol (internal)
//! liquide-xwayland bridge
//!     ↓ SceneNode
//! liquide-compositor scene graph
//! ```

pub mod error;
pub mod process;
pub mod window;
pub mod atoms;
pub mod clipboard;
pub mod dnd;

pub use error::{XWaylandError, Result};
pub use process::{XWaylandProcess, XWaylandConfig, XWaylandState};
pub use window::{X11Window, X11WindowId, X11WindowType, X11WindowState};
pub use atoms::AtomCache;

#[cfg(test)]
mod tests;
