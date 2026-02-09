//! Cursor update types for the out-of-band cursor channel.

use crate::pixel::PixelFormat;

/// Cursor bitmap data.
#[derive(Debug, Clone)]
pub struct CursorBitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub format: PixelFormat,
}

impl CursorBitmap {
    /// Create a new cursor bitmap.
    #[must_use]
    pub fn new(width: u32, height: u32, pixels: Vec<u8>, format: PixelFormat) -> Self {
        Self {
            width,
            height,
            pixels,
            format,
        }
    }
}

/// Cursor update dispatched to the transport on the cursor channel.
#[derive(Debug, Clone)]
pub struct CursorUpdate {
    /// Screen X position.
    pub x: u32,
    /// Screen Y position.
    pub y: u32,
    /// Hotspot X offset within the bitmap.
    pub hotspot_x: u32,
    /// Hotspot Y offset within the bitmap.
    pub hotspot_y: u32,
    /// New cursor image (or `None` for position-only updates).
    pub bitmap: Option<CursorBitmap>,
    /// Whether the cursor is visible.
    pub visible: bool,
}

impl CursorUpdate {
    /// Create a position-only cursor update (no shape change).
    #[must_use]
    pub fn position_only(x: u32, y: u32) -> Self {
        Self {
            x,
            y,
            hotspot_x: 0,
            hotspot_y: 0,
            bitmap: None,
            visible: true,
        }
    }

    /// Create a cursor update with a new bitmap shape.
    #[must_use]
    pub fn with_bitmap(
        x: u32,
        y: u32,
        hotspot_x: u32,
        hotspot_y: u32,
        bitmap: CursorBitmap,
    ) -> Self {
        Self {
            x,
            y,
            hotspot_x,
            hotspot_y,
            bitmap: Some(bitmap),
            visible: true,
        }
    }

    /// Create a hidden-cursor update.
    #[must_use]
    pub fn hidden() -> Self {
        Self {
            x: 0,
            y: 0,
            hotspot_x: 0,
            hotspot_y: 0,
            bitmap: None,
            visible: false,
        }
    }
}
