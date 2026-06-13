//! Authorization subjects — the entity requesting a privileged action.
//!
//! A [`Subject`] describes *who* is asking: their user ID, process ID,
//! session, and group memberships. The authorization framework uses this
//! information to select the appropriate policy defaults and to decide
//! whether admin privileges are already held.

use serde::{Deserialize, Serialize};

/// The kind of entity requesting authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubjectKind {
    /// An interactive user at a console or remote session.
    User,
    /// A background process (daemon, cron job, etc.).
    Process,
    /// A login session (the session itself, not a particular user).
    Session,
}

/// Identifies the entity requesting a privileged action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subject {
    /// Numeric user identifier.
    pub uid: u32,
    /// Process identifier of the requesting process.
    pub pid: u32,
    /// Login-session identifier (opaque string).
    pub session_id: String,
    /// Group names the subject belongs to.
    pub user_groups: Vec<String>,
    /// What kind of entity this is.
    pub kind: SubjectKind,
    /// Whether this subject is on a physically-local session (as opposed
    /// to a remote/SSH session).
    pub local_session: bool,
}

impl Subject {
    /// Create a new subject of kind [`SubjectKind::User`].
    #[must_use]
    pub fn new(uid: u32, pid: u32, session_id: impl Into<String>) -> Self {
        Self {
            uid,
            pid,
            session_id: session_id.into(),
            user_groups: Vec::new(),
            kind: SubjectKind::User,
            local_session: false,
        }
    }

    /// Add a group membership.
    #[must_use]
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.user_groups.push(group.into());
        self
    }

    /// Set the subject kind.
    #[must_use]
    pub fn with_kind(mut self, kind: SubjectKind) -> Self {
        self.kind = kind;
        self
    }

    /// Mark this subject as being on a physically-local session.
    #[must_use]
    pub fn as_local(mut self) -> Self {
        self.local_session = true;
        self
    }

    /// Whether this subject has a local (non-remote) session.
    #[must_use]
    pub fn is_local_session(&self) -> bool {
        self.local_session
    }

    /// Whether the subject belongs to a given group.
    #[must_use]
    pub fn in_group(&self, group: &str) -> bool {
        self.user_groups.iter().any(|g| g == group)
    }
}

impl std::fmt::Display for Subject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Subject(uid={}, pid={}, session={})",
            self.uid, self.pid, self.session_id
        )
    }
}

/// Group names that confer administrative privileges.
const ADMIN_GROUPS: &[&str] = &["admin", "wheel", "sudo", "root"];

/// Check whether a subject has administrative privileges based on
/// group membership or a root-equivalent uid.
#[must_use]
pub fn is_admin(subject: &Subject) -> bool {
    if subject.uid == 0 {
        return true;
    }
    for group in &subject.user_groups {
        if ADMIN_GROUPS.contains(&group.as_str()) {
            return true;
        }
    }
    false
}

/// A simple resource descriptor for ownership checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resource {
    /// The user-id of the resource owner.
    pub owner_uid: u32,
    /// Human-readable path or label.
    pub path: String,
}

impl Resource {
    /// Create a resource descriptor.
    #[must_use]
    pub fn new(owner_uid: u32, path: impl Into<String>) -> Self {
        Self {
            owner_uid,
            path: path.into(),
        }
    }

    /// Stable identifier used for resource-scoped audit entries.
    #[must_use]
    pub fn resource_id(&self) -> &str {
        &self.path
    }
}

/// Check whether `subject` owns `resource`.
#[must_use]
pub fn is_owner(subject: &Subject, resource: &Resource) -> bool {
    subject.uid == resource.owner_uid
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_subject() {
        let s = Subject::new(1000, 42, "session-abc");
        assert_eq!(s.uid, 1000);
        assert_eq!(s.pid, 42);
        assert_eq!(s.session_id, "session-abc");
        assert!(s.user_groups.is_empty());
        assert_eq!(s.kind, SubjectKind::User);
        assert!(!s.local_session);
    }

    #[test]
    fn with_group() {
        let s = Subject::new(1000, 42, "s")
            .with_group("users")
            .with_group("audio");
        assert_eq!(s.user_groups.len(), 2);
        assert!(s.in_group("users"));
        assert!(s.in_group("audio"));
        assert!(!s.in_group("admin"));
    }

    #[test]
    fn with_kind() {
        let s = Subject::new(0, 1, "s").with_kind(SubjectKind::Process);
        assert_eq!(s.kind, SubjectKind::Process);
    }

    #[test]
    fn as_local() {
        let s = Subject::new(1000, 1, "s").as_local();
        assert!(s.is_local_session());
    }

    #[test]
    fn not_local_by_default() {
        let s = Subject::new(1000, 1, "s");
        assert!(!s.is_local_session());
    }

    #[test]
    fn display() {
        let s = Subject::new(1000, 42, "sess-1");
        assert_eq!(s.to_string(), "Subject(uid=1000, pid=42, session=sess-1)");
    }

    #[test]
    fn is_admin_root_uid() {
        let s = Subject::new(0, 1, "s");
        assert!(is_admin(&s));
    }

    #[test]
    fn is_admin_admin_group() {
        let s = Subject::new(1000, 1, "s").with_group("admin");
        assert!(is_admin(&s));
    }

    #[test]
    fn is_admin_wheel_group() {
        let s = Subject::new(1000, 1, "s").with_group("wheel");
        assert!(is_admin(&s));
    }

    #[test]
    fn is_admin_sudo_group() {
        let s = Subject::new(1000, 1, "s").with_group("sudo");
        assert!(is_admin(&s));
    }

    #[test]
    fn is_admin_root_group() {
        let s = Subject::new(1000, 1, "s").with_group("root");
        assert!(is_admin(&s));
    }

    #[test]
    fn is_not_admin_regular_user() {
        let s = Subject::new(1000, 1, "s")
            .with_group("users")
            .with_group("audio");
        assert!(!is_admin(&s));
    }

    #[test]
    fn is_owner_match() {
        let s = Subject::new(1000, 1, "s");
        let r = Resource::new(1000, "/home/user/file.txt");
        assert!(is_owner(&s, &r));
    }

    #[test]
    fn is_owner_mismatch() {
        let s = Subject::new(1000, 1, "s");
        let r = Resource::new(0, "/etc/shadow");
        assert!(!is_owner(&s, &r));
    }

    #[test]
    fn resource_new() {
        let r = Resource::new(500, "/tmp/data");
        assert_eq!(r.owner_uid, 500);
        assert_eq!(r.path, "/tmp/data");
    }

    #[test]
    fn subject_serde_roundtrip() {
        let s = Subject::new(1000, 42, "session-1")
            .with_group("users")
            .with_group("admin")
            .as_local();
        let json = serde_json::to_string(&s).unwrap();
        let back: Subject = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn subject_kind_serde_roundtrip() {
        for kind in [
            SubjectKind::User,
            SubjectKind::Process,
            SubjectKind::Session,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: SubjectKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }
}
