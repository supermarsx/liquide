use std::collections::HashMap;

/// Tracks granted authorizations with optional time-limited keep-alive.
///
/// Each grant is recorded as `(action_id, expires_at)` where `expires_at`
/// is an absolute timestamp (seconds since epoch). A grant is valid if the
/// current time is less than or equal to the expiry time.
#[derive(Debug, Default)]
pub struct AuthorizationStore {
    /// Map from action ID to expiry timestamp (seconds since epoch).
    grants: HashMap<String, u64>,
}

impl AuthorizationStore {
    /// Create a new, empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            grants: HashMap::new(),
        }
    }

    /// Record a granted authorization that expires at the given timestamp.
    ///
    /// If a grant already exists for this action, it is replaced (the new
    /// expiry wins, even if shorter).
    pub fn grant(&mut self, action_id: String, until: u64) {
        self.grants.insert(action_id, until);
    }

    /// Check whether a previously granted authorization is still valid.
    ///
    /// Returns `true` if there is a grant for `action_id` whose expiry
    /// is strictly greater than `now`.
    #[must_use]
    pub fn check(&self, action_id: &str, now: u64) -> bool {
        self.grants
            .get(action_id)
            .is_some_and(|&expiry| now < expiry)
    }

    /// Revoke a specific grant.
    pub fn revoke(&mut self, action_id: &str) {
        self.grants.remove(action_id);
    }

    /// Revoke all grants.
    pub fn revoke_all(&mut self) {
        self.grants.clear();
    }

    /// Remove all grants that have expired as of `now`.
    pub fn cleanup_expired(&mut self, now: u64) {
        self.grants.retain(|_, &mut expiry| now < expiry);
    }

    /// Return the number of active (possibly expired) grants in the store.
    #[must_use]
    pub fn len(&self) -> usize {
        self.grants.len()
    }

    /// Return true if the store contains no grants.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.grants.is_empty()
    }

    /// Return the expiry timestamp for a given action, if any grant exists.
    #[must_use]
    pub fn expiry(&self, action_id: &str) -> Option<u64> {
        self.grants.get(action_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_and_check() {
        let mut store = AuthorizationStore::new();
        store.grant("org.liquide.package.install".to_string(), 1000);
        assert!(store.check("org.liquide.package.install", 500));
        assert!(store.check("org.liquide.package.install", 999));
        assert!(!store.check("org.liquide.package.install", 1000));
        assert!(!store.check("org.liquide.package.install", 1001));
    }

    #[test]
    fn check_nonexistent() {
        let store = AuthorizationStore::new();
        assert!(!store.check("org.liquide.anything", 0));
    }

    #[test]
    fn revoke() {
        let mut store = AuthorizationStore::new();
        store.grant("org.liquide.test".to_string(), 9999);
        assert!(store.check("org.liquide.test", 0));
        store.revoke("org.liquide.test");
        assert!(!store.check("org.liquide.test", 0));
    }

    #[test]
    fn revoke_all() {
        let mut store = AuthorizationStore::new();
        store.grant("a".to_string(), 100);
        store.grant("b".to_string(), 200);
        store.grant("c".to_string(), 300);
        assert_eq!(store.len(), 3);
        store.revoke_all();
        assert!(store.is_empty());
        assert!(!store.check("a", 0));
    }

    #[test]
    fn cleanup_expired() {
        let mut store = AuthorizationStore::new();
        store.grant("expired1".to_string(), 50);
        store.grant("expired2".to_string(), 100);
        store.grant("still_valid".to_string(), 500);
        store.grant("also_valid".to_string(), 1000);

        store.cleanup_expired(200);
        assert_eq!(store.len(), 2);
        assert!(!store.check("expired1", 200));
        assert!(!store.check("expired2", 200));
        assert!(store.check("still_valid", 200));
        assert!(store.check("also_valid", 200));
    }

    #[test]
    fn grant_replaces_existing() {
        let mut store = AuthorizationStore::new();
        store.grant("org.liquide.test".to_string(), 1000);
        assert!(store.check("org.liquide.test", 500));

        // Replace with shorter expiry
        store.grant("org.liquide.test".to_string(), 200);
        assert!(!store.check("org.liquide.test", 500));
        assert!(store.check("org.liquide.test", 100));
    }

    #[test]
    fn expiry() {
        let mut store = AuthorizationStore::new();
        assert_eq!(store.expiry("x"), None);
        store.grant("x".to_string(), 42);
        assert_eq!(store.expiry("x"), Some(42));
    }

    #[test]
    fn empty_store() {
        let store = AuthorizationStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }
}
