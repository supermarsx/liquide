//! High-level clipboard manager with history, sensitive policy, category
//! filtering, and persistence.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::entry::{ClipboardContent, ClipboardEntry, ContentCategory};
use crate::history::ClipboardHistory;
use crate::sensitive::SensitiveClipboardPolicy;

/// The main clipboard manager.
pub struct ClipboardManager {
    history: ClipboardHistory,
    /// When `true`, incoming copies are NOT stored (for password fields, etc).
    pub sensitive_mode: bool,
    /// Policy for automatic sensitive-entry handling.
    sensitive_policy: SensitiveClipboardPolicy,
    /// Monotonic counter used for fallback timestamps when `SystemTime` is
    /// unavailable.
    fallback_ts: u64,
}

impl ClipboardManager {
    /// Create a new manager with default history limits.
    #[must_use]
    pub fn new() -> Self {
        Self {
            history: ClipboardHistory::new(),
            sensitive_mode: false,
            sensitive_policy: SensitiveClipboardPolicy::new(),
            fallback_ts: 0,
        }
    }

    /// Create a manager with custom history limits.
    #[must_use]
    pub fn with_limits(max_entries: usize, max_entry_bytes: usize) -> Self {
        Self {
            history: ClipboardHistory::with_limits(max_entries, max_entry_bytes),
            sensitive_mode: false,
            sensitive_policy: SensitiveClipboardPolicy::new(),
            fallback_ts: 0,
        }
    }

    /// Called when new content is copied.
    ///
    /// If `sensitive_mode` is active the copy is silently discarded.
    /// If the source app is in the sensitive policy's exclusion list, the
    /// entry is automatically marked sensitive (will be auto-cleared).
    /// Returns the id of the (possibly deduplicated) entry, or `None` if the
    /// entry was dropped (sensitive mode or over size limit).
    pub fn on_copy(
        &mut self,
        content: ClipboardContent,
        source_app: Option<String>,
    ) -> Option<u64> {
        if self.sensitive_mode {
            tracing::debug!("sensitive mode active, clipboard entry not stored");
            return None;
        }
        let ts = self.now();
        let mut entry = ClipboardEntry::new(0, content, ts, source_app);

        // Check sensitive policy against source app.
        if let Some(app) = &entry.source_app {
            if self.sensitive_policy.should_mark_sensitive(app) {
                entry.sensitive = true;
            }
        }

        self.history.push(entry)
    }

    /// Paste a specific history entry by id.  Increments the usage counter
    /// and returns a reference to the entry's content, or `None` if the id
    /// was not found.
    pub fn paste(&mut self, id: u64) -> Option<&ClipboardContent> {
        if let Some(e) = self.history.get_mut(id) {
            e.times_pasted = e.times_pasted.saturating_add(1);
            // Re-borrow immutably through the history to satisfy the borrow
            // checker.
        }
        self.history.get(id).map(|e| &e.content)
    }

    /// Paste the most recent entry.  Increments the usage counter and
    /// returns the content, or `None` if history is empty.
    pub fn paste_latest(&mut self) -> Option<&ClipboardContent> {
        let id = self.history.latest().map(|e| e.id)?;
        self.paste(id)
    }

    /// Access the full history.
    #[must_use]
    pub fn history(&self) -> &ClipboardHistory {
        &self.history
    }

    /// Mutable access to the full history (for pin/unpin/delete/clear).
    pub fn history_mut(&mut self) -> &mut ClipboardHistory {
        &mut self.history
    }

    /// Set the maximum number of history entries.
    pub fn set_max_history(&mut self, n: usize) {
        self.history.set_max_entries(n);
    }

    /// Filter entries by content category.
    #[must_use]
    pub fn category_filter(&self, category: ContentCategory) -> Vec<&ClipboardEntry> {
        self.history.filter_by_category(category)
    }

    /// Merge text entries by ids into a single text content.
    #[must_use]
    pub fn merge_text(&self, ids: &[u64], separator: &str) -> Option<ClipboardContent> {
        self.history.merge_text(ids, separator)
    }

    /// Access the sensitive clipboard policy.
    #[must_use]
    pub fn sensitive_policy(&self) -> &SensitiveClipboardPolicy {
        &self.sensitive_policy
    }

    /// Mutable access to the sensitive clipboard policy.
    pub fn sensitive_policy_mut(&mut self) -> &mut SensitiveClipboardPolicy {
        &mut self.sensitive_policy
    }

    /// Set the sensitive clipboard policy.
    pub fn set_sensitive_policy(&mut self, policy: SensitiveClipboardPolicy) {
        self.sensitive_policy = policy;
    }

    /// Expire sensitive entries that have exceeded the auto-clear timeout.
    /// Returns the number of entries removed.
    pub fn expire_sensitive_entries(&mut self) -> usize {
        let now = self.now();
        if let Some(cutoff) = self.sensitive_policy.cutoff_timestamp(now) {
            self.history.expire_sensitive(cutoff)
        } else {
            0
        }
    }

    /// Clear all sensitive entries (e.g. on screen lock).
    /// Returns the number of entries removed.
    pub fn on_screen_lock(&mut self) -> usize {
        if self.sensitive_policy.clear_on_lock {
            self.history.clear_sensitive()
        } else {
            0
        }
    }

    /// Save the current history to a writer.  Sensitive entries are
    /// excluded.  Returns the number of entries written.
    pub fn save_history<W: std::io::Write>(
        &self,
        writer: &mut W,
    ) -> Result<usize, crate::persistence::PersistError> {
        let entries: Vec<ClipboardEntry> = self.history.iter().cloned().collect();
        crate::persistence::save_entries(&entries, writer)
    }

    /// Load history from a reader, appending entries to the current
    /// history.  Returns the number of entries loaded.
    pub fn load_history<R: std::io::Read>(
        &mut self,
        reader: &mut R,
    ) -> Result<usize, crate::persistence::PersistError> {
        let entries = crate::persistence::load_entries(reader)?;
        let count = entries.len();
        for entry in entries {
            // Re-push through history to assign fresh ids and honour limits.
            self.history.push(entry);
        }
        Ok(count)
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Current Unix timestamp in seconds.
    fn now(&mut self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_else(|_| {
                self.fallback_ts += 1;
                self.fallback_ts
            })
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}
