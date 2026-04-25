//! Desktop access control — bitflag-based permission checks.
//!
//! Each thread has an associated desktop. Access checks verify whether a
//! thread is permitted to perform operations on a desktop it may or may not
//! own. By default, threads can only access the desktop they are assigned to.

use crate::types::DesktopId;
use bitflags::bitflags;
use std::collections::HashMap;

bitflags! {
    /// Access rights for desktop operations.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct DesktopAccess: u32 {
        /// Read objects on the desktop (enumerate windows, etc.).
        const READ_OBJECTS    = 0x0001;
        /// Write/modify objects on the desktop.
        const WRITE_OBJECTS   = 0x0002;
        /// Create new windows on the desktop.
        const CREATE_WINDOW   = 0x0004;
        /// Switch to this desktop (make it active).
        const SWITCH_DESKTOP  = 0x0008;
        /// Enumerate desktops in the station.
        const ENUMERATE       = 0x0010;
        /// Create new desktops in the station.
        const CREATE_DESKTOP  = 0x0020;
        /// Hookable — can install hooks on this desktop.
        const HOOK            = 0x0040;
        /// Full access.
        const ALL = Self::READ_OBJECTS.bits()
                  | Self::WRITE_OBJECTS.bits()
                  | Self::CREATE_WINDOW.bits()
                  | Self::SWITCH_DESKTOP.bits()
                  | Self::ENUMERATE.bits()
                  | Self::CREATE_DESKTOP.bits()
                  | Self::HOOK.bits();
    }
}

bitflags! {
    /// Flags controlling window station behaviour.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowStationFlags: u32 {
        /// Station is visible (interactive, connected to display).
        const VISIBLE          = 0x0001;
        /// Clipboard access is allowed.
        const CLIPBOARD_ACCESS = 0x0002;
        /// Creating new desktops is allowed.
        const CREATE_DESKTOP   = 0x0004;
        /// Listing desktops is allowed.
        const ENUMERATE        = 0x0008;
        /// Reading objects is allowed.
        const READ_OBJECTS     = 0x0010;
        /// Writing objects is allowed.
        const WRITE_OBJECTS    = 0x0020;
    }
}

bitflags! {
    /// Flags describing desktop state.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct DesktopFlags: u32 {
        /// This desktop is currently active (visible to the user).
        const ACTIVE      = 0x0001;
        /// Input is being delivered to this desktop.
        const ALLOW_INPUT = 0x0002;
        /// The desktop is locked (secure desktop pattern).
        const LOCKED      = 0x0004;
        /// This is a secure desktop (login, UAC prompts).
        const SECURE      = 0x0008;
    }
}

/// A per-thread access grant: which desktops a thread can access and with
/// what rights.
#[derive(Debug, Clone)]
struct ThreadGrant {
    /// The desktop this thread is assigned to (full access).
    home_desktop: DesktopId,
    /// Additional grants to other desktops.
    extra_grants: HashMap<DesktopId, DesktopAccess>,
}

/// Central access-control database for desktop operations.
#[derive(Debug, Clone)]
pub struct DesktopSecurity {
    /// thread_id -> grant information.
    grants: HashMap<u64, ThreadGrant>,
}

impl DesktopSecurity {
    /// Creates a new, empty security database.
    pub fn new() -> Self {
        Self {
            grants: HashMap::new(),
        }
    }

    /// Registers a thread as belonging to a desktop, granting it full access
    /// to that desktop.
    pub fn assign_thread(&mut self, thread_id: u64, desktop: DesktopId) {
        self.grants.insert(
            thread_id,
            ThreadGrant {
                home_desktop: desktop,
                extra_grants: HashMap::new(),
            },
        );
    }

    /// Removes a thread from the security database.
    pub fn remove_thread(&mut self, thread_id: u64) {
        self.grants.remove(&thread_id);
    }

    /// Grants a thread additional access rights to a specific desktop.
    pub fn grant_access(&mut self, thread_id: u64, desktop: DesktopId, access: DesktopAccess) {
        if let Some(grant) = self.grants.get_mut(&thread_id) {
            let entry = grant
                .extra_grants
                .entry(desktop)
                .or_insert(DesktopAccess::empty());
            *entry |= access;
        }
    }

    /// Revokes specific access rights from a thread for a desktop.
    pub fn revoke_access(&mut self, thread_id: u64, desktop: DesktopId, access: DesktopAccess) {
        if let Some(grant) = self.grants.get_mut(&thread_id) {
            if let Some(entry) = grant.extra_grants.get_mut(&desktop) {
                entry.remove(access);
                if entry.is_empty() {
                    grant.extra_grants.remove(&desktop);
                }
            }
        }
    }

    /// Checks whether a thread has the required access to a desktop.
    ///
    /// A thread always has full access to its home desktop. For other desktops,
    /// access must have been explicitly granted via [`grant_access`](Self::grant_access).
    pub fn check_access(
        &self,
        desktop: DesktopId,
        thread_id: u64,
        required: DesktopAccess,
    ) -> bool {
        let Some(grant) = self.grants.get(&thread_id) else {
            // Unknown thread -> no access.
            return false;
        };

        // Full access to home desktop.
        if grant.home_desktop == desktop {
            return true;
        }

        // Check extra grants.
        if let Some(granted) = grant.extra_grants.get(&desktop) {
            granted.contains(required)
        } else {
            false
        }
    }

    /// Returns the home desktop for a thread, if registered.
    pub fn thread_desktop(&self, thread_id: u64) -> Option<DesktopId> {
        self.grants.get(&thread_id).map(|g| g.home_desktop)
    }

    /// Returns all threads assigned to a given desktop.
    pub fn threads_on_desktop(&self, desktop: DesktopId) -> Vec<u64> {
        self.grants
            .iter()
            .filter(|(_, grant)| grant.home_desktop == desktop)
            .map(|(&tid, _)| tid)
            .collect()
    }
}

impl Default for DesktopSecurity {
    fn default() -> Self {
        Self::new()
    }
}
