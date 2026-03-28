//! Workspace management: create, destroy, switch, reorder, and move windows.
//!
//! The [`WorkspaceManager`] is the central orchestrator. It supports:
//! - Fixed workspace count or GNOME-style dynamic creation (auto-add empty
//!   workspace at the end).
//! - Wrap-around navigation (last -> first, first -> last).
//! - Transition callbacks for animated switches.
//! - An event log via [`WorkspaceEvent`].

use crate::workspace::{Workspace, WorkspaceId};

// ── Configuration ────────────────────────────────────────────────────

/// Controls whether the workspace count is fixed or grows dynamically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceCountMode {
    /// A fixed number of workspaces. New ones cannot be added beyond the cap,
    /// and the last one cannot be deleted below the floor.
    Fixed { count: usize },
    /// GNOME-style dynamic: an empty workspace is automatically appended when
    /// the current last workspace gets its first window, and trailing empty
    /// workspaces (beyond one) are garbage-collected.
    Dynamic {
        /// Minimum number of workspaces (never shrink below this).
        min: usize,
        /// Maximum number of workspaces (0 = unlimited).
        max: usize,
    },
}

impl Default for WorkspaceCountMode {
    fn default() -> Self {
        Self::Dynamic { min: 1, max: 0 }
    }
}

/// Configuration for the workspace manager.
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Fixed vs. dynamic workspace count.
    pub count_mode: WorkspaceCountMode,
    /// Whether next/prev navigation wraps around.
    pub wrap_navigation: bool,
    /// Whether moving a window to another workspace automatically switches
    /// to that workspace.
    pub move_window_switches: bool,
    /// Pattern for auto-generated workspace names. `{}` is replaced with
    /// the 1-based index.
    pub default_name_pattern: String,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            count_mode: WorkspaceCountMode::default(),
            wrap_navigation: true,
            move_window_switches: false,
            default_name_pattern: "Workspace {}".into(),
        }
    }
}

// ── Events ───────────────────────────────────────────────────────────

/// Events emitted by the workspace manager. Consumers (e.g. the compositor
/// or shell) can inspect these to trigger animations, panel updates, etc.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceEvent {
    /// A new workspace was created.
    Created { id: WorkspaceId },
    /// A workspace was destroyed.
    Destroyed { id: WorkspaceId },
    /// The active workspace changed.
    Switched {
        from: WorkspaceId,
        to: WorkspaceId,
    },
    /// A window was moved between workspaces.
    WindowMoved {
        window_id: u64,
        from: WorkspaceId,
        to: WorkspaceId,
    },
    /// A workspace was renamed.
    Renamed {
        id: WorkspaceId,
        old_name: String,
        new_name: String,
    },
    /// Workspaces were reordered.
    Reordered {
        from_idx: usize,
        to_idx: usize,
    },
}

// ── WorkspaceManager ─────────────────────────────────────────────────

/// Central workspace orchestrator.
#[derive(Debug)]
pub struct WorkspaceManager {
    /// Workspaces in index order.
    workspaces: Vec<Workspace>,
    /// The active workspace ID.
    active: WorkspaceId,
    /// Next ID to allocate.
    next_id: u32,
    /// Configuration.
    config: WorkspaceConfig,
    /// Event log (drained by the consumer each frame).
    events: Vec<WorkspaceEvent>,
}

impl WorkspaceManager {
    /// Create a new workspace manager with default configuration and one
    /// initial workspace.
    pub fn new() -> Self {
        Self::with_config(WorkspaceConfig::default())
    }

    /// Create a workspace manager with the given configuration.
    pub fn with_config(config: WorkspaceConfig) -> Self {
        let id = WorkspaceId(1);
        let name = config.default_name_pattern.replace("{}", "1");
        let ws = Workspace::new(id, name, 0);
        let mut mgr = Self {
            workspaces: vec![ws],
            active: id,
            next_id: 2,
            config,
            events: Vec::new(),
        };
        mgr.workspaces[0].is_active = true;

        // If fixed mode, create the remaining workspaces.
        if let WorkspaceCountMode::Fixed { count } = mgr.config.count_mode {
            for i in 1..count {
                let ws_id = mgr.alloc_id();
                let name = mgr
                    .config
                    .default_name_pattern
                    .replace("{}", &(i + 1).to_string());
                let ws = Workspace::new(ws_id, name, i);
                mgr.workspaces.push(ws);
            }
        }

        mgr
    }

    // ── Accessors ────────────────────────────────────────────────────

    /// Return the currently active workspace ID.
    pub fn active_workspace(&self) -> WorkspaceId {
        self.active
    }

    /// Return a reference to the active workspace.
    pub fn active_workspace_ref(&self) -> &Workspace {
        self.workspace(self.active).expect("active workspace must exist")
    }

    /// Return a reference to a workspace by ID.
    pub fn workspace(&self, id: WorkspaceId) -> Option<&Workspace> {
        self.workspaces.iter().find(|ws| ws.id == id)
    }

    /// Return a mutable reference to a workspace by ID.
    pub fn workspace_mut(&mut self, id: WorkspaceId) -> Option<&mut Workspace> {
        self.workspaces.iter_mut().find(|ws| ws.id == id)
    }

    /// Return all workspaces in index order.
    pub fn all_workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    /// Return the number of workspaces.
    pub fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    /// Drain and return all pending events.
    pub fn drain_events(&mut self) -> Vec<WorkspaceEvent> {
        std::mem::take(&mut self.events)
    }

    /// Return a reference to the current configuration.
    pub fn config(&self) -> &WorkspaceConfig {
        &self.config
    }

    /// Update the configuration.
    pub fn set_config(&mut self, config: WorkspaceConfig) {
        self.config = config;
    }

    // ── Create / Destroy ─────────────────────────────────────────────

    /// Create a new workspace at the end. Returns the new ID, or `None` if
    /// the maximum count has been reached.
    pub fn create_workspace(&mut self, name: Option<String>) -> Option<WorkspaceId> {
        if !self.can_create() {
            return None;
        }
        let id = self.alloc_id();
        let index = self.workspaces.len();
        let name = name.unwrap_or_else(|| {
            self.config
                .default_name_pattern
                .replace("{}", &(index + 1).to_string())
        });
        let ws = Workspace::new(id, name, index);
        self.workspaces.push(ws);
        self.events.push(WorkspaceEvent::Created { id });
        Some(id)
    }

    /// Destroy a workspace, moving its windows to the adjacent workspace.
    /// Cannot destroy the last workspace (or go below the minimum in dynamic
    /// mode, or below the fixed count in fixed mode). Returns `true` on
    /// success.
    pub fn destroy_workspace(&mut self, id: WorkspaceId) -> bool {
        if !self.can_destroy() {
            return false;
        }
        let idx = match self.workspaces.iter().position(|ws| ws.id == id) {
            Some(i) => i,
            None => return false,
        };

        // Pick the adjacent workspace (prefer next, else previous).
        let target_idx = if idx + 1 < self.workspaces.len() {
            idx + 1
        } else if idx > 0 {
            idx - 1
        } else {
            return false; // only workspace
        };
        let target_id = self.workspaces[target_idx].id;

        // Move windows to the target.
        let windows: Vec<u64> = self.workspaces[idx].windows.clone();
        for win in windows {
            self.workspaces[target_idx].add_window(win);
        }

        // If the destroyed workspace was active, switch to the target.
        if self.active == id {
            self.workspaces[target_idx].is_active = true;
            self.active = target_id;
        }

        // Remove and re-index.
        self.workspaces.remove(idx);
        self.reindex();

        self.events.push(WorkspaceEvent::Destroyed { id });
        true
    }

    // ── Switching ────────────────────────────────────────────────────

    /// Switch to a specific workspace by ID. Returns `false` if the
    /// workspace does not exist.
    pub fn switch_to(&mut self, id: WorkspaceId) -> bool {
        if !self.workspaces.iter().any(|ws| ws.id == id) {
            return false;
        }
        if self.active == id {
            return true; // already active
        }
        let from = self.active;
        // Deactivate old.
        if let Some(old) = self.workspace_mut(from) {
            old.is_active = false;
        }
        // Activate new.
        if let Some(new_ws) = self.workspace_mut(id) {
            new_ws.is_active = true;
        }
        self.active = id;
        self.events.push(WorkspaceEvent::Switched { from, to: id });
        true
    }

    /// Switch to the next workspace (by index). Wraps if enabled.
    pub fn switch_next(&mut self) -> bool {
        let cur_idx = self.active_index();
        let next_idx = if cur_idx + 1 < self.workspaces.len() {
            cur_idx + 1
        } else if self.config.wrap_navigation {
            0
        } else {
            return false;
        };
        let id = self.workspaces[next_idx].id;
        self.switch_to(id)
    }

    /// Switch to the previous workspace (by index). Wraps if enabled.
    pub fn switch_prev(&mut self) -> bool {
        let cur_idx = self.active_index();
        let prev_idx = if cur_idx > 0 {
            cur_idx - 1
        } else if self.config.wrap_navigation {
            self.workspaces.len() - 1
        } else {
            return false;
        };
        let id = self.workspaces[prev_idx].id;
        self.switch_to(id)
    }

    /// Switch to a workspace by index (0-based). Returns `false` if out of
    /// range.
    pub fn switch_to_index(&mut self, index: usize) -> bool {
        if index >= self.workspaces.len() {
            return false;
        }
        let id = self.workspaces[index].id;
        self.switch_to(id)
    }

    // ── Window operations ────────────────────────────────────────────

    /// Move a window from its current workspace to the target workspace.
    /// If `move_window_switches` is enabled, also switches to the target.
    pub fn move_window_to(&mut self, window_id: u64, target: WorkspaceId) -> bool {
        if !self.workspaces.iter().any(|ws| ws.id == target) {
            return false;
        }

        // Find and remove from current workspace.
        let mut from_id = None;
        for ws in &mut self.workspaces {
            if ws.id != target && ws.remove_window(window_id) {
                from_id = Some(ws.id);
                break;
            }
        }

        // Add to target.
        if let Some(ws) = self.workspace_mut(target) {
            ws.add_window(window_id);
        }

        if let Some(from) = from_id {
            self.events.push(WorkspaceEvent::WindowMoved {
                window_id,
                from,
                to: target,
            });
        }

        // Dynamic mode: auto-create trailing empty workspace if we just added
        // a window to the last workspace.
        self.maybe_auto_create();

        if self.config.move_window_switches {
            self.switch_to(target);
        }

        true
    }

    /// Find which workspace a window belongs to. Returns `None` if not found.
    pub fn workspace_for_window(&self, window_id: u64) -> Option<WorkspaceId> {
        self.workspaces
            .iter()
            .find(|ws| ws.has_window(window_id))
            .map(|ws| ws.id)
    }

    // ── Rename ───────────────────────────────────────────────────────

    /// Rename a workspace. Returns `false` if not found.
    pub fn rename_workspace(&mut self, id: WorkspaceId, new_name: impl Into<String>) -> bool {
        let new_name = new_name.into();
        if let Some(ws) = self.workspace_mut(id) {
            let old_name = ws.name.clone();
            ws.set_name(new_name.clone());
            self.events.push(WorkspaceEvent::Renamed {
                id,
                old_name,
                new_name,
            });
            true
        } else {
            false
        }
    }

    // ── Reorder ──────────────────────────────────────────────────────

    /// Move a workspace from one index position to another. All workspaces
    /// in between shift accordingly.
    pub fn reorder(&mut self, from_idx: usize, to_idx: usize) -> bool {
        let len = self.workspaces.len();
        if from_idx >= len || to_idx >= len || from_idx == to_idx {
            return false;
        }
        let ws = self.workspaces.remove(from_idx);
        self.workspaces.insert(to_idx, ws);
        self.reindex();
        self.events.push(WorkspaceEvent::Reordered {
            from_idx,
            to_idx,
        });
        true
    }

    // ── Dynamic workspace management (GNOME-style) ───────────────────

    /// In dynamic mode, ensure there is always one empty workspace at the
    /// end. Called after adding a window to a workspace.
    fn maybe_auto_create(&mut self) {
        if let WorkspaceCountMode::Dynamic { max, .. } = self.config.count_mode {
            // If the last workspace has windows, create a new empty one.
            if let Some(last) = self.workspaces.last() {
                if !last.windows.is_empty() && self.can_create() {
                    let id = self.alloc_id();
                    let index = self.workspaces.len();
                    let name = self
                        .config
                        .default_name_pattern
                        .replace("{}", &(index + 1).to_string());
                    let ws = Workspace::new(id, name, index);
                    self.workspaces.push(ws);
                    self.events.push(WorkspaceEvent::Created { id });
                }
            }
            // Garbage-collect trailing empty workspaces (keep at most one).
            let _ = max; // used in can_create
            self.gc_trailing_empty();
        }
    }

    /// Remove trailing empty workspaces beyond the first one, respecting
    /// the minimum count.
    fn gc_trailing_empty(&mut self) {
        let min = match self.config.count_mode {
            WorkspaceCountMode::Dynamic { min, .. } => min.max(1),
            _ => return,
        };
        while self.workspaces.len() > min {
            let len = self.workspaces.len();
            if len < 2 {
                break;
            }
            // Only remove if BOTH the last and second-to-last are empty.
            let last_empty = self.workspaces[len - 1].windows.is_empty();
            let penultimate_empty = self.workspaces[len - 2].windows.is_empty();
            if last_empty && penultimate_empty {
                // Don't remove the active workspace.
                let last_id = self.workspaces[len - 1].id;
                if self.active == last_id {
                    break;
                }
                self.workspaces.pop();
                self.events.push(WorkspaceEvent::Destroyed { id: last_id });
            } else {
                break;
            }
        }
        self.reindex();
    }

    // ── Internal helpers ─────────────────────────────────────────────

    fn alloc_id(&mut self) -> WorkspaceId {
        let id = WorkspaceId(self.next_id);
        self.next_id += 1;
        id
    }

    fn active_index(&self) -> usize {
        self.workspaces
            .iter()
            .position(|ws| ws.id == self.active)
            .unwrap_or(0)
    }

    fn reindex(&mut self) {
        for (i, ws) in self.workspaces.iter_mut().enumerate() {
            ws.index = i;
        }
    }

    fn can_create(&self) -> bool {
        match self.config.count_mode {
            WorkspaceCountMode::Fixed { .. } => false,
            WorkspaceCountMode::Dynamic { max, .. } => {
                max == 0 || self.workspaces.len() < max
            }
        }
    }

    fn can_destroy(&self) -> bool {
        match self.config.count_mode {
            WorkspaceCountMode::Fixed { .. } => false,
            WorkspaceCountMode::Dynamic { min, .. } => {
                self.workspaces.len() > min.max(1)
            }
        }
    }

    /// Return the internal next-ID counter (for persistence).
    pub(crate) fn next_id_raw(&self) -> u32 {
        self.next_id
    }

    /// Set the internal next-ID counter (for persistence restore).
    pub(crate) fn set_next_id(&mut self, id: u32) {
        self.next_id = id;
    }

    /// Replace the workspace list (used during restore).
    pub(crate) fn replace_workspaces(&mut self, workspaces: Vec<Workspace>, active: WorkspaceId) {
        self.workspaces = workspaces;
        self.active = active;
        if let Some(ws) = self.workspace_mut(active) {
            ws.is_active = true;
        }
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_manager_has_one_active_workspace() {
        let mgr = WorkspaceManager::new();
        assert_eq!(mgr.workspace_count(), 1);
        let ws = mgr.active_workspace_ref();
        assert!(ws.is_active);
        assert_eq!(ws.index, 0);
    }

    #[test]
    fn fixed_mode_creates_all_workspaces() {
        let config = WorkspaceConfig {
            count_mode: WorkspaceCountMode::Fixed { count: 4 },
            ..Default::default()
        };
        let mgr = WorkspaceManager::with_config(config);
        assert_eq!(mgr.workspace_count(), 4);
    }

    #[test]
    fn fixed_mode_cannot_create_or_destroy() {
        let config = WorkspaceConfig {
            count_mode: WorkspaceCountMode::Fixed { count: 2 },
            ..Default::default()
        };
        let mut mgr = WorkspaceManager::with_config(config);
        assert!(mgr.create_workspace(None).is_none());
        let first_id = mgr.all_workspaces()[0].id;
        assert!(!mgr.destroy_workspace(first_id));
    }

    #[test]
    fn create_workspace_dynamic() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.create_workspace(Some("Code".into())).unwrap();
        assert_eq!(mgr.workspace_count(), 2);
        assert_eq!(mgr.workspace(id).unwrap().name, "Code");
    }

    #[test]
    fn create_workspace_auto_name() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.create_workspace(None).unwrap();
        assert_eq!(mgr.workspace(id).unwrap().name, "Workspace 2");
    }

    #[test]
    fn destroy_workspace_moves_windows() {
        let mut mgr = WorkspaceManager::new();
        let id2 = mgr.create_workspace(Some("Doomed".into())).unwrap();
        mgr.workspace_mut(id2).unwrap().add_window(42);
        assert!(mgr.destroy_workspace(id2));
        assert_eq!(mgr.workspace_count(), 1);
        // Window moved to remaining workspace.
        assert!(mgr.active_workspace_ref().has_window(42));
    }

    #[test]
    fn cannot_destroy_last_workspace() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.active_workspace();
        assert!(!mgr.destroy_workspace(id));
    }

    #[test]
    fn switch_to_valid() {
        let mut mgr = WorkspaceManager::new();
        let id2 = mgr.create_workspace(None).unwrap();
        let from = mgr.active_workspace();
        assert!(mgr.switch_to(id2));
        assert_eq!(mgr.active_workspace(), id2);
        assert!(mgr.workspace(id2).unwrap().is_active);
        assert!(!mgr.workspace(from).unwrap().is_active);
    }

    #[test]
    fn switch_to_same_is_noop() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.active_workspace();
        assert!(mgr.switch_to(id));
        assert_eq!(mgr.drain_events().len(), 0);
    }

    #[test]
    fn switch_to_invalid() {
        let mut mgr = WorkspaceManager::new();
        assert!(!mgr.switch_to(WorkspaceId(999)));
    }

    #[test]
    fn switch_next_wraps() {
        let mut mgr = WorkspaceManager::new();
        let id1 = mgr.active_workspace();
        let id2 = mgr.create_workspace(None).unwrap();
        assert!(mgr.switch_next());
        assert_eq!(mgr.active_workspace(), id2);
        assert!(mgr.switch_next());
        assert_eq!(mgr.active_workspace(), id1); // wrapped
    }

    #[test]
    fn switch_prev_wraps() {
        let mut mgr = WorkspaceManager::new();
        let _id1 = mgr.active_workspace();
        let id2 = mgr.create_workspace(None).unwrap();
        // On id1, prev wraps to id2.
        assert!(mgr.switch_prev());
        assert_eq!(mgr.active_workspace(), id2);
    }

    #[test]
    fn switch_next_no_wrap() {
        let config = WorkspaceConfig {
            wrap_navigation: false,
            ..Default::default()
        };
        let mut mgr = WorkspaceManager::with_config(config);
        mgr.create_workspace(None);
        mgr.switch_next(); // to second
        assert!(!mgr.switch_next()); // no wrap, already at end
    }

    #[test]
    fn switch_prev_no_wrap() {
        let config = WorkspaceConfig {
            wrap_navigation: false,
            ..Default::default()
        };
        let mut mgr = WorkspaceManager::with_config(config);
        mgr.create_workspace(None);
        // Already at index 0, can't go prev.
        assert!(!mgr.switch_prev());
    }

    #[test]
    fn switch_to_index() {
        let mut mgr = WorkspaceManager::new();
        let _id2 = mgr.create_workspace(None).unwrap();
        let id3 = mgr.create_workspace(None).unwrap();
        assert!(mgr.switch_to_index(2));
        assert_eq!(mgr.active_workspace(), id3);
        assert!(!mgr.switch_to_index(99));
    }

    #[test]
    fn move_window_to_workspace() {
        let mut mgr = WorkspaceManager::new();
        let id1 = mgr.active_workspace();
        let id2 = mgr.create_workspace(None).unwrap();
        mgr.workspace_mut(id1).unwrap().add_window(100);
        assert!(mgr.move_window_to(100, id2));
        assert!(!mgr.workspace(id1).unwrap().has_window(100));
        assert!(mgr.workspace(id2).unwrap().has_window(100));
    }

    #[test]
    fn move_window_to_nonexistent_workspace() {
        let mut mgr = WorkspaceManager::new();
        mgr.workspace_mut(mgr.active_workspace())
            .unwrap()
            .add_window(100);
        assert!(!mgr.move_window_to(100, WorkspaceId(999)));
    }

    #[test]
    fn move_window_switches_when_configured() {
        let config = WorkspaceConfig {
            move_window_switches: true,
            ..Default::default()
        };
        let mut mgr = WorkspaceManager::with_config(config);
        let id1 = mgr.active_workspace();
        let id2 = mgr.create_workspace(None).unwrap();
        mgr.workspace_mut(id1).unwrap().add_window(100);
        mgr.move_window_to(100, id2);
        assert_eq!(mgr.active_workspace(), id2);
    }

    #[test]
    fn workspace_for_window() {
        let mut mgr = WorkspaceManager::new();
        let id1 = mgr.active_workspace();
        mgr.workspace_mut(id1).unwrap().add_window(42);
        assert_eq!(mgr.workspace_for_window(42), Some(id1));
        assert_eq!(mgr.workspace_for_window(999), None);
    }

    #[test]
    fn rename_workspace() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.active_workspace();
        assert!(mgr.rename_workspace(id, "Code"));
        assert_eq!(mgr.workspace(id).unwrap().name, "Code");
    }

    #[test]
    fn rename_nonexistent() {
        let mut mgr = WorkspaceManager::new();
        assert!(!mgr.rename_workspace(WorkspaceId(999), "X"));
    }

    #[test]
    fn reorder_workspaces() {
        let mut mgr = WorkspaceManager::new();
        let id1 = mgr.active_workspace();
        let id2 = mgr.create_workspace(Some("B".into())).unwrap();
        let id3 = mgr.create_workspace(Some("C".into())).unwrap();
        // Move first to last: [id1, id2, id3] -> [id2, id3, id1]
        assert!(mgr.reorder(0, 2));
        assert_eq!(mgr.all_workspaces()[0].id, id2);
        assert_eq!(mgr.all_workspaces()[1].id, id3);
        assert_eq!(mgr.all_workspaces()[2].id, id1);
        // Indices updated.
        assert_eq!(mgr.all_workspaces()[0].index, 0);
        assert_eq!(mgr.all_workspaces()[1].index, 1);
        assert_eq!(mgr.all_workspaces()[2].index, 2);
    }

    #[test]
    fn reorder_out_of_bounds() {
        let mut mgr = WorkspaceManager::new();
        assert!(!mgr.reorder(0, 5));
    }

    #[test]
    fn reorder_same_index() {
        let mut mgr = WorkspaceManager::new();
        assert!(!mgr.reorder(0, 0));
    }

    #[test]
    fn events_are_emitted() {
        let mut mgr = WorkspaceManager::new();
        mgr.drain_events(); // clear initial
        let id2 = mgr.create_workspace(None).unwrap();
        let events = mgr.drain_events();
        assert!(events.iter().any(|e| matches!(e, WorkspaceEvent::Created { id } if *id == id2)));
    }

    #[test]
    fn events_drained() {
        let mut mgr = WorkspaceManager::new();
        mgr.create_workspace(None);
        let first = mgr.drain_events();
        assert!(!first.is_empty());
        let second = mgr.drain_events();
        assert!(second.is_empty());
    }

    #[test]
    fn dynamic_auto_create_on_last_workspace_window() {
        let mut mgr = WorkspaceManager::new();
        let id1 = mgr.active_workspace();
        // Adding a window to the only (last) workspace should auto-create
        // a trailing empty one.
        mgr.move_window_to(100, id1);
        // The auto-create happens inside move_window_to.
        // We should have at least 2 workspaces now if the last got a window.
        // Note: since we're moving to the current workspace (already there),
        // move_window_to adds it. Let's check:
        assert!(mgr.workspace_count() >= 2);
    }

    #[test]
    fn dynamic_max_limit_respected() {
        let config = WorkspaceConfig {
            count_mode: WorkspaceCountMode::Dynamic { min: 1, max: 3 },
            ..Default::default()
        };
        let mut mgr = WorkspaceManager::with_config(config);
        mgr.create_workspace(None); // 2
        mgr.create_workspace(None); // 3
        assert!(mgr.create_workspace(None).is_none()); // at max
    }

    #[test]
    fn destroy_active_switches_to_adjacent() {
        let mut mgr = WorkspaceManager::new();
        let id1 = mgr.active_workspace();
        let id2 = mgr.create_workspace(None).unwrap();
        // Active is id1, destroy it -> switches to id2.
        assert!(mgr.destroy_workspace(id1));
        assert_eq!(mgr.active_workspace(), id2);
    }

    #[test]
    fn default_name_pattern() {
        let config = WorkspaceConfig {
            default_name_pattern: "Desktop {}".into(),
            ..Default::default()
        };
        let mgr = WorkspaceManager::with_config(config);
        assert_eq!(mgr.active_workspace_ref().name, "Desktop 1");
    }
}
