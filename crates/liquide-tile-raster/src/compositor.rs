//! Tile compositor: blits rasterized tiles into the final framebuffer.

use crate::grid::{PixelRect, TileGrid};
use crate::tile::TileId;

/// Composite all tiles from the grid into a destination framebuffer.
///
/// The framebuffer is a flat RGBA (or BGRA) byte array with the given stride
/// (bytes per row). Only tiles that were recently rasterized (generation >
/// `min_generation`) are blitted, skipping clean tiles that haven't changed.
/// Pass `min_generation = 0` to force-blit all tiles.
pub fn composite(grid: &TileGrid, framebuffer: &mut [u8], stride: u32, min_generation: u64) {
    let tile_size = grid.tile_size();
    let bpp = 4usize;
    let fb_stride = stride as usize;

    for tile in grid.iter() {
        // Skip tiles that haven't been updated since min_generation.
        if tile.generation < min_generation {
            continue;
        }

        let dst_x = tile.id.col * tile_size;
        let dst_y = tile.id.row * tile_size;
        let tw = tile.width as usize;
        let th = tile.height as usize;
        let row_bytes = tw * bpp;
        let tile_stride = tw * bpp;

        for row in 0..th {
            let src_off = row * tile_stride;
            let dst_off = (dst_y as usize + row) * fb_stride + dst_x as usize * bpp;

            if src_off + row_bytes > tile.pixels.len() {
                break;
            }
            if dst_off + row_bytes > framebuffer.len() {
                break;
            }

            framebuffer[dst_off..dst_off + row_bytes]
                .copy_from_slice(&tile.pixels[src_off..src_off + row_bytes]);
        }
    }
}

/// Composite only tiles that intersect the given region.
///
/// More efficient than full composite when only a small part of the screen
/// changed. Finds all tiles touching the region and blits them.
pub fn composite_region(
    grid: &TileGrid,
    framebuffer: &mut [u8],
    stride: u32,
    dirty_region: &PixelRect,
) {
    let tile_ids = grid.tiles_for_rect(dirty_region);
    let tile_size = grid.tile_size();
    let bpp = 4usize;
    let fb_stride = stride as usize;

    for id in tile_ids {
        let tile = grid.tile_at(id.col, id.row);
        let dst_x = id.col * tile_size;
        let dst_y = id.row * tile_size;
        let tw = tile.width as usize;
        let th = tile.height as usize;
        let row_bytes = tw * bpp;
        let tile_stride = tw * bpp;

        for row in 0..th {
            let src_off = row * tile_stride;
            let dst_off = (dst_y as usize + row) * fb_stride + dst_x as usize * bpp;

            if src_off + row_bytes > tile.pixels.len() {
                break;
            }
            if dst_off + row_bytes > framebuffer.len() {
                break;
            }

            framebuffer[dst_off..dst_off + row_bytes]
                .copy_from_slice(&tile.pixels[src_off..src_off + row_bytes]);
        }
    }
}

/// Composite a single tile into the framebuffer.
pub fn composite_tile(grid: &TileGrid, tile_id: TileId, framebuffer: &mut [u8], stride: u32) {
    let tile = grid.tile_at(tile_id.col, tile_id.row);
    let tile_size = grid.tile_size();
    let bpp = 4usize;
    let fb_stride = stride as usize;
    let dst_x = tile_id.col * tile_size;
    let dst_y = tile_id.row * tile_size;
    let tw = tile.width as usize;
    let th = tile.height as usize;
    let row_bytes = tw * bpp;
    let tile_stride = tw * bpp;

    for row in 0..th {
        let src_off = row * tile_stride;
        let dst_off = (dst_y as usize + row) * fb_stride + dst_x as usize * bpp;

        if src_off + row_bytes > tile.pixels.len() {
            break;
        }
        if dst_off + row_bytes > framebuffer.len() {
            break;
        }

        framebuffer[dst_off..dst_off + row_bytes]
            .copy_from_slice(&tile.pixels[src_off..src_off + row_bytes]);
    }
}

/// Calculate the total bytes needed for a framebuffer of the given dimensions.
#[inline]
pub fn framebuffer_size(width: u32, height: u32) -> usize {
    width as usize * height as usize * 4
}

/// Calculate the stride (bytes per row) for a given width.
#[inline]
pub fn framebuffer_stride(width: u32) -> u32 {
    width * 4
}
