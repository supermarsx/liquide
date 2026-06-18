//! The feature-off stub client.
//!
//! When the `net` feature is disabled, the workspace must still build and link
//! against this crate's public API without pulling in reqwest/tokio.
//! `NullHttpClient` mirrors the real [`crate::HttpClient`] surface: `fetch`
//! still hands back a [`RequestId`] (so call sites are identical), and the
//! matching result delivered by `poll_results` is always
//! [`HttpError::Unavailable`] — the same way `liquide-platform`'s `Null*` hosts
//! stand in for an absent backend.

use crate::{FetchResult, HttpClientApi, HttpConfig, HttpError, RequestId};
use std::sync::Mutex;

/// A no-op HTTP client used when the networking stack is not compiled in.
///
/// Construct it the same way you'd construct the real client (`new` /
/// `with_config`), so call sites don't branch on feature flags. Each `fetch`
/// allocates an id and immediately queues an [`HttpError::Unavailable`] result,
/// which the next `poll_results` returns.
#[derive(Debug)]
pub struct NullHttpClient {
    next_id: Mutex<u64>,
    pending: Mutex<Vec<(RequestId, FetchResult)>>,
}

impl Default for NullHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NullHttpClient {
    /// Construct a null client with default config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: Mutex::new(0),
            pending: Mutex::new(Vec::new()),
        }
    }

    /// Construct a null client. The config is accepted (so the call site matches
    /// the real client) but ignored — nothing is fetched.
    #[must_use]
    pub fn with_config(_config: HttpConfig) -> Self {
        Self::new()
    }
}

impl HttpClientApi for NullHttpClient {
    fn fetch(&self, _url: &str) -> RequestId {
        let id = {
            let mut next = self.next_id.lock().expect("next_id poisoned");
            let id = RequestId(*next);
            *next += 1;
            id
        };
        self.pending
            .lock()
            .expect("pending poisoned")
            .push((id, Err(HttpError::Unavailable)));
        id
    }

    fn poll_results(&self) -> Vec<(RequestId, FetchResult)> {
        std::mem::take(&mut *self.pending.lock().expect("pending poisoned"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_client_fetch_resolves_to_unavailable() {
        let client = NullHttpClient::new();
        let id = client.fetch("http://example.invalid/tile.png");
        let results = client.poll_results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);
        assert!(matches!(results[0].1, Err(HttpError::Unavailable)));
        // Drained: a second poll yields nothing.
        assert!(client.poll_results().is_empty());
    }
}
