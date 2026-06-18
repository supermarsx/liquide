//! The tile manager: turns a viewport's visible-tile set into HTTP fetch
//! requests for the tiles it does not yet have, drains completed fetches, and
//! stores their bytes in an LRU cache (so re-panned tiles are not re-fetched).
//!
//! The HTTP client is INJECTED as a `&dyn liquide_http::HttpClientApi`, so the
//! manager itself is feature-agnostic: the default build hands it a
//! [`liquide_http::NullHttpClient`] (every fetch resolves to `Unavailable` → a
//! tile goes `Failed` and the surface shows the offline placeholder grid), and
//! the `net` build hands it a real `liquide_http::HttpClient`. Tests inject a
//! tiny in-process fake client (no real network).

use std::collections::HashMap;

use liquide_http::{Bytes, FetchResult, HttpClientApi, HttpError, RequestId};

use crate::cache::TileCache;
use crate::slippy::TileId;
use crate::tile_url::tile_url;

/// The lifecycle state of a single tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileState {
    /// Fetched: the raw (encoded, e.g. PNG) bytes are available to decode.
    Ready(Bytes),
    /// A fetch is in flight (we are waiting on this [`RequestId`]).
    Pending(RequestId),
    /// The fetch failed (offline / 404 / transport). The surface shows the
    /// placeholder for this tile instead.
    Failed,
}

/// Drives tile fetching + caching for a slippy map.
///
/// Holds an LRU cache of READY tile bytes (so re-panning is free) plus a small
/// table of in-flight requests keyed by [`RequestId`] so completed fetches route
/// back to the right tile.
#[derive(Debug)]
pub struct TileManager {
    cache: TileCache<Bytes>,
    /// Tiles currently being fetched (request id → which tile).
    in_flight: HashMap<RequestId, TileId>,
    /// Reverse index so we don't issue a second fetch for a pending tile.
    pending_tiles: HashMap<TileId, RequestId>,
    /// Tiles whose fetch failed, so we don't hammer a dead endpoint every frame.
    failed: HashMap<TileId, ()>,
    /// The tile-server URL template (e.g. the OSM `{z}/{x}/{y}.png` form).
    url_template: String,
}

impl TileManager {
    /// A manager with an LRU cache of `cache_capacity` tiles using the default
    /// OSM URL template.
    #[must_use]
    pub fn new(cache_capacity: usize) -> Self {
        Self::with_template(cache_capacity, crate::tile_url::DEFAULT_OSM_TEMPLATE)
    }

    /// A manager using a custom `{z}/{x}/{y}` URL template (e.g. a localhost
    /// tile server in tests).
    #[must_use]
    pub fn with_template(cache_capacity: usize, url_template: impl Into<String>) -> Self {
        Self {
            cache: TileCache::new(cache_capacity),
            in_flight: HashMap::new(),
            pending_tiles: HashMap::new(),
            failed: HashMap::new(),
            url_template: url_template.into(),
        }
    }

    /// Number of tiles currently cached (READY).
    #[must_use]
    pub fn cached_len(&self) -> usize {
        self.cache.len()
    }

    /// Number of in-flight fetches.
    #[must_use]
    pub fn in_flight_len(&self) -> usize {
        self.in_flight.len()
    }

    /// The current state of a tile, if known: `Ready` (cached bytes),
    /// `Pending` (in flight), `Failed`, or `None` (never requested / evicted).
    #[must_use]
    pub fn tile_state(&self, key: &TileId) -> Option<TileState> {
        if let Some(req) = self.pending_tiles.get(key) {
            return Some(TileState::Pending(*req));
        }
        if self.cache.contains(key) {
            return Some(TileState::Ready(Bytes::new()));
        }
        if self.failed.contains_key(key) {
            return Some(TileState::Failed);
        }
        None
    }

    /// Borrow a tile's READY bytes if cached (marks it most-recently-used).
    pub fn ready_bytes(&mut self, key: &TileId) -> Option<Bytes> {
        self.cache.get(key).cloned()
    }

    /// Whether a tile's bytes are cached (does not count as a use).
    #[must_use]
    pub fn has(&self, key: &TileId) -> bool {
        self.cache.contains(key)
    }

    /// Request every tile in `wanted` that we don't already have (cached, in
    /// flight, or recently failed) via the injected HTTP client. Returns the
    /// number of NEW fetches issued. A re-panned tile already in the cache or in
    /// flight issues nothing (the de-dupe the LRU cache exists for).
    pub fn request_missing(&mut self, wanted: &[TileId], client: &dyn HttpClientApi) -> usize {
        let mut issued = 0;
        for &key in wanted {
            let key = key.canonical();
            if self.cache.contains(&key)
                || self.pending_tiles.contains_key(&key)
                || self.failed.contains_key(&key)
            {
                continue;
            }
            let url = tile_url(&self.url_template, key);
            let req = client.fetch(&url);
            self.in_flight.insert(req, key);
            self.pending_tiles.insert(key, req);
            issued += 1;
        }
        issued
    }

    /// Drain the client's completed fetches and fold each into the manager:
    /// a success caches the bytes (READY), a failure marks the tile `Failed`.
    /// Returns the tile ids whose state CHANGED this drain (so the caller can
    /// decode the new bytes and damage the surface).
    pub fn drain_results(&mut self, client: &dyn HttpClientApi) -> Vec<TileId> {
        let mut changed = Vec::new();
        for (req, result) in client.poll_results() {
            let Some(key) = self.in_flight.remove(&req) else {
                continue; // a result for a request we don't track (not ours)
            };
            self.pending_tiles.remove(&key);
            match result {
                Ok(bytes) => {
                    self.failed.remove(&key);
                    self.cache.put(key, bytes);
                    changed.push(key);
                }
                Err(err) => {
                    self.note_failed(key, &err);
                    changed.push(key);
                }
            }
        }
        changed
    }

    fn note_failed(&mut self, key: TileId, err: &HttpError) {
        tracing::debug!(?key, %err, "tile fetch failed; showing placeholder");
        self.failed.insert(key, ());
    }

    /// Clear the recorded failures so previously-failed tiles can be retried
    /// (e.g. after the network came back). Cached tiles are kept.
    pub fn clear_failures(&mut self) {
        self.failed.clear();
    }
}

/// A typed view of a completed fetch (for callers that want the raw result).
pub type DrainedResult = (RequestId, FetchResult);

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    /// An in-process FAKE HTTP client: `fetch` records the url and queues a
    /// canned result; `poll_results` drains them. NO real network, NO sockets.
    /// This is the anti-fake-green seam — the request/cache flow is driven by a
    /// deterministic injected client exactly like a real one would behave.
    struct FakeClient {
        next: RefCell<u64>,
        /// What the next fetch should resolve to (Ok bytes / an error).
        program: RefCell<HashMap<String, FetchResult>>,
        ready: RefCell<VecDeque<(RequestId, FetchResult)>>,
        fetched_urls: RefCell<Vec<String>>,
    }

    impl FakeClient {
        fn new() -> Self {
            Self {
                next: RefCell::new(0),
                program: RefCell::new(HashMap::new()),
                ready: RefCell::new(VecDeque::new()),
                fetched_urls: RefCell::new(Vec::new()),
            }
        }
        /// Make every url return these bytes.
        fn always_ok(bytes: &'static [u8]) -> Self {
            let c = Self::new();
            c.program
                .borrow_mut()
                .insert("*".into(), Ok(Bytes::from_static(bytes)));
            c
        }
        fn fetched_count(&self) -> usize {
            self.fetched_urls.borrow().len()
        }
    }

    impl HttpClientApi for FakeClient {
        fn fetch(&self, url: &str) -> RequestId {
            let id = {
                let mut n = self.next.borrow_mut();
                let id = RequestId(*n);
                *n += 1;
                id
            };
            self.fetched_urls.borrow_mut().push(url.to_string());
            let prog = self.program.borrow();
            let result = prog
                .get(url)
                .or_else(|| prog.get("*"))
                .cloned()
                .unwrap_or(Err(HttpError::Transport("no program".into())));
            self.ready.borrow_mut().push_back((id, result));
            id
        }
        fn poll_results(&self) -> Vec<(RequestId, FetchResult)> {
            self.ready.borrow_mut().drain(..).collect()
        }
    }

    fn ids() -> Vec<TileId> {
        vec![
            TileId::new(2, 0, 0),
            TileId::new(2, 1, 0),
            TileId::new(2, 0, 1),
        ]
    }

    #[test]
    fn requests_missing_then_caches_on_drain() {
        let client = FakeClient::always_ok(b"PNGDATA");
        let mut mgr = TileManager::new(64);
        let wanted = ids();
        let issued = mgr.request_missing(&wanted, &client);
        assert_eq!(issued, 3, "all three tiles are missing → three fetches");
        assert_eq!(mgr.in_flight_len(), 3);

        let changed = mgr.drain_results(&client);
        assert_eq!(changed.len(), 3, "all three completed");
        assert_eq!(mgr.cached_len(), 3, "bytes cached");
        assert_eq!(mgr.in_flight_len(), 0, "no longer in flight");
        // Ready bytes are the fetched payload.
        let b = mgr.ready_bytes(&TileId::new(2, 0, 0)).expect("ready");
        assert_eq!(&b[..], b"PNGDATA");
    }

    #[test]
    fn re_requesting_cached_or_pending_tiles_issues_nothing() {
        let client = FakeClient::always_ok(b"X");
        let mut mgr = TileManager::new(64);
        let wanted = ids();
        // First pass: 3 fetches.
        assert_eq!(mgr.request_missing(&wanted, &client), 3);
        // Re-request the SAME set while still pending → 0 new fetches (de-dupe).
        assert_eq!(
            mgr.request_missing(&wanted, &client),
            0,
            "pending tiles must not be re-fetched"
        );
        mgr.drain_results(&client);
        // Now cached; re-request again → still 0 (re-pan is free).
        assert_eq!(
            mgr.request_missing(&wanted, &client),
            0,
            "cached tiles must not be re-fetched"
        );
        assert_eq!(client.fetched_count(), 3, "only the original 3 fetches");
    }

    #[test]
    fn failed_fetch_marks_tile_failed_and_is_not_retried() {
        let client = FakeClient::new();
        client
            .program
            .borrow_mut()
            .insert("*".into(), Err(HttpError::Status(404)));
        let mut mgr = TileManager::new(64);
        let wanted = vec![TileId::new(2, 0, 0)];
        assert_eq!(mgr.request_missing(&wanted, &client), 1);
        let changed = mgr.drain_results(&client);
        assert_eq!(changed, vec![TileId::new(2, 0, 0)]);
        assert_eq!(mgr.tile_state(&TileId::new(2, 0, 0)), Some(TileState::Failed));
        // A failed tile is not retried on the next request pass.
        assert_eq!(mgr.request_missing(&wanted, &client), 0);
        // ...until failures are cleared.
        mgr.clear_failures();
        assert_eq!(mgr.request_missing(&wanted, &client), 1);
    }

    #[test]
    fn null_client_offline_path_does_not_panic_and_fails_gracefully() {
        // The DEFAULT (no-net) client: every fetch resolves to Unavailable.
        let client = liquide_http::NullHttpClient::new();
        let mut mgr = TileManager::new(8);
        let wanted = ids();
        let issued = mgr.request_missing(&wanted, &client);
        assert_eq!(issued, 3);
        let changed = mgr.drain_results(&client);
        assert_eq!(changed.len(), 3);
        // Nothing cached; all tiles Failed (→ placeholder grid). No panic.
        assert_eq!(mgr.cached_len(), 0);
        for id in ids() {
            assert_eq!(mgr.tile_state(&id.canonical()), Some(TileState::Failed));
        }
    }
}
