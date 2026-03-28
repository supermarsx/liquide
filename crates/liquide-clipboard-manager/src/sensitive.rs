//! Sensitive clipboard handling — automatic expiry for password-manager
//! entries and screen-lock purge policy.

use crate::entry::ClipboardEntry;

/// Default auto-clear timeout for sensitive entries: 30 seconds.
const DEFAULT_AUTO_CLEAR_TIMEOUT_SECS: u64 = 30;

/// Policy controlling how sensitive clipboard entries are handled.
#[derive(Debug, Clone)]
pub struct SensitiveClipboardPolicy {
    /// How many seconds after creation a sensitive entry is auto-cleared.
    /// Set to `0` to disable timeout-based clearing (entries persist until
    /// manual clear or screen lock).
    pub auto_clear_timeout_secs: u64,

    /// Whether to clear all sensitive entries when the screen is locked.
    pub clear_on_lock: bool,

    /// Application names (case-insensitive exact match) whose clipboard
    /// content is always treated as sensitive.  Typical entries:
    /// "keepassxc", "1password", "bitwarden", "lastpass", "gnome-keyring".
    excluded_apps: Vec<String>,
}

impl SensitiveClipboardPolicy {
    /// Create a policy with sensible defaults (30 s timeout, clear on lock,
    /// no excluded apps yet).
    #[must_use]
    pub fn new() -> Self {
        Self {
            auto_clear_timeout_secs: DEFAULT_AUTO_CLEAR_TIMEOUT_SECS,
            clear_on_lock: true,
            excluded_apps: Vec::new(),
        }
    }

    /// Create a disabled policy — nothing is treated as sensitive.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            auto_clear_timeout_secs: 0,
            clear_on_lock: false,
            excluded_apps: Vec::new(),
        }
    }

    /// Add an application name to the exclusion list (always-sensitive).
    pub fn add_excluded_app(&mut self, app_name: &str) {
        let lower = app_name.to_lowercase();
        if !self.excluded_apps.contains(&lower) {
            self.excluded_apps.push(lower);
        }
    }

    /// Remove an application from the exclusion list.
    pub fn remove_excluded_app(&mut self, app_name: &str) {
        let lower = app_name.to_lowercase();
        self.excluded_apps.retain(|a| a != &lower);
    }

    /// Return the current list of excluded (always-sensitive) apps.
    #[must_use]
    pub fn excluded_apps(&self) -> &[String] {
        &self.excluded_apps
    }

    /// Determine whether content from `source_app` should be marked
    /// sensitive.  Returns `true` if the app name matches any entry in the
    /// exclusion list (case-insensitive).
    #[must_use]
    pub fn should_mark_sensitive(&self, source_app: &str) -> bool {
        let lower = source_app.to_lowercase();
        self.excluded_apps.iter().any(|a| a == &lower)
    }

    /// Check whether a sensitive entry has expired based on `now_secs`
    /// (seconds since epoch).  Non-sensitive entries never expire via this
    /// check.  A timeout of `0` means "never expire by timeout".
    #[must_use]
    pub fn is_expired(&self, entry: &ClipboardEntry, now_secs: u64) -> bool {
        if !entry.sensitive {
            return false;
        }
        if self.auto_clear_timeout_secs == 0 {
            return false;
        }
        now_secs.saturating_sub(entry.timestamp) >= self.auto_clear_timeout_secs
    }

    /// Compute the cutoff timestamp for expiry: entries with
    /// `timestamp < cutoff` are expired.  Returns `None` when timeout-based
    /// clearing is disabled.
    #[must_use]
    pub fn cutoff_timestamp(&self, now_secs: u64) -> Option<u64> {
        if self.auto_clear_timeout_secs == 0 {
            return None;
        }
        Some(now_secs.saturating_sub(self.auto_clear_timeout_secs))
    }
}

impl Default for SensitiveClipboardPolicy {
    fn default() -> Self {
        Self::new()
    }
}
