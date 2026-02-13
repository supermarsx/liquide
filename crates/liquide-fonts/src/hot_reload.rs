//! Hot-reload font watcher — monitors font directories for changes
//! and triggers re-indexing when fonts are added, removed, or modified.

use std::path::PathBuf;
use std::time::Instant;

/// Watches font directories for changes.
pub struct FontWatcher {
    /// Directories being watched.
    watch_dirs: Vec<PathBuf>,
    /// Whether the watcher is currently active.
    active: bool,
    /// Last time the directories were scanned.
    last_scan: Option<Instant>,
    /// Interval between poll-based scans (milliseconds).
    poll_interval_ms: u64,
    /// Callbacks pending from detected changes.
    pending_changes: Vec<FontChange>,
}

/// A change detected in a watched font directory.
#[derive(Debug, Clone)]
pub struct FontChange {
    /// Path of the changed font file.
    pub path: PathBuf,
    /// Kind of change.
    pub kind: FontChangeKind,
    /// Timestamp of the change.
    pub timestamp: Instant,
}

/// Kind of font file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontChangeKind {
    /// A new font file was added.
    Added,
    /// An existing font file was modified.
    Modified,
    /// A font file was removed.
    Removed,
}

impl FontWatcher {
    /// Create a new font watcher for the given directories.
    #[must_use]
    pub fn new(watch_dirs: Vec<PathBuf>) -> Self {
        Self {
            watch_dirs,
            active: false,
            last_scan: None,
            poll_interval_ms: 2000,
            pending_changes: Vec::new(),
        }
    }

    /// Start watching.
    pub fn start(&mut self) {
        self.active = true;
        self.last_scan = None;
        tracing::info!(
            dirs = ?self.watch_dirs,
            "font watcher started"
        );
    }

    /// Stop watching.
    pub fn stop(&mut self) {
        self.active = false;
        tracing::info!("font watcher stopped");
    }

    /// Whether the watcher is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the watched directories.
    #[must_use]
    pub fn watch_dirs(&self) -> &[PathBuf] {
        &self.watch_dirs
    }

    /// Add a directory to watch.
    pub fn add_dir(&mut self, dir: PathBuf) {
        if !self.watch_dirs.contains(&dir) {
            self.watch_dirs.push(dir);
        }
    }

    /// Remove a directory from the watch list.
    pub fn remove_dir(&mut self, dir: &PathBuf) {
        self.watch_dirs.retain(|d| d != dir);
    }

    /// Set the poll interval.
    pub fn set_poll_interval(&mut self, ms: u64) {
        self.poll_interval_ms = ms;
    }

    /// Drain pending changes.
    pub fn drain_changes(&mut self) -> Vec<FontChange> {
        std::mem::take(&mut self.pending_changes)
    }

    /// Check if enough time has passed for a poll and return whether
    /// a scan should be performed.
    #[must_use]
    pub fn should_poll(&self) -> bool {
        if !self.active {
            return false;
        }
        match self.last_scan {
            None => true,
            Some(last) => last.elapsed().as_millis() as u64 >= self.poll_interval_ms,
        }
    }

    /// Record that a scan was performed.
    pub fn mark_scanned(&mut self) {
        self.last_scan = Some(Instant::now());
    }
}
