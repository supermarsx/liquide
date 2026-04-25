//! Recent files tracking.
//!
//! Maintains an in-memory store of recently accessed files, sorted by last
//! access time.  Supports persistence via a simple line-oriented text format.
//! Modelled after the freedesktop recently-used specification and GNOME
//! Nautilus recent-files behaviour.

use serde::{Deserialize, Serialize};

/// Default maximum number of entries kept in the recent store.
pub const DEFAULT_MAX_ENTRIES: usize = 500;

// ---------------------------------------------------------------------------
// RecentEntry
// ---------------------------------------------------------------------------

/// A single recently-accessed file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentEntry {
    /// URI of the file (e.g. `file:///home/user/doc.txt`).
    pub uri: String,
    /// Human-readable name.
    pub display_name: String,
    /// MIME type of the file.
    pub mime_type: String,
    /// Timestamp of the most recent access (milliseconds since epoch).
    pub last_accessed_ms: u64,
    /// Total number of times this file was accessed.
    pub access_count: u32,
    /// Application that last opened the file (desktop-file id, e.g.
    /// `org.gnome.TextEditor`).
    pub app_id: String,
}

impl RecentEntry {
    /// Create a new recent entry with an initial access count of 1.
    #[must_use]
    pub fn new(
        uri: String,
        display_name: String,
        mime_type: String,
        timestamp_ms: u64,
        app_id: String,
    ) -> Self {
        Self {
            uri,
            display_name,
            mime_type,
            last_accessed_ms: timestamp_ms,
            access_count: 1,
            app_id,
        }
    }

    /// Record another access, bumping the counter and updating the timestamp.
    pub fn touch(&mut self, timestamp_ms: u64, app_id: &str) {
        self.access_count += 1;
        self.last_accessed_ms = timestamp_ms;
        self.app_id = app_id.to_string();
    }
}

// ---------------------------------------------------------------------------
// RecentStore
// ---------------------------------------------------------------------------

/// In-memory store of recent file entries.
pub struct RecentStore {
    entries: Vec<RecentEntry>,
    max_entries: usize,
}

impl RecentStore {
    /// Create an empty store with the default capacity.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }

    /// Create a store with a custom maximum entry count.
    #[must_use]
    pub fn with_max(max: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries: max,
        }
    }

    /// Add or update a file in the recent list.
    ///
    /// If the URI already exists the access count is incremented and the
    /// timestamp updated.  Otherwise a new entry is appended.  If the store
    /// exceeds `max_entries` the oldest entry is evicted.
    pub fn add(
        &mut self,
        uri: &str,
        display_name: &str,
        mime_type: &str,
        timestamp_ms: u64,
        app_id: &str,
    ) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.uri == uri) {
            existing.touch(timestamp_ms, app_id);
        } else {
            self.entries.push(RecentEntry::new(
                uri.to_string(),
                display_name.to_string(),
                mime_type.to_string(),
                timestamp_ms,
                app_id.to_string(),
            ));
        }
        // Enforce capacity.
        self.enforce_limit();
    }

    /// Remove a specific URI from the recent list.
    pub fn remove(&mut self, uri: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.uri != uri);
        self.entries.len() < before
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// List entries sorted by `last_accessed_ms` descending (most recent
    /// first).
    #[must_use]
    pub fn list(&self) -> Vec<&RecentEntry> {
        let mut sorted: Vec<&RecentEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.last_accessed_ms.cmp(&a.last_accessed_ms));
        sorted
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return the `limit` most frequently accessed entries, sorted by
    /// `access_count` descending.
    #[must_use]
    pub fn frequently_used(&self, limit: usize) -> Vec<&RecentEntry> {
        let mut sorted: Vec<&RecentEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.access_count.cmp(&a.access_count));
        sorted.truncate(limit);
        sorted
    }

    /// Remove all entries whose `last_accessed_ms` is older than `days` days
    /// relative to `now_ms`.
    pub fn purge_older_than(&mut self, days: u32, now_ms: u64) {
        let cutoff_ms = now_ms.saturating_sub(days as u64 * 86_400_000);
        self.entries.retain(|e| e.last_accessed_ms >= cutoff_ms);
    }

    /// Find an entry by URI.
    #[must_use]
    pub fn find(&self, uri: &str) -> Option<&RecentEntry> {
        self.entries.iter().find(|e| e.uri == uri)
    }

    /// Maximum number of entries this store will hold.
    #[must_use]
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// Update the maximum entry limit.  If the current size exceeds the new
    /// limit the oldest entries are evicted immediately.
    pub fn set_max_entries(&mut self, max: usize) {
        self.max_entries = max;
        self.enforce_limit();
    }

    // -----------------------------------------------------------------------
    // Serialization (simple line format)
    // -----------------------------------------------------------------------

    /// Serialize the store to a simple line-oriented text format.
    ///
    /// Each entry is one line: `uri\tdisplay_name\tmime_type\tlast_accessed_ms\taccess_count\tapp_id`
    #[must_use]
    pub fn serialize(&self) -> String {
        let mut out = String::new();
        for e in &self.entries {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                e.uri, e.display_name, e.mime_type, e.last_accessed_ms, e.access_count, e.app_id,
            ));
        }
        out
    }

    /// Deserialize from the line-oriented text format produced by
    /// [`serialize`](Self::serialize).
    pub fn deserialize(&mut self, data: &str) {
        self.entries.clear();
        for line in data.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 6 {
                continue;
            }
            let last_accessed_ms = parts[3].parse::<u64>().unwrap_or(0);
            let access_count = parts[4].parse::<u32>().unwrap_or(1);
            self.entries.push(RecentEntry {
                uri: parts[0].to_string(),
                display_name: parts[1].to_string(),
                mime_type: parts[2].to_string(),
                last_accessed_ms,
                access_count,
                app_id: parts[5].to_string(),
            });
        }
        self.enforce_limit();
    }

    // -----------------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------------

    /// Evict the oldest entries when the store exceeds `max_entries`.
    fn enforce_limit(&mut self) {
        if self.entries.len() > self.max_entries {
            // Sort ascending by timestamp so we can truncate the tail (newest).
            self.entries.sort_by_key(|e| e.last_accessed_ms);
            let excess = self.entries.len() - self.max_entries;
            self.entries.drain(..excess);
        }
    }
}

impl Default for RecentStore {
    fn default() -> Self {
        Self::new()
    }
}
