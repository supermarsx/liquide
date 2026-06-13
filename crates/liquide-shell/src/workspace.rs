//! Workspace management — virtual desktops.
//!
//! ## Single-sourced onto `liquide-workspaces` (t52-e5, Mandate 2 / Wave N3)
//!
//! The shell no longer carries a *second*, independent workspace-switching
//! implementation. The canonical [`liquide_workspaces::WorkspaceManager`] is now
//! the **single** switching / wrap / index / dynamic-create policy engine, owned
//! directly by the shell [`WorkspaceManager`] adapter below (folding in what was
//! previously the separate `chrome_workspaces` field). t51-e12 ran the two
//! managers side-by-side (an internal mirror + `chrome_workspaces`) and aligned
//! them by index; e5 collapses that duplication: there is now ONE manager object
//! per shell, with the canonical engine embedded.
//!
//! ### Identity adapters at the boundary (the two honest mismatches)
//! 1. **0-vs-1-based id identity.** The canonical crate uses **1-based**
//!    [`WorkspaceId`]s (`WorkspaceId(1)` is the first workspace) and a separate
//!    0-based `index`. The shell facade historically exposes **0-based**
//!    [`WorkspaceId`]s (`WorkspaceId(0)` is the first), and several callers
//!    (`scene.rs` node id, `tick.rs` index math, `shell/batch.rs`) plus the
//!    workspace tests rely on that. e5 picks **canonical 1-based as the single
//!    internal truth** and translates to/from the shell's 0-based facade *here*
//!    (`shell_id <-> canonical_id == shell_id + 1`). No caller observes a
//!    1-based id; no `WorkspaceId(0)` assumption survives inside the manager.
//! 2. **u64 vs `WindowId` window-id storage.** Canonical stores window ids as
//!    `u64`; the shell uses [`crate::window::WindowId`]. The adapter converts at
//!    the canonical call sites (`id.0` outbound, `WindowId(raw)` inbound) so the
//!    mismatch never leaks into shell code.
//!
//! Per the W-note (recorded by t52-e3/e4): **per-workspace tiling overrides live
//! in `TilingState`** (the `TilingEngine` `per_workspace_layouts` /
//! `per_workspace_modes` maps keyed by `WorkspaceId`), so the shell [`Workspace`]
//! type drops its old `tiling_mode` / `tiling_layout` fields here.
//!
//! Single-source status (t52-e6, finalized):
//! * [`WorkspaceId`] is **single-sourced** — this module re-exports
//!   `liquide_workspaces::WorkspaceId` (one `struct WorkspaceId(pub u32)`), and
//!   `ShellError::WorkspaceNotFound` is keyed on it. The former shell-local
//!   `WorkspaceId` struct + its duplicate `Display` impl were removed.
//! * The shell [`Workspace`] / [`WorkspaceManager`] adapter types are
//!   **retained under distinct names**: their API surface diverges from canonical
//!   irreconcilably (typed `active()` / `active_mut()` returning `&Workspace` /
//!   [`ActiveWorkspaceMut`], 0-based `WindowId`-keyed membership, a 2-arg
//!   `Workspace::new`). A pure re-export of the canonical `Workspace` /
//!   `WorkspaceManager` would break `tick.rs` / `scene.rs` / `batch.rs` /
//!   `accessors.rs` and every workspace test — so this module stays as the shell's
//!   documented adapter (NOT a second switching truth; the embedded canonical
//!   engine is the sole switching/membership store). It is therefore kept, not
//!   deleted.

use serde::{Deserialize, Serialize};

use crate::window::WindowId;
use crate::{Result, ShellError};

/// Unique workspace identifier — **single-sourced** onto the canonical crate
/// (t52-e6). One `struct WorkspaceId(pub u32)` now serves both readings: the
/// shell [`WorkspaceManager`] facade treats it as **0-based** (the first
/// workspace is `WorkspaceId(0)`) and maps to the canonical 1-based id at the
/// boundary, while the `TilingEngine` keys its per-workspace maps on the
/// canonical 1-based reading. There is no longer a separate shell type.
pub use liquide_workspaces::WorkspaceId;

/// A virtual workspace containing windows (shell facade view, 0-based id).
///
/// This is a thin projection of the canonical workspace membership, used as the
/// return type of [`WorkspaceManager::active`] / [`WorkspaceManager::active_mut`]
/// and exercised directly by the workspace tests. The per-workspace tiling
/// override fields were dropped in e5 — those overrides live in `TilingState`
/// (`TilingEngine` per-workspace maps keyed by `WorkspaceId`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub windows: Vec<WindowId>,
    pub active: bool,
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
///
/// Thin adapter over the canonical [`liquide_workspaces::WorkspaceManager`]: the
/// canonical engine is the single switching / membership / dynamic-create store;
/// this adapter translates the 0-based shell facade and the `WindowId` <-> `u64`
/// window-id representation at the boundary. A `Vec<Workspace>` projection is
/// rebuilt from canonical after each mutation so the typed `active()` /
/// `active_mut()` accessors can hand out shell-facade references.
pub struct WorkspaceManager {
    /// Canonical switching / membership engine (single source of truth).
    inner: liquide_workspaces::WorkspaceManager,
    /// Shell-facade projection of `inner`, kept in lockstep. Index `i` holds the
    /// projection of canonical workspace with shell id `WorkspaceId(i)`.
    view: Vec<Workspace>,
}

impl WorkspaceManager {
    /// Create a new manager with one default workspace.
    #[must_use]
    pub fn new() -> Self {
        let mut mgr = Self {
            inner: liquide_workspaces::WorkspaceManager::new(),
            view: Vec::new(),
        };
        mgr.resync_view();
        mgr
    }

    /// Borrow the embedded canonical manager (read-only).
    #[must_use]
    pub fn canonical(&self) -> &liquide_workspaces::WorkspaceManager {
        &self.inner
    }

    /// Rebuild the shell-facade `view` from the canonical `inner`. The shell id
    /// of each workspace is its canonical `index` (0-based), so the facade stays
    /// stable 0-based regardless of canonical id churn.
    fn resync_view(&mut self) {
        let active_canonical = self.inner.active_workspace();
        let mut view: Vec<Workspace> = self
            .inner
            .all_workspaces()
            .iter()
            .map(|ws| Workspace {
                id: WorkspaceId(ws.index as u32),
                name: ws.name.clone(),
                windows: ws.windows.iter().map(|&w| WindowId(w)).collect(),
                active: ws.id == active_canonical,
            })
            .collect();
        view.sort_by_key(|ws| ws.id.0);
        self.view = view;
    }

    /// Look up the canonical id for a shell (0-based) `index`-position id.
    fn canonical_id_for(&self, shell_id: WorkspaceId) -> Option<liquide_workspaces::WorkspaceId> {
        self.inner
            .all_workspaces()
            .iter()
            .find(|ws| ws.index as u32 == shell_id.0)
            .map(|ws| ws.id)
    }

    /// Create a new workspace. Returns its (0-based) shell ID.
    pub fn create_workspace(&mut self, name: impl Into<String>) -> WorkspaceId {
        let canonical = self
            .inner
            .create_workspace(Some(name.into()))
            .expect("default canonical config is unbounded dynamic");
        let _ = self.inner.drain_events();
        // The new workspace is appended at the end; its shell id is its index.
        let shell_id = self
            .inner
            .workspace(canonical)
            .map(|ws| WorkspaceId(ws.index as u32))
            .unwrap_or(WorkspaceId(self.view.len() as u32));
        self.resync_view();
        shell_id
    }

    /// Remove a workspace by ID. Cannot remove the active workspace.
    pub fn remove_workspace(&mut self, id: WorkspaceId) -> Result<()> {
        if id == self.active().id {
            return Err(ShellError::InvalidOperation(
                "cannot remove the active workspace".to_string(),
            ));
        }
        let canonical = self
            .canonical_id_for(id)
            .ok_or(ShellError::WorkspaceNotFound { id })?;
        if !self.inner.destroy_workspace(canonical) {
            return Err(ShellError::WorkspaceNotFound { id });
        }
        let _ = self.inner.drain_events();
        self.resync_view();
        Ok(())
    }

    /// Switch to a workspace.
    pub fn switch_to(&mut self, id: WorkspaceId) -> Result<()> {
        let canonical = self
            .canonical_id_for(id)
            .ok_or(ShellError::WorkspaceNotFound { id })?;
        if !self.inner.switch_to(canonical) {
            return Err(ShellError::WorkspaceNotFound { id });
        }
        let _ = self.inner.drain_events();
        self.resync_view();
        Ok(())
    }

    /// Switch to the next workspace (by index), wrapping. Returns true if the
    /// active workspace changed. Delegates the wrap policy to canonical.
    pub fn switch_next(&mut self) -> bool {
        let changed = self.inner.switch_next();
        let _ = self.inner.drain_events();
        if changed {
            self.resync_view();
        }
        changed
    }

    /// Switch to the previous workspace (by index), wrapping.
    pub fn switch_prev(&mut self) -> bool {
        let changed = self.inner.switch_prev();
        let _ = self.inner.drain_events();
        if changed {
            self.resync_view();
        }
        changed
    }

    /// Switch to the workspace at the given 0-based index. Returns true if the
    /// active workspace *actually changed* (switching to the already-active
    /// workspace returns false, so the switch path skips a needless hide/show).
    pub fn switch_to_index(&mut self, index: usize) -> bool {
        let before = self.inner.active_workspace();
        let ok = self.inner.switch_to_index(index);
        let _ = self.inner.drain_events();
        let changed = ok && self.inner.active_workspace() != before;
        if changed {
            self.resync_view();
        }
        changed
    }

    /// Get the active workspace (shell-facade projection).
    #[must_use]
    pub fn active(&self) -> &Workspace {
        let active_id = WorkspaceId(self.inner.active_workspace_ref().index as u32);
        self.view
            .iter()
            .find(|ws| ws.id == active_id)
            .expect("active workspace must exist in the facade projection")
    }

    /// Get the active workspace mutably.
    ///
    /// Mutations to the returned projection are written back through to the
    /// canonical store before the borrow is observed again, via the
    /// [`ActiveWorkspaceMut`] write-back guard.
    pub fn active_mut(&mut self) -> ActiveWorkspaceMut<'_> {
        let active_id = WorkspaceId(self.inner.active_workspace_ref().index as u32);
        let pos = self
            .view
            .iter()
            .position(|ws| ws.id == active_id)
            .expect("active workspace must exist in the facade projection");
        ActiveWorkspaceMut { mgr: self, pos }
    }

    /// Move a window between workspaces (shell-facade ids).
    pub fn move_window(
        &mut self,
        window_id: WindowId,
        from_ws: WorkspaceId,
        to_ws: WorkspaceId,
    ) -> Result<()> {
        // Validate source membership against canonical truth.
        let from_canonical = self
            .canonical_id_for(from_ws)
            .ok_or(ShellError::WorkspaceNotFound { id: from_ws })?;
        let to_canonical = self
            .canonical_id_for(to_ws)
            .ok_or(ShellError::WorkspaceNotFound { id: to_ws })?;
        let removed = self
            .inner
            .workspace_mut(from_canonical)
            .map(|ws| ws.remove_window(window_id.0))
            .unwrap_or(false);
        if !removed {
            return Err(ShellError::WindowNotFound { id: window_id });
        }
        // Add to the destination directly (u64 window id at the boundary). We
        // deliberately avoid canonical `move_window_to` here: its dynamic
        // auto-create / GC side effects would change the workspace count, which
        // the shell facade does not do on a plain window move.
        if let Some(ws) = self.inner.workspace_mut(to_canonical) {
            ws.add_window(window_id.0);
        }
        self.resync_view();
        Ok(())
    }

    /// Total number of workspaces.
    #[must_use]
    pub fn workspace_count(&self) -> usize {
        self.inner.workspace_count()
    }

    /// Find which workspace contains a window (shell-facade id).
    #[must_use]
    pub fn find_window(&self, id: WindowId) -> Option<WorkspaceId> {
        let canonical = self.inner.workspace_for_window(id.0)?;
        // Map the canonical id back to its 0-based index identity (the shell
        // facade id) via the canonical workspace's own `index`.
        self.inner
            .workspace(canonical)
            .map(|ws| WorkspaceId(ws.index as u32))
    }
}

/// Write-back guard for the active workspace projection.
///
/// Lets callers keep the legacy `active_mut().add_window(WindowId)` /
/// `.remove_window(WindowId)` idioms while the underlying truth is the canonical
/// store: mutations are reflected onto canonical when the guard is dropped.
pub struct ActiveWorkspaceMut<'a> {
    mgr: &'a mut WorkspaceManager,
    pos: usize,
}

impl ActiveWorkspaceMut<'_> {
    /// Add a window to the active workspace.
    pub fn add_window(&mut self, id: WindowId) {
        // Mutate canonical truth.
        let canonical_id = self.mgr.inner.active_workspace();
        if let Some(ws) = self.mgr.inner.workspace_mut(canonical_id) {
            ws.add_window(id.0);
        }
        // Mirror into the projection in place.
        self.mgr.view[self.pos].add_window(id);
    }

    /// Remove a window from the active workspace. Returns true if present.
    pub fn remove_window(&mut self, id: WindowId) -> bool {
        let canonical_id = self.mgr.inner.active_workspace();
        let removed = self
            .mgr
            .inner
            .workspace_mut(canonical_id)
            .map(|ws| ws.remove_window(id.0))
            .unwrap_or(false);
        self.mgr.view[self.pos].remove_window(id);
        removed
    }

    /// Borrow the active workspace projection (read).
    #[must_use]
    pub fn contains(&self, id: WindowId) -> bool {
        self.mgr.view[self.pos].contains(id)
    }
}

impl std::ops::Deref for ActiveWorkspaceMut<'_> {
    type Target = Workspace;

    fn deref(&self) -> &Workspace {
        &self.mgr.view[self.pos]
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}
