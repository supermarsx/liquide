//! Slippy-map (Web Mercator) coordinate math.
//!
//! The OpenStreetMap "slippy map" tiling scheme covers the world in a quadtree
//! of square 256×256-pixel tiles. At zoom level `z` the world is a
//! `2^z × 2^z` grid of tiles, addressed by integer `(x, y)` with `x` increasing
//! eastward (from −180° longitude) and `y` increasing **southward** (from the
//! top, ≈ +85.0511° latitude). The projection is the spherical Web Mercator
//! ("EPSG:3857") used by OSM, Google, Bing, etc.
//!
//! References (the canonical OSM wiki formulas, reproduced from first
//! principles — no copied code):
//!   * lon→x:  `x = (lon + 180) / 360 * n`
//!   * lat→y:  `y = (1 − ln(tan(lat) + sec(lat)) / π) / 2 * n`   where `n = 2^z`
//! and their inverses. `lat` is clamped to the Mercator-valid range so the poles
//! (where the projection diverges) never produce NaN/∞.
//!
//! This module deals in *fractional* tile coordinates (`f64`) so a viewport can
//! sit between tiles and pan smoothly; callers floor them to integer tile ids.

/// Pixel size of one OSM tile edge (the de-facto standard for the slippy scheme).
pub const TILE_SIZE: u32 = 256;

/// The latitude (degrees) at the top/bottom edge of the Web-Mercator square.
/// Beyond this the projection diverges, so latitude is clamped to ±this value.
/// `atan(sinh(π))` ≈ 85.05112877980659°.
pub const MAX_LATITUDE: f64 = 85.051_128_779_806_59;

/// The maximum zoom level OSM tiles are generally available at. The math itself
/// works for any non-negative zoom, but a viewport clamps to this for fetching.
pub const MAX_ZOOM: u32 = 19;

/// Clamp a latitude to the Web-Mercator-valid range `[-MAX_LATITUDE, MAX_LATITUDE]`.
#[must_use]
pub fn clamp_latitude(lat: f64) -> f64 {
    lat.clamp(-MAX_LATITUDE, MAX_LATITUDE)
}

/// Wrap a longitude into the canonical `[-180, 180)` range (the world is
/// cylindrical east-west, so longitudes wrap around).
#[must_use]
pub fn wrap_longitude(lon: f64) -> f64 {
    // Bring into [-180, 180): shift to [0, 360), modulo, shift back.
    let wrapped = (lon + 180.0).rem_euclid(360.0) - 180.0;
    // rem_euclid(360) yields [0,360); the subtraction maps it to [-180,180).
    wrapped
}

/// The number of tiles per axis at zoom `z` (`2^z`), as `f64`.
#[must_use]
pub fn tiles_per_axis(z: u32) -> f64 {
    f64::from(1u32 << z.min(30))
}

/// A *fractional* tile coordinate at a given zoom: the continuous position in
/// tile space, where the integer part is the tile id and the fraction is the
/// position WITHIN that tile (`0.0..1.0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileCoord {
    pub x: f64,
    pub y: f64,
    pub z: u32,
}

impl TileCoord {
    /// The integer tile id containing this fractional coordinate.
    #[must_use]
    pub fn floor(self) -> TileId {
        TileId {
            x: self.x.floor() as i64,
            y: self.y.floor() as i64,
            z: self.z,
        }
    }
}

/// An integer slippy-tile address `z/x/y`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileId {
    pub z: u32,
    pub x: i64,
    pub y: i64,
}

impl TileId {
    #[must_use]
    pub fn new(z: u32, x: i64, y: i64) -> Self {
        Self { z, x, y }
    }

    /// Whether this tile id is in range for its zoom (`0 <= x,y < 2^z`).
    /// Out-of-range ids occur when a viewport pans past the world edge; the
    /// north/south edges have no tiles, the east/west edges WRAP (see
    /// [`wrapped_x`](Self::wrapped_x)).
    #[must_use]
    pub fn in_y_range(self) -> bool {
        let n = 1i64 << self.z.min(30);
        self.y >= 0 && self.y < n
    }

    /// The x id wrapped into `[0, 2^z)` (the world repeats east-west). A viewport
    /// panned past +180° shows the same tiles again, so the fetch key uses the
    /// wrapped id while the on-screen position uses the unwrapped id.
    #[must_use]
    pub fn wrapped_x(self) -> i64 {
        let n = 1i64 << self.z.min(30);
        self.x.rem_euclid(n)
    }

    /// The canonical, wrapped tile id used as a fetch / cache key.
    #[must_use]
    pub fn canonical(self) -> TileId {
        TileId {
            z: self.z,
            x: self.wrapped_x(),
            y: self.y,
        }
    }
}

/// A geographic coordinate in WGS84 degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

impl LatLon {
    #[must_use]
    pub fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }
}

/// Convert a lat/lon (degrees) at zoom `z` to a fractional tile coordinate.
///
/// Latitude is clamped to the Mercator-valid range so the poles don't diverge;
/// longitude is wrapped into `[-180, 180)` first so a wrapped-around viewport
/// still maps into `[0, 2^z)`.
#[must_use]
pub fn lat_lon_to_tile(coord: LatLon, z: u32) -> TileCoord {
    let n = tiles_per_axis(z);
    let lon = wrap_longitude(coord.lon);
    let lat = clamp_latitude(coord.lat).to_radians();

    let x = (lon + 180.0) / 360.0 * n;
    // y = (1 - asinh(tan(lat)) / PI) / 2 * n ; asinh(tan(lat)) == ln(tan+sec).
    let y = (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) / 2.0 * n;

    TileCoord { x, y, z }
}

/// Convert a fractional tile coordinate back to a lat/lon (degrees).
///
/// The exact inverse of [`lat_lon_to_tile`] within Mercator's valid range.
#[must_use]
pub fn tile_to_lat_lon(tile: TileCoord) -> LatLon {
    let n = tiles_per_axis(tile.z);
    let lon = tile.x / n * 360.0 - 180.0;
    // lat = atan(sinh(PI * (1 - 2*y/n)))
    let lat_rad =
        (std::f64::consts::PI * (1.0 - 2.0 * tile.y / n)).sinh().atan();
    LatLon {
        lat: lat_rad.to_degrees(),
        lon,
    }
}

/// Convert a lat/lon to absolute WORLD-PIXEL coordinates at zoom `z` (the tile
/// coordinate times [`TILE_SIZE`]). World pixels are the natural space for
/// positioning a viewport's tiles on screen.
#[must_use]
pub fn lat_lon_to_world_px(coord: LatLon, z: u32) -> (f64, f64) {
    let t = lat_lon_to_tile(coord, z);
    (t.x * f64::from(TILE_SIZE), t.y * f64::from(TILE_SIZE))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn known_point_maps_to_known_tile() {
        // The canonical OSM-wiki worked example: Berlin-ish? Use the documented
        // reference: lat 0, lon 0 at z=1 is the exact centre of the world → the
        // boundary of the four tiles, fractional (1.0, 1.0).
        let t = lat_lon_to_tile(LatLon::new(0.0, 0.0), 1);
        assert!(approx(t.x, 1.0, 1e-9), "x={}", t.x);
        assert!(approx(t.y, 1.0, 1e-9), "y={}", t.y);

        // A well-known concrete tile: the OSM wiki example for
        // lat=52.5200, lon=13.4050 (Berlin) at z=12 is tile (2200, 1343).
        let berlin = lat_lon_to_tile(LatLon::new(52.5200, 13.4050), 12);
        assert_eq!(berlin.floor(), TileId::new(12, 2200, 1343));
    }

    #[test]
    fn top_left_corner_is_origin() {
        // (max-lat, -180) is the top-left of the world → tile (0,0).
        let t = lat_lon_to_tile(LatLon::new(MAX_LATITUDE, -180.0), 5);
        assert!(approx(t.x, 0.0, 1e-6), "x={}", t.x);
        assert!(approx(t.y, 0.0, 1e-6), "y={}", t.y);
    }

    #[test]
    fn round_trip_lat_lon_through_tile_space() {
        for &(lat, lon, z) in &[
            (0.0, 0.0, 0),
            (52.5200, 13.4050, 12),
            (-33.8688, 151.2093, 10), // Sydney
            (35.6895, 139.6917, 8),   // Tokyo
            (-90.0, 200.0, 6),        // out of range → clamped/wrapped
        ] {
            let z: u32 = z;
            let original = LatLon::new(lat, lon);
            let t = lat_lon_to_tile(original, z);
            let back = tile_to_lat_lon(t);
            // Compare against the CLAMPED/WRAPPED input (the projection is only
            // invertible inside its valid domain).
            let exp_lat = clamp_latitude(lat);
            let exp_lon = wrap_longitude(lon);
            assert!(
                approx(back.lat, exp_lat, 1e-6),
                "lat round-trip {lat} -> {} (exp {exp_lat})",
                back.lat
            );
            assert!(
                approx(back.lon, exp_lon, 1e-6),
                "lon round-trip {lon} -> {} (exp {exp_lon})",
                back.lon
            );
        }
    }

    #[test]
    fn world_pixels_scale_with_tile_size() {
        let (px, py) = lat_lon_to_world_px(LatLon::new(0.0, 0.0), 1);
        // z=1 → 2 tiles/axis, centre is tile (1,1) → pixel (256, 256).
        assert!(approx(px, 256.0, 1e-6), "px={px}");
        assert!(approx(py, 256.0, 1e-6), "py={py}");
    }

    #[test]
    fn longitude_wraps_around_the_world() {
        assert!(approx(wrap_longitude(190.0), -170.0, 1e-9));
        assert!(approx(wrap_longitude(-190.0), 170.0, 1e-9));
        assert!(approx(wrap_longitude(540.0), 180.0 - 360.0, 1e-9)); // 540 → 180 → -180
        assert!(approx(wrap_longitude(13.405), 13.405, 1e-9));
    }

    #[test]
    fn latitude_clamps_at_the_poles() {
        assert!(approx(clamp_latitude(89.0), MAX_LATITUDE, 1e-9));
        assert!(approx(clamp_latitude(-89.0), -MAX_LATITUDE, 1e-9));
        // No NaN/inf at the clamped extreme.
        let t = lat_lon_to_tile(LatLon::new(89.0, 0.0), 4);
        assert!(t.x.is_finite() && t.y.is_finite());
    }

    #[test]
    fn tile_x_wraps_but_y_does_not() {
        // At z=2 there are 4 tiles/axis. x=-1 wraps to 3; x=4 wraps to 0.
        assert_eq!(TileId::new(2, -1, 1).wrapped_x(), 3);
        assert_eq!(TileId::new(2, 4, 1).wrapped_x(), 0);
        // y range is hard-bounded (no wrap): -1 and 4 are out, 0..3 are in.
        assert!(!TileId::new(2, 0, -1).in_y_range());
        assert!(!TileId::new(2, 0, 4).in_y_range());
        assert!(TileId::new(2, 0, 0).in_y_range());
        assert!(TileId::new(2, 0, 3).in_y_range());
    }
}
