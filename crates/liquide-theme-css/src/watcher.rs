//! File watcher for hot-reloading themes

use crate::error::Result;
use crate::parser::ThemeParser;
use crate::stylesheet::StyleSheet;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{debug, error, info};

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
        if self.callback.is_none() {
            error!("No callback set for theme updates");
            return Ok(());
        }
        
        let (tx, rx) = channel();
        let callback = self.callback.clone().unwrap();
        let paths = self.paths.clone();
        
        // Create watcher
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
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
        let parser = ThemeParser::new();
        
        for event in rx {
            debug!("File system event: {:?}", event);
            
            // Check if event is for a watched path
            let mut should_reload = false;
            for path in event.paths {
                if paths.iter().any(|p| path.starts_with(p)) {
                    should_reload = true;
                    break;
                }
            }
            
            if !should_reload {
                continue;
            }
            
            // Reload themes
            match Self::load_themes(&parser, &paths) {
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
    
    fn load_themes(parser: &ThemeParser, paths: &[PathBuf]) -> Result<StyleSheet> {
        let mut combined = StyleSheet::new();
        
        for path in paths {
            if path.is_file() && path.extension().map(|e| e == "css").unwrap_or(false) {
                let sheet = parser.parse_file(path)?;
                combined.merge(&sheet);
            } else if path.is_dir() {
                // Load all CSS files in directory
                for entry in std::fs::read_dir(path)? {
                    let entry = entry?;
                    let entry_path = entry.path();
                    
                    if entry_path.is_file()
                        && entry_path.extension().map(|e| e == "css").unwrap_or(false)
                    {
                        let sheet = parser.parse_file(&entry_path)?;
                        combined.merge(&sheet);
                    }
                }
            }
        }
        
        Ok(combined)
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
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tempfile::NamedTempFile;
    
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
}
