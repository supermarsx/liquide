//! Geometry adapters between local tile-raster primitives
//! (`PixelRect`, `TileId`) and canonical `liquide_common::{FRect, IRect}`
//! from Phase 1.
//!
//! See t8 review §3.5 Medium / §5.1: damage-tile / `region::Rect` /
//! `TileId` had no conversion paths. These adapters let
//! damage/tile/layer code round-trip at the boundary without a bulk
//! type migration.

use crate::grid::PixelRect;
use crate::tile::TileId;
use liquide_common::geometry::{FRect, IRect};

impl From<PixelRect> for FRect {
    #[inline]
    fn from(r: PixelRect) -> Self {
        FRect::new(r.x, r.y, r.width, r.height)
    }
}

impl From<FRect> for PixelRect {
    #[inline]
    fn from(r: FRect) -> Self {
        PixelRect::new(r.x, r.y, r.width, r.height)
    }
}

impl From<PixelRect> for IRect {
    /// Snap a `PixelRect` to an enclosing integer rect using
    /// `floor(top-left) / ceil(bottom-right)` so the result fully
    /// contains the original footprint.
    fn from(r: PixelRect) -> Self {
        let x = r.x.floor() as i32;
        let y = r.y.floor() as i32;
        let right = (r.x + r.width).ceil() as i32;
        let bottom = (r.y + r.height).ceil() as i32;
        IRect::new(x, y, (right - x).max(0), (bottom - y).max(0))
    }
}

/// Convert a `TileId` + tile size into the pixel-space `FRect` the tile
/// occupies on screen.
#[must_use]
pub fn tile_id_to_frect(id: TileId, tile_size: u32) -> FRect {
    FRect::new(
        (id.col * tile_size) as f32,
        (id.row * tile_size) as f32,
        tile_size as f32,
        tile_size as f32,
    )
}

/// Convert a `TileId` + tile size into an integer `IRect`.
#[must_use]
pub fn tile_id_to_irect(id: TileId, tile_size: u32) -> IRect {
    IRect::new(
        (id.col * tile_size) as i32,
        (id.row * tile_size) as i32,
        tile_size as i32,
        tile_size as i32,
    )
}

/// Given a canonical `IRect` and a tile size, return the `TileId` of the
/// top-left tile it starts in (used when routing damage → tile grid).
#[must_use]
pub fn irect_top_left_tile(r: IRect, tile_size: u32) -> TileId {
    let ts = tile_size.max(1) as i32;
    let col = (r.x.max(0) / ts) as u32;
    let row = (r.y.max(0) / ts) as u32;
    TileId::new(col, row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_rect_frect_roundtrip() {
        let p = PixelRect::new(1.5, 2.25, 10.0, 20.0);
        let f: FRect = p.into();
        assert_eq!(f, FRect::new(1.5, 2.25, 10.0, 20.0));
        let back: PixelRect = f.into();
        assert_eq!(back, p);
    }

    #[test]
    fn pixel_rect_to_irect_encloses() {
        let p = PixelRect::new(0.4, 0.6, 10.2, 20.9);
        let i: IRect = p.into();
        assert_eq!(i, IRect::new(0, 0, 11, 22));
    }

    #[test]
    fn tile_id_to_frect_matches_grid_origin() {
        let id = TileId::new(3, 4);
        let f = tile_id_to_frect(id, 256);
        assert_eq!(f, FRect::new(768.0, 1024.0, 256.0, 256.0));
    }

    #[test]
    fn tile_id_to_irect_matches_grid_origin() {
        let id = TileId::new(2, 1);
        let i = tile_id_to_irect(id, 128);
        assert_eq!(i, IRect::new(256, 128, 128, 128));
    }

    #[test]
    fn irect_top_left_tile_division() {
        // A damage rect starting at (260, 130) with a 128 tile size
        // begins in tile (2, 1).
        let r = IRect::new(260, 130, 50, 50);
        let id = irect_top_left_tile(r, 128);
        assert_eq!(id, TileId::new(2, 1));
    }

    #[test]
    fn tile_id_roundtrip_via_frect_and_irect() {
        // tile(5, 7) at size 256 → frect → irect → tile origin matches.
        let id = TileId::new(5, 7);
        let f = tile_id_to_frect(id, 256);
        let p: PixelRect = f.into();
        let i: IRect = p.into();
        let back = irect_top_left_tile(i, 256);
        assert_eq!(back, id);
    }
}
