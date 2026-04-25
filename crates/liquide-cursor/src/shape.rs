//! Cursor shape definitions.

use serde::{Deserialize, Serialize};

/// Direction for resize cursors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResizeDirection {
    /// North (top edge).
    North,
    /// South (bottom edge).
    South,
    /// East (right edge).
    East,
    /// West (left edge).
    West,
    /// North-East (top-right corner).
    NorthEast,
    /// North-West (top-left corner).
    NorthWest,
    /// South-East (bottom-right corner).
    SouthEast,
    /// South-West (bottom-left corner).
    SouthWest,
}

impl ResizeDirection {
    /// Returns the CSS cursor name for this resize direction.
    pub fn css_name(&self) -> &'static str {
        match self {
            Self::North | Self::South => "ns-resize",
            Self::East | Self::West => "ew-resize",
            Self::NorthEast | Self::SouthWest => "nesw-resize",
            Self::NorthWest | Self::SouthEast => "nwse-resize",
        }
    }
}

impl std::fmt::Display for ResizeDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::North => "north",
            Self::South => "south",
            Self::East => "east",
            Self::West => "west",
            Self::NorthEast => "north-east",
            Self::NorthWest => "north-west",
            Self::SouthEast => "south-east",
            Self::SouthWest => "south-west",
        };
        write!(f, "{name}")
    }
}

/// Standard cursor shapes for the Liquide desktop.
///
/// Includes 27 predefined cursor types covering all common use cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CursorShape {
    /// Default arrow pointer (normal selection).
    Arrow,

    /// Four-way move cursor (window dragging).
    Move,

    /// Resize cursor for a specific direction/edge.
    Resize(ResizeDirection),

    /// Pointing hand (clickable items, links, buttons).
    Pointer,

    /// Text selection I-beam (text fields, editors).
    Text,

    /// Not-allowed / forbidden (invalid drop targets).
    NotAllowed,

    /// Busy / waiting (spinning wheel, hourglass).
    Wait,

    /// Progress (arrow + wait, background operation).
    Progress,

    /// Help (arrow + question mark).
    Help,

    /// Crosshair / precise selection (drawing, cropping).
    Crosshair,

    /// Open hand (pan/grab mode).
    Grab,

    /// Closed hand (actively grabbing/panning).
    Grabbing,

    /// Zoom in (magnifying glass with +).
    ZoomIn,

    /// Zoom out (magnifying glass with -).
    ZoomOut,

    /// Context menu available (right-click hint).
    ContextMenu,

    /// Alias / shortcut (link indicator).
    Alias,

    /// Copy operation (drag & drop).
    Copy,

    /// No drop / invalid drop target.
    NoDrop,

    /// Cell selection (spreadsheets).
    Cell,

    /// Vertical text selection (East Asian text).
    VerticalText,

    /// Column resize (table columns).
    ColResize,

    /// Row resize (table rows).
    RowResize,

    /// All-scroll (omnidirectional scrolling).
    AllScroll,

    /// Custom cursor with external image data.
    Custom { id: u64 },

    /// Hidden / invisible cursor.
    Hidden,
}

impl Default for CursorShape {
    fn default() -> Self {
        Self::Arrow
    }
}

impl CursorShape {
    /// Returns the CSS cursor name for this shape.
    pub fn css_name(&self) -> &str {
        match self {
            Self::Arrow => "default",
            Self::Move => "move",
            Self::Resize(dir) => dir.css_name(),
            Self::Pointer => "pointer",
            Self::Text => "text",
            Self::NotAllowed => "not-allowed",
            Self::Wait => "wait",
            Self::Progress => "progress",
            Self::Help => "help",
            Self::Crosshair => "crosshair",
            Self::Grab => "grab",
            Self::Grabbing => "grabbing",
            Self::ZoomIn => "zoom-in",
            Self::ZoomOut => "zoom-out",
            Self::ContextMenu => "context-menu",
            Self::Alias => "alias",
            Self::Copy => "copy",
            Self::NoDrop => "no-drop",
            Self::Cell => "cell",
            Self::VerticalText => "vertical-text",
            Self::ColResize => "col-resize",
            Self::RowResize => "row-resize",
            Self::AllScroll => "all-scroll",
            Self::Custom { .. } => "custom",
            Self::Hidden => "none",
        }
    }

    /// Returns true if this is a resize cursor.
    pub fn is_resize(&self) -> bool {
        matches!(self, Self::Resize(_))
    }

    /// Returns true if this cursor indicates interactivity.
    pub fn is_interactive(&self) -> bool {
        matches!(
            self,
            Self::Pointer | Self::Grab | Self::Grabbing | Self::Move
        )
    }
}

impl std::fmt::Display for CursorShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arrow => write!(f, "arrow"),
            Self::Move => write!(f, "move"),
            Self::Resize(dir) => write!(f, "resize-{dir}"),
            Self::Pointer => write!(f, "pointer"),
            Self::Text => write!(f, "text"),
            Self::NotAllowed => write!(f, "not-allowed"),
            Self::Wait => write!(f, "wait"),
            Self::Progress => write!(f, "progress"),
            Self::Help => write!(f, "help"),
            Self::Crosshair => write!(f, "crosshair"),
            Self::Grab => write!(f, "grab"),
            Self::Grabbing => write!(f, "grabbing"),
            Self::ZoomIn => write!(f, "zoom-in"),
            Self::ZoomOut => write!(f, "zoom-out"),
            Self::ContextMenu => write!(f, "context-menu"),
            Self::Alias => write!(f, "alias"),
            Self::Copy => write!(f, "copy"),
            Self::NoDrop => write!(f, "no-drop"),
            Self::Cell => write!(f, "cell"),
            Self::VerticalText => write!(f, "vertical-text"),
            Self::ColResize => write!(f, "col-resize"),
            Self::RowResize => write!(f, "row-resize"),
            Self::AllScroll => write!(f, "all-scroll"),
            Self::Custom { id } => write!(f, "custom-{id}"),
            Self::Hidden => write!(f, "hidden"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_css_names() {
        assert_eq!(CursorShape::Arrow.css_name(), "default");
        assert_eq!(CursorShape::Pointer.css_name(), "pointer");
        assert_eq!(
            CursorShape::Resize(ResizeDirection::North).css_name(),
            "ns-resize"
        );
    }

    #[test]
    fn test_is_resize() {
        assert!(CursorShape::Resize(ResizeDirection::North).is_resize());
        assert!(!CursorShape::Arrow.is_resize());
    }

    #[test]
    fn test_is_interactive() {
        assert!(CursorShape::Pointer.is_interactive());
        assert!(CursorShape::Grab.is_interactive());
        assert!(!CursorShape::Arrow.is_interactive());
    }
}
