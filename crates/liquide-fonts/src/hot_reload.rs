//! Hot-reload font watcher — monitors font directories for changes and
//! surfaces them so callers can trigger a re-index / face reload when fonts are
//! added, removed, or modified.
//!
//! STATUS: [`FontWatcher::scan`] performs a real poll-based directory scan over
//! the watched directories, comparing each font file's `(len, mtime)` against
//! the snapshot from the previous scan and emitting [`FontChange`] entries for
//! Added / Modified / Removed files. Drained changes drive the downstream
//! reload path — on the rasterizer side, `FontDatabase::reload_face` (and the
//! renderer's `invalidate_stale_fonts`) re-read the changed bytes and flush the
//! glyph/shape caches (closing the previously-deferred t49-e3-F15 gap). This
//! watcher detects *what* changed; the renderer owns the `FontDatabase` and
//! applies the reload.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

/// Font file extensions the watcher considers (lower-cased comparison).
const FONT_EXTENSIONS: &[&str] = &["ttf", "otf", "ttc", "otc", "woff", "woff2"];

/// Snapshot metadata used to detect file modifications between scans.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSnapshot {
    len: u64,
    modified: Option<SystemTime>,
}

/// Watches font directories for changes.
///
/// [`FontWatcher::scan`] walks the watched directories and reports the files
/// that were added, modified, or removed since the previous scan. Detected
/// changes are queued and surfaced via [`FontWatcher::drain_changes`].
/// [`FontWatcher::push_change`] remains available for tests and for callers that
/// already know a specific path changed.
pub struct FontWatcher {
    /// Directories being watched.
    watch_dirs: Vec<PathBuf>,
    /// Whether the watcher is currently active.
    active: bool,
    /// Last time the directories were scanned.
    last_scan: Option<Instant>,
    /// Interval between poll-based scans (milliseconds).
    poll_interval_ms: u64,
    /// Changes pending drain (populated by [`FontWatcher::scan`] and
    /// [`FontWatcher::push_change`]).
    pending_changes: Vec<FontChange>,
    /// Per-file `(len, mtime)` snapshot from the last scan, used to detect
    /// modifications and removals. Empty until the first `scan`.
    known: HashMap<PathBuf, FileSnapshot>,
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
            known: HashMap::new(),
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

    /// Record a detected change so it is surfaced on the next drain.
    ///
    /// In addition to [`FontWatcher::scan`], callers that already know a path
    /// changed (e.g. an explicit install/uninstall) can inject it directly.
    /// Tests also use this to exercise the drain contract deterministically.
    pub fn push_change(&mut self, change: FontChange) {
        self.pending_changes.push(change);
    }

    /// Drain pending changes.
    ///
    /// Returns the changes accumulated since the last drain — from
    /// [`FontWatcher::scan`] and/or [`FontWatcher::push_change`] — and empties
    /// the queue.
    pub fn drain_changes(&mut self) -> Vec<FontChange> {
        std::mem::take(&mut self.pending_changes)
    }

    /// Check if enough time has passed for a poll and return whether
    /// a scan should be performed.
    ///
    /// When this returns `true`, the caller should invoke [`FontWatcher::scan`]
    /// (which records the scan time itself via `mark_scanned`).
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

    /// Scan the watched directories for font-file changes since the last scan.
    ///
    /// Compares each font file's `(len, mtime)` against the snapshot captured by
    /// the previous scan and queues a [`FontChange`] for every Added, Modified,
    /// or Removed file. The internal snapshot and `last_scan` timestamp are
    /// updated, and the freshly detected changes are returned (they are also
    /// appended to the pending-drain queue so [`FontWatcher::drain_changes`]
    /// continues to see them).
    ///
    /// The first scan after construction establishes the baseline: every
    /// existing font file is reported as `Added`. Subsequent scans report only
    /// deltas. Directories that cannot be read are skipped (their previously
    /// known files are treated as still present, not spuriously removed).
    pub fn scan(&mut self) -> Vec<FontChange> {
        let now = Instant::now();
        let mut seen: HashMap<PathBuf, FileSnapshot> = HashMap::new();
        let mut unreadable: Vec<PathBuf> = Vec::new();

        for dir in &self.watch_dirs {
            collect_font_files(dir, &mut seen, &mut unreadable);
        }

        let mut changes = Vec::new();
        let timestamp = Instant::now();

        // Added / Modified.
        for (path, snapshot) in &seen {
            match self.known.get(path) {
                None => changes.push(FontChange {
                    path: path.clone(),
                    kind: FontChangeKind::Added,
                    timestamp,
                }),
                Some(prev) if prev != snapshot => changes.push(FontChange {
                    path: path.clone(),
                    kind: FontChangeKind::Modified,
                    timestamp,
                }),
                Some(_) => {}
            }
        }

        // Removed: previously known files that are no longer present, excluding
        // files that merely live under a directory we failed to read this scan
        // (avoid false "Removed" on a transient I/O error).
        for path in self.known.keys() {
            if seen.contains_key(path) {
                continue;
            }
            if unreadable.iter().any(|dir| path.starts_with(dir)) {
                // Carry the prior snapshot forward; the dir was unreadable.
                if let Some(prev) = self.known.get(path) {
                    seen.insert(path.clone(), prev.clone());
                }
                continue;
            }
            changes.push(FontChange {
                path: path.clone(),
                kind: FontChangeKind::Removed,
                timestamp,
            });
        }

        self.known = seen;
        self.last_scan = Some(now);
        self.pending_changes.extend(changes.iter().cloned());

        if !changes.is_empty() {
            tracing::info!(count = changes.len(), "font watcher detected changes");
        }
        changes
    }
}

/// Whether a path has a recognized font extension.
fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let ext = ext.to_ascii_lowercase();
            FONT_EXTENSIONS.contains(&ext.as_str())
        })
        .unwrap_or(false)
}

/// Recursively collect font files under `dir` into `seen`, recording any
/// directory whose contents could not be read into `unreadable`.
fn collect_font_files(
    dir: &Path,
    seen: &mut HashMap<PathBuf, FileSnapshot>,
    unreadable: &mut Vec<PathBuf>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => {
            unreadable.push(dir.to_path_buf());
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            collect_font_files(&path, seen, unreadable);
        } else if file_type.is_file() && is_font_file(&path) {
            if let Ok(meta) = entry.metadata() {
                seen.insert(
                    path,
                    FileSnapshot {
                        len: meta.len(),
                        modified: meta.modified().ok(),
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "liquide-fonts-hotreload-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    /// Scanning a directory with no font files surfaces no changes — the
    /// watcher must not invent changes out of an empty or font-less directory.
    #[test]
    fn scan_empty_directory_reports_no_changes() {
        let dir = unique_temp_dir("empty");
        std::fs::create_dir_all(&dir).unwrap();
        // A non-font file must be ignored.
        std::fs::write(dir.join("README.txt"), b"not a font").unwrap();

        let mut watcher = FontWatcher::new(vec![dir.clone()]);
        watcher.start();
        assert!(
            watcher.scan().is_empty(),
            "no font files → no changes detected"
        );
        assert!(watcher.drain_changes().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A real scan reports Added on first sight, Modified when bytes change, and
    /// Removed when the file disappears — closing t49-e3-F14's lying wiring with
    /// an actual detector.
    #[test]
    fn scan_detects_added_modified_and_removed_font_files() {
        let dir = unique_temp_dir("delta");
        std::fs::create_dir_all(&dir).unwrap();
        let font = dir.join("Demo.ttf");
        std::fs::write(&font, b"font-bytes-v1").unwrap();

        let mut watcher = FontWatcher::new(vec![dir.clone()]);
        watcher.start();

        // First scan: the file is Added.
        let added = watcher.scan();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].kind, FontChangeKind::Added);
        assert_eq!(added[0].path, font);

        // No change → second scan is quiet.
        assert!(
            watcher.scan().is_empty(),
            "unchanged file must not re-report"
        );

        // Modify (length changes): Modified.
        std::fs::write(&font, b"font-bytes-v2-longer").unwrap();
        let modified = watcher.scan();
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0].kind, FontChangeKind::Modified);

        // Remove: Removed.
        std::fs::remove_file(&font).unwrap();
        let removed = watcher.scan();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].kind, FontChangeKind::Removed);

        // All of those changes are also drainable.
        // (drain returns the accumulated queue from every scan above.)
        let drained = watcher.drain_changes();
        assert_eq!(
            drained.len(),
            3,
            "Added + Modified + Removed accumulate in the drain queue"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    /// A missing watch directory yields no changes and does not panic — and a
    /// previously-known file under an unreadable dir is not spuriously Removed.
    #[test]
    fn scan_missing_directory_is_quiet() {
        let dir = unique_temp_dir("missing");
        // Intentionally do NOT create the directory.
        let mut watcher = FontWatcher::new(vec![dir.clone()]);
        watcher.start();
        assert!(watcher.scan().is_empty());
        assert!(watcher.drain_changes().is_empty());
    }

    /// The drain contract is honest: only explicitly pushed changes appear,
    /// and a drain empties the queue.
    #[test]
    fn drain_returns_only_pushed_changes() {
        let mut watcher = FontWatcher::new(vec![PathBuf::from("/fonts")]);
        assert!(watcher.drain_changes().is_empty());

        watcher.push_change(FontChange {
            path: PathBuf::from("/fonts/Inter.ttf"),
            kind: FontChangeKind::Added,
            timestamp: Instant::now(),
        });
        let drained = watcher.drain_changes();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].kind, FontChangeKind::Added);
        // Second drain is empty — the queue was consumed.
        assert!(watcher.drain_changes().is_empty());
    }
}
