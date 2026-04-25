//! Machine registry — saved servers, groups, and status tracking.

use std::collections::HashMap;

/// A saved remote machine entry.
#[derive(Debug, Clone)]
pub struct MachineEntry {
    id: String,
    name: String,
    address: String,
    username: Option<String>,
    group: Option<String>,
    last_connected: Option<u64>,
    session_available: bool,
    online: Option<bool>,
    thumbnail_path: Option<String>,
}

impl MachineEntry {
    /// Create a new machine entry.
    #[must_use]
    pub fn new(id: String, name: String, address: String) -> Self {
        Self {
            id,
            name,
            address,
            username: None,
            group: None,
            last_connected: None,
            session_available: false,
            online: None,
            thumbnail_path: None,
        }
    }

    /// Unique identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Human-readable name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Server address (host:port).
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Optional username for this machine.
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    /// Group this machine belongs to, if any.
    #[must_use]
    pub fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    /// Epoch timestamp of the last successful connection.
    #[must_use]
    pub fn last_connected(&self) -> Option<u64> {
        self.last_connected
    }

    /// Whether the machine has an active session ready to join.
    #[must_use]
    pub fn has_active_session(&self) -> bool {
        self.session_available
    }

    /// Whether the machine is believed to be online.
    #[must_use]
    pub fn is_online(&self) -> Option<bool> {
        self.online
    }

    /// Path to a cached thumbnail, if available.
    #[must_use]
    pub fn thumbnail_path(&self) -> Option<&str> {
        self.thumbnail_path.as_deref()
    }

    /// Human-friendly display name. Falls back to the address if name is empty.
    #[must_use]
    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.address
        } else {
            &self.name
        }
    }

    /// Set the username.
    pub fn set_username(&mut self, username: Option<String>) {
        self.username = username;
    }

    /// Set the group.
    pub fn set_group(&mut self, group: Option<String>) {
        self.group = group;
    }

    /// Update online and session status.
    pub fn set_status(&mut self, online: Option<bool>, session_available: bool) {
        self.online = online;
        self.session_available = session_available;
    }

    /// Record a successful connection at the given epoch timestamp.
    pub fn record_connection(&mut self, timestamp: u64) {
        self.last_connected = Some(timestamp);
    }

    /// Set the thumbnail path.
    pub fn set_thumbnail_path(&mut self, path: Option<String>) {
        self.thumbnail_path = path;
    }
}

/// A named group of machines in the sidebar.
#[derive(Debug, Clone)]
pub struct MachineGroup {
    pub name: String,
    pub machine_ids: Vec<String>,
    pub collapsed: bool,
}

/// Manages the set of known machines and groups.
pub struct MachineManager {
    machines: HashMap<String, MachineEntry>,
    groups: Vec<MachineGroup>,
    next_id: u64,
}

impl MachineManager {
    /// Create an empty machine manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            machines: HashMap::new(),
            groups: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a machine. Returns the assigned id.
    pub fn add_machine(&mut self, name: &str, address: &str) -> String {
        let id = format!("machine-{}", self.next_id);
        self.next_id += 1;
        let entry = MachineEntry::new(id.clone(), name.to_string(), address.to_string());
        self.machines.insert(id.clone(), entry);
        id
    }

    /// Remove a machine by id. Returns `true` if found.
    pub fn remove_machine(&mut self, id: &str) -> bool {
        let removed = self.machines.remove(id).is_some();
        if removed {
            for group in &mut self.groups {
                group.machine_ids.retain(|mid| mid != id);
            }
        }
        removed
    }

    /// Get a machine by id.
    #[must_use]
    pub fn get_machine(&self, id: &str) -> Option<&MachineEntry> {
        self.machines.get(id)
    }

    /// Update the status of a machine.
    pub fn update_status(
        &mut self,
        id: &str,
        online: Option<bool>,
        session_available: bool,
    ) -> bool {
        if let Some(entry) = self.machines.get_mut(id) {
            entry.set_status(online, session_available);
            true
        } else {
            false
        }
    }

    /// All machine ids that belong to the named group.
    #[must_use]
    pub fn machines_in_group(&self, group_name: &str) -> Vec<&MachineEntry> {
        if let Some(group) = self.groups.iter().find(|g| g.name == group_name) {
            group
                .machine_ids
                .iter()
                .filter_map(|id| self.machines.get(id))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// All known machines.
    #[must_use]
    pub fn all_machines(&self) -> Vec<&MachineEntry> {
        self.machines.values().collect()
    }

    /// Machines sorted by most recently connected first.
    #[must_use]
    pub fn recent_machines(&self) -> Vec<&MachineEntry> {
        let mut entries: Vec<&MachineEntry> = self.machines.values().collect();
        entries.sort_by(|a, b| b.last_connected().cmp(&a.last_connected()));
        entries
    }

    /// Create a new group. Returns `false` if the name already exists.
    pub fn create_group(&mut self, name: &str) -> bool {
        if self.groups.iter().any(|g| g.name == name) {
            return false;
        }
        self.groups.push(MachineGroup {
            name: name.to_string(),
            machine_ids: Vec::new(),
            collapsed: false,
        });
        true
    }

    /// Delete a group by name. Does not remove the machines themselves.
    pub fn delete_group(&mut self, name: &str) -> bool {
        let before = self.groups.len();
        self.groups.retain(|g| g.name != name);
        self.groups.len() < before
    }

    /// Record a connection timestamp for a machine.
    pub fn record_connection(&mut self, id: &str, timestamp: u64) {
        if let Some(entry) = self.machines.get_mut(id) {
            entry.record_connection(timestamp);
        }
    }

    /// Move a machine into a group. Creates the group if it does not exist.
    pub fn move_to_group(&mut self, machine_id: &str, group_name: &str) {
        if !self.machines.contains_key(machine_id) {
            return;
        }

        // Remove from any current group.
        for group in &mut self.groups {
            group.machine_ids.retain(|mid| mid != machine_id);
        }

        // Ensure group exists.
        if !self.groups.iter().any(|g| g.name == group_name) {
            self.groups.push(MachineGroup {
                name: group_name.to_string(),
                machine_ids: Vec::new(),
                collapsed: false,
            });
        }

        if let Some(group) = self.groups.iter_mut().find(|g| g.name == group_name) {
            group.machine_ids.push(machine_id.to_string());
        }

        if let Some(entry) = self.machines.get_mut(machine_id) {
            entry.set_group(Some(group_name.to_string()));
        }
    }
}

impl Default for MachineManager {
    fn default() -> Self {
        Self::new()
    }
}
