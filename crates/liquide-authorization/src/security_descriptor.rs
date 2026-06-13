//! Resource-scoped authorization descriptors.
//!
//! This module models object-level access checks without tying Liquide to any
//! platform ACL format. Descriptors are explicit, ordered, and evaluated
//! against the existing [`Subject`](crate::subject::Subject) model.

use serde::{Deserialize, Serialize};

use crate::policy_db::AuthDecision;
use crate::subject::{self, Subject};

/// Bitset of capabilities requested or granted for a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct CapabilitySet {
    bits: u64,
}

impl CapabilitySet {
    /// Empty capability set.
    pub const EMPTY: Self = Self { bits: 0 };
    /// Read or observe a resource.
    pub const READ: Self = Self { bits: 1 << 0 };
    /// Mutate a resource.
    pub const WRITE: Self = Self { bits: 1 << 1 };
    /// Execute or activate a resource.
    pub const EXECUTE: Self = Self { bits: 1 << 2 };
    /// Change ownership, descriptors, or policy.
    pub const ADMIN: Self = Self { bits: 1 << 3 };
    /// Capture or inspect screen/display data.
    pub const CAPTURE: Self = Self { bits: 1 << 4 };
    /// Redirect input or device events.
    pub const INPUT: Self = Self { bits: 1 << 5 };
    /// Clipboard transfer access.
    pub const CLIPBOARD: Self = Self { bits: 1 << 6 };
    /// Network-facing access.
    pub const NETWORK: Self = Self { bits: 1 << 7 };
    /// Device/hardware access.
    pub const DEVICE: Self = Self { bits: 1 << 8 };

    /// Create from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// Raw bit representation.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.bits
    }

    /// Return true when no capabilities are set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    /// Return true when all `other` capabilities are present.
    #[must_use]
    pub const fn contains_all(self, other: Self) -> bool {
        (self.bits & other.bits) == other.bits
    }

    /// Return true when any `other` capability is present.
    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        (self.bits & other.bits) != 0
    }

    /// Add capabilities.
    pub fn insert(&mut self, other: Self) {
        self.bits |= other.bits;
    }

    /// Remove capabilities.
    pub fn remove(&mut self, other: Self) {
        self.bits &= !other.bits;
    }

    /// Union with another set.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    /// Intersection with another set.
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }

    /// Capabilities in `self` that are not present in `other`.
    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self {
            bits: self.bits & !other.bits,
        }
    }
}

impl std::ops::BitOr for CapabilitySet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for CapabilitySet {
    fn bitor_assign(&mut self, rhs: Self) {
        self.insert(rhs);
    }
}

impl std::ops::BitAnd for CapabilitySet {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.intersection(rhs)
    }
}

/// Principal matched by an access-control entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Principal {
    /// Any subject.
    Any,
    /// Subject with a specific uid.
    Uid(u32),
    /// Subject in a named group.
    Group(String),
    /// Subject from a specific session.
    Session(String),
    /// Subject with administrative privileges.
    Admin,
    /// Owner of the resource described by the descriptor.
    Owner,
}

impl Principal {
    /// Return true if this principal matches `subject` for a descriptor owner.
    #[must_use]
    pub fn matches(&self, subject: &Subject, owner_uid: u32) -> bool {
        match self {
            Self::Any => true,
            Self::Uid(uid) => subject.uid == *uid,
            Self::Group(group) => subject.in_group(group),
            Self::Session(session_id) => subject.session_id == *session_id,
            Self::Admin => subject::is_admin(subject),
            Self::Owner => subject.uid == owner_uid,
        }
    }
}

/// Whether an ACE grants or denies capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AceEffect {
    /// Grant matching capabilities.
    Allow,
    /// Deny matching capabilities.
    Deny,
}

/// One access-control entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessControlEntry {
    /// Allow or deny.
    pub effect: AceEffect,
    /// Principal this ACE applies to.
    pub principal: Principal,
    /// Capabilities covered by this ACE.
    pub capabilities: CapabilitySet,
    /// Optional explanation for audit UIs.
    pub description: Option<String>,
}

impl AccessControlEntry {
    /// Create a new ACE.
    #[must_use]
    pub fn new(effect: AceEffect, principal: Principal, capabilities: CapabilitySet) -> Self {
        Self {
            effect,
            principal,
            capabilities,
            description: None,
        }
    }

    /// Create an allow ACE.
    #[must_use]
    pub fn allow(principal: Principal, capabilities: CapabilitySet) -> Self {
        Self::new(AceEffect::Allow, principal, capabilities)
    }

    /// Create a deny ACE.
    #[must_use]
    pub fn deny(principal: Principal, capabilities: CapabilitySet) -> Self {
        Self::new(AceEffect::Deny, principal, capabilities)
    }

    /// Attach an audit-facing description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Result of a resource access check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessCheckResult {
    /// Authorization decision.
    pub decision: AuthDecision,
    /// Capabilities granted before the decision was reached.
    pub granted: CapabilitySet,
    /// Capabilities still missing for a grant.
    pub missing: CapabilitySet,
    /// Index of the denying ACE, if a deny matched.
    pub denied_by: Option<usize>,
}

impl AccessCheckResult {
    /// True when the check allowed the requested access.
    #[must_use]
    pub fn is_allow(&self) -> bool {
        self.decision.is_allow()
    }
}

/// Resource-level security descriptor with ordered ACEs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityDescriptor {
    /// Stable resource identifier, such as `window:42` or `session:abc`.
    pub resource_id: String,
    /// Optional scope label, such as `window`, `session`, or `device`.
    pub scope: Option<String>,
    /// Resource owner uid.
    pub owner_uid: u32,
    /// Ordered access-control entries.
    entries: Vec<AccessControlEntry>,
}

impl SecurityDescriptor {
    /// Create an empty descriptor.
    #[must_use]
    pub fn new(resource_id: impl Into<String>, owner_uid: u32) -> Self {
        Self {
            resource_id: resource_id.into(),
            scope: None,
            owner_uid,
            entries: Vec::new(),
        }
    }

    /// Attach a scope label.
    #[must_use]
    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// Append an ACE.
    pub fn push_entry(&mut self, entry: AccessControlEntry) {
        self.entries.push(entry);
    }

    /// Append an allow ACE.
    pub fn allow(&mut self, principal: Principal, capabilities: CapabilitySet) {
        self.push_entry(AccessControlEntry::allow(principal, capabilities));
    }

    /// Append a deny ACE.
    pub fn deny(&mut self, principal: Principal, capabilities: CapabilitySet) {
        self.push_entry(AccessControlEntry::deny(principal, capabilities));
    }

    /// Immutable ACE list.
    #[must_use]
    pub fn entries(&self) -> &[AccessControlEntry] {
        &self.entries
    }

    /// Check whether `subject` has all requested capabilities.
    #[must_use]
    pub fn check_access(&self, subject: &Subject, desired: CapabilitySet) -> AccessCheckResult {
        if desired.is_empty() {
            return AccessCheckResult {
                decision: AuthDecision::Allow,
                granted: CapabilitySet::EMPTY,
                missing: CapabilitySet::EMPTY,
                denied_by: None,
            };
        }

        let mut granted = CapabilitySet::EMPTY;

        for (index, entry) in self.entries.iter().enumerate() {
            if !entry.principal.matches(subject, self.owner_uid) {
                continue;
            }

            let covered = entry.capabilities.intersection(desired);
            if covered.is_empty() {
                continue;
            }

            match entry.effect {
                AceEffect::Deny => {
                    return AccessCheckResult {
                        decision: AuthDecision::Deny,
                        granted,
                        missing: desired.without(granted),
                        denied_by: Some(index),
                    };
                }
                AceEffect::Allow => {
                    granted.insert(covered);
                    if granted.contains_all(desired) {
                        return AccessCheckResult {
                            decision: AuthDecision::Allow,
                            granted,
                            missing: CapabilitySet::EMPTY,
                            denied_by: None,
                        };
                    }
                }
            }
        }

        AccessCheckResult {
            decision: AuthDecision::Deny,
            granted,
            missing: desired.without(granted),
            denied_by: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject() -> Subject {
        Subject::new(1000, 42, "session-1").with_group("users")
    }

    #[test]
    fn security_descriptor_deny_before_allow_wins() {
        let mut descriptor = SecurityDescriptor::new("window:7", 1000);
        descriptor.deny(Principal::Any, CapabilitySet::READ);
        descriptor.allow(Principal::Owner, CapabilitySet::READ);

        let result = descriptor.check_access(&subject(), CapabilitySet::READ);

        assert!(result.decision.is_deny());
        assert_eq!(result.denied_by, Some(0));
    }

    #[test]
    fn security_descriptor_owner_can_be_granted() {
        let mut descriptor = SecurityDescriptor::new("window:7", 1000);
        descriptor.allow(Principal::Owner, CapabilitySet::READ | CapabilitySet::WRITE);

        let result =
            descriptor.check_access(&subject(), CapabilitySet::READ | CapabilitySet::WRITE);

        assert!(result.is_allow());
        assert!(
            result
                .granted
                .contains_all(CapabilitySet::READ | CapabilitySet::WRITE)
        );
        assert!(result.missing.is_empty());
    }

    #[test]
    fn security_descriptor_reports_missing_partial_grant() {
        let mut descriptor = SecurityDescriptor::new("window:7", 1000);
        descriptor.allow(Principal::Owner, CapabilitySet::READ);

        let result =
            descriptor.check_access(&subject(), CapabilitySet::READ | CapabilitySet::WRITE);

        assert!(result.decision.is_deny());
        assert!(result.granted.contains_all(CapabilitySet::READ));
        assert!(result.missing.contains_all(CapabilitySet::WRITE));
    }

    #[test]
    fn security_descriptor_group_principal_matches() {
        let mut descriptor = SecurityDescriptor::new("device:audio", 0);
        descriptor.allow(Principal::Group("users".to_string()), CapabilitySet::DEVICE);

        let result = descriptor.check_access(&subject(), CapabilitySet::DEVICE);

        assert!(result.is_allow());
    }

    #[test]
    fn security_descriptor_admin_principal_matches_root() {
        let mut descriptor = SecurityDescriptor::new("session:root", 0);
        descriptor.allow(Principal::Admin, CapabilitySet::ADMIN);

        let root = Subject::new(0, 1, "session-root");
        let result = descriptor.check_access(&root, CapabilitySet::ADMIN);

        assert!(result.is_allow());
    }
}
