//! dconf-inspired hierarchical key-value settings backend.
//!
//! Provides a path-based settings store similar to GNOME dconf, with
//! hierarchical key lookups, admin locks, change subscriptions, and
//! a simple text-based persistence format.

use crate::schema::SettingValue;
use std::collections::HashMap;
use std::fmt;

/// A validated dconf-style path (e.g. "/org/liquide/desktop/background").
/// Paths use forward slashes and must start with '/'. Directory paths end
/// with '/', key paths do not.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DconfPath(String);

impl DconfPath {
    /// Create and validate a dconf path. Returns `None` if invalid.
    /// A valid path must start with '/' and contain only alphanumeric chars,
    /// hyphens, underscores, dots, and forward slashes. It must not contain
    /// consecutive slashes.
    pub fn new(s: &str) -> Option<Self> {
        if s.is_empty() || !s.starts_with('/') {
            return None;
        }
        if s.contains("//") {
            return None;
        }
        let valid = s.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '/' || c == '-' || c == '_' || c == '.'
        });
        if !valid {
            return None;
        }
        Some(Self(s.to_string()))
    }

    /// Create a path without validation (for internal/test use).
    pub fn unchecked(s: &str) -> Self {
        Self(s.to_string())
    }

    /// Return the path string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check whether this path is a directory path (ends with '/').
    pub fn is_dir(&self) -> bool {
        self.0.ends_with('/')
    }

    /// Check whether this path is a key path (does not end with '/').
    pub fn is_key(&self) -> bool {
        !self.0.ends_with('/')
    }

    /// Return the parent directory path, or None if this is the root.
    pub fn parent(&self) -> Option<DconfPath> {
        if self.0 == "/" {
            return None;
        }
        let trimmed = self.0.trim_end_matches('/');
        match trimmed.rfind('/') {
            Some(idx) => Some(DconfPath(trimmed[..=idx].to_string())),
            None => None,
        }
    }

    /// Return the last segment of the path (key name or dir name).
    pub fn name(&self) -> &str {
        let trimmed = self.0.trim_end_matches('/');
        match trimmed.rfind('/') {
            Some(idx) => &trimmed[idx + 1..],
            None => trimmed,
        }
    }

    /// Check whether this path is a prefix of another path.
    pub fn is_prefix_of(&self, other: &DconfPath) -> bool {
        if self.0 == "/" {
            return true;
        }
        let prefix = if self.0.ends_with('/') {
            &self.0
        } else {
            // For a non-dir path, we need it to match exactly or as a dir prefix
            return self.0 == other.0;
        };
        other.0.starts_with(prefix)
    }
}

impl fmt::Display for DconfPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type alias for subscription callback ID.
pub type SubscriptionId = u64;

/// A subscription to changes under a path prefix.
struct Subscription {
    id: SubscriptionId,
    prefix: String,
    callback: Box<dyn Fn(&str, &SettingValue) + Send>,
}

/// Hierarchical key-value store with path-based lookups, change
/// notifications, admin locks, and text-file persistence.
pub struct DconfStore {
    /// Current values indexed by full path string.
    values: HashMap<String, SettingValue>,
    /// Default values (used when a key is reset).
    defaults: HashMap<String, SettingValue>,
    /// Admin-locked keys (cannot be changed by set/reset).
    locks: HashMap<String, DconfLock>,
    /// Active subscriptions.
    subscriptions: Vec<Subscription>,
    /// Next subscription ID.
    next_sub_id: SubscriptionId,
}

/// An admin lock preventing user modification of a key.
#[derive(Debug, Clone)]
pub struct DconfLock {
    /// The locked path.
    pub path: String,
    /// Optional forced value. If set, this overrides any user value.
    pub forced_value: Option<SettingValue>,
}

/// Errors from dconf store operations.
#[derive(Debug, Clone)]
pub enum DconfError {
    /// The path is invalid.
    InvalidPath(String),
    /// The key is locked by an administrator.
    Locked(String),
    /// Key not found.
    NotFound(String),
}

impl fmt::Display for DconfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(p) => write!(f, "invalid dconf path: {}", p),
            Self::Locked(p) => write!(f, "key is admin-locked: {}", p),
            Self::NotFound(p) => write!(f, "key not found: {}", p),
        }
    }
}

impl std::error::Error for DconfError {}

impl DconfStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            defaults: HashMap::new(),
            locks: HashMap::new(),
            subscriptions: Vec::new(),
            next_sub_id: 1,
        }
    }

    /// Get the value at a key path. Returns the locked/forced value if present,
    /// then the user value, then the default.
    pub fn get(&self, path: &str) -> Option<&SettingValue> {
        // Check for a forced lock value first.
        if let Some(lock) = self.locks.get(path) {
            if let Some(ref forced) = lock.forced_value {
                return Some(forced);
            }
        }
        self.values.get(path).or_else(|| self.defaults.get(path))
    }

    /// Set a key's value. Fails if the key is admin-locked.
    pub fn set(&mut self, path: &str, value: SettingValue) -> Result<(), DconfError> {
        if self.locks.contains_key(path) {
            return Err(DconfError::Locked(path.to_string()));
        }
        self.values.insert(path.to_string(), value.clone());
        self.notify(path, &value);
        Ok(())
    }

    /// Reset a key to its default value. Fails if the key is admin-locked.
    pub fn reset(&mut self, path: &str) -> Result<(), DconfError> {
        if self.locks.contains_key(path) {
            return Err(DconfError::Locked(path.to_string()));
        }
        self.values.remove(path);
        if let Some(default) = self.defaults.get(path).cloned() {
            self.notify(path, &default);
        }
        Ok(())
    }

    /// Set a default value for a key (used when no user override exists).
    pub fn set_default(&mut self, path: &str, value: SettingValue) {
        self.defaults.insert(path.to_string(), value);
    }

    /// List all keys (not directories) under a directory path.
    /// The `dir_path` should end with '/'.
    pub fn list(&self, dir_path: &str) -> Vec<String> {
        let prefix = if dir_path.ends_with('/') {
            dir_path.to_string()
        } else {
            format!("{}/", dir_path)
        };

        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Collect from both values and defaults.
        for key in self.values.keys().chain(self.defaults.keys()) {
            if key.starts_with(&prefix) && !seen.contains(key.as_str()) {
                results.push(key.clone());
                seen.insert(key.clone());
            }
        }
        results.sort();
        results
    }

    /// List immediate children (one level deep) under a directory path.
    /// Returns both sub-directory names (with trailing '/') and key names.
    pub fn list_children(&self, dir_path: &str) -> Vec<String> {
        let prefix = if dir_path.ends_with('/') {
            dir_path.to_string()
        } else {
            format!("{}/", dir_path)
        };

        let mut children = std::collections::HashSet::new();

        for key in self.values.keys().chain(self.defaults.keys()) {
            if let Some(suffix) = key.strip_prefix(&prefix) {
                if suffix.is_empty() {
                    continue;
                }
                match suffix.find('/') {
                    Some(idx) => {
                        // It's a sub-directory; include with trailing '/'
                        children.insert(format!("{}/", &suffix[..idx]));
                    }
                    None => {
                        children.insert(suffix.to_string());
                    }
                }
            }
        }

        let mut result: Vec<String> = children.into_iter().collect();
        result.sort();
        result
    }

    /// Add an admin lock for a key. Optionally force a specific value.
    pub fn add_lock(&mut self, path: &str, forced_value: Option<SettingValue>) {
        self.locks.insert(path.to_string(), DconfLock {
            path: path.to_string(),
            forced_value,
        });
    }

    /// Remove an admin lock.
    pub fn remove_lock(&mut self, path: &str) {
        self.locks.remove(path);
    }

    /// Check whether a key is admin-locked.
    pub fn is_locked(&self, path: &str) -> bool {
        self.locks.contains_key(path)
    }

    /// Subscribe to changes under a path prefix. Returns a subscription ID
    /// that can be used to unsubscribe.
    pub fn subscribe<F>(&mut self, path_prefix: &str, callback: F) -> SubscriptionId
    where
        F: Fn(&str, &SettingValue) + Send + 'static,
    {
        let id = self.next_sub_id;
        self.next_sub_id += 1;
        self.subscriptions.push(Subscription {
            id,
            prefix: path_prefix.to_string(),
            callback: Box::new(callback),
        });
        id
    }

    /// Unsubscribe by ID.
    pub fn unsubscribe(&mut self, id: SubscriptionId) -> bool {
        let len_before = self.subscriptions.len();
        self.subscriptions.retain(|s| s.id != id);
        self.subscriptions.len() < len_before
    }

    /// Return the number of active subscriptions.
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Return the total number of keys with values (user overrides).
    pub fn key_count(&self) -> usize {
        self.values.len()
    }

    /// Return the total number of default keys.
    pub fn default_count(&self) -> usize {
        self.defaults.len()
    }

    /// Return the number of locked keys.
    pub fn lock_count(&self) -> usize {
        self.locks.len()
    }

    /// Serialize the store to a text format suitable for persistence.
    /// Format: `path=type:value` per line. Only user overrides are saved.
    pub fn save_to_text(&self) -> String {
        let mut lines: Vec<String> = self
            .values
            .iter()
            .map(|(k, v)| format!("{}={}", k, v.serialize()))
            .collect();
        lines.sort();
        if lines.is_empty() {
            "# dconf store (empty)\n".to_string()
        } else {
            lines.join("\n") + "\n"
        }
    }

    /// Load user overrides from the text persistence format.
    /// Does not clear existing values — merges into the store.
    pub fn load_from_text(&mut self, text: &str) -> Result<usize, DconfError> {
        let mut count = 0;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((path, serialized)) = line.split_once('=') {
                let path = path.trim();
                let serialized = serialized.trim();
                if let Some(value) = SettingValue::deserialize(serialized) {
                    if !self.locks.contains_key(path) {
                        self.values.insert(path.to_string(), value);
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    }

    // ── Internal helpers ──────────────────────────────────────────────

    /// Notify all matching subscriptions of a value change.
    fn notify(&self, path: &str, value: &SettingValue) {
        for sub in &self.subscriptions {
            if path.starts_with(&sub.prefix) {
                (sub.callback)(path, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::{AtomicU32, Ordering}};

    #[test]
    fn dconf_path_valid() {
        assert!(DconfPath::new("/org/liquide/desktop/background").is_some());
        assert!(DconfPath::new("/").is_some());
        assert!(DconfPath::new("/org/liquide/").is_some());
        assert!(DconfPath::new("/a-b_c.d").is_some());
    }

    #[test]
    fn dconf_path_invalid() {
        assert!(DconfPath::new("").is_none());
        assert!(DconfPath::new("no-leading-slash").is_none());
        assert!(DconfPath::new("/double//slash").is_none());
        assert!(DconfPath::new("/inv alid").is_none());
    }

    #[test]
    fn dconf_path_is_dir_and_key() {
        assert!(DconfPath::unchecked("/org/liquide/").is_dir());
        assert!(!DconfPath::unchecked("/org/liquide/theme").is_dir());
        assert!(DconfPath::unchecked("/org/liquide/theme").is_key());
    }

    #[test]
    fn dconf_path_parent() {
        let p = DconfPath::unchecked("/org/liquide/desktop/theme");
        let parent = p.parent().unwrap();
        assert_eq!(parent.as_str(), "/org/liquide/desktop/");

        let root = DconfPath::unchecked("/");
        assert!(root.parent().is_none());
    }

    #[test]
    fn dconf_path_name() {
        assert_eq!(DconfPath::unchecked("/org/liquide/theme").name(), "theme");
        assert_eq!(DconfPath::unchecked("/org/liquide/").name(), "liquide");
    }

    #[test]
    fn dconf_path_prefix() {
        let dir = DconfPath::unchecked("/org/liquide/");
        let key = DconfPath::unchecked("/org/liquide/theme");
        let other = DconfPath::unchecked("/org/other/theme");

        assert!(dir.is_prefix_of(&key));
        assert!(!dir.is_prefix_of(&other));
    }

    #[test]
    fn dconf_path_display() {
        let p = DconfPath::unchecked("/org/liquide/desktop");
        assert_eq!(format!("{}", p), "/org/liquide/desktop");
    }

    #[test]
    fn store_get_set() {
        let mut store = DconfStore::new();
        assert!(store.get("/org/liquide/theme").is_none());

        store.set("/org/liquide/theme", SettingValue::String("night".into())).unwrap();
        assert_eq!(
            store.get("/org/liquide/theme"),
            Some(&SettingValue::String("night".into()))
        );
    }

    #[test]
    fn store_defaults() {
        let mut store = DconfStore::new();
        store.set_default("/org/liquide/theme", SettingValue::String("liquid-glass".into()));
        assert_eq!(
            store.get("/org/liquide/theme"),
            Some(&SettingValue::String("liquid-glass".into()))
        );

        // User override wins
        store.set("/org/liquide/theme", SettingValue::String("night".into())).unwrap();
        assert_eq!(
            store.get("/org/liquide/theme"),
            Some(&SettingValue::String("night".into()))
        );
    }

    #[test]
    fn store_reset() {
        let mut store = DconfStore::new();
        store.set_default("/org/liquide/theme", SettingValue::String("liquid-glass".into()));
        store.set("/org/liquide/theme", SettingValue::String("night".into())).unwrap();

        store.reset("/org/liquide/theme").unwrap();
        assert_eq!(
            store.get("/org/liquide/theme"),
            Some(&SettingValue::String("liquid-glass".into()))
        );
    }

    #[test]
    fn store_list() {
        let mut store = DconfStore::new();
        store.set("/org/liquide/desktop/theme", SettingValue::String("night".into())).unwrap();
        store.set("/org/liquide/desktop/wallpaper", SettingValue::String("/bg.png".into())).unwrap();
        store.set("/org/liquide/input/speed", SettingValue::Float(1.5)).unwrap();

        let keys = store.list("/org/liquide/desktop/");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"/org/liquide/desktop/theme".to_string()));
        assert!(keys.contains(&"/org/liquide/desktop/wallpaper".to_string()));
    }

    #[test]
    fn store_list_children() {
        let mut store = DconfStore::new();
        store.set("/org/liquide/desktop/theme", SettingValue::String("night".into())).unwrap();
        store.set("/org/liquide/desktop/wallpaper", SettingValue::String("/bg.png".into())).unwrap();
        store.set("/org/liquide/input/speed", SettingValue::Float(1.5)).unwrap();

        let children = store.list_children("/org/liquide/");
        assert_eq!(children.len(), 2);
        assert!(children.contains(&"desktop/".to_string()));
        assert!(children.contains(&"input/".to_string()));
    }

    #[test]
    fn store_lock_prevents_set() {
        let mut store = DconfStore::new();
        store.add_lock("/org/liquide/theme", None);
        let result = store.set("/org/liquide/theme", SettingValue::String("night".into()));
        assert!(result.is_err());
        match result.unwrap_err() {
            DconfError::Locked(p) => assert_eq!(p, "/org/liquide/theme"),
            other => panic!("expected Locked, got {:?}", other),
        }
    }

    #[test]
    fn store_lock_prevents_reset() {
        let mut store = DconfStore::new();
        store.set("/org/liquide/theme", SettingValue::String("night".into())).unwrap();
        store.add_lock("/org/liquide/theme", None);
        assert!(store.reset("/org/liquide/theme").is_err());
    }

    #[test]
    fn store_lock_forced_value() {
        let mut store = DconfStore::new();
        store.set("/org/liquide/theme", SettingValue::String("night".into())).unwrap();
        store.add_lock(
            "/org/liquide/theme",
            Some(SettingValue::String("corporate".into())),
        );
        // Forced value wins even though user set "night"
        assert_eq!(
            store.get("/org/liquide/theme"),
            Some(&SettingValue::String("corporate".into()))
        );
    }

    #[test]
    fn store_remove_lock() {
        let mut store = DconfStore::new();
        store.add_lock("/org/liquide/theme", None);
        assert!(store.is_locked("/org/liquide/theme"));
        store.remove_lock("/org/liquide/theme");
        assert!(!store.is_locked("/org/liquide/theme"));
        // Should now be settable
        assert!(store.set("/org/liquide/theme", SettingValue::String("night".into())).is_ok());
    }

    #[test]
    fn store_subscribe_and_notify() {
        let mut store = DconfStore::new();
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        store.subscribe("/org/liquide/", move |_path, _value| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        store.set("/org/liquide/theme", SettingValue::String("night".into())).unwrap();
        store.set("/org/liquide/wallpaper", SettingValue::String("/bg.png".into())).unwrap();
        // This should NOT trigger the subscription (different prefix)
        store.set("/org/other/key", SettingValue::Bool(true)).unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn store_unsubscribe() {
        let mut store = DconfStore::new();
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let id = store.subscribe("/org/", move |_path, _value| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        store.set("/org/a", SettingValue::Bool(true)).unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        assert!(store.unsubscribe(id));
        store.set("/org/b", SettingValue::Bool(false)).unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1); // no change
    }

    #[test]
    fn store_subscription_count() {
        let mut store = DconfStore::new();
        assert_eq!(store.subscription_count(), 0);
        let id = store.subscribe("/", |_, _| {});
        assert_eq!(store.subscription_count(), 1);
        store.unsubscribe(id);
        assert_eq!(store.subscription_count(), 0);
    }

    #[test]
    fn store_counts() {
        let mut store = DconfStore::new();
        store.set_default("/a", SettingValue::Bool(true));
        store.set("/b", SettingValue::Bool(false)).unwrap();
        store.add_lock("/c", None);

        assert_eq!(store.default_count(), 1);
        assert_eq!(store.key_count(), 1);
        assert_eq!(store.lock_count(), 1);
    }

    #[test]
    fn store_save_load_roundtrip() {
        let mut store = DconfStore::new();
        store.set("/org/liquide/theme", SettingValue::String("night".into())).unwrap();
        store.set("/org/liquide/font-size", SettingValue::Int(16)).unwrap();
        store.set("/org/liquide/dpi", SettingValue::Float(1.5)).unwrap();
        store.set("/org/liquide/dark-mode", SettingValue::Bool(true)).unwrap();

        let text = store.save_to_text();

        let mut store2 = DconfStore::new();
        let count = store2.load_from_text(&text).unwrap();
        assert_eq!(count, 4);

        assert_eq!(
            store2.get("/org/liquide/theme"),
            Some(&SettingValue::String("night".into()))
        );
        assert_eq!(
            store2.get("/org/liquide/font-size"),
            Some(&SettingValue::Int(16))
        );
        assert_eq!(
            store2.get("/org/liquide/dpi"),
            Some(&SettingValue::Float(1.5))
        );
        assert_eq!(
            store2.get("/org/liquide/dark-mode"),
            Some(&SettingValue::Bool(true))
        );
    }

    #[test]
    fn store_load_skips_locked() {
        let mut store = DconfStore::new();
        store.add_lock("/locked/key", Some(SettingValue::String("forced".into())));

        let text = "/locked/key=string:user-value\n/free/key=string:hello\n";
        let count = store.load_from_text(text).unwrap();
        assert_eq!(count, 1); // only /free/key loaded

        assert_eq!(
            store.get("/locked/key"),
            Some(&SettingValue::String("forced".into()))
        );
        assert_eq!(
            store.get("/free/key"),
            Some(&SettingValue::String("hello".into()))
        );
    }

    #[test]
    fn store_save_empty() {
        let store = DconfStore::new();
        let text = store.save_to_text();
        assert!(text.contains("empty"));
    }

    #[test]
    fn store_load_comments_and_blanks() {
        let mut store = DconfStore::new();
        let text = "# Comment\n\n/key=bool:true\n# Another comment\n";
        let count = store.load_from_text(text).unwrap();
        assert_eq!(count, 1);
        assert_eq!(store.get("/key"), Some(&SettingValue::Bool(true)));
    }

    #[test]
    fn dconf_error_display() {
        let err = DconfError::InvalidPath("/bad path".into());
        assert!(format!("{}", err).contains("/bad path"));

        let err = DconfError::Locked("/locked/key".into());
        assert!(format!("{}", err).contains("locked"));

        let err = DconfError::NotFound("/missing".into());
        assert!(format!("{}", err).contains("/missing"));
    }

    #[test]
    fn store_list_includes_defaults() {
        let mut store = DconfStore::new();
        store.set_default("/org/liquide/default-key", SettingValue::Bool(true));
        store.set("/org/liquide/user-key", SettingValue::Bool(false)).unwrap();

        let keys = store.list("/org/liquide/");
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn root_path_is_prefix_of_everything() {
        let root = DconfPath::unchecked("/");
        assert!(root.is_prefix_of(&DconfPath::unchecked("/org/liquide/theme")));
        assert!(root.is_prefix_of(&DconfPath::unchecked("/anything")));
    }

    #[test]
    fn store_reset_notifies() {
        let mut store = DconfStore::new();
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        store.set_default("/org/theme", SettingValue::String("default".into()));
        store.set("/org/theme", SettingValue::String("custom".into())).unwrap();

        store.subscribe("/org/", move |_path, _value| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        store.reset("/org/theme").unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
