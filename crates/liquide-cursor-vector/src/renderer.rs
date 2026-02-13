//! Vector cursor renderer using resvg

use crate::cursor_set::VectorCursor;
use crate::error::{Result, VectorCursorError};
use tiny_skia::{Pixmap, Transform};
use usvg::{Options, Tree};

/// High-definition vector cursor renderer
pub struct VectorCursorRenderer {
    options: Options,
}

impl Default for VectorCursorRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorCursorRenderer {
    /// Create a new vector cursor renderer
    pub fn new() -> Self {
        let mut options = Options::default();
        options.fontdb_mut().load_system_fonts();
        
        Self { options }
    }
    
    /// Render a vector cursor to RGBA8 pixels
    ///
    /// # Arguments
    ///
    /// * `cursor` - The vector cursor to render
    /// * `size` - Output size in pixels
    /// * `scale` - Additional scale factor (e.g., 2.0 for 2x displays)
    ///
    /// # Returns
    ///
    /// RGBA8 pixel data with dimensions `(size * scale, size * scale)`
    pub fn render(&self, cursor: &VectorCursor, size: u32, scale: f32) -> Result<Vec<u8>> {
        let physical_size = (size as f32 * scale) as u32;
        
        // Parse SVG
        let tree = Tree::from_str(&cursor.svg_data, &self.options)
            .map_err(|e| VectorCursorError::SvgParse(e.to_string()))?;
        
        // Create pixmap
        let mut pixmap = Pixmap::new(physical_size, physical_size)
            .ok_or_else(|| VectorCursorError::RenderFailed("Failed to create pixmap".to_string()))?;
        
        // Calculate scale transform
        let svg_size = tree.size();
        let scale_x = physical_size as f32 / svg_size.width();
        let scale_y = physical_size as f32 / svg_size.height();
        let scale_factor = scale_x.min(scale_y);
        
        let transform = Transform::from_scale(scale_factor, scale_factor);
        
        // Render
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        
        Ok(pixmap.data().to_vec())
    }
    
    /// Render a cursor and return as image::RgbaImage
    pub fn render_to_image(
        &self,
        cursor: &VectorCursor,
        size: u32,
        scale: f32,
    ) -> Result<image::RgbaImage> {
        let pixels = self.render(cursor, size, scale)?;
        let physical_size = (size as f32 * scale) as u32;
        
        image::RgbaImage::from_raw(physical_size, physical_size, pixels)
            .ok_or_else(|| VectorCursorError::RenderFailed("Failed to create image".to_string()))
    }
    
    /// Render multiple sizes at once (for caching)
    pub fn render_multi_size(
        &self,
        cursor: &VectorCursor,
        sizes: &[u32],
        scale: f32,
    ) -> Result<Vec<(u32, Vec<u8>)>> {
        sizes
            .iter()
            .map(|&size| {
                let pixels = self.render(cursor, size, scale)?;
                Ok((size, pixels))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor_set::VectorCursorSet;
    use liquide_cursor::CursorShape;
    
    #[test]
    fn test_render_arrow() {
        let set = VectorCursorSet::load_default().unwrap();
        let cursor = set.get(CursorShape::Arrow).unwrap();
        
        let renderer = VectorCursorRenderer::new();
        let pixels = renderer.render(cursor, 32, 1.0).unwrap();
        
        assert_eq!(pixels.len(), 32 * 32 * 4); // RGBA
    }
    
    #[test]
    fn test_render_scaled() {
        let set = VectorCursorSet::load_default().unwrap();
        let cursor = set.get(CursorShape::Pointer).unwrap();
        
        let renderer = VectorCursorRenderer::new();
        let pixels = renderer.render(cursor, 32, 2.0).unwrap();
        
        assert_eq!(pixels.len(), 64 * 64 * 4); // 2x scale
    }
    
    #[test]
    fn test_render_multi_size() {
        let set = VectorCursorSet::load_default().unwrap();
        let cursor = set.get(CursorShape::Text).unwrap();
        
        let renderer = VectorCursorRenderer::new();
        let sizes = vec![16, 24, 32, 48, 64];
        let results = renderer.render_multi_size(cursor, &sizes, 1.0).unwrap();
        
        assert_eq!(results.len(), 5);
        assert_eq!(results[0].1.len(), 16 * 16 * 4);
        assert_eq!(results[4].1.len(), 64 * 64 * 4);
    }
}
