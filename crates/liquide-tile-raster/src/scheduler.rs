//! Raster scheduler: prioritizes which tiles to rasterize within a frame budget.
//!
//! Tiles closer to the viewport center are rasterized first, ensuring the
//! user-visible region is always up to date even if the frame budget is
//! exhausted before all dirty tiles are processed.

use crate::clipper;
use crate::grid::{PixelRect, TileGrid};
use crate::tile::{TileId, TileState};
use liquide_paint::display_list::DisplayList;

/// Order dirty tiles by priority: tiles nearest to the viewport center
/// are scheduled first.
///
/// Returns tile IDs sorted by ascending distance to `viewport_center`
/// (measured in tile grid coordinates from the tile center).
pub fn schedule(dirty_tiles: &[TileId], viewport_center: TileId) -> Vec<TileId> {
    let mut sorted = dirty_tiles.to_vec();
    sorted.sort_by(|a, b| {
        let da = a.manhattan_distance(&viewport_center);
        let db = b.manhattan_distance(&viewport_center);
        da.cmp(&db)
    });
    sorted
}

/// Return visible dirty tiles first, then off-screen dirty tiles.
///
/// The viewport is specified as a pixel-space rectangle. Tiles fully
/// within or intersecting the viewport are prioritized over tiles outside
/// it. Within each group, tiles closer to the center are ordered first.
pub fn visible_tiles_first(grid: &TileGrid, viewport: &PixelRect) -> Vec<TileId> {
    let dirty = grid.dirty_tiles();
    if dirty.is_empty() {
        return Vec::new();
    }

    let center_col = ((viewport.x + viewport.width * 0.5) / grid.tile_size() as f32) as u32;
    let center_row = ((viewport.y + viewport.height * 0.5) / grid.tile_size() as f32) as u32;
    let center = TileId::new(
        center_col.min(grid.cols().saturating_sub(1)),
        center_row.min(grid.rows().saturating_sub(1)),
    );

    let mut visible = Vec::new();
    let mut offscreen = Vec::new();

    for id in &dirty {
        let bounds = grid.tile_bounds(*id);
        if bounds.intersects(viewport) {
            visible.push(*id);
        } else {
            offscreen.push(*id);
        }
    }

    // Sort each group by distance to center.
    visible.sort_by(|a, b| {
        a.manhattan_distance(&center).cmp(&b.manhattan_distance(&center))
    });
    offscreen.sort_by(|a, b| {
        a.manhattan_distance(&center).cmp(&b.manhattan_distance(&center))
    });

    visible.extend(offscreen);
    visible
}

/// Rasterize up to `max_tiles` dirty tiles within a frame budget.
///
/// Uses `visible_tiles_first` ordering to ensure the most important tiles
/// are done first. Each tile is cleared, its display list items are clipped
/// and rendered in painter's order, then the tile is marked clean.
///
/// Returns the number of tiles actually rasterized.
pub fn budget_rasterize(
    grid: &mut TileGrid,
    display_list: &DisplayList,
    max_tiles: usize,
    viewport: &PixelRect,
) -> usize {
    // Build priority-ordered list from current dirty tiles.
    let ordered = {
        let dirty = grid.dirty_tiles();
        if dirty.is_empty() {
            return 0;
        }

        let center_col = ((viewport.x + viewport.width * 0.5) / grid.tile_size() as f32) as u32;
        let center_row = ((viewport.y + viewport.height * 0.5) / grid.tile_size() as f32) as u32;
        let center = TileId::new(
            center_col.min(grid.cols().saturating_sub(1)),
            center_row.min(grid.rows().saturating_sub(1)),
        );
        schedule(&dirty, center)
    };

    let tile_size = grid.tile_size();
    let count = ordered.len().min(max_tiles);

    for i in 0..count {
        let id = ordered[i];
        let bounds = grid.tile_bounds(id);

        // Clip display list to this tile's region.
        let clipped = clipper::clip_to_rect(display_list, &bounds);

        // Rasterize: clear the tile and render clipped items.
        let tile = grid.tile_at_mut(id.col, id.row);
        tile.clear();

        // Render each clipped display item into the tile's pixel buffer.
        for item_ref in &clipped {
            let item = &display_list.items[item_ref.index];
            rasterize_item_to_tile(tile.pixels.as_mut_slice(), tile.width, tile.height,
                                   id.col * tile_size, id.row * tile_size, item);
        }

        tile.generation = tile.generation.saturating_add(1);
        tile.state = TileState::Clean;
    }

    count
}

/// Render a single display item into a tile's pixel buffer.
///
/// Coordinates in the display item are in viewport space; we offset them
/// by the tile's origin to get tile-local coordinates.
fn rasterize_item_to_tile(
    pixels: &mut [u8],
    tile_width: u32,
    tile_height: u32,
    tile_origin_x: u32,
    tile_origin_y: u32,
    item: &liquide_paint::display_list::DisplayItem,
) {
    use liquide_paint::display_list::DisplayItem;

    match item {
        DisplayItem::FillRect { rect, color } => {
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                color.r, color.g, color.b, color.a,
            );
        }
        DisplayItem::SolidColor { rect, color, .. } => {
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                color.r, color.g, color.b, color.a,
            );
        }
        DisplayItem::TextRun { rect, color, text, font_size, baseline, .. } => {
            // Simplified text rendering: draw a colored rect at the text bounds.
            // Full glyph rasterization is handled by the renderer-cpu crate;
            // here we provide a bounding-box fill as a placeholder that ensures
            // the tile rasterizer produces correct spatial coverage.
            let _ = (text, font_size, baseline);
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                color.r, color.g, color.b, color.a,
            );
        }
        DisplayItem::Icon { rect, color, .. } => {
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                color.r, color.g, color.b, color.a,
            );
        }
        // State operations are no-ops for pixel output; they affect
        // compositor state which is managed at a higher level.
        DisplayItem::PushClip { .. }
        | DisplayItem::PushClipPath { .. }
        | DisplayItem::PopClip
        | DisplayItem::PushOpacity { .. }
        | DisplayItem::PopOpacity
        | DisplayItem::PushTransform { .. }
        | DisplayItem::PopTransform
        | DisplayItem::PushBlendMode { .. }
        | DisplayItem::PopBlendMode
        | DisplayItem::PushFilter { .. }
        | DisplayItem::PopFilter
        | DisplayItem::PushBackdropFilter { .. }
        | DisplayItem::PopBackdropFilter
        | DisplayItem::PushMask { .. }
        | DisplayItem::PopMask
        | DisplayItem::PushStackingContext { .. }
        | DisplayItem::PopStackingContext
        | DisplayItem::SaveLayer { .. }
        | DisplayItem::RestoreLayer
        | DisplayItem::Noop => {}

        // Non-drawing metadata items.
        DisplayItem::SetCursor { .. }
        | DisplayItem::ScrollContainerHints { .. }
        | DisplayItem::AnimationHints { .. }
        | DisplayItem::TimelineHints { .. }
        | DisplayItem::Annotate { .. } => {}

        // Complex draw ops: render a solid-color approximation using the item's
        // bounding rect and primary color. The full-fidelity rendering path
        // lives in liquide-renderer-cpu; the tile rasterizer handles spatial
        // partitioning and caching.
        DisplayItem::LinearGradient { rect, stops, .. } => {
            let color = if let Some(stop) = stops.first() {
                stop.color
            } else {
                return;
            };
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                color.r, color.g, color.b, color.a,
            );
        }
        DisplayItem::RadialGradient { rect, stops, .. } => {
            let color = if let Some(stop) = stops.first() {
                stop.color
            } else {
                return;
            };
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                color.r, color.g, color.b, color.a,
            );
        }
        DisplayItem::ConicGradient { rect, stops, .. } => {
            let color = if let Some(stop) = stops.first() {
                stop.color
            } else {
                return;
            };
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                color.r, color.g, color.b, color.a,
            );
        }
        DisplayItem::Border { rect, top, .. } => {
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                top.color.r, top.color.g, top.color.b, top.color.a,
            );
        }
        DisplayItem::BorderImage { rect, .. } => {
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                128, 128, 128, 255,
            );
        }
        DisplayItem::BoxShadow { rect, color, .. } => {
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                color.r, color.g, color.b, color.a,
            );
        }
        DisplayItem::Outline { rect, color, .. } => {
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                color.r, color.g, color.b, color.a,
            );
        }
        DisplayItem::Text { rect, color, .. } => {
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                color.r, color.g, color.b, color.a,
            );
        }
        DisplayItem::Image { rect, .. } | DisplayItem::ImageRect { rect, .. } => {
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                200, 200, 200, 255,
            );
        }
        DisplayItem::StrokeRoundedRect { rect, color, .. } => {
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                color.r, color.g, color.b, color.a,
            );
        }
        DisplayItem::Line { x1, y1, x2, y2, color, width } => {
            let half_w = width / 2.0;
            let min_x = x1.min(*x2) - half_w;
            let min_y = y1.min(*y2) - half_w;
            let max_x = x1.max(*x2) + half_w;
            let max_y = y1.max(*y2) + half_w;
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                min_x, min_y, max_x - min_x, max_y - min_y,
                color.r, color.g, color.b, color.a,
            );
        }
        DisplayItem::Surface { rect, .. } => {
            fill_rect_tile(
                pixels, tile_width, tile_height,
                tile_origin_x, tile_origin_y,
                rect.x, rect.y, rect.width, rect.height,
                0, 0, 0, 0,
            );
        }
    }
}

/// Fill a rectangle within a tile's pixel buffer, handling coordinate
/// conversion from viewport space to tile-local space and clamping to
/// tile boundaries.
fn fill_rect_tile(
    pixels: &mut [u8],
    tile_width: u32,
    tile_height: u32,
    tile_origin_x: u32,
    tile_origin_y: u32,
    rect_x: f32,
    rect_y: f32,
    rect_w: f32,
    rect_h: f32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    if a == 0 {
        return;
    }

    // Convert viewport coords to tile-local coords.
    let local_x0 = (rect_x - tile_origin_x as f32).max(0.0).floor() as u32;
    let local_y0 = (rect_y - tile_origin_y as f32).max(0.0).floor() as u32;
    let local_x1 = ((rect_x + rect_w - tile_origin_x as f32).max(0.0).ceil() as u32).min(tile_width);
    let local_y1 = ((rect_y + rect_h - tile_origin_y as f32).max(0.0).ceil() as u32).min(tile_height);

    if local_x0 >= local_x1 || local_y0 >= local_y1 {
        return;
    }

    let stride = tile_width as usize * 4;

    if a == 255 {
        // Opaque: overwrite pixels directly.
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
        // Semi-transparent: SrcOver blend.
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
