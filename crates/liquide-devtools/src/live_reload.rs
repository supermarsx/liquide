//! Live reload watcher for component templates and CSS files.
//!
//! Watches `assets/templates/` and `assets/themes/components/` for changes,
//! debounces rapid saves, and emits [`ReloadEvent`]s that the compositor
//! uses to trigger an immediate re-render of affected components.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// The kind of file that changed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReloadTarget {
    /// An HTML template file changed (e.g., `dock.html`).
    Template(String),
    /// A component CSS file changed (e.g., `dock.css`).
    ComponentCss(String),
    /// A theme CSS file changed (e.g., `night.css`).
    ThemeCss(String),
}

/// An event emitted when watched files change.
#[derive(Debug, Clone)]
pub struct ReloadEvent {
    /// Which files changed.
    pub targets: Vec<ReloadTarget>,
    /// Monotonic timestamp of the event.
    pub timestamp: Instant,
}

/// Configuration for the live reload watcher.
#[derive(Debug, Clone)]
pub struct LiveReloadConfig {
    /// Root path of the project (parent of `assets/`).
    pub project_root: PathBuf,
    /// Debounce window for rapid saves (default: 100ms).
    pub debounce_ms: u64,
    /// Additional paths to watch.
    pub extra_paths: Vec<PathBuf>,
}

impl Default for LiveReloadConfig {
    fn default() -> Self {
        Self {
            project_root: PathBuf::from("."),
            debounce_ms: 100,
            extra_paths: Vec::new(),
        }
    }
}

/// Watches template and CSS files for changes and emits reload events.
///
/// # Usage
///
/// ```ignore
/// let mut watcher = LiveReloadWatcher::new(config);
/// let rx = watcher.start().expect("failed to start watcher");
///
/// // In your event loop:
/// while let Ok(event) = rx.try_recv() {
///     for target in &event.targets {
///         match target {
///             ReloadTarget::Template(name) => reload_template(name),
///             ReloadTarget::ComponentCss(name) => reload_css(name),
///             ReloadTarget::ThemeCss(name) => reload_theme(name),
///         }
///     }
/// }
/// ```
pub struct LiveReloadWatcher {
    config: LiveReloadConfig,
    _watcher: Option<RecommendedWatcher>,
    /// Internal sender to keep the fs notification channel alive.
    event_tx: Option<Sender<Event>>,
}

impl LiveReloadWatcher {
    /// Create a new watcher with the given configuration.
    pub fn new(config: LiveReloadConfig) -> Self {
        Self {
            config,
            _watcher: None,
            event_tx: None,
        }
    }

    /// Start watching. Returns a receiver for [`ReloadEvent`]s.
    ///
    /// This spawns a background thread that debounces file system notifications
    /// and coalesces them into batched reload events.
    pub fn start(&mut self) -> Result<Receiver<ReloadEvent>, notify::Error> {
        let (consumer_tx, consumer_rx) = mpsc::channel::<ReloadEvent>();
        let (fs_tx, fs_rx) = mpsc::channel::<Event>();

        let debounce_ms = self.config.debounce_ms;
        let project_root = self.config.project_root.clone();

        // Spawn the debounce thread.
        thread::Builder::new()
            .name("devtools-live-reload".into())
            .spawn(move || {
                Self::debounce_loop(fs_rx, consumer_tx, debounce_ms, &project_root);
            })
            .expect("failed to spawn live-reload thread");

        // Create the notify watcher.
        let watcher_tx = fs_tx.clone();
        let mut watcher =
            notify::recommended_watcher(move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    let _ = watcher_tx.send(event);
                }
                Err(e) => {
                    error!("file watcher error: {:?}", e);
                }
            })?;

        // Watch template and CSS directories.
        let templates_dir = self.config.project_root.join("assets").join("templates");
        let components_css_dir = self
            .config
            .project_root
            .join("assets")
            .join("themes")
            .join("components");
        let themes_dir = self.config.project_root.join("assets").join("themes");

        for dir in [&templates_dir, &components_css_dir, &themes_dir] {
            if dir.exists() {
                watcher.watch(dir, RecursiveMode::Recursive)?;
                info!("live-reload watching: {}", dir.display());
            } else {
                warn!("live-reload path does not exist: {}", dir.display());
            }
        }

        for extra in &self.config.extra_paths {
            if extra.exists() {
                watcher.watch(extra, RecursiveMode::Recursive)?;
                info!("live-reload watching (extra): {}", extra.display());
            }
        }

        self._watcher = Some(watcher);
        self.event_tx = Some(fs_tx);

        info!(
            "live-reload started (debounce={}ms)",
            self.config.debounce_ms
        );
        Ok(consumer_rx)
    }

    /// The debounce loop: collects file events, waits for a quiet period,
    /// then emits a batched [`ReloadEvent`].
    fn debounce_loop(
        rx: Receiver<Event>,
        tx: Sender<ReloadEvent>,
        debounce_ms: u64,
        project_root: &Path,
    ) {
        let debounce = Duration::from_millis(debounce_ms);
        let mut pending: HashSet<ReloadTarget> = HashSet::new();
        let mut last_event = Instant::now();

        loop {
            match rx.recv_timeout(debounce) {
                Ok(event) => {
                    // Only care about writes and creates.
                    match event.kind {
                        EventKind::Create(_) | EventKind::Modify(_) => {}
                        _ => continue,
                    }

                    for path in &event.paths {
                        if let Some(target) = Self::classify_path(path, project_root) {
                            debug!("file changed: {:?} → {:?}", path, target);
                            pending.insert(target);
                        }
                    }
                    last_event = Instant::now();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Debounce window expired — emit if we have pending changes.
                    if !pending.is_empty() && last_event.elapsed() >= debounce {
                        let targets: Vec<ReloadTarget> = pending.drain().collect();
                        let event = ReloadEvent {
                            targets,
                            timestamp: Instant::now(),
                        };
                        info!("live-reload: {} target(s) changed", event.targets.len());
                        if tx.send(event).is_err() {
                            debug!("live-reload consumer disconnected, exiting");
                            return;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    debug!("live-reload watcher disconnected, exiting");
                    return;
                }
            }
        }
    }

    /// Classify a changed file path into a [`ReloadTarget`].
    fn classify_path(path: &Path, project_root: &Path) -> Option<ReloadTarget> {
        let ext = path.extension()?.to_str()?;
        let stem = path.file_stem()?.to_str()?.to_string();

        // Normalize to relative path from project root.
        let rel = path.strip_prefix(project_root).ok()?;
        let rel_str = rel.to_string_lossy();

        if ext == "html" && rel_str.contains("templates") {
            Some(ReloadTarget::Template(stem))
        } else if ext == "css" && rel_str.contains("components") {
            Some(ReloadTarget::ComponentCss(stem))
        } else if ext == "css" && rel_str.contains("themes") {
            Some(ReloadTarget::ThemeCss(stem))
        } else {
            None
        }
    }

    /// Stop watching (drops the watcher).
    pub fn stop(&mut self) {
        self._watcher = None;
        self.event_tx = None;
        info!("live-reload stopped");
    }
}

impl Drop for LiveReloadWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Convenience: create and start a watcher for the default project layout.
pub fn start_live_reload(
    project_root: impl Into<PathBuf>,
) -> Result<(LiveReloadWatcher, Receiver<ReloadEvent>), notify::Error> {
    let config = LiveReloadConfig {
        project_root: project_root.into(),
        ..Default::default()
    };
    let mut watcher = LiveReloadWatcher::new(config);
    let rx = watcher.start()?;
    Ok((watcher, rx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_template() {
        let root = PathBuf::from("/project");
        let path = PathBuf::from("/project/assets/templates/dock.html");
        assert_eq!(
            LiveReloadWatcher::classify_path(&path, &root),
            Some(ReloadTarget::Template("dock".into()))
        );
    }

    #[test]
    fn classify_component_css() {
        let root = PathBuf::from("/project");
        let path = PathBuf::from("/project/assets/themes/components/dock.css");
        assert_eq!(
            LiveReloadWatcher::classify_path(&path, &root),
            Some(ReloadTarget::ComponentCss("dock".into()))
        );
    }

    #[test]
    fn classify_theme_css() {
        let root = PathBuf::from("/project");
        let path = PathBuf::from("/project/assets/themes/night.css");
        assert_eq!(
            LiveReloadWatcher::classify_path(&path, &root),
            Some(ReloadTarget::ThemeCss("night".into()))
        );
    }

    #[test]
    fn classify_unrelated() {
        let root = PathBuf::from("/project");
        let path = PathBuf::from("/project/src/main.rs");
        assert_eq!(LiveReloadWatcher::classify_path(&path, &root), None);
    }

    #[test]
    fn config_defaults() {
        let config = LiveReloadConfig::default();
        assert_eq!(config.project_root, PathBuf::from("."));
        assert_eq!(config.debounce_ms, 100);
        assert!(config.extra_paths.is_empty());
    }

    #[test]
    fn reload_target_equality() {
        let t1 = ReloadTarget::Template("dock".into());
        let t2 = ReloadTarget::Template("dock".into());
        let t3 = ReloadTarget::ComponentCss("dock".into());
        assert_eq!(t1, t2);
        assert_ne!(t1, t3);
    }

    #[test]
    fn classify_nested_template() {
        let root = PathBuf::from("/project");
        let path = PathBuf::from("/project/assets/templates/widgets/clock.html");
        assert_eq!(
            LiveReloadWatcher::classify_path(&path, &root),
            Some(ReloadTarget::Template("clock".into()))
        );
    }

    #[test]
    fn classify_non_css_in_themes() {
        let root = PathBuf::from("/project");
        let path = PathBuf::from("/project/assets/themes/README.md");
        assert_eq!(LiveReloadWatcher::classify_path(&path, &root), None);
    }
}
