//! Workspace management — virtual desktops.

use serde::{Deserialize, Serialize};

use crate::tiling::{TilingLayoutKind, TilingMode};
use crate::window::WindowId;
use crate::{Result, ShellError};

/// Unique workspace identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(pub u32);

/// A virtual workspace containing windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub windows: Vec<WindowId>,
    pub active: bool,
    /// Per-workspace tiling mode override.
    pub tiling_mode: Option<TilingMode>,
    /// Per-workspace tiling layout override.
    pub tiling_layout: Option<TilingLayoutKind>,
}

impl Workspace {
    /// Create a new workspace.
    #[must_use]
    pub fn new(id: WorkspaceId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            windows: Vec::new(),
            active: false,
            tiling_mode: None,
            tiling_layout: None,
        }
    }

    /// Add a window to this workspace.
    pub fn add_window(&mut self, id: WindowId) {
        if !self.windows.contains(&id) {
            self.windows.push(id);
        }
    }

    /// Remove a window. Returns true if it was present.
    pub fn remove_window(&mut self, id: WindowId) -> bool {
        let before = self.windows.len();
        self.windows.retain(|w| *w != id);
        self.windows.len() < before
    }

    /// Check if this workspace contains a window.
    #[must_use]
    pub fn contains(&self, id: WindowId) -> bool {
        self.windows.contains(&id)
    }

    /// Number of windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Is the workspace empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}

/// Manages multiple workspaces.
pub struct WorkspaceManager {
    workspaces: Vec<Workspace>,
    active_workspace: WorkspaceId,
    next_id: u32,
}

impl WorkspaceManager {
    /// Create a new manager with one default workspace.
    #[must_use]
    pub fn new() -> Self {
        let mut ws = Workspace::new(WorkspaceId(0), "Default");
        ws.active = true;
        Self {
            workspaces: vec![ws],
            active_workspace: WorkspaceId(0),
            next_id: 1,
        }
    }

    /// Create a new workspace. Returns its ID.
    pub fn create_workspace(&mut self, name: impl Into<String>) -> WorkspaceId {
        let id = WorkspaceId(self.next_id);
        self.next_id += 1;
        self.workspaces.push(Workspace::new(id, name));
        id
    }

    /// Remove a workspace by ID. Cannot remove the active workspace.
    pub fn remove_workspace(&mut self, id: WorkspaceId) -> Result<()> {
        if id == self.active_workspace {
            return Err(ShellError::InvalidOperation(
                "cannot remove the active workspace".to_string(),
            ));
        }
        let before = self.workspaces.len();
        self.workspaces.retain(|ws| ws.id != id);
        if self.workspaces.len() == before {
            return Err(ShellError::WorkspaceNotFound { id });
        }
        Ok(())
    }

    /// Switch to a workspace.
    pub fn switch_to(&mut self, id: WorkspaceId) -> Result<()> {
        let found = self.workspaces.iter().any(|ws| ws.id == id);
        if !found {
            return Err(ShellError::WorkspaceNotFound { id });
        }
        for ws in &mut self.workspaces {
            ws.active = ws.id == id;
        }
        self.active_workspace = id;
        Ok(())
    }

    /// Get the active workspace.
    #[must_use]
    pub fn active(&self) -> &Workspace {
        self.workspaces
            .iter()
            .find(|ws| ws.id == self.active_workspace)
            .expect("active workspace must exist")
    }

    /// Get the active workspace mutably.
    pub fn active_mut(&mut self) -> &mut Workspace {
        let active_id = self.active_workspace;
        self.workspaces
            .iter_mut()
            .find(|ws| ws.id == active_id)
            .expect("active workspace must exist")
    }

    /// Move a window between workspaces.
    pub fn move_window(
        &mut self,
        window_id: WindowId,
        from_ws: WorkspaceId,
        to_ws: WorkspaceId,
    ) -> Result<()> {
        let source = self
            .workspaces
            .iter_mut()
            .find(|ws| ws.id == from_ws)
            .ok_or(ShellError::WorkspaceNotFound { id: from_ws })?;
        if !source.remove_window(window_id) {
            return Err(ShellError::WindowNotFound { id: window_id });
        }
        let dest = self
            .workspaces
            .iter_mut()
            .find(|ws| ws.id == to_ws)
            .ok_or(ShellError::WorkspaceNotFound { id: to_ws })?;
        dest.add_window(window_id);
        Ok(())
    }

    /// Total number of workspaces.
    #[must_use]
    pub fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    /// Find which workspace contains a window.
    #[must_use]
    pub fn find_window(&self, id: WindowId) -> Option<WorkspaceId> {
        self.workspaces
            .iter()
            .find(|ws| ws.contains(id))
            .map(|ws| ws.id)
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Workspace({})", self.0)
    }
}
