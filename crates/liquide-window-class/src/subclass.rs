use std::collections::HashMap;

/// A single entry in a window's subclass chain.
///
/// Subclassing replaces the window procedure on a per-instance basis.
/// Multiple subclass handlers can be stacked; the most recently installed
/// handler is called first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubclassEntry {
    /// The window being subclassed.
    pub window_id: u64,
    /// The replacement handler function identifier.
    pub handler_id: u64,
    /// Opaque reference data passed to the handler on every call.
    pub ref_data: u64,
}

/// Manages per-window subclass handler chains.
///
/// Each window can have zero or more subclass entries.  Entries are stored in
/// installation order; the last-installed handler is conceptually called first
/// (LIFO).
pub struct SubclassManager {
    chains: HashMap<u64, Vec<SubclassEntry>>,
}

impl SubclassManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self {
            chains: HashMap::new(),
        }
    }

    /// Install a new subclass handler for `window_id`.
    ///
    /// If a handler with the same `handler_id` is already installed on this
    /// window, the existing entry's `ref_data` is updated instead of adding a
    /// duplicate.
    pub fn install(&mut self, window_id: u64, handler_id: u64, ref_data: u64) {
        let chain = self.chains.entry(window_id).or_default();
        // Update existing entry with same handler_id
        for entry in chain.iter_mut() {
            if entry.handler_id == handler_id {
                entry.ref_data = ref_data;
                return;
            }
        }
        chain.push(SubclassEntry {
            window_id,
            handler_id,
            ref_data,
        });
    }

    /// Remove a subclass handler identified by `handler_id` from `window_id`.
    ///
    /// Returns `true` if the entry was found and removed.
    pub fn remove(&mut self, window_id: u64, handler_id: u64) -> bool {
        if let Some(chain) = self.chains.get_mut(&window_id) {
            let before = chain.len();
            chain.retain(|e| e.handler_id != handler_id);
            let removed = chain.len() < before;
            if chain.is_empty() {
                self.chains.remove(&window_id);
            }
            removed
        } else {
            false
        }
    }

    /// Returns the subclass chain for a window in call order (last installed
    /// first — i.e., reversed from installation order).
    pub fn get_chain(&self, window_id: u64) -> Vec<SubclassEntry> {
        match self.chains.get(&window_id) {
            Some(chain) => {
                let mut out = chain.clone();
                out.reverse();
                out
            }
            None => Vec::new(),
        }
    }

    /// Remove all subclass entries for a window (called on window destruction).
    pub fn remove_all(&mut self, window_id: u64) {
        self.chains.remove(&window_id);
    }

    /// Returns the number of subclass entries for a given window.
    pub fn chain_len(&self, window_id: u64) -> usize {
        self.chains.get(&window_id).map_or(0, |c| c.len())
    }

    /// Returns `true` if `handler_id` is installed on `window_id`.
    pub fn is_installed(&self, window_id: u64, handler_id: u64) -> bool {
        self.chains
            .get(&window_id)
            .map_or(false, |c| c.iter().any(|e| e.handler_id == handler_id))
    }
}

impl Default for SubclassManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_and_get_chain() {
        let mut mgr = SubclassManager::new();
        mgr.install(1, 100, 0);
        mgr.install(1, 200, 0);
        let chain = mgr.get_chain(1);
        // Last installed first
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].handler_id, 200);
        assert_eq!(chain[1].handler_id, 100);
    }

    #[test]
    fn remove_handler() {
        let mut mgr = SubclassManager::new();
        mgr.install(1, 100, 0);
        mgr.install(1, 200, 0);
        assert!(mgr.remove(1, 100));
        let chain = mgr.get_chain(1);
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].handler_id, 200);
    }

    #[test]
    fn remove_nonexistent() {
        let mut mgr = SubclassManager::new();
        assert!(!mgr.remove(1, 999));
    }

    #[test]
    fn duplicate_handler_updates_ref_data() {
        let mut mgr = SubclassManager::new();
        mgr.install(1, 100, 42);
        mgr.install(1, 100, 99);
        let chain = mgr.get_chain(1);
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].ref_data, 99);
    }

    #[test]
    fn remove_all() {
        let mut mgr = SubclassManager::new();
        mgr.install(1, 100, 0);
        mgr.install(1, 200, 0);
        mgr.remove_all(1);
        assert_eq!(mgr.get_chain(1).len(), 0);
    }

    #[test]
    fn chain_len() {
        let mut mgr = SubclassManager::new();
        assert_eq!(mgr.chain_len(1), 0);
        mgr.install(1, 100, 0);
        assert_eq!(mgr.chain_len(1), 1);
    }

    #[test]
    fn is_installed() {
        let mut mgr = SubclassManager::new();
        mgr.install(1, 100, 0);
        assert!(mgr.is_installed(1, 100));
        assert!(!mgr.is_installed(1, 200));
        assert!(!mgr.is_installed(2, 100));
    }

    #[test]
    fn separate_windows() {
        let mut mgr = SubclassManager::new();
        mgr.install(1, 100, 0);
        mgr.install(2, 200, 0);
        assert_eq!(mgr.chain_len(1), 1);
        assert_eq!(mgr.chain_len(2), 1);
        assert_eq!(mgr.get_chain(1)[0].handler_id, 100);
        assert_eq!(mgr.get_chain(2)[0].handler_id, 200);
    }
}
