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

pub mod atoms;
pub mod clipboard;
pub mod dnd;
pub mod error;
pub mod process;
pub mod window;

pub use atoms::AtomCache;
pub use error::{Result, XWaylandError};
pub use process::{XWaylandConfig, XWaylandProcess, XWaylandState};
pub use window::{X11Window, X11WindowId, X11WindowState, X11WindowType};

#[cfg(test)]
mod tests;
