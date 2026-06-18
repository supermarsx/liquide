//! An OpenStreetMap slippy-map element for liquide.
//!
//! A pannable / zoomable tiled map: it computes the Web-Mercator tile grid a
//! [`Viewport`] needs, REQUESTS the missing tiles through an injected HTTP client
//! (`liquide-http`), caches the fetched bytes in a bounded LRU [`TileCache`]
//! (so re-panned tiles are not re-fetched), and exposes each visible tile's
//! on-screen rect + a stable image key so the host can composite the decoded
//! tiles into the scene as positioned `Image` nodes.
//!
//! ## Layers
//!
//! * [`slippy`] — pure lat/lon ↔ tile `z/x/y` ↔ world-pixel math (Web Mercator).
//! * [`viewport`] — the [`Viewport`] (centre + zoom + box), pan/zoom state
//!   changes, and the visible-tile-set computation with per-tile screen rects.
//! * [`cache`] — the bounded LRU [`TileCache`].
//! * [`tile_url`] — the `{z}/{x}/{y}` URL template + the stable per-tile image key.
//! * [`manager`] — the [`TileManager`] that fetches missing tiles via a
//!   `liquide_http::HttpClientApi` and folds completed fetches into the cache.
//! * [`MapState`] (here) — the small façade the shell/session drive: a viewport
//!   plus a tile manager, with one `tick` that requests + drains tiles and one
//!   `placement` that returns the visible tiles + their image keys.
//!
//! ## Networking is feature-gated (offline by default)
//!
//! The HTTP client is INJECTED, so this crate is feature-agnostic. The default
//! build pays for no network: the host hands the tick a
//! [`liquide_http::NullHttpClient`] (every fetch resolves to `Unavailable`), so
//! tiles go `Failed` and the host paints a graceful PLACEHOLDER grid — no panic,
//! no required network. Enabling the crate's `net` feature pulls in
//! `liquide-http/net` so a real `liquide_http::HttpClient` can be injected.

#![forbid(unsafe_code)]

pub mod cache;
pub mod manager;
pub mod slippy;
pub mod tile_url;
pub mod viewport;

pub use cache::TileCache;
pub use manager::{TileManager, TileState};
pub use slippy::{LatLon, TileCoord, TileId, MAX_ZOOM, TILE_SIZE};
pub use tile_url::{parse_tile_image_key, tile_image_key, DEFAULT_OSM_TEMPLATE};
pub use viewport::{PlacedTile, Viewport};

use liquide_http::HttpClientApi;

/// The default LRU tile cache capacity (entries). A few screens' worth of tiles
/// across a couple of zoom levels, bounded so a long pan can't grow unbounded.
pub const DEFAULT_TILE_CACHE_CAPACITY: usize = 256;

/// One visible tile ready to be composited: its screen rect plus the stable
/// image key the host binds the decoded RGBA to, and whether its bytes are
/// available yet (so the host can paint a placeholder for a not-yet-loaded one).
#[derive(Debug, Clone, PartialEq)]
pub struct TilePlacement {
    /// The placed tile (id, fetch key, screen rect).
    pub tile: PlacedTile,
    /// The stable image key (`tile://z/x/y`) the tile's `Image` node uses as its
    /// `background-image` src — hashed to the renderer image id on both the
    /// compositing and the decode sides.
    pub image_key: String,
    /// Whether the tile's bytes are cached (loaded). `false` → the host paints
    /// the placeholder for this slot until the fetch completes.
    pub loaded: bool,
}

/// The drivable map state: a [`Viewport`] plus a [`TileManager`]. The shell maps
/// a `Map { center_lat, center_lon, zoom }` node onto one of these, drives
/// pan/zoom through it, and the session render loop ticks its tile lifecycle.
#[derive(Debug)]
pub struct MapState {
    pub viewport: Viewport,
    pub tiles: TileManager,
}

impl MapState {
    /// A map state centred on `center` at `zoom`, sized `width`×`height` px, with
    /// the default OSM tile template + cache capacity.
    #[must_use]
    pub fn new(center: LatLon, zoom: u32, width: f64, height: f64) -> Self {
        Self {
            viewport: Viewport::new(center, zoom, width, height),
            tiles: TileManager::new(DEFAULT_TILE_CACHE_CAPACITY),
        }
    }

    /// A map state with a custom tile-server template + cache capacity (tests use
    /// this to point at a localhost / fake server).
    #[must_use]
    pub fn with_template(
        center: LatLon,
        zoom: u32,
        width: f64,
        height: f64,
        cache_capacity: usize,
        url_template: impl Into<String>,
    ) -> Self {
        Self {
            viewport: Viewport::new(center, zoom, width, height),
            tiles: TileManager::with_template(cache_capacity, url_template),
        }
    }

    /// Run one tile-lifecycle tick against the injected client: drain any
    /// completed fetches into the cache, then request the tiles the current
    /// viewport needs but doesn't have. Returns the tile ids whose state changed
    /// this tick (newly loaded / failed), so the caller can decode + register
    /// them and damage the surface.
    pub fn tick(&mut self, client: &dyn HttpClientApi) -> Vec<TileId> {
        let changed = self.tiles.drain_results(client);
        let wanted: Vec<TileId> = self
            .viewport
            .visible_tiles()
            .into_iter()
            .map(|t| t.key)
            .collect();
        self.tiles.request_missing(&wanted, client);
        changed
    }

    /// The visible tiles with their screen rects + image keys + loaded flags.
    /// This is what the shell emits as positioned `Image` nodes (loaded tiles
    /// paint their texture; not-yet-loaded ones get a placeholder).
    #[must_use]
    pub fn placement(&self) -> Vec<TilePlacement> {
        self.viewport
            .visible_tiles()
            .into_iter()
            .map(|tile| {
                let loaded = self.tiles.has(&tile.key);
                TilePlacement {
                    image_key: tile_image_key(tile.key),
                    tile,
                    loaded,
                }
            })
            .collect()
    }

    /// Pan the viewport by a screen-pixel drag delta.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.viewport.pan_pixels(dx, dy);
    }

    /// Zoom toward a screen anchor (wheel-zoom). Returns whether zoom changed.
    pub fn zoom_at(&mut self, delta: i32, anchor_x: f64, anchor_y: f64) -> bool {
        self.viewport.zoom_at(delta, anchor_x, anchor_y)
    }

    /// Zoom keeping the centre fixed (button-zoom). Returns whether zoom changed.
    pub fn zoom_by(&mut self, delta: i32) -> bool {
        self.viewport.zoom_by(delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_placement_marks_every_tile_unloaded_without_panicking() {
        // Default offline path: a Null client, nothing ever loads → every slot is
        // a placeholder, but placement still returns the full grid (positioned).
        let client = liquide_http::NullHttpClient::new();
        let mut map = MapState::new(LatLon::new(0.0, 0.0), 2, 512.0, 512.0);
        // Tick a few times — must never panic and never load a tile offline.
        for _ in 0..3 {
            map.tick(&client);
        }
        let placement = map.placement();
        assert!(!placement.is_empty(), "placement must cover the box");
        assert!(
            placement.iter().all(|p| !p.loaded),
            "offline: no tile can be loaded"
        );
        // Each placement carries a stable, parseable image key.
        for p in &placement {
            assert_eq!(parse_tile_image_key(&p.image_key), Some(p.tile.key));
        }
    }

    #[test]
    fn pan_then_re_pan_does_not_refetch_with_a_fake_client() {
        use liquide_http::{Bytes, FetchResult, HttpClientApi, RequestId};
        use std::cell::RefCell;
        use std::collections::VecDeque;

        struct FakeOk {
            next: RefCell<u64>,
            ready: RefCell<VecDeque<(RequestId, FetchResult)>>,
            fetches: RefCell<usize>,
        }
        impl HttpClientApi for FakeOk {
            fn fetch(&self, _url: &str) -> RequestId {
                let mut n = self.next.borrow_mut();
                let id = RequestId(*n);
                *n += 1;
                *self.fetches.borrow_mut() += 1;
                self.ready
                    .borrow_mut()
                    .push_back((id, Ok(Bytes::from_static(b"PNG"))));
                id
            }
            fn poll_results(&self) -> Vec<(RequestId, FetchResult)> {
                self.ready.borrow_mut().drain(..).collect()
            }
        }
        let client = FakeOk {
            next: RefCell::new(0),
            ready: RefCell::new(VecDeque::new()),
            fetches: RefCell::new(0),
        };

        let mut map = MapState::new(LatLon::new(0.0, 0.0), 3, 256.0, 256.0);
        map.tick(&client); // request initial tiles
        map.tick(&client); // drain → cached
        let first = *client.fetches.borrow();
        assert!(first > 0, "initial tiles fetched");
        assert!(map.placement().iter().all(|p| p.loaded), "all loaded now");

        // Pan one full tile east, then back. The new column fetches once; panning
        // BACK re-uses the cache (no new fetch for the originally-cached tiles).
        map.pan(-256.0, 0.0);
        map.tick(&client);
        map.tick(&client);
        let after_pan = *client.fetches.borrow();
        assert!(after_pan > first, "the newly-revealed column fetched");

        map.pan(256.0, 0.0); // back to the start
        map.tick(&client);
        map.tick(&client);
        let after_return = *client.fetches.borrow();
        assert_eq!(
            after_return, after_pan,
            "re-panning over cached tiles must NOT re-fetch"
        );
    }
}
