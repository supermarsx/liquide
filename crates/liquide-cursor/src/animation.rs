//! Animated cursor support - multi-frame cursors with timing.

use crate::{CursorShape, CursorState};
use serde::{Deserialize, Serialize};

/// A single frame in an animated cursor sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorFrame {
    /// RGBA8 image data for this frame.
    #[serde(skip)]
    pub image_data: Vec<u8>,

    /// Image width in pixels.
    pub width: u32,

    /// Image height in pixels.
    pub height: u32,

    /// Hotspot X offset.
    pub hotspot_x: u32,

    /// Hotspot Y offset.
    pub hotspot_y: u32,

    /// Duration to display this frame in milliseconds.
    pub duration_ms: u32,
}

/// An animated cursor with multiple frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimatedCursor {
    /// Unique identifier for this animated cursor.
    pub id: u64,

    /// Cursor shape (usually Wait or Progress for animated cursors).
    pub shape: CursorShape,

    /// Frame sequence.
    pub frames: Vec<CursorFrame>,

    /// Total duration of the animation loop in milliseconds.
    pub total_duration_ms: u32,

    /// Current frame index.
    #[serde(skip)]
    current_frame: usize,

    /// Elapsed time in current frame (milliseconds).
    #[serde(skip)]
    frame_elapsed_ms: u32,
}

impl AnimatedCursor {
    /// Create a new animated cursor.
    pub fn new(id: u64, shape: CursorShape, frames: Vec<CursorFrame>) -> Self {
        let total_duration_ms = frames.iter().map(|f| f.duration_ms).sum();

        Self {
            id,
            shape,
            frames,
            total_duration_ms,
            current_frame: 0,
            frame_elapsed_ms: 0,
        }
    }

    /// Update the animation state by the given delta time.
    ///
    /// Returns true if the frame changed.
    pub fn update(&mut self, delta_ms: u32) -> bool {
        if self.frames.is_empty() {
            return false;
        }

        self.frame_elapsed_ms += delta_ms;

        let current_duration = self.frames[self.current_frame].duration_ms;
        if self.frame_elapsed_ms >= current_duration {
            self.frame_elapsed_ms = 0;
            self.current_frame = (self.current_frame + 1) % self.frames.len();
            return true;
        }

        false
    }

    /// Get the current frame.
    pub fn current_frame(&self) -> Option<&CursorFrame> {
        self.frames.get(self.current_frame)
    }

    /// Get the current frame index.
    pub fn current_frame_index(&self) -> usize {
        self.current_frame
    }

    /// Reset animation to the first frame.
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.frame_elapsed_ms = 0;
    }

    /// Apply the current frame to a cursor state.
    pub fn apply_to_state(&self, state: &mut CursorState) {
        if let Some(frame) = self.current_frame() {
            let _ = state.set_custom_image(
                self.id,
                frame.image_data.clone(),
                frame.width,
                frame.height,
                frame.hotspot_x,
                frame.hotspot_y,
            );
        }
    }
}

/// Builder for creating animated cursors.
#[allow(dead_code)]
pub struct AnimatedCursorBuilder {
    id: u64,
    shape: CursorShape,
    frames: Vec<CursorFrame>,
}

#[allow(dead_code)]
impl AnimatedCursorBuilder {
    /// Create a new builder.
    pub fn new(id: u64, shape: CursorShape) -> Self {
        Self {
            id,
            shape,
            frames: Vec::new(),
        }
    }

    /// Add a frame to the animation.
    pub fn add_frame(
        mut self,
        image_data: Vec<u8>,
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
        duration_ms: u32,
    ) -> Self {
        self.frames.push(CursorFrame {
            image_data,
            width,
            height,
            hotspot_x,
            hotspot_y,
            duration_ms,
        });
        self
    }

    /// Build the animated cursor.
    pub fn build(self) -> AnimatedCursor {
        AnimatedCursor::new(self.id, self.shape, self.frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animated_cursor_update() {
        let frames = vec![
            CursorFrame {
                image_data: vec![0; 24 * 24 * 4],
                width: 24,
                height: 24,
                hotspot_x: 12,
                hotspot_y: 12,
                duration_ms: 100,
            },
            CursorFrame {
                image_data: vec![255; 24 * 24 * 4],
                width: 24,
                height: 24,
                hotspot_x: 12,
                hotspot_y: 12,
                duration_ms: 100,
            },
        ];

        let mut cursor = AnimatedCursor::new(1, CursorShape::Wait, frames);

        assert_eq!(cursor.current_frame_index(), 0);

        // Advance to next frame
        let changed = cursor.update(100);
        assert!(changed);
        assert_eq!(cursor.current_frame_index(), 1);

        // Wrap around
        let changed = cursor.update(100);
        assert!(changed);
        assert_eq!(cursor.current_frame_index(), 0);
    }

    #[test]
    fn test_builder() {
        let cursor = AnimatedCursorBuilder::new(1, CursorShape::Wait)
            .add_frame(vec![0; 24 * 24 * 4], 24, 24, 12, 12, 100)
            .add_frame(vec![255; 24 * 24 * 4], 24, 24, 12, 12, 100)
            .build();

        assert_eq!(cursor.frames.len(), 2);
        assert_eq!(cursor.total_duration_ms, 200);
    }
}
