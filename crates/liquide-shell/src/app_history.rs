//! Application usage history — per-app tracking with position memory.

use std::collections::HashMap;
use std::fmt;

use liquide_compositor::geometry::Rect;
use serde::{Deserialize, Serialize};

use crate::window::WindowId;

/// A single open-to-close session for an application window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSession {
    pub window_id: WindowId,
    pub opened_at: u64,
    pub closed_at: Option<u64>,
    pub last_bounds: Rect,
}

impl fmt::Display for AppSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AppSession({}, opened={}, closed={:?})",
            self.window_id, self.opened_at, self.closed_at
        )
    }
}

/// Tracked information for a single application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppInfo {
    pub app_id: String,
    pub first_seen: u64,
    pub last_seen: u64,
    pub total_windows_opened: u64,
    pub active_window_count: u32,
    pub sessions: Vec<AppSession>,
    pub last_bounds: Option<Rect>,
}

impl fmt::Display for AppInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AppInfo(app_id={}, windows_opened={}, active={})",
            self.app_id, self.total_windows_opened, self.active_window_count
        )
    }
}

/// Tracks application usage patterns.
pub struct AppHistory {
    apps: HashMap<String, AppInfo>,
    max_tracked: usize,
}

impl AppHistory {
    /// Create a new app history tracker.
    #[must_use]
    pub fn new(max_tracked: usize) -> Self {
        Self {
            apps: HashMap::new(),
            max_tracked,
        }
    }

    /// Record that a window was opened for the given app.
    pub fn record_open(&mut self, app_id: &str, window_id: WindowId, bounds: Rect, timestamp: u64) {
        if app_id.is_empty() {
            return;
        }

        // Eviction if at capacity and this is a new app
        if !self.apps.contains_key(app_id) && self.apps.len() >= self.max_tracked {
            self.evict_one();
        }

        let info = self
            .apps
            .entry(app_id.to_string())
            .or_insert_with(|| AppInfo {
                app_id: app_id.to_string(),
                first_seen: timestamp,
                last_seen: timestamp,
                total_windows_opened: 0,
                active_window_count: 0,
                sessions: Vec::new(),
                last_bounds: None,
            });

        info.last_seen = timestamp;
        info.total_windows_opened += 1;
        info.active_window_count += 1;
        info.sessions.push(AppSession {
            window_id,
            opened_at: timestamp,
            closed_at: None,
            last_bounds: bounds,
        });
    }

    /// Record that a window was closed for the given app.
    pub fn record_close(
        &mut self,
        app_id: &str,
        window_id: WindowId,
        bounds: Rect,
        timestamp: u64,
    ) {
        if let Some(info) = self.apps.get_mut(app_id) {
            info.last_seen = timestamp;
            info.active_window_count = info.active_window_count.saturating_sub(1);
            info.last_bounds = Some(bounds);

            // Find the matching open session and close it
            for session in info.sessions.iter_mut().rev() {
                if session.window_id == window_id && session.closed_at.is_none() {
                    session.closed_at = Some(timestamp);
                    session.last_bounds = bounds;
                    break;
                }
            }
        }
    }

    /// Update the last-seen timestamp for an app.
    pub fn touch(&mut self, app_id: &str, timestamp: u64) {
        if let Some(info) = self.apps.get_mut(app_id) {
            info.last_seen = timestamp;
        }
    }

    /// Get info for a specific app.
    #[must_use]
    pub fn app_info(&self, app_id: &str) -> Option<&AppInfo> {
        self.apps.get(app_id)
    }

    /// Get the most recently used apps (sorted by `last_seen` descending).
    #[must_use]
    pub fn most_recent(&self, n: usize) -> Vec<&AppInfo> {
        let mut apps: Vec<&AppInfo> = self.apps.values().collect();
        apps.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        apps.truncate(n);
        apps
    }

    /// Get the most frequently used apps (sorted by `total_windows_opened` descending).
    #[must_use]
    pub fn most_frequent(&self, n: usize) -> Vec<&AppInfo> {
        let mut apps: Vec<&AppInfo> = self.apps.values().collect();
        apps.sort_by(|a, b| b.total_windows_opened.cmp(&a.total_windows_opened));
        apps.truncate(n);
        apps
    }

    /// Get the last remembered bounds for an app.
    #[must_use]
    pub fn last_bounds_for(&self, app_id: &str) -> Option<Rect> {
        self.apps.get(app_id).and_then(|info| info.last_bounds)
    }

    /// Number of tracked apps.
    #[must_use]
    pub fn tracked_count(&self) -> usize {
        self.apps.len()
    }

    /// Max tracked apps capacity.
    #[must_use]
    pub fn max_tracked(&self) -> usize {
        self.max_tracked
    }

    /// Clear all tracked app data.
    pub fn clear(&mut self) {
        self.apps.clear();
    }

    /// Evict the least-recently-seen app with no active windows.
    fn evict_one(&mut self) {
        let evict_key = self
            .apps
            .iter()
            .filter(|(_, info)| info.active_window_count == 0)
            .min_by_key(|(_, info)| info.last_seen)
            .map(|(key, _)| key.clone());

        if let Some(key) = evict_key {
            self.apps.remove(&key);
        }
        // If all have active windows, allow soft-cap overflow
    }
}

impl fmt::Display for AppHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AppHistory({}/{} apps)",
            self.tracked_count(),
            self.max_tracked
        )
    }
}
