//! The real reqwest + tokio backed fetch service (feature `net`).
//!
//! [`HttpClient`] owns a multi-thread [`tokio`] runtime on its own background
//! thread. The calling (render/event) thread interacts with it only through
//! non-blocking operations:
//!
//! - [`fetch`](HttpClient::fetch) allocates a [`RequestId`], spawns a task on
//!   the runtime, and returns — it never touches the socket itself.
//! - The task acquires a concurrency permit, checks the cache, issues the GET
//!   with a per-request timeout, caps the body size, populates the cache, and
//!   sends `(id, result)` down an [`mpsc`](std::sync::mpsc) channel.
//! - [`poll_results`](HttpClient::poll_results) drains that channel without
//!   blocking, so the main loop collects completed bodies once per frame.
//!
//! Dropping the client shuts the runtime down.

use crate::{Bytes, FetchResult, HttpClientApi, HttpConfig, HttpError, RequestId, ResponseCache};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tokio::sync::Semaphore;

/// A non-blocking async HTTP GET service backed by reqwest + tokio.
pub struct HttpClient {
    /// The background runtime. Held so it stays alive for the client's lifetime
    /// and is shut down on drop. The render thread never blocks on it.
    runtime: Arc<Runtime>,
    /// reqwest client (connection pool, TLS) shared by every task.
    client: reqwest::Client,
    /// Bounds in-flight requests so a flood of tile fetches can't open
    /// unbounded sockets; queued tasks await a permit.
    permits: Arc<Semaphore>,
    /// URL-keyed LRU response cache shared between the calling thread (lookup on
    /// `fetch`) and the runtime tasks (insert on completion).
    cache: Arc<Mutex<ResponseCache>>,
    /// Result channel: tasks send completions, `poll_results` drains them.
    tx: Sender<(RequestId, FetchResult)>,
    rx: Receiver<(RequestId, FetchResult)>,
    /// Monotonic request id source.
    next_id: AtomicU64,
    config: HttpConfig,
}

impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpClient")
            .field("config", &self.config)
            .field("next_id", &self.next_id.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl HttpClient {
    /// Build a client with default [`HttpConfig`].
    ///
    /// # Errors
    ///
    /// Fails if the tokio runtime or the reqwest client cannot be constructed.
    pub fn new() -> std::result::Result<Self, HttpError> {
        Self::with_config(HttpConfig::default())
    }

    /// Build a client with the given configuration.
    ///
    /// Spins up a multi-thread tokio runtime on its own thread(s) and a reqwest
    /// client carrying the configured User-Agent. Does not perform any I/O.
    ///
    /// # Errors
    ///
    /// Fails ([`HttpError::Transport`]) if the runtime or reqwest client cannot
    /// be built (e.g. the platform refuses to spawn the runtime threads).
    pub fn with_config(config: HttpConfig) -> std::result::Result<Self, HttpError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config.max_concurrency.max(1).min(4))
            .enable_all()
            .thread_name("liquide-http")
            .build()
            .map_err(|e| HttpError::Transport(format!("failed to build runtime: {e}")))?;

        let client = reqwest::Client::builder()
            .user_agent(config.user_agent.clone())
            // A connect timeout in addition to the per-request timeout so a
            // black-holed host fails the connect promptly rather than only on
            // the overall deadline.
            .connect_timeout(config.request_timeout)
            .build()
            .map_err(|e| HttpError::Transport(format!("failed to build client: {e}")))?;

        let (tx, rx) = std::sync::mpsc::channel();

        Ok(Self {
            runtime: Arc::new(runtime),
            client,
            permits: Arc::new(Semaphore::new(config.max_concurrency.max(1))),
            cache: Arc::new(Mutex::new(ResponseCache::new(config.cache_capacity))),
            tx,
            rx,
            next_id: AtomicU64::new(0),
            config,
        })
    }

    fn alloc_id(&self) -> RequestId {
        RequestId(self.next_id.fetch_add(1, Ordering::Relaxed))
    }
}

impl HttpClientApi for HttpClient {
    fn fetch(&self, url: &str) -> RequestId {
        let id = self.alloc_id();
        let url = url.to_string();

        // Fast path: a cache hit is delivered through the SAME channel so the
        // caller has one uniform completion path. We still spawn nothing — just
        // queue the result. (Lookup is a cheap mutex + hashmap touch; it does
        // not block on the network.)
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(body) = cache.get(&url) {
                let _ = self.tx.send((id, Ok(body)));
                return id;
            }
        }

        let client = self.client.clone();
        let permits = Arc::clone(&self.permits);
        let cache = Arc::clone(&self.cache);
        let tx = self.tx.clone();
        let timeout = self.config.request_timeout;
        let max_body = self.config.max_body_bytes;

        self.runtime.spawn(async move {
            // Bound concurrency: queue here until a permit is free. If the
            // semaphore is closed (runtime shutting down) just bail.
            let _permit = match permits.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    let _ = tx.send((id, Err(HttpError::Shutdown)));
                    return;
                }
            };

            let result = fetch_once(&client, &url, timeout, max_body).await;

            // Populate the cache on success so a repeat URL is served locally.
            if let Ok(ref body) = result {
                if let Ok(mut cache) = cache.lock() {
                    cache.put(url.clone(), body.clone());
                }
            }

            // The receiver lives on the main thread; if it has been dropped the
            // client is going away and there is nothing to deliver to.
            let _ = tx.send((id, result));
        });

        id
    }

    fn poll_results(&self) -> Vec<(RequestId, FetchResult)> {
        // try_recv drains everything ready without blocking.
        self.rx.try_iter().collect()
    }
}

/// Perform a single GET with a per-request timeout and a body-size cap.
async fn fetch_once(
    client: &reqwest::Client,
    url: &str,
    timeout: std::time::Duration,
    max_body: usize,
) -> FetchResult {
    let request = client.get(url).timeout(timeout).send();

    // tokio::time::timeout is a belt-and-braces wall-clock bound on top of
    // reqwest's own per-request timeout, so even a stall outside reqwest's
    // accounting (e.g. a hung TLS handshake on some stacks) still resolves.
    let response = match tokio::time::timeout(timeout, request).await {
        Err(_) => return Err(HttpError::Timeout(timeout)),
        Ok(Err(e)) if e.is_timeout() => return Err(HttpError::Timeout(timeout)),
        Ok(Err(e)) => return Err(HttpError::Transport(e.to_string())),
        Ok(Ok(resp)) => resp,
    };

    let status = response.status();
    if !status.is_success() {
        return Err(HttpError::Status(status.as_u16()));
    }

    // Reject an over-cap body declared via Content-Length before buffering it.
    if let Some(len) = response.content_length() {
        if len > max_body as u64 {
            return Err(HttpError::Transport(format!(
                "response body of {len} bytes exceeds cap of {max_body}"
            )));
        }
    }

    match tokio::time::timeout(timeout, response.bytes()).await {
        Err(_) => Err(HttpError::Timeout(timeout)),
        Ok(Err(e)) => Err(HttpError::Transport(e.to_string())),
        Ok(Ok(body)) if body.len() > max_body => Err(HttpError::Transport(format!(
            "response body of {} bytes exceeds cap of {max_body}",
            body.len()
        ))),
        Ok(Ok(body)) => Ok(Bytes::from(body)),
    }
}
