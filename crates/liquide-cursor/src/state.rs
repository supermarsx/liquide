//! Cursor state management - position, visibility, and shape tracking.

use crate::shape::CursorShape;
use serde::{Deserialize, Serialize};

/// Visibility state of the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CursorVisibility {
    /// Cursor is visible.
    Visible,
    /// Cursor is hidden.
    Hidden,
    /// Cursor is confined to a region (grabbed by application).
    Confined,
}

impl Default for CursorVisibility {
    fn default() -> Self {
        Self::Visible
    }
}

/// Complete state of a cursor including position, shape, and custom image data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorState {
    /// X position in surface coordinates.
    pub x: f32,

    /// Y position in surface coordinates.
    pub y: f32,

    /// Current cursor shape.
    pub shape: CursorShape,

    /// Visibility state.
    pub visibility: CursorVisibility,

    /// Custom cursor image data (RGBA8, row-major).
    /// Only used when shape is Custom.
    #[serde(skip)]
    pub custom_image: Option<Vec<u8>>,

    /// Width of the custom cursor image in pixels.
    pub custom_width: u32,

    /// Height of the custom cursor image in pixels.
    pub custom_height: u32,

    /// Hotspot X offset within the custom image.
    pub hotspot_x: u32,

    /// Hotspot Y offset within the custom image.
    pub hotspot_y: u32,

    /// Scale factor for the cursor (1.0 = normal size).
    pub scale: f32,
}

impl Default for CursorState {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            shape: CursorShape::Arrow,
            visibility: CursorVisibility::Visible,
            custom_image: None,
            custom_width: 0,
            custom_height: 0,
            hotspot_x: 0,
            hotspot_y: 0,
            scale: 1.0,
        }
    }
}

impl CursorState {
    /// Create a new cursor state at the given position with default shape.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            ..Default::default()
        }
    }

    /// Set the cursor position.
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    /// Set the cursor shape.
    pub fn set_shape(&mut self, shape: CursorShape) {
        self.shape = shape;
        // Clear custom image data if shape is not Custom
        if !matches!(shape, CursorShape::Custom { .. }) {
            self.custom_image = None;
            self.custom_width = 0;
            self.custom_height = 0;
        }
    }

    /// Set the visibility state.
    pub fn set_visibility(&mut self, visibility: CursorVisibility) {
        self.visibility = visibility;
    }

    /// Show the cursor.
    pub fn show(&mut self) {
        self.visibility = CursorVisibility::Visible;
    }

    /// Hide the cursor.
    pub fn hide(&mut self) {
        self.visibility = CursorVisibility::Hidden;
    }

    /// Returns true if the cursor is visible.
    pub fn is_visible(&self) -> bool {
        self.visibility == CursorVisibility::Visible
    }

    /// Set a custom cursor image.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Image dimensions don't match the provided data
    /// - Hotspot is outside image bounds
    pub fn set_custom_image(
        &mut self,
        id: u64,
        image_data: Vec<u8>,
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> crate::Result<()> {
        // Validate image data size
        let expected_size = (width * height * 4) as usize; // RGBA8
        if image_data.len() != expected_size {
            return Err(crate::CursorError::InvalidImage(format!(
                "expected {} bytes for {}x{} RGBA8 image, got {}",
                expected_size,
                width,
                height,
                image_data.len()
            )));
        }

        // Validate hotspot
        if hotspot_x >= width || hotspot_y >= height {
            return Err(crate::CursorError::InvalidHotspot {
                x: hotspot_x,
                y: hotspot_y,
                width,
                height,
            });
        }

        self.shape = CursorShape::Custom { id };
        self.custom_image = Some(image_data);
        self.custom_width = width;
        self.custom_height = height;
        self.hotspot_x = hotspot_x;
        self.hotspot_y = hotspot_y;

        Ok(())
    }

    /// Get the effective cursor size accounting for scale.
    pub fn effective_size(&self) -> (u32, u32) {
        if self.custom_image.is_some() {
            let w = (self.custom_width as f32 * self.scale) as u32;
            let h = (self.custom_height as f32 * self.scale) as u32;
            (w, h)
        } else {
            // Default cursor size
            let size = (24.0 * self.scale) as u32;
            (size, size)
        }
    }

    /// Get the hotspot position accounting for scale.
    pub fn effective_hotspot(&self) -> (f32, f32) {
        let hx = self.hotspot_x as f32 * self.scale;
        let hy = self.hotspot_y as f32 * self.scale;
        (hx, hy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = CursorState::default();
        assert_eq!(state.shape, CursorShape::Arrow);
        assert!(state.is_visible());
    }

    #[test]
    fn test_visibility() {
        let mut state = CursorState::default();
        assert!(state.is_visible());

        state.hide();
        assert!(!state.is_visible());

        state.show();
        assert!(state.is_visible());
    }

    #[test]
    fn test_custom_image_validation() {
        let mut state = CursorState::default();

        // Invalid size
        let result = state.set_custom_image(1, vec![0; 100], 32, 32, 0, 0);
        assert!(result.is_err());

        // Invalid hotspot
        let valid_data = vec![0; 32 * 32 * 4];
        let result = state.set_custom_image(1, valid_data.clone(), 32, 32, 50, 50);
        assert!(result.is_err());

        // Valid
        let result = state.set_custom_image(1, valid_data, 32, 32, 16, 16);
        assert!(result.is_ok());
        assert_eq!(state.custom_width, 32);
        assert_eq!(state.custom_height, 32);
    }

    #[test]
    fn test_effective_size_with_scale() {
        let mut state = CursorState::default();
        state.scale = 2.0;

        let (w, h) = state.effective_size();
        assert_eq!((w, h), (48, 48)); // 24 * 2.0
    }
}
