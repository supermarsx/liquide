//! The map viewport: what part of the world is shown, at what zoom, in a
//! given on-screen box — plus pan (drag) and zoom (wheel/buttons) state changes
//! and the computation of the set of tiles visible in the box.
//!
//! All geometry is in **world pixels** (tile coordinate × [`TILE_SIZE`]) at the
//! viewport's integer zoom. The viewport's centre lat/lon maps to a world-pixel
//! point; the on-screen box is centred on that point; each visible tile's screen
//! rect is `tile_world_origin − viewport_world_origin` (so panning the world
//! origin slides every tile).

use crate::slippy::{
    LatLon, MAX_ZOOM, TILE_SIZE, TileCoord, TileId, clamp_latitude, lat_lon_to_world_px,
    tile_to_lat_lon, wrap_longitude,
};

/// A tile placed at a screen position: its (possibly unwrapped) id, the
/// canonical (wrapped) id used to fetch/cache it, and its screen rect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedTile {
    /// The on-screen id (x may be outside `[0,2^z)` if the world wrapped).
    pub id: TileId,
    /// The canonical wrapped id used as the fetch / cache key.
    pub key: TileId,
    /// Screen-space rect (pixels, relative to the viewport box top-left).
    pub x: f64,
    pub y: f64,
    pub size: f64,
}

/// A pannable / zoomable slippy-map viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Geographic centre of the on-screen box.
    pub center: LatLon,
    /// Integer zoom level (`0..=MAX_ZOOM`).
    pub zoom: u32,
    /// On-screen box size in device pixels.
    pub width: f64,
    pub height: f64,
}

impl Viewport {
    /// A viewport centred on `center` at `zoom`, sized `width`×`height` px.
    #[must_use]
    pub fn new(center: LatLon, zoom: u32, width: f64, height: f64) -> Self {
        Self {
            center: LatLon::new(clamp_latitude(center.lat), wrap_longitude(center.lon)),
            zoom: zoom.min(MAX_ZOOM),
            width: width.max(0.0),
            height: height.max(0.0),
        }
    }

    /// Resize the on-screen box (e.g. the map element was laid out at a new size).
    pub fn set_size(&mut self, width: f64, height: f64) {
        self.width = width.max(0.0);
        self.height = height.max(0.0);
    }

    /// World-pixel coordinate of the viewport centre at the current zoom.
    #[must_use]
    pub fn center_world_px(&self) -> (f64, f64) {
        lat_lon_to_world_px(self.center, self.zoom)
    }

    /// World-pixel coordinate of the viewport box's top-left corner.
    #[must_use]
    pub fn origin_world_px(&self) -> (f64, f64) {
        let (cx, cy) = self.center_world_px();
        (cx - self.width / 2.0, cy - self.height / 2.0)
    }

    /// Pan the viewport by a screen-pixel delta (a drag): dragging the map
    /// content right (`dx > 0`) moves the world the same way, so the centre
    /// shifts LEFT in world space. Latitude is re-clamped so a drag can't push
    /// the centre past the poles.
    pub fn pan_pixels(&mut self, dx: f64, dy: f64) {
        let (cx, cy) = self.center_world_px();
        // Dragging content by +dx reveals content to its left → centre world x
        // decreases by dx.
        let new_cx = cx - dx;
        let new_cy = cy - dy;
        let world_px = f64::from(TILE_SIZE) * crate::slippy::tiles_per_axis(self.zoom);
        // Wrap x around the world; clamp y to the valid vertical span.
        let wx = new_cx.rem_euclid(world_px);
        let wy = new_cy.clamp(0.0, world_px);
        let new_center = tile_to_lat_lon(TileCoord {
            x: wx / f64::from(TILE_SIZE),
            y: wy / f64::from(TILE_SIZE),
            z: self.zoom,
        });
        self.center = LatLon::new(
            clamp_latitude(new_center.lat),
            wrap_longitude(new_center.lon),
        );
    }

    /// Zoom in/out by `delta` integer levels, keeping the viewport CENTRE fixed.
    /// Clamped to `0..=MAX_ZOOM`. Returns `true` if the zoom actually changed.
    pub fn zoom_by(&mut self, delta: i32) -> bool {
        let new_zoom = (self.zoom as i32 + delta).clamp(0, MAX_ZOOM as i32) as u32;
        if new_zoom == self.zoom {
            return false;
        }
        self.zoom = new_zoom;
        true
    }

    /// Zoom in/out by `delta` levels while keeping the geographic point under the
    /// given SCREEN pixel anchor fixed (wheel-zoom toward the cursor). Returns
    /// `true` if the zoom changed.
    pub fn zoom_at(&mut self, delta: i32, anchor_x: f64, anchor_y: f64) -> bool {
        let anchor_geo = self.screen_to_lat_lon(anchor_x, anchor_y);
        if !self.zoom_by(delta) {
            return false;
        }
        // Re-centre so `anchor_geo` lands back under the same screen pixel.
        let (ax, ay) = lat_lon_to_world_px(anchor_geo, self.zoom);
        // We want: anchor world px - origin world px == (anchor_x, anchor_y).
        // origin = anchor_world - (anchor_x, anchor_y); centre = origin + size/2.
        let new_cx = ax - anchor_x + self.width / 2.0;
        let new_cy = ay - anchor_y + self.height / 2.0;
        let new_center = tile_to_lat_lon(TileCoord {
            x: new_cx / f64::from(TILE_SIZE),
            y: new_cy / f64::from(TILE_SIZE),
            z: self.zoom,
        });
        self.center = LatLon::new(
            clamp_latitude(new_center.lat),
            wrap_longitude(new_center.lon),
        );
        true
    }

    /// Map a screen pixel (relative to the box top-left) to a lat/lon.
    #[must_use]
    pub fn screen_to_lat_lon(&self, sx: f64, sy: f64) -> LatLon {
        let (ox, oy) = self.origin_world_px();
        let wx = ox + sx;
        let wy = oy + sy;
        tile_to_lat_lon(TileCoord {
            x: wx / f64::from(TILE_SIZE),
            y: wy / f64::from(TILE_SIZE),
            z: self.zoom,
        })
    }

    /// The set of tiles needed to cover the viewport box, each with its screen
    /// rect. Tiles whose `y` is out of range (north/south of the world) are
    /// skipped (there is nothing to draw there); `x` is allowed to be out of
    /// range and is wrapped for the fetch key while keeping its on-screen slot,
    /// so a viewport straddling the date line tiles seamlessly.
    #[must_use]
    pub fn visible_tiles(&self) -> Vec<PlacedTile> {
        let size = f64::from(TILE_SIZE);
        let (ox, oy) = self.origin_world_px();

        // First/last tile indices covering the box (inclusive).
        let tx0 = (ox / size).floor() as i64;
        let ty0 = (oy / size).floor() as i64;
        let tx1 = ((ox + self.width) / size).floor() as i64;
        let ty1 = ((oy + self.height) / size).floor() as i64;

        let mut tiles = Vec::new();
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let id = TileId::new(self.zoom, tx, ty);
                if !id.in_y_range() {
                    continue; // no tiles above the north / below the south edge
                }
                // Screen position = tile world origin − viewport world origin.
                let sx = (tx as f64) * size - ox;
                let sy = (ty as f64) * size - oy;
                tiles.push(PlacedTile {
                    id,
                    key: id.canonical(),
                    x: sx,
                    y: sy,
                    size,
                });
            }
        }
        tiles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn visible_set_covers_the_box_and_is_centred() {
        // A 512×512 box at z=2 (4 tiles/axis, world = 1024px) centred at (0,0).
        let vp = Viewport::new(LatLon::new(0.0, 0.0), 2, 512.0, 512.0);
        let tiles = vp.visible_tiles();
        // 512/256 = 2 full tiles per axis (the box edge lands exactly on the
        // tile-3 boundary, so the inclusive floor adds one boundary column/row →
        // a 3×3 grid that fully COVERS the box, never under-covers it).
        assert_eq!(tiles.len(), 9, "tiles: {tiles:?}");
        // Coverage: every screen pixel in the box falls inside some tile's rect.
        for &(sx, sy) in &[(0.0_f64, 0.0_f64), (511.0, 511.0), (256.0, 256.0)] {
            assert!(
                tiles.iter().any(|t| sx >= t.x
                    && sx < t.x + t.size
                    && sy >= t.y
                    && sy < t.y + t.size),
                "pixel ({sx},{sy}) must be covered by a tile"
            );
        }
        // The top-left visible tile is (1,1) at screen (0,0).
        let tl = tiles
            .iter()
            .find(|t| t.id == TileId::new(2, 1, 1))
            .expect("tile (1,1)");
        assert!(approx(tl.x, 0.0, 1e-6) && approx(tl.y, 0.0, 1e-6), "tl={tl:?}");
        // Tile (2,1) sits one tile to the right.
        let tr = tiles
            .iter()
            .find(|t| t.id == TileId::new(2, 2, 1))
            .expect("tile (2,1)");
        assert!(approx(tr.x, 256.0, 1e-6), "tr.x={}", tr.x);
    }

    #[test]
    fn pan_shifts_the_visible_set() {
        let mut vp = Viewport::new(LatLon::new(0.0, 0.0), 3, 256.0, 256.0);
        let before: Vec<TileId> = vp.visible_tiles().iter().map(|t| t.id).collect();
        // Pan content LEFT by a whole tile (drag the map left = dx negative):
        // the centre moves east → the visible tile column advances.
        vp.pan_pixels(-256.0, 0.0);
        let after: Vec<TileId> = vp.visible_tiles().iter().map(|t| t.id).collect();
        assert_ne!(before, after, "panning a full tile must shift the set");
        // Every x id advanced by exactly 1 (we panned exactly one tile east).
        let bx: Vec<i64> = before.iter().map(|t| t.x).collect();
        let ax: Vec<i64> = after.iter().map(|t| t.x).collect();
        assert!(
            ax.iter().all(|x| bx.contains(&(x - 1))),
            "after x ids must each be +1 of a before id: before={bx:?} after={ax:?}"
        );
    }

    #[test]
    fn zoom_changes_the_tile_level() {
        let mut vp = Viewport::new(LatLon::new(0.0, 0.0), 2, 256.0, 256.0);
        assert!(vp.visible_tiles().iter().all(|t| t.id.z == 2));
        assert!(vp.zoom_by(1));
        assert_eq!(vp.zoom, 3);
        assert!(
            vp.visible_tiles().iter().all(|t| t.id.z == 3),
            "after zoom-in every visible tile must be at z=3"
        );
        // Zoom is clamped at both ends.
        let mut lo = Viewport::new(LatLon::new(0.0, 0.0), 0, 256.0, 256.0);
        assert!(!lo.zoom_by(-1), "cannot zoom below 0");
        assert_eq!(lo.zoom, 0);
        let mut hi = Viewport::new(LatLon::new(0.0, 0.0), MAX_ZOOM, 256.0, 256.0);
        assert!(!hi.zoom_by(1), "cannot zoom above MAX_ZOOM");
    }

    #[test]
    fn zoom_at_keeps_the_anchor_point_fixed() {
        let mut vp = Viewport::new(LatLon::new(40.0, -74.0), 5, 400.0, 300.0);
        let anchor = (320.0, 80.0);
        let geo_before = vp.screen_to_lat_lon(anchor.0, anchor.1);
        assert!(vp.zoom_at(1, anchor.0, anchor.1));
        let geo_after = vp.screen_to_lat_lon(anchor.0, anchor.1);
        // The geographic point under the cursor stays put across the zoom.
        assert!(
            approx(geo_before.lat, geo_after.lat, 1e-6)
                && approx(geo_before.lon, geo_after.lon, 1e-6),
            "anchor moved: {geo_before:?} -> {geo_after:?}"
        );
    }

    #[test]
    fn screen_centre_maps_back_to_the_viewport_centre() {
        let vp = Viewport::new(LatLon::new(48.8566, 2.3522), 9, 600.0, 400.0);
        let c = vp.screen_to_lat_lon(300.0, 200.0);
        assert!(approx(c.lat, vp.center.lat, 1e-6), "lat {} vs {}", c.lat, vp.center.lat);
        assert!(approx(c.lon, vp.center.lon, 1e-6), "lon {} vs {}", c.lon, vp.center.lon);
    }

    #[test]
    fn x_wraps_at_the_date_line_keeping_screen_slots() {
        // Centre at the far east edge so the box straddles +180°.
        let vp = Viewport::new(LatLon::new(0.0, 179.9), 2, 512.0, 256.0);
        let tiles = vp.visible_tiles();
        // Some on-screen tile id x is >= 2^z (it wrapped), but its KEY is in range.
        let n = 1i64 << 2;
        assert!(
            tiles.iter().any(|t| t.id.x >= n || t.id.x < 0),
            "a date-line-straddling viewport should produce an out-of-range on-screen x"
        );
        for t in &tiles {
            assert!(
                t.key.x >= 0 && t.key.x < n,
                "every fetch key x must be wrapped into range: {t:?}"
            );
        }
    }
}
