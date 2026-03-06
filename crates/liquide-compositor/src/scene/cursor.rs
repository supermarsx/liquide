//! Cursor shape types and legacy compatibility.

use serde::{Deserialize, Serialize};

// Re-export cursor types from liquide-cursor crate
pub use liquide_cursor::{CursorShape as NewCursorShape, ResizeDirection};

/// Legacy cursor shape enum for backward compatibility.
///
/// **Deprecated**: Use `liquide_cursor::CursorShape` directly.
/// This enum provides compatibility with existing code but will be removed in a future version.
#[deprecated(since = "0.1.0", note = "use liquide_cursor::CursorShape instead")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LegacyCursorShape {
    Arrow,
    Move,
    ResizeNS,
    ResizeEW,
    ResizeNWSE,
    ResizeNESW,
    Pointer,
    Text,
    NotAllowed,
    Wait,
    Progress,
    Help,
    Crosshair,
    Grab,
    Grabbing,
    ZoomIn,
    ZoomOut,
    ContextMenu,
    Alias,
    Copy,
    NoDrop,
    Cell,
    VerticalText,
    AllScroll,
    ExpandH,
    ExpandV,
}

/// Current cursor shape type alias.
/// Points to the new unified cursor type from liquide-cursor crate.
pub type CursorShape = NewCursorShape;

/// Convert legacy cursor shape to new format.
#[allow(deprecated)]
impl From<LegacyCursorShape> for NewCursorShape {
    fn from(legacy: LegacyCursorShape) -> Self {
        match legacy {
            LegacyCursorShape::Arrow => NewCursorShape::Arrow,
            LegacyCursorShape::Move => NewCursorShape::Move,
            LegacyCursorShape::ResizeNS => NewCursorShape::Resize(ResizeDirection::North),
            LegacyCursorShape::ResizeEW => NewCursorShape::Resize(ResizeDirection::East),
            LegacyCursorShape::ResizeNWSE => NewCursorShape::Resize(ResizeDirection::NorthWest),
            LegacyCursorShape::ResizeNESW => NewCursorShape::Resize(ResizeDirection::NorthEast),
            LegacyCursorShape::Pointer => NewCursorShape::Pointer,
            LegacyCursorShape::Text => NewCursorShape::Text,
            LegacyCursorShape::NotAllowed => NewCursorShape::NotAllowed,
            LegacyCursorShape::Wait => NewCursorShape::Wait,
            LegacyCursorShape::Progress => NewCursorShape::Progress,
            LegacyCursorShape::Help => NewCursorShape::Help,
            LegacyCursorShape::Crosshair => NewCursorShape::Crosshair,
            LegacyCursorShape::Grab => NewCursorShape::Grab,
            LegacyCursorShape::Grabbing => NewCursorShape::Grabbing,
            LegacyCursorShape::ZoomIn => NewCursorShape::ZoomIn,
            LegacyCursorShape::ZoomOut => NewCursorShape::ZoomOut,
            LegacyCursorShape::ContextMenu => NewCursorShape::ContextMenu,
            LegacyCursorShape::Alias => NewCursorShape::Alias,
            LegacyCursorShape::Copy => NewCursorShape::Copy,
            LegacyCursorShape::NoDrop => NewCursorShape::NoDrop,
            LegacyCursorShape::Cell => NewCursorShape::Cell,
            LegacyCursorShape::VerticalText => NewCursorShape::VerticalText,
            LegacyCursorShape::AllScroll => NewCursorShape::AllScroll,
            LegacyCursorShape::ExpandH => NewCursorShape::ColResize,
            LegacyCursorShape::ExpandV => NewCursorShape::RowResize,
        }
    }
}

#[allow(deprecated)]
impl Default for LegacyCursorShape {
    fn default() -> Self {
        #[allow(deprecated)]
        Self::Arrow
    }
}
