//! File watcher for hot-reloading themes

use crate::error::Result;
use crate::stylesheet::StyleSheet;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

/// Debounce window for hot-reloads. Most editors emit a burst of
/// modify/create/rename events when saving a single file (particularly
/// atomic-write editors like vim and VS Code). Coalesce them into one
/// stylesheet reload.
const RELOAD_DEBOUNCE: Duration = Duration::from_millis(100);

/// Callback type for theme updates
pub type ThemeUpdateCallback = Arc<dyn Fn(StyleSheet) + Send + Sync>;

/// Theme file watcher for hot-reloading
pub struct ThemeWatcher {
    watcher: Option<RecommendedWatcher>,
    paths: Vec<PathBuf>,
    callback: Option<ThemeUpdateCallback>,
}

impl ThemeWatcher {
    /// Create a new theme watcher
    pub fn new() -> Self {
        Self {
            watcher: None,
            paths: Vec::new(),
            callback: None,
        }
    }

    /// Watch a theme file or directory
    pub fn watch<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref().to_path_buf();

        if !self.paths.contains(&path) {
            self.paths.push(path.clone());
        }

        // If watcher is already started, add the path
        if let Some(ref mut watcher) = self.watcher {
            watcher.watch(&path, RecursiveMode::Recursive)?;
            info!("Now watching: {}", path.display());
        }

        Ok(())
    }

    /// Set callback for theme updates
    pub fn on_update<F>(&mut self, callback: F)
    where
        F: Fn(StyleSheet) + Send + Sync + 'static,
    {
        self.callback = Some(Arc::new(callback));
    }

    /// Start watching (blocking)
    pub fn start(&mut self) -> Result<()> {
        let callback = match self.callback.clone() {
            Some(cb) => cb,
            None => {
                error!("No callback set for theme updates");
                return Ok(());
            }
        };

        let (tx, rx) = channel();
        let paths = self.paths.clone();

        // Create watcher
        let mut watcher =
            notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        if let Err(e) = tx.send(event) {
                            error!("Failed to send event: {}", e);
                        }
                    }
                    Err(e) => error!("Watch error: {:?}", e),
                }
            })?;

        // Watch all paths
        for path in &self.paths {
            watcher.watch(path, RecursiveMode::Recursive)?;
            info!("Watching theme file: {}", path.display());
        }

        self.watcher = Some(watcher);

        // Spawn event handler
        thread::spawn(move || {
            Self::handle_events(rx, callback, paths);
        });

        Ok(())
    }

    fn handle_events(rx: Receiver<Event>, callback: ThemeUpdateCallback, paths: Vec<PathBuf>) {
        loop {
            // Block for the first event in a burst...
            let ev = match rx.recv() {
                Ok(e) => e,
                Err(_) => return, // sender hung up
            };
            debug!("File system event: {:?}", ev);
            let mut pending = event_paths_match(&ev, &paths);

            // ...then drain everything arriving within the debounce window
            // so atomic-save bursts (vim, VS Code) produce one reload.
            let deadline = Instant::now() + RELOAD_DEBOUNCE;
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match rx.recv_timeout(remaining) {
                    Ok(e) => {
                        debug!("Coalesced event: {:?}", e);
                        pending |= event_paths_match(&e, &paths);
                    }
                    Err(_) => break, // timeout or disconnect
                }
            }

            if !pending {
                continue;
            }

            match Self::load_themes(&paths) {
                Ok(stylesheet) => {
                    info!("Theme reloaded successfully");
                    callback(stylesheet);
                }
                Err(e) => {
                    error!("Failed to reload theme: {}", e);
                }
            }
        }
    }

    fn load_themes(paths: &[PathBuf]) -> Result<StyleSheet> {
        StyleSheet::load_paths_with_imports(paths)
    }
}

/// Does any path in an `Event` fall under one of the watched roots?
fn event_paths_match(event: &Event, watched: &[PathBuf]) -> bool {
    for p in &event.paths {
        if watched.iter().any(|w| p.starts_with(w)) {
            return true;
        }
    }
    false
}

impl Drop for ThemeWatcher {
    /// Explicitly unwatch every registered path on drop so the underlying
    /// `notify` thread stops firing events against a callback whose owning
    /// code has already gone away.
    fn drop(&mut self) {
        if let Some(ref mut watcher) = self.watcher {
            for path in &self.paths {
                if let Err(e) = watcher.unwatch(path) {
                    debug!("unwatch on drop failed for {}: {e}", path.display());
                }
            }
        }
    }
}

impl Default for ThemeWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;
    use tempfile::tempdir;

    #[test]
    fn test_watcher_creation() {
        let watcher = ThemeWatcher::new();
        assert!(watcher.paths.is_empty());
    }

    #[test]
    fn test_watch_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut watcher = ThemeWatcher::new();

        watcher.watch(temp_file.path()).unwrap();
        assert_eq!(watcher.paths.len(), 1);
    }

    #[test]
    fn test_load_themes_recurses_nested_directories_in_sorted_order() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir_all(&nested).unwrap();

        fs::write(dir.path().join("a.css"), "button { color: #ff0000; }").unwrap();
        fs::write(nested.join("b.css"), "button { color: #0000ff; }").unwrap();

        let sheet = ThemeWatcher::load_themes(&[dir.path().to_path_buf()]).unwrap();
        let styles = sheet.compute_styles("button", &[], None, &[]);
        let color = styles.get("color").unwrap().as_color().unwrap();

        assert_eq!(sheet.rule_count(), 2);
        assert_eq!(color.b, 255);
    }
}
