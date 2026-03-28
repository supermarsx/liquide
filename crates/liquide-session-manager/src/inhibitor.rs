//! Logout/sleep inhibitor system.
//!
//! Applications can register inhibitors to prevent the session from logging out,
//! switching users, suspending, or going idle. The session manager checks these
//! before proceeding with a state transition.

use std::fmt;

/// Flags describing what a given inhibitor prevents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InhibitFlag(u32);

impl InhibitFlag {
    /// Prevent session logout.
    pub const LOGOUT: Self = Self(1 << 0);
    /// Prevent user switching.
    pub const SWITCH_USER: Self = Self(1 << 1);
    /// Prevent system suspend/sleep.
    pub const SUSPEND: Self = Self(1 << 2);
    /// Prevent idle timeout (e.g. screen dimming or auto-lock).
    pub const IDLE: Self = Self(1 << 3);

    /// All flags combined.
    pub const ALL: Self = Self(0b1111);

    /// Empty (no flags set).
    pub const NONE: Self = Self(0);

    /// Combine two flag sets.
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check whether `self` contains all bits in `other`.
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Check whether any bit overlaps between `self` and `other`.
    #[inline]
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// The raw bits.
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Construct from raw bits (unrecognized bits are preserved).
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }
}

impl fmt::Display for InhibitFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.contains(Self::LOGOUT) {
            parts.push("logout");
        }
        if self.contains(Self::SWITCH_USER) {
            parts.push("switch-user");
        }
        if self.contains(Self::SUSPEND) {
            parts.push("suspend");
        }
        if self.contains(Self::IDLE) {
            parts.push("idle");
        }
        if parts.is_empty() {
            write!(f, "none")
        } else {
            write!(f, "{}", parts.join("|"))
        }
    }
}

/// A single registered inhibitor.
#[derive(Debug, Clone)]
pub struct Inhibitor {
    /// Unique identifier assigned by the registry.
    pub id: u64,
    /// Application that created this inhibitor.
    pub app_id: String,
    /// Human-readable reason (shown to the user if they try to e.g. log out).
    pub reason: String,
    /// What this inhibitor prevents.
    pub flags: InhibitFlag,
    /// When this inhibitor was created (unix epoch milliseconds).
    pub created_ms: u64,
}

/// Registry that tracks all active inhibitors.
pub struct InhibitorRegistry {
    inhibitors: Vec<Inhibitor>,
    next_id: u64,
}

impl InhibitorRegistry {
    pub fn new() -> Self {
        Self {
            inhibitors: Vec::new(),
            next_id: 1,
        }
    }

    /// Register a new inhibitor. Returns its unique id.
    pub fn add(&mut self, app_id: &str, reason: &str, flags: InhibitFlag) -> u64 {
        self.add_with_time(app_id, reason, flags, 0)
    }

    /// Register a new inhibitor with an explicit creation timestamp.
    pub fn add_with_time(
        &mut self,
        app_id: &str,
        reason: &str,
        flags: InhibitFlag,
        created_ms: u64,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.inhibitors.push(Inhibitor {
            id,
            app_id: app_id.to_string(),
            reason: reason.to_string(),
            flags,
            created_ms,
        });
        id
    }

    /// Remove an inhibitor by id. Returns `true` if it existed.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.inhibitors.len();
        self.inhibitors.retain(|inh| inh.id != id);
        self.inhibitors.len() < before
    }

    /// Check whether any active inhibitor blocks the given flag.
    pub fn is_inhibited(&self, flag: InhibitFlag) -> bool {
        self.inhibitors.iter().any(|inh| inh.flags.intersects(flag))
    }

    /// Return a reference to all active inhibitors.
    pub fn active_inhibitors(&self) -> &[Inhibitor] {
        &self.inhibitors
    }

    /// Return inhibitors that match a specific flag.
    pub fn inhibitors_for(&self, flag: InhibitFlag) -> Vec<&Inhibitor> {
        self.inhibitors
            .iter()
            .filter(|inh| inh.flags.intersects(flag))
            .collect()
    }

    /// Remove inhibitors older than `max_age_ms` relative to `now_ms`.
    pub fn clear_expired(&mut self, now_ms: u64, max_age_ms: u64) {
        self.inhibitors.retain(|inh| {
            // Guard against underflow if now_ms < created_ms
            now_ms.saturating_sub(inh.created_ms) < max_age_ms
        });
    }

    /// Number of active inhibitors.
    pub fn count(&self) -> usize {
        self.inhibitors.len()
    }

    /// Remove all inhibitors from a specific application.
    pub fn remove_all_for_app(&mut self, app_id: &str) {
        self.inhibitors.retain(|inh| inh.app_id != app_id);
    }
}

impl Default for InhibitorRegistry {
    fn default() -> Self {
        Self::new()
    }
}
