//! # liquide-tile-raster
//!
//! Tile-based incremental rasterization engine.
//!
//! Instead of re-rasterizing the entire framebuffer every frame, the screen
//! is divided into fixed-size tiles (128, 256, or 512 pixels). Only tiles
//! that intersect damaged regions are re-rasterized, and results are cached
//! in an LRU tile cache.
//!
//! ## Architecture
//!
//! ```text
//! DamageTracker ─── damage rects ──► TileGrid.invalidate_rect()
//!                                         │
//!                                    dirty tiles
//!                                         │
//! DisplayList ──► DisplayListClipper ──► RasterScheduler
//!                                         │
//!                                    priority-ordered tiles
//!                                         │
//!                                    TileRasterizer
//!                                         │
//!                                    rasterized pixels
//!                                         │
//!                               TileCompositor ──► framebuffer
//! ```

pub mod tile;
pub mod grid;
pub mod damage;
pub mod cache;
pub mod clipper;
pub mod compositor;
pub mod scheduler;

#[cfg(test)]
mod tests;

pub use tile::{Tile, TileId, TileState, DEFAULT_TILE_SIZE};
pub use grid::{PixelRect, TileGrid, TileStateCounts};
pub use damage::DamageTracker;
pub use cache::{CacheStats, TileCache};
pub use clipper::DisplayItemRef;

use liquide_paint::display_list::DisplayList;

/// Rasterizes individual tiles from a display list.
pub struct TileRasterizer {
    /// Tile size in pixels.
    tile_size: u32,
}

impl TileRasterizer {
    /// Create a new tile rasterizer with the given tile size.
    pub fn new(tile_size: u32) -> Self {
        Self { tile_size }
    }

    /// Rasterize a single tile, returning RGBA pixel data.
    ///
    /// The display list is clipped to the tile's bounds, and only items
    /// that intersect the tile are rendered. Items are rendered in painter's
    /// order (list order).
    pub fn rasterize_tile(
        &self,
        tile_id: TileId,
        display_list: &DisplayList,
        tile_width: u32,
        tile_height: u32,
    ) -> Vec<u8> {
        let clip_rect = PixelRect::new(
            (tile_id.col * self.tile_size) as f32,
            (tile_id.row * self.tile_size) as f32,
            tile_width as f32,
            tile_height as f32,
        );

        let clipped_items = clipper::clip_to_rect(display_list, &clip_rect);

        let pixel_count = (tile_width as usize) * (tile_height as usize) * 4;
        let mut pixels = vec![0u8; pixel_count];

        let tile_origin_x = tile_id.col * self.tile_size;
        let tile_origin_y = tile_id.row * self.tile_size;

        for item_ref in &clipped_items {
            let item = &display_list.items[item_ref.index];
            render_item_to_buffer(
                &mut pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y, item,
            );
        }

        pixels
    }

    /// Rasterize all dirty tiles in the grid.
    ///
    /// Each dirty tile is cleared, rendered from the display list, and
    /// marked as Clean. The tile's generation counter is incremented.
    pub fn rasterize_dirty(&self, grid: &mut TileGrid, display_list: &DisplayList) {
        let dirty_ids = grid.dirty_tiles();
        let tile_size = self.tile_size;

        for id in dirty_ids {
            let bounds = grid.tile_bounds(id);
            let clipped = clipper::clip_to_rect(display_list, &bounds);

            let tile = grid.tile_at_mut(id.col, id.row);
            tile.clear();

            let origin_x = id.col * tile_size;
            let origin_y = id.row * tile_size;

            for item_ref in &clipped {
                let item = &display_list.items[item_ref.index];
                render_item_to_buffer(
                    &mut tile.pixels, tile.width, tile.height,
                    origin_x, origin_y, item,
                );
            }

            tile.generation += 1;
            tile.state = TileState::Clean;
        }
    }

    /// Get the tile size.
    #[inline]
    pub fn tile_size(&self) -> u32 {
        self.tile_size
    }
}

/// Render a single display item into a pixel buffer (tile-local coordinates).
fn render_item_to_buffer(
    pixels: &mut [u8],
    tile_width: u32,
    tile_height: u32,
    tile_origin_x: u32,
    tile_origin_y: u32,
    item: &liquide_paint::display_list::DisplayItem,
) {
    use liquide_paint::display_list::DisplayItem;

    // Extract rect and color for draw operations, skip state ops.
    let (rect_x, rect_y, rect_w, rect_h, r, g, b, a) = match item {
        DisplayItem::FillRect { rect, color } => {
            (rect.x, rect.y, rect.width, rect.height, color.r, color.g, color.b, color.a)
        }
        DisplayItem::SolidColor { rect, color, .. } => {
            (rect.x, rect.y, rect.width, rect.height, color.r, color.g, color.b, color.a)
        }
        DisplayItem::TextRun { rect, color, .. }
        | DisplayItem::Text { rect, color, .. } => {
            (rect.x, rect.y, rect.width, rect.height, color.r, color.g, color.b, color.a)
        }
        DisplayItem::Icon { rect, color, .. } => {
            (rect.x, rect.y, rect.width, rect.height, color.r, color.g, color.b, color.a)
        }
        DisplayItem::LinearGradient { rect, stops, .. }
        | DisplayItem::RadialGradient { rect, stops, .. }
        | DisplayItem::ConicGradient { rect, stops, .. } => {
            if let Some(stop) = stops.first() {
                (rect.x, rect.y, rect.width, rect.height, stop.color.r, stop.color.g, stop.color.b, stop.color.a)
            } else {
                return;
            }
        }
        DisplayItem::Border { rect, top, .. } => {
            (rect.x, rect.y, rect.width, rect.height, top.color.r, top.color.g, top.color.b, top.color.a)
        }
        DisplayItem::BoxShadow { rect, color, .. } => {
            (rect.x, rect.y, rect.width, rect.height, color.r, color.g, color.b, color.a)
        }
        DisplayItem::Outline { rect, color, .. } => {
            (rect.x, rect.y, rect.width, rect.height, color.r, color.g, color.b, color.a)
        }
        DisplayItem::StrokeRoundedRect { rect, color, .. } => {
            (rect.x, rect.y, rect.width, rect.height, color.r, color.g, color.b, color.a)
        }
        DisplayItem::Line { x1, y1, x2, y2, color, width } => {
            let half_w = width / 2.0;
            let min_x = x1.min(*x2) - half_w;
            let min_y = y1.min(*y2) - half_w;
            let max_x = x1.max(*x2) + half_w;
            let max_y = y1.max(*y2) + half_w;
            (min_x, min_y, max_x - min_x, max_y - min_y, color.r, color.g, color.b, color.a)
        }
        DisplayItem::Image { rect, .. } | DisplayItem::ImageRect { rect, .. } => {
            (rect.x, rect.y, rect.width, rect.height, 200, 200, 200, 255)
        }
        DisplayItem::BorderImage { rect, .. } => {
            (rect.x, rect.y, rect.width, rect.height, 128, 128, 128, 255)
        }
        DisplayItem::Surface { rect, .. } => {
            (rect.x, rect.y, rect.width, rect.height, 0, 0, 0, 0)
        }
        // State ops and metadata items produce no pixels.
        _ => return,
    };

    if a == 0 {
        return;
    }

    // Convert viewport coords to tile-local coords.
    let local_x0 = (rect_x - tile_origin_x as f32).max(0.0) as u32;
    let local_y0 = (rect_y - tile_origin_y as f32).max(0.0) as u32;
    let local_x1 = ((rect_x + rect_w - tile_origin_x as f32).ceil() as u32).min(tile_width);
    let local_y1 = ((rect_y + rect_h - tile_origin_y as f32).ceil() as u32).min(tile_height);

    if local_x0 >= local_x1 || local_y0 >= local_y1 {
        return;
    }

    let stride = tile_width as usize * 4;

    if a == 255 {
        let pixel = [r, g, b, a];
        for y in local_y0..local_y1 {
            let row_start = y as usize * stride + local_x0 as usize * 4;
            for x in 0..(local_x1 - local_x0) as usize {
                let off = row_start + x * 4;
                if off + 3 < pixels.len() {
                    pixels[off..off + 4].copy_from_slice(&pixel);
                }
            }
        }
    } else {
        let sa = a as f32 / 255.0;
        let sr = r as f32 * sa;
        let sg = g as f32 * sa;
        let sb = b as f32 * sa;
        let inv_sa = 1.0 - sa;

        for y in local_y0..local_y1 {
            let row_start = y as usize * stride + local_x0 as usize * 4;
            for x in 0..(local_x1 - local_x0) as usize {
                let off = row_start + x * 4;
                if off + 3 < pixels.len() {
                    let dr = pixels[off] as f32;
                    let dg = pixels[off + 1] as f32;
                    let db = pixels[off + 2] as f32;
                    let da = pixels[off + 3] as f32 / 255.0;

                    pixels[off] = (sr + dr * inv_sa).min(255.0) as u8;
                    pixels[off + 1] = (sg + dg * inv_sa).min(255.0) as u8;
                    pixels[off + 2] = (sb + db * inv_sa).min(255.0) as u8;
                    pixels[off + 3] = ((sa + da * inv_sa) * 255.0).min(255.0) as u8;
                }
            }
        }
    }
}

impl std::fmt::Debug for TileRasterizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TileRasterizer")
            .field("tile_size", &self.tile_size)
            .finish()
    }
}
