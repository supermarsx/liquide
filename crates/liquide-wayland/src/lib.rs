//! Wayland-compatible protocol abstraction layer for LiquiDE.
//!
//! Provides a pure-Rust implementation of core Wayland protocol concepts:
//! wire format encoding/decoding, surface management with double-buffered
//! state, xdg_shell window roles, input seat handling, and output
//! (monitor) description.
//!
//! This crate does not depend on any system Wayland libraries — it
//! implements the protocol semantics from scratch based on the published
//! Wayland protocol specification.

pub mod compositor;
pub mod output;
pub mod protocol;
pub mod seat;
pub mod shell;
pub mod surface;

// Re-export primary types at crate root.
pub use compositor::WlCompositor;
pub use output::{
    Output, OutputGeometry, OutputMode, OutputModeFlags, OutputTransform, SubpixelOrder,
};
pub use protocol::{Arg, ArgType, Interface, MessageDesc, MessageHeader, ObjectId, WlMessage};
pub use seat::{
    Axis, AxisSource, ButtonState, KeyState, KeyboardEvent, KeymapFormat, Modifiers, Pointer,
    PointerEvent, Seat, SeatCapability, Touch, TouchEvent,
};
pub use shell::{
    Anchor, ConfigureEvent, ConstraintAdjustment, Gravity, PopupPositioner, ResizeEdge,
    ToplevelState, XdgPopup, XdgSurface, XdgToplevel,
};
pub use surface::{DamageRect, Region, SubsurfaceMode, Surface, SurfaceState, Transform};
