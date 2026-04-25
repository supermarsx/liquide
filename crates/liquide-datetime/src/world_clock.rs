use crate::datetime::DateTime;
use crate::timezone_db::TimeZoneDatabase;

/// A single entry in the world clock panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldClockEntry {
    /// User-facing label, e.g. "Tokyo Office".
    pub label: String,
    /// IANA timezone ID, e.g. "Asia/Tokyo".
    pub timezone_id: String,
}

impl WorldClockEntry {
    /// Create a new world clock entry.
    pub fn new(label: impl Into<String>, timezone_id: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            timezone_id: timezone_id.into(),
        }
    }
}

/// Multiple-timezone display (world clock widget).
#[derive(Debug, Clone)]
pub struct WorldClock {
    /// Ordered list of world clock entries.
    pub clocks: Vec<WorldClockEntry>,
}

impl WorldClock {
    /// Create an empty world clock.
    pub fn new() -> Self {
        Self { clocks: Vec::new() }
    }

    /// Add a clock entry. Returns the index of the new entry.
    pub fn add_clock(&mut self, label: impl Into<String>, timezone_id: impl Into<String>) -> usize {
        self.clocks.push(WorldClockEntry::new(label, timezone_id));
        self.clocks.len() - 1
    }

    /// Remove a clock entry by index. Returns the removed entry, or `None` if
    /// the index is out of bounds.
    pub fn remove_clock(&mut self, index: usize) -> Option<WorldClockEntry> {
        if index < self.clocks.len() {
            Some(self.clocks.remove(index))
        } else {
            None
        }
    }

    /// Move a clock entry from one position to another.
    /// Does nothing if either index is out of bounds.
    pub fn reorder(&mut self, from: usize, to: usize) {
        if from >= self.clocks.len() || to >= self.clocks.len() {
            return;
        }
        let entry = self.clocks.remove(from);
        self.clocks.insert(to, entry);
    }

    /// Given a UTC reference time, compute the local time for each clock entry.
    ///
    /// Returns `(label, local_datetime)` pairs. Unknown timezone IDs use
    /// UTC offset 0 as a fallback.
    pub fn all_times(&self, now_utc: &DateTime) -> Vec<(String, DateTime)> {
        let db = TimeZoneDatabase::new();
        self.clocks
            .iter()
            .map(|entry| {
                let offset = db
                    .find(&entry.timezone_id)
                    .map(|tz| tz.utc_offset_minutes)
                    .unwrap_or(0);
                let local = now_utc.with_offset_minutes(offset);
                (entry.label.clone(), local)
            })
            .collect()
    }

    /// Number of clock entries.
    pub fn len(&self) -> usize {
        self.clocks.len()
    }

    /// Returns true if there are no clock entries.
    pub fn is_empty(&self) -> bool {
        self.clocks.is_empty()
    }
}

impl Default for WorldClock {
    fn default() -> Self {
        Self::new()
    }
}
