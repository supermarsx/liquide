use std::collections::HashMap;

use crate::group::{GroupId, WindowGroup, WindowId};
use crate::grouping::{GroupEvent, GroupEventLog};
use crate::policy::{AutoGroupPolicy, GroupMinimizePolicy};
use crate::tabs::{TabGroup, TabGroupId};

/// Central manager for window groups and tab groups.
#[derive(Debug)]
pub struct GroupManager {
    /// All window groups, keyed by GroupId.
    groups: HashMap<GroupId, WindowGroup>,
    /// All tab groups, keyed by TabGroupId.
    tab_groups: HashMap<TabGroupId, TabGroup>,
    /// Reverse index: window -> group.
    window_to_group: HashMap<WindowId, GroupId>,
    /// Reverse index: window -> tab group.
    window_to_tab_group: HashMap<WindowId, TabGroupId>,
    /// Auto-group index: app_id -> group_id (used when policy is ByApplication).
    app_group_index: HashMap<String, GroupId>,
    /// Workspace-group index: workspace_id -> group_id (used when policy is ByWorkspace).
    workspace_group_index: HashMap<u64, GroupId>,
    /// Next ID for groups.
    next_group_id: GroupId,
    /// Next ID for tab groups.
    next_tab_group_id: TabGroupId,
    /// Current auto-group policy.
    pub auto_group_policy: AutoGroupPolicy,
    /// Current minimize policy.
    pub minimize_policy: GroupMinimizePolicy,
    /// Default tab bar height for new tab groups.
    pub default_tab_bar_height: f32,
    /// Event log for external consumers to poll.
    pub events: GroupEventLog,
}

impl Default for GroupManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GroupManager {
    /// Create a new GroupManager with default settings.
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
            tab_groups: HashMap::new(),
            window_to_group: HashMap::new(),
            window_to_tab_group: HashMap::new(),
            app_group_index: HashMap::new(),
            workspace_group_index: HashMap::new(),
            next_group_id: 1,
            next_tab_group_id: 1,
            auto_group_policy: AutoGroupPolicy::Manual,
            minimize_policy: GroupMinimizePolicy::Individual,
            default_tab_bar_height: 32.0,
            events: GroupEventLog::new(),
        }
    }

    // ---- Group operations ----

    /// Create a new empty window group with the given label.
    pub fn create_group(&mut self, label: impl Into<String>) -> GroupId {
        let id = self.next_group_id;
        self.next_group_id += 1;
        let group = WindowGroup::new(id, label.into());
        self.groups.insert(id, group);
        self.events.push(GroupEvent::Created { group_id: id });
        id
    }

    /// Get a reference to a window group.
    pub fn get_group(&self, group_id: GroupId) -> Option<&WindowGroup> {
        self.groups.get(&group_id)
    }

    /// Get a mutable reference to a window group.
    pub fn get_group_mut(&mut self, group_id: GroupId) -> Option<&mut WindowGroup> {
        self.groups.get_mut(&group_id)
    }

    /// Add a window to an existing group.
    /// Returns false if the group doesn't exist or the window is already in a group.
    pub fn add_to_group(&mut self, group_id: GroupId, window_id: WindowId) -> bool {
        // A window can only be in one group at a time.
        if self.window_to_group.contains_key(&window_id) {
            return false;
        }
        let Some(group) = self.groups.get_mut(&group_id) else {
            return false;
        };
        if !group.add_window(window_id) {
            return false;
        }
        self.window_to_group.insert(window_id, group_id);
        self.events.push(GroupEvent::WindowAdded {
            group_id,
            window_id,
        });
        true
    }

    /// Remove a window from a group.
    /// Returns false if the group doesn't exist or the window isn't in it.
    pub fn remove_from_group(&mut self, group_id: GroupId, window_id: WindowId) -> bool {
        let Some(group) = self.groups.get_mut(&group_id) else {
            return false;
        };
        if !group.remove_window(window_id) {
            return false;
        }
        self.window_to_group.remove(&window_id);
        self.events.push(GroupEvent::WindowRemoved {
            group_id,
            window_id,
        });
        true
    }

    /// Delete a group entirely, removing all window associations.
    pub fn delete_group(&mut self, group_id: GroupId) -> bool {
        let Some(group) = self.groups.remove(&group_id) else {
            return false;
        };
        for &wid in &group.windows {
            self.window_to_group.remove(&wid);
        }
        // Clean up app_group_index entries pointing to this group.
        self.app_group_index.retain(|_, &mut gid| gid != group_id);
        self.workspace_group_index
            .retain(|_, &mut gid| gid != group_id);
        self.events.push(GroupEvent::Dissolved { group_id });
        true
    }

    /// Find which group (if any) a window belongs to.
    pub fn group_for_window(&self, window_id: WindowId) -> Option<GroupId> {
        self.window_to_group.get(&window_id).copied()
    }

    /// Returns an iterator over all groups.
    pub fn groups(&self) -> impl Iterator<Item = &WindowGroup> {
        self.groups.values()
    }

    // ---- Tab group operations ----

    /// Convert a window group into a tab group. The original group remains
    /// but its windows are now also in a tab group. Returns the TabGroupId.
    /// Returns None if the group doesn't exist or has no windows.
    pub fn merge_into_tabs(&mut self, group_id: GroupId) -> Option<TabGroupId> {
        let group = self.groups.get(&group_id)?;
        if group.windows.is_empty() {
            return None;
        }
        let tabs = group.windows.clone();
        let tab_id = self.next_tab_group_id;
        self.next_tab_group_id += 1;
        let tab_group = TabGroup::new(tab_id, tabs.clone(), self.default_tab_bar_height);
        for &wid in &tabs {
            self.window_to_tab_group.insert(wid, tab_id);
        }
        self.tab_groups.insert(tab_id, tab_group);
        self.events.push(GroupEvent::TabGroupCreated {
            tab_group_id: tab_id,
            group_id,
        });
        Some(tab_id)
    }

    /// Get a reference to a tab group.
    pub fn get_tab_group(&self, tab_group_id: TabGroupId) -> Option<&TabGroup> {
        self.tab_groups.get(&tab_group_id)
    }

    /// Get a mutable reference to a tab group.
    pub fn get_tab_group_mut(&mut self, tab_group_id: TabGroupId) -> Option<&mut TabGroup> {
        self.tab_groups.get_mut(&tab_group_id)
    }

    /// Detach a tab from a tab group, making it a free window again.
    /// Returns false if the tab group doesn't exist or the window isn't in it.
    pub fn split_tab(&mut self, tab_group_id: TabGroupId, window_id: WindowId) -> bool {
        let Some(tab_group) = self.tab_groups.get_mut(&tab_group_id) else {
            return false;
        };
        if !tab_group.remove_tab(window_id) {
            return false;
        }
        self.window_to_tab_group.remove(&window_id);
        self.events.push(GroupEvent::TabDetached {
            tab_group_id,
            window_id,
        });

        // If the tab group is now empty, remove it.
        if tab_group.tabs.is_empty() {
            self.tab_groups.remove(&tab_group_id);
            self.events
                .push(GroupEvent::TabGroupDissolved { tab_group_id });
        }
        true
    }

    /// Reorder a tab within a tab group.
    /// Returns false if the tab group doesn't exist or indices are invalid.
    pub fn reorder_tab(
        &mut self,
        tab_group_id: TabGroupId,
        from_index: usize,
        to_index: usize,
    ) -> bool {
        let Some(tab_group) = self.tab_groups.get_mut(&tab_group_id) else {
            return false;
        };
        tab_group.reorder(from_index, to_index)
    }

    /// Set the active tab in a tab group.
    /// Returns false if the tab group doesn't exist or is empty.
    pub fn set_active_tab(&mut self, tab_group_id: TabGroupId, index: usize) -> bool {
        let Some(tab_group) = self.tab_groups.get_mut(&tab_group_id) else {
            return false;
        };
        tab_group.set_active(index)
    }

    /// Find which tab group (if any) a window belongs to.
    pub fn tab_group_for_window(&self, window_id: WindowId) -> Option<TabGroupId> {
        self.window_to_tab_group.get(&window_id).copied()
    }

    /// Returns an iterator over all tab groups.
    pub fn tab_groups(&self) -> impl Iterator<Item = &TabGroup> {
        self.tab_groups.values()
    }

    /// Delete a tab group entirely, removing all tab associations.
    /// The windows remain in their window groups (if any).
    pub fn delete_tab_group(&mut self, tab_group_id: TabGroupId) -> bool {
        let Some(tab_group) = self.tab_groups.remove(&tab_group_id) else {
            return false;
        };
        for &wid in &tab_group.tabs {
            self.window_to_tab_group.remove(&wid);
        }
        true
    }

    // ---- Auto-grouping ----

    /// Register a window with the auto-grouping system.
    /// If the policy is `ByApplication`, the window is added to the group
    /// for its `app_id` (creating one if needed).
    /// If the policy is `ByWorkspace`, the window is added to the group
    /// for its `workspace_id` (creating one if needed).
    /// Returns the GroupId the window was added to, or None if manual policy
    /// or the window is already grouped.
    pub fn auto_group_window(
        &mut self,
        window_id: WindowId,
        app_id: Option<&str>,
        workspace_id: Option<u64>,
    ) -> Option<GroupId> {
        if self.window_to_group.contains_key(&window_id) {
            return None;
        }

        match self.auto_group_policy {
            AutoGroupPolicy::ByApplication => {
                let app = app_id?;
                let group_id = if let Some(&gid) = self.app_group_index.get(app) {
                    gid
                } else {
                    let gid = self.create_group(app);
                    if let Some(g) = self.groups.get_mut(&gid) {
                        g.app_id = Some(app.to_string());
                    }
                    self.app_group_index.insert(app.to_string(), gid);
                    gid
                };
                self.add_to_group(group_id, window_id);
                Some(group_id)
            }
            AutoGroupPolicy::ByWorkspace => {
                let ws = workspace_id?;
                let group_id = if let Some(&gid) = self.workspace_group_index.get(&ws) {
                    gid
                } else {
                    let label = format!("Workspace {}", ws);
                    let gid = self.create_group(label);
                    self.workspace_group_index.insert(ws, gid);
                    gid
                };
                self.add_to_group(group_id, window_id);
                Some(group_id)
            }
            AutoGroupPolicy::Manual => None,
        }
    }

    /// Unregister a window from all groups and tab groups.
    /// Call this when a window is destroyed.
    pub fn unregister_window(&mut self, window_id: WindowId) {
        // Remove from window group.
        if let Some(gid) = self.window_to_group.remove(&window_id) {
            if let Some(group) = self.groups.get_mut(&gid) {
                group.remove_window(window_id);
            }
        }
        // Remove from tab group.
        if let Some(tgid) = self.window_to_tab_group.remove(&window_id) {
            if let Some(tab_group) = self.tab_groups.get_mut(&tgid) {
                tab_group.remove_tab(window_id);
                if tab_group.tabs.is_empty() {
                    self.tab_groups.remove(&tgid);
                }
            }
        }
    }

    // ---- Minimize policy ----

    // ---- Tab navigation ----

    /// Switch to the next tab in a tab group. Wraps around.
    /// Returns the newly active window, or None if the group doesn't exist.
    pub fn tab_next(&mut self, tab_group_id: TabGroupId) -> Option<WindowId> {
        let tg = self.tab_groups.get_mut(&tab_group_id)?;
        if tg.tabs.is_empty() {
            return None;
        }
        let old = tg.active_tab;
        let new_idx = (old + 1) % tg.tabs.len();
        tg.active_tab = new_idx;
        let wid = tg.tabs[new_idx];
        self.events.push(GroupEvent::TabChanged {
            tab_group_id,
            old_index: old,
            new_index: new_idx,
            window_id: wid,
        });
        Some(wid)
    }

    /// Switch to the previous tab in a tab group. Wraps around.
    /// Returns the newly active window, or None if the group doesn't exist.
    pub fn tab_prev(&mut self, tab_group_id: TabGroupId) -> Option<WindowId> {
        let tg = self.tab_groups.get_mut(&tab_group_id)?;
        if tg.tabs.is_empty() {
            return None;
        }
        let old = tg.active_tab;
        let new_idx = if old == 0 { tg.tabs.len() - 1 } else { old - 1 };
        tg.active_tab = new_idx;
        let wid = tg.tabs[new_idx];
        self.events.push(GroupEvent::TabChanged {
            tab_group_id,
            old_index: old,
            new_index: new_idx,
            window_id: wid,
        });
        Some(wid)
    }

    /// Switch to a specific tab by index in a tab group.
    /// Returns the newly active window, or None if invalid.
    pub fn tab_to(&mut self, tab_group_id: TabGroupId, index: usize) -> Option<WindowId> {
        let tg = self.tab_groups.get_mut(&tab_group_id)?;
        if index >= tg.tabs.len() {
            return None;
        }
        let old = tg.active_tab;
        tg.active_tab = index;
        let wid = tg.tabs[index];
        if old != index {
            self.events.push(GroupEvent::TabChanged {
                tab_group_id,
                old_index: old,
                new_index: index,
                window_id: wid,
            });
        }
        Some(wid)
    }

    // ---- Minimize policy ----

    /// Given that a window is being minimized, returns the list of
    /// additional windows that should also be minimized according to
    /// the current minimize policy.
    pub fn windows_to_minimize_with(&self, window_id: WindowId) -> Vec<WindowId> {
        match self.minimize_policy {
            GroupMinimizePolicy::Individual => Vec::new(),
            GroupMinimizePolicy::All => {
                let mut result = Vec::new();
                if let Some(&gid) = self.window_to_group.get(&window_id) {
                    if let Some(group) = self.groups.get(&gid) {
                        for &wid in &group.windows {
                            if wid != window_id {
                                result.push(wid);
                            }
                        }
                    }
                }
                result
            }
        }
    }
}
