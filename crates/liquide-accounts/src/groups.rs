//! Group management types.

/// A system group.
#[derive(Debug, Clone)]
pub struct Group {
    /// Numeric group identifier (GID).
    pub gid: u32,
    /// Group name (e.g. `"sudo"`, `"wheel"`, `"users"`).
    pub name: String,
    /// UIDs of users who are members of this group.
    pub members: Vec<u32>,
}

impl Group {
    /// Returns `true` if the given user is a member of this group.
    pub fn contains(&self, uid: u32) -> bool {
        self.members.contains(&uid)
    }

    /// Number of members in the group.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
}

impl std::fmt::Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (gid={}, {} members)",
            self.name,
            self.gid,
            self.members.len()
        )
    }
}
