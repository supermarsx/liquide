//! Raster scheduler: prioritizes which tiles to rasterize within a frame budget.
//!
//! Tiles closer to the viewport center are rasterized first, ensuring the
//! user-visible region is always up to date even if the frame budget is
//! exhausted before all dirty tiles are processed.
//!
//! The rasterization itself is delegated to the canonical
//! [`crate::TileRasterizer`] — this module owns only the priority /
//! budget policy, so the pixel path is not duplicated.

use crate::TileRasterizer;
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

    visible.sort_by(|a, b| {
        a.manhattan_distance(&center)
            .cmp(&b.manhattan_distance(&center))
    });
    offscreen.sort_by(|a, b| {
        a.manhattan_distance(&center)
            .cmp(&b.manhattan_distance(&center))
    });

    visible.extend(offscreen);
    visible
}

/// Rasterize up to `max_tiles` dirty tiles within a frame budget.
///
/// Uses [`visible_tiles_first`] ordering to ensure the most important
/// tiles are done first. Rasterization is delegated to
/// [`TileRasterizer`] (which maintains the full clip / opacity /
/// transform stack while walking display items).
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

    let count = ordered.len().min(max_tiles);
    let rasterizer = TileRasterizer::new(grid.tile_size());

    for &id in ordered.iter().take(count) {
        let tile_width;
        let tile_height;
        {
            let tile = grid.tile_at(id.col, id.row);
            tile_width = tile.width;
            tile_height = tile.height;
        }

        let pixels = rasterizer.rasterize_tile(id, display_list, tile_width, tile_height);

        let tile = grid.tile_at_mut(id.col, id.row);
        tile.pixels = pixels;
        tile.generation = tile.generation.saturating_add(1);
        tile.state = TileState::Clean;
    }

    count
}
