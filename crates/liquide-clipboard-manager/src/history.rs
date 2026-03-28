//! Ring-buffer clipboard history with dedup, search, and pinning.

use std::collections::VecDeque;

use crate::entry::{ClipboardContent, ClipboardEntry, ContentCategory};

/// Default maximum number of entries kept in history.
const DEFAULT_MAX_ENTRIES: usize = 500;

/// Default maximum size of a single entry (10 MB).
const DEFAULT_MAX_ENTRY_BYTES: usize = 10 * 1024 * 1024;

/// Ring-buffer clipboard history.
pub struct ClipboardHistory {
    /// All entries (newest at the front).
    entries: VecDeque<ClipboardEntry>,
    /// Next unique id to assign.
    next_id: u64,
    /// Maximum number of entries (excluding pinned entries which are always
    /// kept).
    max_entries: usize,
    /// Maximum size of a single entry payload in bytes.
    max_entry_bytes: usize,
}

impl ClipboardHistory {
    /// Create a new history with default limits.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            next_id: 1,
            max_entries: DEFAULT_MAX_ENTRIES,
            max_entry_bytes: DEFAULT_MAX_ENTRY_BYTES,
        }
    }

    /// Create a history with custom limits.
    #[must_use]
    pub fn with_limits(max_entries: usize, max_entry_bytes: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            next_id: 1,
            max_entries: max_entries.max(1),
            max_entry_bytes,
        }
    }

    /// Maximum number of entries the history will hold.
    #[must_use]
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// Set the maximum number of history entries.  Excess unpinned entries are
    /// evicted from the oldest end.
    pub fn set_max_entries(&mut self, n: usize) {
        self.max_entries = n.max(1);
        self.evict();
    }

    /// Maximum payload size for a single entry in bytes.
    #[must_use]
    pub fn max_entry_bytes(&self) -> usize {
        self.max_entry_bytes
    }

    /// Set the maximum payload size for a single entry in bytes.
    pub fn set_max_entry_bytes(&mut self, n: usize) {
        self.max_entry_bytes = n;
    }

    /// Total number of entries (including pinned).
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Push a new entry.  If the content matches the most recent entry the
    /// existing entry's timestamp is updated instead of creating a duplicate.
    ///
    /// Returns `None` if the entry exceeds the per-entry size limit.
    /// Returns `Some(id)` of the (possibly reused) entry otherwise.
    pub fn push(&mut self, mut entry: ClipboardEntry) -> Option<u64> {
        // Enforce per-entry size limit.
        if entry.content.size_bytes() > self.max_entry_bytes {
            tracing::warn!(
                size = entry.content.size_bytes(),
                max = self.max_entry_bytes,
                "clipboard entry exceeds size limit, dropping"
            );
            return None;
        }

        // Dedup: if the newest entry has identical content, just refresh its
        // timestamp and source.
        if let Some(front) = self.entries.front_mut() {
            if front.content.content_eq(&entry.content) {
                front.timestamp = entry.timestamp;
                if entry.source_app.is_some() {
                    front.source_app = entry.source_app;
                }
                return Some(front.id);
            }
        }

        // Assign a fresh id.
        let id = self.next_id;
        self.next_id += 1;
        entry.id = id;

        self.entries.push_front(entry);
        self.evict();
        Some(id)
    }

    /// Look up an entry by id.
    #[must_use]
    pub fn get(&self, id: u64) -> Option<&ClipboardEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Look up an entry mutably by id.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut ClipboardEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Return the `count` most recent entries (newest first).
    #[must_use]
    pub fn recent(&self, count: usize) -> Vec<&ClipboardEntry> {
        self.entries.iter().take(count).collect()
    }

    /// Return the most recent entry, if any.
    #[must_use]
    pub fn latest(&self) -> Option<&ClipboardEntry> {
        self.entries.front()
    }

    /// Full-text search across text entries (plain text and rich-text
    /// fallback).  Case-insensitive substring match.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&ClipboardEntry> {
        if query.is_empty() {
            return Vec::new();
        }
        let lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.content
                    .as_searchable_text()
                    .is_some_and(|t| t.to_lowercase().contains(&lower))
            })
            .collect()
    }

    /// Search across all entry types — text (substring), file paths
    /// (path substring), colours (hex match).  Case-insensitive.
    #[must_use]
    pub fn search_all(&self, query: &str) -> Vec<&ClipboardEntry> {
        if query.is_empty() {
            return Vec::new();
        }
        let lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                // Text content.
                if let Some(t) = e.content.as_searchable_text() {
                    if t.to_lowercase().contains(&lower) {
                        return true;
                    }
                }
                // File paths.
                if let Some(paths) = e.content.as_file_paths() {
                    if paths.iter().any(|p| p.to_lowercase().contains(&lower)) {
                        return true;
                    }
                }
                // Colour hex.
                if let ClipboardContent::Color { r, g, b, a } = &e.content {
                    let hex = format!("#{r:02x}{g:02x}{b:02x}{a:02x}");
                    if hex.contains(&lower) {
                        return true;
                    }
                }
                // Source app.
                if let Some(app) = &e.source_app {
                    if app.to_lowercase().contains(&lower) {
                        return true;
                    }
                }
                false
            })
            .collect()
    }

    /// Remove all entries that are marked sensitive and whose timestamp is
    /// older than `cutoff_ts` (seconds since epoch).  Pinned entries are NOT
    /// removed even if sensitive+expired — the user explicitly kept them.
    /// Returns the number of entries removed.
    pub fn expire_sensitive(&mut self, cutoff_ts: u64) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| {
            if e.sensitive && !e.pinned && e.timestamp < cutoff_ts {
                return false;
            }
            true
        });
        before - self.entries.len()
    }

    /// Remove all entries that are marked sensitive (e.g. on screen lock).
    /// Pinned entries are preserved.  Returns the number of entries removed.
    pub fn clear_sensitive(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| !e.sensitive || e.pinned);
        before - self.entries.len()
    }

    /// Return all entries as a slice-like iterator (newest first).
    pub fn iter(&self) -> impl Iterator<Item = &ClipboardEntry> {
        self.entries.iter()
    }

    /// Total number of entries (including pinned).  Same as `len()` but
    /// named for clarity alongside `unpinned_count`.
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.entries.len()
    }

    /// Pin an entry so it survives [`Self::clear()`].
    pub fn pin(&mut self, id: u64) {
        if let Some(e) = self.get_mut(id) {
            e.pinned = true;
        }
    }

    /// Unpin an entry.
    pub fn unpin(&mut self, id: u64) {
        if let Some(e) = self.get_mut(id) {
            e.pinned = false;
        }
    }

    /// Delete an entry by id (even if pinned).
    pub fn delete(&mut self, id: u64) {
        self.entries.retain(|e| e.id != id);
    }

    /// Clear all non-pinned entries.
    pub fn clear(&mut self) {
        self.entries.retain(|e| e.pinned);
    }

    /// Return all pinned entries (newest first, preserving insertion order).
    #[must_use]
    pub fn pinned(&self) -> Vec<&ClipboardEntry> {
        self.entries.iter().filter(|e| e.pinned).collect()
    }

    /// Filter entries by content category.
    #[must_use]
    pub fn filter_by_category(&self, category: ContentCategory) -> Vec<&ClipboardEntry> {
        self.entries
            .iter()
            .filter(|e| e.content.category() == category)
            .collect()
    }

    /// Merge the text content of the given entry ids (in order) into a single
    /// `ClipboardContent::Text`.  Non-text entries or unknown ids are
    /// silently skipped.  Returns `None` if no text content was found.
    #[must_use]
    pub fn merge_text(&self, ids: &[u64], separator: &str) -> Option<ClipboardContent> {
        let texts: Vec<&str> = ids
            .iter()
            .filter_map(|id| self.get(*id))
            .filter_map(|e| e.content.as_searchable_text())
            .collect();
        if texts.is_empty() {
            return None;
        }
        Some(ClipboardContent::Text(texts.join(separator)))
    }

    // ------------------------------------------------------------------
    // Internal
    // ------------------------------------------------------------------

    /// Evict unpinned entries from the tail until we're within `max_entries`.
    fn evict(&mut self) {
        // Count unpinned entries.
        while self.unpinned_count() > self.max_entries {
            // Remove the oldest unpinned entry (from the back).
            if let Some(pos) = self.entries.iter().rposition(|e| !e.pinned) {
                self.entries.remove(pos);
            } else {
                break;
            }
        }
    }

    /// Number of unpinned entries.
    fn unpinned_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.pinned).count()
    }
}

impl Default for ClipboardHistory {
    fn default() -> Self {
        Self::new()
    }
}
