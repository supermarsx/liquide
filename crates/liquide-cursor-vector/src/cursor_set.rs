//! Vector cursor set management

use crate::error::{Result, VectorCursorError};
use liquide_cursor::CursorShape;
use std::collections::HashMap;
use std::path::Path;

/// A single vector cursor
#[derive(Debug, Clone)]
pub struct VectorCursor {
    /// SVG data
    pub svg_data: String,
    
    /// Hotspot X coordinate (normalized 0.0-1.0)
    pub hotspot_x: f32,
    
    /// Hotspot Y coordinate (normalized 0.0-1.0)
    pub hotspot_y: f32,
    
    /// Nominal size (used for scaling reference)
    pub nominal_size: u32,
}

impl VectorCursor {
    /// Create a new vector cursor from SVG data
    pub fn new(svg_data: String, hotspot_x: f32, hotspot_y: f32) -> Self {
        Self {
            svg_data,
            hotspot_x,
            hotspot_y,
            nominal_size: 32,
        }
    }
    
    /// Set nominal size
    pub fn with_nominal_size(mut self, size: u32) -> Self {
        self.nominal_size = size;
        self
    }
    
    /// Get hotspot in pixels for a given size
    pub fn hotspot_pixels(&self, size: u32) -> (u32, u32) {
        let x = (self.hotspot_x * size as f32) as u32;
        let y = (self.hotspot_y * size as f32) as u32;
        (x, y)
    }
    
    /// Load from SVG file
    pub fn from_file<P: AsRef<Path>>(path: P, hotspot_x: f32, hotspot_y: f32) -> Result<Self> {
        let svg_data = std::fs::read_to_string(path)?;
        Ok(Self::new(svg_data, hotspot_x, hotspot_y))
    }
}

/// Collection of vector cursors
#[derive(Debug, Default)]
pub struct VectorCursorSet {
    cursors: HashMap<CursorShape, VectorCursor>,
}

impl VectorCursorSet {
    /// Create an empty cursor set
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Load the default built-in cursor set
    pub fn load_default() -> Result<Self> {
        let mut set = Self::new();
        
        // Add built-in high-quality SVG cursors
        set.add(CursorShape::Arrow, Self::arrow_svg());
        set.add(CursorShape::Pointer, Self::pointer_svg());
        set.add(CursorShape::Text, Self::text_svg());
        set.add(CursorShape::Move, Self::move_svg());
        set.add(CursorShape::Wait, Self::wait_svg());
        set.add(CursorShape::Crosshair, Self::crosshair_svg());
        set.add(CursorShape::NotAllowed, Self::not_allowed_svg());
        set.add(CursorShape::Grab, Self::grab_svg());
        set.add(CursorShape::Grabbing, Self::grabbing_svg());
        
        // Add resize cursors
        use liquide_cursor::ResizeDirection;
        set.add(
            CursorShape::Resize(ResizeDirection::North),
            Self::resize_vertical_svg(),
        );
        set.add(
            CursorShape::Resize(ResizeDirection::East),
            Self::resize_horizontal_svg(),
        );
        set.add(
            CursorShape::Resize(ResizeDirection::NorthEast),
            Self::resize_diagonal_ne_svg(),
        );
        set.add(
            CursorShape::Resize(ResizeDirection::NorthWest),
            Self::resize_diagonal_nw_svg(),
        );
        
        Ok(set)
    }
    
    /// Add a cursor to the set
    pub fn add(&mut self, shape: CursorShape, cursor: VectorCursor) {
        self.cursors.insert(shape, cursor);
    }
    
    /// Get a cursor by shape
    pub fn get(&self, shape: CursorShape) -> Result<&VectorCursor> {
        self.cursors
            .get(&shape)
            .ok_or_else(|| VectorCursorError::NotFound(format!("{:?}", shape)))
    }
    
    /// Check if shape is available
    pub fn has(&self, shape: CursorShape) -> bool {
        self.cursors.contains_key(&shape)
    }
    
    /// Get all available shapes
    pub fn shapes(&self) -> Vec<CursorShape> {
        self.cursors.keys().copied().collect()
    }
    
    // Built-in SVG cursor definitions
    
    fn arrow_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <defs>
                    <filter id="shadow">
                        <feDropShadow dx="1" dy="1" stdDeviation="1" flood-opacity="0.5"/>
                    </filter>
                </defs>
                <path d="M 4 4 L 4 28 L 12 20 L 16 28 L 20 26 L 16 18 L 24 18 Z" 
                      fill="white" stroke="black" stroke-width="1.5" filter="url(#shadow)"/>
            </svg>"#.to_string(),
            0.125, // hotspot at 12.5% (4/32)
            0.125,
        )
    }
    
    fn pointer_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <path d="M 16 4 L 12 12 L 8 14 L 14 16 L 16 22 L 18 16 L 24 14 L 20 12 Z" 
                      fill="white" stroke="black" stroke-width="1.5"/>
                <circle cx="16" cy="16" r="2" fill="black"/>
            </svg>"#.to_string(),
            0.5,
            0.5,
        )
    }
    
    fn text_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <line x1="16" y1="4" x2="16" y2="28" stroke="black" stroke-width="2"/>
                <line x1="12" y1="4" x2="20" y2="4" stroke="black" stroke-width="2"/>
                <line x1="12" y1="28" x2="20" y2="28" stroke="black" stroke-width="2"/>
            </svg>"#.to_string(),
            0.5,
            0.5,
        )
    }
    
    fn move_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <path d="M 16 4 L 12 8 L 14 8 L 14 14 L 8 14 L 8 12 L 4 16 L 8 20 L 8 18 L 14 18 L 14 24 L 12 24 L 16 28 L 20 24 L 18 24 L 18 18 L 24 18 L 24 20 L 28 16 L 24 12 L 24 14 L 18 14 L 18 8 L 20 8 Z" 
                      fill="black" stroke="white" stroke-width="1"/>
            </svg>"#.to_string(),
            0.5,
            0.5,
        )
    }
    
    fn wait_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <circle cx="16" cy="16" r="12" fill="none" stroke="black" stroke-width="2"/>
                <path d="M 16 16 L 16 8" stroke="black" stroke-width="2" stroke-linecap="round"/>
                <path d="M 16 16 L 22 16" stroke="black" stroke-width="1.5" stroke-linecap="round"/>
            </svg>"#.to_string(),
            0.5,
            0.5,
        )
    }
    
    fn crosshair_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <line x1="16" y1="0" x2="16" y2="32" stroke="black" stroke-width="1"/>
                <line x1="0" y1="16" x2="32" y2="16" stroke="black" stroke-width="1"/>
                <circle cx="16" cy="16" r="6" fill="none" stroke="black" stroke-width="1"/>
            </svg>"#.to_string(),
            0.5,
            0.5,
        )
    }
    
    fn not_allowed_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <circle cx="16" cy="16" r="12" fill="red" opacity="0.8" stroke="darkred" stroke-width="2"/>
                <line x1="8" y1="8" x2="24" y2="24" stroke="white" stroke-width="3" stroke-linecap="round"/>
            </svg>"#.to_string(),
            0.5,
            0.5,
        )
    }
    
    fn grab_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <g transform="translate(4, 4)">
                    <path d="M 8 4 L 8 12 M 12 2 L 12 12 M 16 2 L 16 12 M 20 4 L 20 12" 
                          stroke="black" stroke-width="2" stroke-linecap="round" fill="none"/>
                    <path d="M 4 12 Q 4 8, 8 8 L 8 12 L 4 12" fill="black"/>
                    <rect x="6" y="12" width="16" height="8" rx="2" fill="black"/>
                </g>
            </svg>"#.to_string(),
            0.5,
            0.4,
        )
    }
    
    fn grabbing_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <g transform="translate(4, 6)">
                    <path d="M 8 6 L 8 14 M 12 4 L 12 14 M 16 4 L 16 14 M 20 6 L 20 14" 
                          stroke="black" stroke-width="2" stroke-linecap="round" fill="none"/>
                    <rect x="6" y="14" width="16" height="8" rx="2" fill="black"/>
                </g>
            </svg>"#.to_string(),
            0.5,
            0.5,
        )
    }
    
    fn resize_vertical_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <line x1="16" y1="4" x2="16" y2="28" stroke="black" stroke-width="2"/>
                <path d="M 16 4 L 12 8 L 20 8 Z" fill="black"/>
                <path d="M 16 28 L 12 24 L 20 24 Z" fill="black"/>
            </svg>"#.to_string(),
            0.5,
            0.5,
        )
    }
    
    fn resize_horizontal_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <line x1="4" y1="16" x2="28" y2="16" stroke="black" stroke-width="2"/>
                <path d="M 4 16 L 8 12 L 8 20 Z" fill="black"/>
                <path d="M 28 16 L 24 12 L 24 20 Z" fill="black"/>
            </svg>"#.to_string(),
            0.5,
            0.5,
        )
    }
    
    fn resize_diagonal_ne_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <line x1="8" y1="24" x2="24" y2="8" stroke="black" stroke-width="2"/>
                <path d="M 24 8 L 20 8 L 24 12 Z" fill="black"/>
                <path d="M 8 24 L 12 24 L 8 20 Z" fill="black"/>
            </svg>"#.to_string(),
            0.5,
            0.5,
        )
    }
    
    fn resize_diagonal_nw_svg() -> VectorCursor {
        VectorCursor::new(
            r#"<svg width="32" height="32" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">
                <line x1="8" y1="8" x2="24" y2="24" stroke="black" stroke-width="2"/>
                <path d="M 8 8 L 12 8 L 8 12 Z" fill="black"/>
                <path d="M 24 24 L 20 24 L 24 20 Z" fill="black"/>
            </svg>"#.to_string(),
            0.5,
            0.5,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_load_default_set() {
        let set = VectorCursorSet::load_default().unwrap();
        assert!(set.has(CursorShape::Arrow));
        assert!(set.has(CursorShape::Pointer));
        assert!(set.has(CursorShape::Text));
    }
    
    #[test]
    fn test_hotspot_calculation() {
        let cursor = VectorCursor::new("".to_string(), 0.5, 0.25);
        let (x, y) = cursor.hotspot_pixels(32);
        assert_eq!(x, 16);
        assert_eq!(y, 8);
    }
}
