//! Core workspace model.
//!
//! A [`Workspace`] is a virtual desktop that contains a set of windows.
//! Each workspace has a unique [`WorkspaceId`], a human-readable name, an
//! index that determines its position in the workspace strip, and an optional
//! wallpaper override.

use serde::{Deserialize, Serialize};
use std::fmt;

// ── WorkspaceId newtype ──────────────────────────────────────────────

/// Unique identifier for a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(pub u32);

impl WorkspaceId {
    /// Return the inner numeric value.
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Workspace({})", self.0)
    }
}

// ── Workspace ────────────────────────────────────────────────────────

/// A virtual desktop workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Unique workspace identifier.
    pub id: WorkspaceId,
    /// Human-readable name (e.g. "Code", "Browser", "Music").
    pub name: String,
    /// Position index within the workspace strip (0-based).
    pub index: usize,
    /// Windows currently on this workspace (ordered front-to-back).
    pub windows: Vec<u64>,
    /// Whether this workspace is the currently active (visible) one.
    pub is_active: bool,
    /// Optional wallpaper path override for this workspace.
    pub wallpaper_override: Option<String>,
}

impl Workspace {
    /// Create a new workspace with the given id, name, and index.
    pub fn new(id: WorkspaceId, name: String, index: usize) -> Self {
        Self {
            id,
            name,
            index,
            windows: Vec::new(),
            is_active: false,
            wallpaper_override: None,
        }
    }

    /// Add a window to this workspace. Returns `false` if already present.
    pub fn add_window(&mut self, window_id: u64) -> bool {
        if self.windows.contains(&window_id) {
            return false;
        }
        self.windows.push(window_id);
        true
    }

    /// Remove a window from this workspace. Returns `false` if not found.
    pub fn remove_window(&mut self, window_id: u64) -> bool {
        let before = self.windows.len();
        self.windows.retain(|&w| w != window_id);
        self.windows.len() < before
    }

    /// Returns `true` if this workspace contains the given window.
    pub fn has_window(&self, window_id: u64) -> bool {
        self.windows.contains(&window_id)
    }

    /// Return the number of windows on this workspace.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Change the workspace name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Set or clear the wallpaper override.
    pub fn set_wallpaper(&mut self, path: Option<String>) {
        self.wallpaper_override = path;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_id_display() {
        let id = WorkspaceId(7);
        assert_eq!(format!("{id}"), "Workspace(7)");
        assert_eq!(id.raw(), 7);
    }

    #[test]
    fn new_workspace_defaults() {
        let ws = Workspace::new(WorkspaceId(1), "Main".into(), 0);
        assert_eq!(ws.id, WorkspaceId(1));
        assert_eq!(ws.name, "Main");
        assert_eq!(ws.index, 0);
        assert!(ws.windows.is_empty());
        assert!(!ws.is_active);
        assert!(ws.wallpaper_override.is_none());
    }

    #[test]
    fn add_window_success() {
        let mut ws = Workspace::new(WorkspaceId(1), "A".into(), 0);
        assert!(ws.add_window(100));
        assert_eq!(ws.window_count(), 1);
        assert!(ws.has_window(100));
    }

    #[test]
    fn add_window_duplicate_returns_false() {
        let mut ws = Workspace::new(WorkspaceId(1), "A".into(), 0);
        assert!(ws.add_window(100));
        assert!(!ws.add_window(100));
        assert_eq!(ws.window_count(), 1);
    }

    #[test]
    fn remove_window_success() {
        let mut ws = Workspace::new(WorkspaceId(1), "A".into(), 0);
        ws.add_window(100);
        assert!(ws.remove_window(100));
        assert!(!ws.has_window(100));
        assert_eq!(ws.window_count(), 0);
    }

    #[test]
    fn remove_window_not_found() {
        let mut ws = Workspace::new(WorkspaceId(1), "A".into(), 0);
        assert!(!ws.remove_window(999));
    }

    #[test]
    fn set_name() {
        let mut ws = Workspace::new(WorkspaceId(1), "Old".into(), 0);
        ws.set_name("New");
        assert_eq!(ws.name, "New");
    }

    #[test]
    fn set_wallpaper() {
        let mut ws = Workspace::new(WorkspaceId(1), "A".into(), 0);
        ws.set_wallpaper(Some("/usr/share/wallpapers/forest.png".into()));
        assert_eq!(
            ws.wallpaper_override.as_deref(),
            Some("/usr/share/wallpapers/forest.png")
        );
        ws.set_wallpaper(None);
        assert!(ws.wallpaper_override.is_none());
    }

    #[test]
    fn has_window_false_on_empty() {
        let ws = Workspace::new(WorkspaceId(1), "A".into(), 0);
        assert!(!ws.has_window(42));
    }

    #[test]
    fn window_count_tracks_additions_and_removals() {
        let mut ws = Workspace::new(WorkspaceId(1), "A".into(), 0);
        ws.add_window(1);
        ws.add_window(2);
        ws.add_window(3);
        assert_eq!(ws.window_count(), 3);
        ws.remove_window(2);
        assert_eq!(ws.window_count(), 2);
    }

    #[test]
    fn workspace_id_equality() {
        assert_eq!(WorkspaceId(5), WorkspaceId(5));
        assert_ne!(WorkspaceId(5), WorkspaceId(6));
    }
}
