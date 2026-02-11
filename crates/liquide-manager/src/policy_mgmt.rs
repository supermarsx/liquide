//! Policy management operations.

use serde::{Deserialize, Serialize};

/// A versioned policy snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyVersion {
    /// Monotonic version number.
    pub version: u64,
    /// Who made the change.
    pub changed_by: String,
    /// When the change was made (epoch seconds).
    pub timestamp: u64,
    /// Human-readable description.
    pub description: String,
    /// The policy entries at this version.
    pub entries: Vec<PolicyEntry>,
}

/// A single policy key-value entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEntry {
    /// Policy key (e.g. `clipboard.enabled`).
    pub key: String,
    /// Policy value as a JSON string.
    pub value: String,
    /// Scope: default, group, or user.
    pub scope: PolicyScope,
    /// Target (group name or username), empty for default scope.
    pub target: String,
}

/// Policy scope level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyScope {
    /// Server-wide default.
    Default,
    /// Applied to a group.
    Group,
    /// Applied to a specific user.
    User,
    /// Per-session override (ephemeral).
    Session,
}

impl std::fmt::Display for PolicyScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => write!(f, "default"),
            Self::Group => write!(f, "group"),
            Self::User => write!(f, "user"),
            Self::Session => write!(f, "session"),
        }
    }
}

/// A diff entry describing what changed between versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDiff {
    /// The key that changed.
    pub key: String,
    /// Previous value (None if newly added).
    pub old_value: Option<String>,
    /// New value (None if removed).
    pub new_value: Option<String>,
}

/// Policy version store with history.
pub struct PolicyStore {
    versions: Vec<PolicyVersion>,
    current_version: u64,
}

impl PolicyStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            versions: Vec::new(),
            current_version: 0,
        }
    }

    /// Get the current version number.
    #[must_use]
    pub fn current_version(&self) -> u64 {
        self.current_version
    }

    /// Get the current policy entries.
    #[must_use]
    pub fn current_entries(&self) -> Vec<&PolicyEntry> {
        self.versions
            .last()
            .map(|v| v.entries.iter().collect())
            .unwrap_or_default()
    }

    /// Create a new version with the given entries.
    pub fn commit(
        &mut self,
        entries: Vec<PolicyEntry>,
        changed_by: String,
        description: String,
        timestamp: u64,
    ) -> u64 {
        self.current_version += 1;
        let version = PolicyVersion {
            version: self.current_version,
            changed_by,
            timestamp,
            description,
            entries,
        };
        self.versions.push(version);
        self.current_version
    }

    /// Get a specific version.
    #[must_use]
    pub fn get_version(&self, version: u64) -> Option<&PolicyVersion> {
        self.versions.iter().find(|v| v.version == version)
    }

    /// Get version history.
    #[must_use]
    pub fn history(&self) -> &[PolicyVersion] {
        &self.versions
    }

    /// Compute the diff between two versions.
    #[must_use]
    pub fn diff(&self, from: u64, to: u64) -> Vec<PolicyDiff> {
        let from_entries = self
            .get_version(from)
            .map(|v| &v.entries[..])
            .unwrap_or_default();
        let to_entries = self
            .get_version(to)
            .map(|v| &v.entries[..])
            .unwrap_or_default();

        let mut diffs = Vec::new();

        // Check for modified or removed entries.
        for old in from_entries {
            match to_entries.iter().find(|e| e.key == old.key && e.scope == old.scope && e.target == old.target) {
                Some(new) if new.value != old.value => {
                    diffs.push(PolicyDiff {
                        key: old.key.clone(),
                        old_value: Some(old.value.clone()),
                        new_value: Some(new.value.clone()),
                    });
                }
                None => {
                    diffs.push(PolicyDiff {
                        key: old.key.clone(),
                        old_value: Some(old.value.clone()),
                        new_value: None,
                    });
                }
                _ => {}
            }
        }

        // Check for added entries.
        for new in to_entries {
            if !from_entries.iter().any(|e| e.key == new.key && e.scope == new.scope && e.target == new.target) {
                diffs.push(PolicyDiff {
                    key: new.key.clone(),
                    old_value: None,
                    new_value: Some(new.value.clone()),
                });
            }
        }

        diffs
    }

    /// Rollback to a previous version. Returns the new version number.
    pub fn rollback(&mut self, target_version: u64, admin: String, timestamp: u64) -> crate::Result<u64> {
        let entries = self
            .get_version(target_version)
            .ok_or_else(|| crate::ManagerError::PolicyError(format!("version {target_version} not found")))?
            .entries
            .clone();

        Ok(self.commit(
            entries,
            admin,
            format!("rollback to version {target_version}"),
            timestamp,
        ))
    }

    /// Total version count.
    #[must_use]
    pub fn version_count(&self) -> usize {
        self.versions.len()
    }
}

impl Default for PolicyStore {
    fn default() -> Self {
        Self::new()
    }
}
