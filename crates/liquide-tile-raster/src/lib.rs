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

pub mod cache;
pub mod clipper;
pub mod compositor;
pub mod damage;
pub mod geometry_adapter;
pub mod grid;
pub mod scheduler;
pub mod tile;

#[cfg(test)]
mod tests;

pub use cache::{CacheStats, TileCache};
pub use clipper::DisplayItemRef;
pub use damage::DamageTracker;
pub use grid::{PixelRect, TileGrid, TileStateCounts};
pub use tile::{DEFAULT_TILE_SIZE, Tile, TileId, TileState};

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

        let mut ctx = RasterContext::new(PixelRect::new(
            tile_origin_x as f32,
            tile_origin_y as f32,
            tile_width as f32,
            tile_height as f32,
        ));

        for item_ref in &clipped_items {
            let item = &display_list.items[item_ref.index];
            render_item_to_buffer(
                &mut pixels,
                tile_width,
                tile_height,
                tile_origin_x,
                tile_origin_y,
                item,
                &mut ctx,
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

            let mut ctx = RasterContext::new(PixelRect::new(
                origin_x as f32,
                origin_y as f32,
                tile.width as f32,
                tile.height as f32,
            ));

            for item_ref in &clipped {
                let item = &display_list.items[item_ref.index];
                render_item_to_buffer(
                    &mut tile.pixels,
                    tile.width,
                    tile.height,
                    origin_x,
                    origin_y,
                    item,
                    &mut ctx,
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

/// Visitor state maintained while walking a display list inside one tile.
///
/// The display list is a flat stream with `Push*`/`Pop*` state operations.
/// The rasterizer previously ignored these entirely, which meant clip,
/// opacity, transform, filter and blend-mode state was silently dropped.
/// [`RasterContext`] maintains the canonical set of stacks so the per-item
/// draw path can query the accumulated clip rect, accumulated opacity,
/// and current translation offset.
///
/// Non-affine transforms (rotation / skew / non-uniform scale) are not
/// applied at the pixel level by the tile rasterizer — they remain the
/// renderer's responsibility. When a non-translation transform is pushed,
/// the context flips `non_affine_pending` so the caller can bail to the
/// full renderer path. Rotated clips therefore over-damage by their
/// axis-aligned bounding box at this layer (see report §3.5 Low).
#[derive(Debug, Clone)]
pub struct RasterContext {
    /// Stack of accumulated clip rectangles in viewport pixel space.
    /// `clip_stack.last()` is the currently active clip (intersection
    /// of all ancestor clips and the tile bounds).
    pub clip_stack: Vec<PixelRect>,
    /// Stack of accumulated opacities. `opacity_stack.last()` is the
    /// current multiplicative opacity.
    pub opacity_stack: Vec<f32>,
    /// Stack of accumulated translation offsets (x, y). Non-translation
    /// transform pushes are tracked via [`Self::non_affine_pending`].
    pub translation_stack: Vec<(f32, f32)>,
    /// Stack of active blend modes (not executed at this layer, but
    /// preserved so an upper layer can route blends correctly).
    pub blend_mode_stack: Vec<u8>,
    /// True while a non-translation transform is pushed. When set the
    /// scheduler may choose to skip in-tile rasterization for affected
    /// items rather than produce wrong output.
    pub non_affine_pending: bool,
}

impl RasterContext {
    /// Create a new context seeded with the tile bounds as the outermost
    /// clip and identity opacity / translation.
    #[must_use]
    pub fn new(tile_bounds: PixelRect) -> Self {
        Self {
            clip_stack: vec![tile_bounds],
            opacity_stack: vec![1.0],
            translation_stack: vec![(0.0, 0.0)],
            blend_mode_stack: Vec::new(),
            non_affine_pending: false,
        }
    }

    /// Currently active clip (always non-empty because the tile bounds
    /// are pushed at construction).
    #[inline]
    pub fn active_clip(&self) -> PixelRect {
        *self
            .clip_stack
            .last()
            .unwrap_or(&PixelRect::new(0.0, 0.0, 0.0, 0.0))
    }

    /// Currently active opacity in `0..=1`.
    #[inline]
    pub fn active_opacity(&self) -> f32 {
        *self.opacity_stack.last().unwrap_or(&1.0)
    }

    /// Currently active translation offset.
    #[inline]
    pub fn active_translation(&self) -> (f32, f32) {
        *self.translation_stack.last().unwrap_or(&(0.0, 0.0))
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
    ctx: &mut RasterContext,
) {
    use liquide_paint::display_list::DisplayItem;

    // Handle state operations first — these don't emit pixels, they
    // maintain the visitor stacks.
    match item {
        DisplayItem::PushClip { rect, .. } => {
            let top = ctx.active_clip();
            let c = PixelRect::new(rect.x, rect.y, rect.width, rect.height);
            let clipped = top
                .intersection(&c)
                .unwrap_or(PixelRect::new(0.0, 0.0, 0.0, 0.0));
            ctx.clip_stack.push(clipped);
            return;
        }
        DisplayItem::PushClipPath { .. } => {
            // clip-path is not pixel-evaluated here; push the current clip
            // unchanged so pops remain balanced.
            let top = ctx.active_clip();
            ctx.clip_stack.push(top);
            return;
        }
        DisplayItem::PopClip => {
            if ctx.clip_stack.len() > 1 {
                ctx.clip_stack.pop();
            }
            return;
        }
        DisplayItem::PushOpacity { opacity, .. } => {
            let new_op = (ctx.active_opacity() * opacity).clamp(0.0, 1.0);
            ctx.opacity_stack.push(new_op);
            return;
        }
        DisplayItem::PopOpacity => {
            if ctx.opacity_stack.len() > 1 {
                ctx.opacity_stack.pop();
            }
            return;
        }
        DisplayItem::PushTransform {
            transform: matrix, ..
        } => {
            // Only honour pure translation at the tile rasterizer;
            // flag non-affine otherwise.
            let (a, b, c, d, tx, ty) =
                (matrix.a, matrix.b, matrix.c, matrix.d, matrix.tx, matrix.ty);
            let is_translation = (a - 1.0).abs() < 1e-4
                && b.abs() < 1e-4
                && c.abs() < 1e-4
                && (d - 1.0).abs() < 1e-4;
            let (cx, cy) = ctx.active_translation();
            if is_translation {
                ctx.translation_stack.push((cx + tx, cy + ty));
            } else {
                ctx.translation_stack.push((cx, cy));
                ctx.non_affine_pending = true;
            }
            return;
        }
        DisplayItem::PopTransform => {
            if ctx.translation_stack.len() > 1 {
                ctx.translation_stack.pop();
            }
            // Recompute non_affine_pending from remaining stack depth:
            // simplest is to clear when stack returns to baseline.
            if ctx.translation_stack.len() == 1 {
                ctx.non_affine_pending = false;
            }
            return;
        }
        DisplayItem::PushBlendMode { mode } => {
            let raw = *mode as u8;
            ctx.blend_mode_stack.push(raw);
            return;
        }
        DisplayItem::PopBlendMode => {
            ctx.blend_mode_stack.pop();
            return;
        }
        DisplayItem::PushFilter { .. }
        | DisplayItem::PopFilter
        | DisplayItem::PushBackdropFilter { .. }
        | DisplayItem::PopBackdropFilter
        | DisplayItem::PushMask { .. }
        | DisplayItem::PopMask
        | DisplayItem::PushStackingContext { .. }
        | DisplayItem::PopStackingContext
        | DisplayItem::SaveLayer { .. }
        | DisplayItem::RestoreLayer
        | DisplayItem::Noop
        | DisplayItem::SetCursor { .. }
        | DisplayItem::ScrollContainerHints { .. }
        | DisplayItem::AnimationHints { .. }
        | DisplayItem::TimelineHints { .. }
        | DisplayItem::Annotate { .. } => return,
        _ => {}
    }

    // Extract rect and color for draw operations, skip state ops.
    let (rect_x, rect_y, rect_w, rect_h, r, g, b, a) = match item {
        DisplayItem::FillRect { rect, color } => (
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            color.r,
            color.g,
            color.b,
            color.a,
        ),
        DisplayItem::SolidColor { rect, color, .. } => (
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            color.r,
            color.g,
            color.b,
            color.a,
        ),
        DisplayItem::TextRun { rect, color, .. } | DisplayItem::Text { rect, color, .. } => (
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            color.r,
            color.g,
            color.b,
            color.a,
        ),
        DisplayItem::Icon { rect, color, .. } => (
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            color.r,
            color.g,
            color.b,
            color.a,
        ),
        DisplayItem::LinearGradient { rect, stops, .. }
        | DisplayItem::RadialGradient { rect, stops, .. }
        | DisplayItem::ConicGradient { rect, stops, .. } => {
            if let Some(stop) = stops.first() {
                (
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                    stop.color.r,
                    stop.color.g,
                    stop.color.b,
                    stop.color.a,
                )
            } else {
                return;
            }
        }
        DisplayItem::Border { rect, top, .. } => (
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            top.color.r,
            top.color.g,
            top.color.b,
            top.color.a,
        ),
        DisplayItem::BoxShadow { rect, color, .. } => (
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            color.r,
            color.g,
            color.b,
            color.a,
        ),
        DisplayItem::Outline { rect, color, .. } => (
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            color.r,
            color.g,
            color.b,
            color.a,
        ),
        DisplayItem::StrokeRoundedRect { rect, color, .. } => (
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            color.r,
            color.g,
            color.b,
            color.a,
        ),
        DisplayItem::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            width,
        } => {
            let half_w = width / 2.0;
            let min_x = x1.min(*x2) - half_w;
            let min_y = y1.min(*y2) - half_w;
            let max_x = x1.max(*x2) + half_w;
            let max_y = y1.max(*y2) + half_w;
            (
                min_x,
                min_y,
                max_x - min_x,
                max_y - min_y,
                color.r,
                color.g,
                color.b,
                color.a,
            )
        }
        DisplayItem::Image { rect, .. } | DisplayItem::ImageRect { rect, .. } => {
            (rect.x, rect.y, rect.width, rect.height, 200, 200, 200, 255)
        }
        DisplayItem::BorderImage { rect, .. } => {
            (rect.x, rect.y, rect.width, rect.height, 128, 128, 128, 255)
        }
        DisplayItem::Surface { rect, .. } => (rect.x, rect.y, rect.width, rect.height, 0, 0, 0, 0),
        // State ops and metadata items produce no pixels.
        _ => return,
    };

    if a == 0 {
        return;
    }

    // Apply accumulated translation from PushTransform stack.
    let (tx_off, ty_off) = ctx.active_translation();
    let rect_x = rect_x + tx_off;
    let rect_y = rect_y + ty_off;

    // Apply accumulated opacity from PushOpacity stack.
    let op = ctx.active_opacity();
    let a = ((a as f32) * op).round().clamp(0.0, 255.0) as u8;
    if a == 0 {
        return;
    }

    // Intersect the drawing rectangle with the active clip.
    let draw_rect = PixelRect::new(rect_x, rect_y, rect_w, rect_h);
    let clipped = match draw_rect.intersection(&ctx.active_clip()) {
        Some(r) => r,
        None => return,
    };
    let rect_x = clipped.x;
    let rect_y = clipped.y;
    let rect_w = clipped.width;
    let rect_h = clipped.height;

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
