//! Core ID types used throughout the desktop model.

/// Unique identifier for a window station.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowStationId(pub u32);

/// Unique identifier for a desktop within a window station.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DesktopId(pub u32);

/// Unique identifier for a window within a desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u32);

/// An interned string identifier (atom).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Atom(pub u32);
