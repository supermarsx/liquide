//! Recent-sessions ring buffer — keeps the last N session snapshots.

use crate::state::SessionState;

/// Summary of a stored session, for listing without loading full state.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    /// Unix-epoch microseconds.
    pub timestamp: u64,
    pub window_count: usize,
    pub workspace_count: usize,
    pub theme_id: String,
}

impl SessionSummary {
    fn from_state(state: &SessionState) -> Self {
        Self {
            timestamp: state.timestamp,
            window_count: state.windows.len(),
            workspace_count: state.workspaces.len(),
            theme_id: state.theme_id.clone(),
        }
    }
}

/// A bounded collection of recent session snapshots.
///
/// When the capacity is exceeded the oldest snapshot is dropped.
pub struct RecentSessions {
    sessions: Vec<SessionState>,
    max_count: usize,
}

impl RecentSessions {
    /// Create a new collection that keeps at most `max_count` sessions.
    ///
    /// # Panics
    /// Panics if `max_count` is 0.
    pub fn new(max_count: usize) -> Self {
        assert!(max_count > 0, "max_count must be at least 1");
        Self {
            sessions: Vec::with_capacity(max_count),
            max_count,
        }
    }

    /// Create with the default capacity of 5.
    pub fn default_capacity() -> Self {
        Self::new(5)
    }

    /// Add a session snapshot. If the collection is full the oldest is removed.
    pub fn add(&mut self, state: SessionState) {
        if self.sessions.len() >= self.max_count {
            // Remove the oldest (lowest timestamp). We keep them sorted by
            // timestamp ascending, so the oldest is at the front.
            self.sessions.remove(0);
        }
        self.sessions.push(state);
        // Re-sort so newest is last.
        self.sessions.sort_by_key(|s| s.timestamp);
    }

    /// Return summaries of all stored sessions, newest first.
    pub fn list(&self) -> Vec<SessionSummary> {
        self.sessions
            .iter()
            .rev()
            .map(SessionSummary::from_state)
            .collect()
    }

    /// Get a session by index (0 = newest).
    pub fn get(&self, index: usize) -> Option<&SessionState> {
        if index >= self.sessions.len() {
            return None;
        }
        // Index 0 = newest = last in Vec.
        let vec_index = self.sessions.len() - 1 - index;
        Some(&self.sessions[vec_index])
    }

    /// Number of stored sessions.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Maximum number of sessions this collection will hold.
    pub fn capacity(&self) -> usize {
        self.max_count
    }

    /// Remove all stored sessions.
    pub fn clear(&mut self) {
        self.sessions.clear();
    }

    /// Get the most recent session, if any.
    pub fn latest(&self) -> Option<&SessionState> {
        self.sessions.last()
    }
}
