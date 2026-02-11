//! Update checking and batch updates.

use crate::package::Version;

/// Summary of a pending update.
#[derive(Debug, Clone)]
pub struct PendingUpdate {
    pub package_id: String,
    pub package_name: String,
    pub current_version: Version,
    pub new_version: Version,
    pub download_size: u64,
    pub changelog: String,
}

impl PendingUpdate {
    /// Human-readable version change.
    #[must_use]
    pub fn version_change(&self) -> String {
        format!("{} -> {}", self.current_version, self.new_version)
    }
}

/// Update manager state.
pub struct UpdateManager {
    pending: Vec<PendingUpdate>,
    last_check: u64,
    auto_check: bool,
}

impl UpdateManager {
    #[must_use]
    pub fn new(auto_check: bool) -> Self {
        Self { pending: Vec::new(), last_check: 0, auto_check }
    }

    /// Set the list of pending updates.
    pub fn set_pending(&mut self, updates: Vec<PendingUpdate>) {
        self.pending = updates;
    }

    /// Get all pending updates.
    #[must_use]
    pub fn pending(&self) -> &[PendingUpdate] { &self.pending }

    /// Number of pending updates.
    #[must_use]
    pub fn count(&self) -> usize { self.pending.len() }

    /// Total download size for all updates.
    #[must_use]
    pub fn total_download_size(&self) -> u64 {
        self.pending.iter().map(|u| u.download_size).sum()
    }

    /// Get update IDs for batch install.
    #[must_use]
    pub fn update_ids(&self) -> Vec<&str> {
        self.pending.iter().map(|u| u.package_id.as_str()).collect()
    }

    /// Remove a pending update by ID (e.g. after installing).
    pub fn remove(&mut self, package_id: &str) {
        self.pending.retain(|u| u.package_id != package_id);
    }

    /// Record that a check was performed.
    pub fn mark_checked(&mut self, timestamp: u64) {
        self.last_check = timestamp;
    }

    /// Last check timestamp.
    #[must_use]
    pub fn last_check(&self) -> u64 { self.last_check }

    /// Whether auto-checking is enabled.
    #[must_use]
    pub fn auto_check(&self) -> bool { self.auto_check }

    /// Clear all pending updates.
    pub fn clear(&mut self) { self.pending.clear(); }
}
