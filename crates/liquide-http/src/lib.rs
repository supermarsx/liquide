//! A non-blocking async HTTP fetch service for liquide.
//!
//! The desktop renders and dispatches events on a single thread; that thread
//! must never block on the network. This crate gives it a fire-and-forget
//! `fetch(url) -> RequestId` that returns immediately, runs the request on a
//! background [`tokio`] runtime (its own thread), and delivers the result back
//! through a channel the main loop drains once per frame via
//! [`poll_results`](HttpClientApi::poll_results).
//!
//! The intended consumer is the OSM slippy-map element (and any other remote
//! resource: images, icons). This crate keeps a clean library boundary: it is
//! deliberately NOT wired into the shell/session/map here — that is the map
//! element's job next.
//!
//! ## Non-blocking model
//!
//! ```text
//!   main thread                         background runtime (own thread)
//!   -----------                         ------------------------------
//!   id = fetch(url)  --- submit ---->   spawn task (bounded by a semaphore)
//!   ... render frame ...                  GET url (per-request timeout)
//!   poll_results()   <--- channel ---   send (id, Result<Bytes>)
//! ```
//!
//! - `fetch` never blocks: it allocates a [`RequestId`], hands the URL to the
//!   runtime, and returns. The caller learns the outcome later from
//!   `poll_results`, which is itself non-blocking (drains whatever is ready).
//! - Concurrency is **bounded** (a semaphore) so a flood of tile requests can't
//!   open unbounded sockets.
//! - Every request has a **timeout** so a hung/slow endpoint can't pin a worker
//!   forever — it resolves to [`HttpError::Timeout`].
//! - A small **in-memory LRU cache** keyed by URL serves repeat tile fetches
//!   without touching the network (capped, so it can't grow without bound).
//!
//! ## Feature gating
//!
//! The real reqwest+tokio backed client lives behind the **`net`** cargo
//! feature, which is **off by default**. With the feature off, [`NullHttpClient`]
//! provides the identical [`HttpClientApi`] surface and answers every `fetch`
//! result with [`HttpError::Unavailable`], so the workspace builds and links
//! without the reqwest/tokio dependency tree (the same pattern as the platform
//! `Null*` hosts and [`liquide-wasm-host`]'s `NullWasmHost`).
//!
//! [`liquide-wasm-host`]: https://docs.rs/liquide-wasm-host

#![forbid(unsafe_code)]

pub mod cache;
pub mod config;

pub use bytes::Bytes;
pub use cache::ResponseCache;
pub use config::HttpConfig;

use thiserror::Error;

/// An opaque, monotonically-increasing handle for an in-flight fetch.
///
/// `fetch` returns one immediately; the matching `(RequestId, Result)` pair
/// comes back later from [`poll_results`](HttpClientApi::poll_results). Callers
/// (e.g. a tile cache) map the id back to what they were loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub u64);

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "req#{}", self.0)
    }
}

/// Errors a fetch can produce (delivered per-request via `poll_results`).
#[derive(Debug, Error, Clone)]
pub enum HttpError {
    /// The `net` feature is disabled, so there is no real client. Every
    /// [`NullHttpClient`] fetch resolves to this.
    #[error("http client unavailable: built without the `net` feature")]
    Unavailable,

    /// The request did not complete before the per-request timeout elapsed
    /// (slow or hung endpoint). The worker is freed, not pinned.
    #[error("request timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// The server answered with a non-success (>= 400) HTTP status.
    #[error("server returned status {0}")]
    Status(u16),

    /// The transport failed (connection refused, DNS, TLS, body read, ...).
    #[error("transport error: {0}")]
    Transport(String),

    /// The background runtime has shut down (the client was dropped) and can no
    /// longer accept or deliver work.
    #[error("the http runtime is shut down")]
    Shutdown,
}

/// Result alias for a fetched body.
pub type FetchResult = std::result::Result<Bytes, HttpError>;

/// The behaviour every HTTP client (real or null) exposes.
///
/// This is the clean library boundary the map element / shell will wire to
/// later. Both [`HttpClient`] (feature `net`) and [`NullHttpClient`] implement
/// it, so callers are written once against the trait and the concrete type is
/// chosen by the build's features.
pub trait HttpClientApi {
    /// Submit a GET for `url`. Returns immediately with a [`RequestId`]; the
    /// body (or error) arrives later via [`poll_results`](Self::poll_results).
    ///
    /// Never blocks on the network. A cache hit is still delivered through
    /// `poll_results` (not returned inline) so callers have a single, uniform
    /// completion path.
    fn fetch(&self, url: &str) -> RequestId;

    /// Drain all fetch results that have completed since the last call.
    ///
    /// Non-blocking: returns whatever is ready (possibly empty). The main loop
    /// calls this once per frame to deliver tile/image bytes.
    fn poll_results(&self) -> Vec<(RequestId, FetchResult)>;
}

#[cfg(feature = "net")]
mod client;
#[cfg(feature = "net")]
pub use client::HttpClient;

mod null;
pub use null::NullHttpClient;
