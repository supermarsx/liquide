//! Tunables for the fetch service.

use std::time::Duration;

/// Configuration for an [`HttpClient`](crate::HttpClient).
///
/// Sensible defaults are tuned for tile/resource fetching: bounded concurrency
/// so a screenful of tiles can't open unbounded sockets, a per-request timeout
/// so a hung endpoint can't pin a worker, and a capped in-memory cache so
/// re-panning a map re-serves tiles without re-fetching (and without unbounded
/// growth).
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// Maximum number of requests allowed in flight at once. Further `fetch`
    /// calls queue (their tasks await a semaphore permit) rather than opening
    /// more sockets.
    pub max_concurrency: usize,

    /// Per-request timeout. A request that has not produced a body within this
    /// window resolves to [`HttpError::Timeout`](crate::HttpError::Timeout).
    pub request_timeout: Duration,

    /// Maximum number of cached responses kept in memory (LRU eviction). Set to
    /// `0` to disable caching entirely.
    pub cache_capacity: usize,

    /// Maximum size of a single response body, in bytes. Larger bodies resolve
    /// to a [`Transport`](crate::HttpError::Transport) error rather than being
    /// buffered — a backstop against a hostile/huge endpoint exhausting memory.
    pub max_body_bytes: usize,

    /// `User-Agent` sent on every request. OSM tile usage policy *requires* a
    /// descriptive UA; default identifies liquide.
    pub user_agent: String,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 8,
            request_timeout: Duration::from_secs(15),
            cache_capacity: 256,
            max_body_bytes: 16 * 1024 * 1024,
            user_agent: concat!("liquide-http/", env!("CARGO_PKG_VERSION")).to_string(),
        }
    }
}
