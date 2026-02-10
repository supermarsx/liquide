//! Remote cursor state and shape management.

use serde::{Deserialize, Serialize};

/// Resize direction for resize cursors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResizeDirection {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
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

/// Cursor shape for the remote desktop.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CursorShape {
    /// Default arrow cursor.
    #[default]
    Arrow,
    /// Hand / pointer cursor (links, buttons).
    Hand,
    /// Text / I-beam cursor (text fields).
    Text,
    /// Crosshair cursor (precision selection).
    Crosshair,
    /// Wait / busy cursor.
    Wait,
    /// Help cursor (question mark).
    Help,
    /// Not-allowed / forbidden cursor.
    NotAllowed,
    /// Resize cursor in a specific direction.
    Resize(ResizeDirection),
    /// Custom cursor with image data.
    Custom,
    /// Cursor is hidden.
    Hidden,
}

impl std::fmt::Display for CursorShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arrow => write!(f, "arrow"),
            Self::Hand => write!(f, "hand"),
            Self::Text => write!(f, "text"),
            Self::Crosshair => write!(f, "crosshair"),
            Self::Wait => write!(f, "wait"),
            Self::Help => write!(f, "help"),
            Self::NotAllowed => write!(f, "not-allowed"),
            Self::Resize(dir) => write!(f, "resize-{dir}"),
            Self::Custom => write!(f, "custom"),
            Self::Hidden => write!(f, "hidden"),
        }
    }
}

/// State of the remote cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorState {
    /// X position in surface coordinates.
    pub x: i32,
    /// Y position in surface coordinates.
    pub y: i32,
    /// Current cursor shape.
    pub shape: CursorShape,
    /// Whether the cursor is visible.
    pub visible: bool,
    /// Custom cursor image data (RGBA8, row-major).
    #[serde(skip)]
    pub custom_image: Option<Vec<u8>>,
    /// Width of the custom cursor image.
    pub custom_width: u32,
    /// Height of the custom cursor image.
    pub custom_height: u32,
    /// Hotspot X offset within the custom image.
    pub hotspot_x: u32,
    /// Hotspot Y offset within the custom image.
    pub hotspot_y: u32,
}

impl CursorState {
    /// Create a new cursor state with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            shape: CursorShape::Arrow,
            visible: true,
            custom_image: None,
            custom_width: 0,
            custom_height: 0,
            hotspot_x: 0,
            hotspot_y: 0,
        }
    }

    /// Update the cursor position.
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    /// Update the cursor shape.
    pub fn set_shape(&mut self, shape: CursorShape) {
        self.shape = shape;
    }

    /// Show the cursor.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the cursor.
    pub fn hide(&mut self) {
        self.visible = false;
        self.shape = CursorShape::Hidden;
    }

    /// Set a custom cursor image.
    pub fn set_custom_image(
        &mut self,
        image: Vec<u8>,
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    ) {
        self.custom_image = Some(image);
        self.custom_width = width;
        self.custom_height = height;
        self.hotspot_x = hotspot_x;
        self.hotspot_y = hotspot_y;
        self.shape = CursorShape::Custom;
    }

    /// Check if this cursor has a custom image.
    #[must_use]
    pub fn has_custom_image(&self) -> bool {
        self.custom_image.is_some()
    }
}

impl Default for CursorState {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CursorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CursorState({}, {}, shape={}, visible={})",
            self.x, self.y, self.shape, self.visible
        )
    }
}
