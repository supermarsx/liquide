//! The `ActionId` catalog: a **data** registry of the privileged operations
//! the runtime knows how to gate.
//!
//! Checkpoint A (which operations get gated) is intentionally expressed as
//! data here, not as control flow. To change which operations require
//! authorization, edit the [`seed_catalog`] table or call
//! [`ActionCatalog::set_gated`] at runtime — never a code change to the
//! enforcement path.
//!
//! Each catalog entry maps a stable, ergonomic *catalog key* (the
//! [`ActionId`], e.g. `"accounts.create_user"`) to:
//!   * the full [`AuthorizationAction`] (its reverse-domain id +
//!     required [`AuthLevel`]) that the authorization agent understands, and
//!   * a `gated` flag — whether the operation must clear authorization.
//!
//! The documented default (Checkpoint A) is: destructive / system-state
//! changing operations are gated; read-only and per-session cosmetic
//! operations are ungated. The `gated` column below encodes that default and
//! is toggleable.

use std::collections::BTreeMap;

use liquide_authorization::{AuthLevel, AuthorizationAction};

/// Stable, ergonomic catalog key for a privileged operation
/// (e.g. `"accounts.create_user"`). Distinct from the reverse-domain
/// [`AuthorizationAction::id`] (`"org.liquide.accounts.create_user"`) the
/// authorization agent matches policy rules against.
pub type ActionId = &'static str;

/// One row in the [`ActionCatalog`].
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    /// The full authorization action (reverse-domain id + required level).
    pub action: AuthorizationAction,
    /// Whether this operation must clear authorization before proceeding.
    ///
    /// When `false`, the runtime treats the operation as always-allowed but
    /// still audits it and forwards an event (so ungated ops remain visible).
    pub gated: bool,
}

impl CatalogEntry {
    /// Build an entry from its parts.
    #[must_use]
    pub fn new(
        reverse_domain_id: impl Into<String>,
        description: impl Into<String>,
        message: impl Into<String>,
        required_level: AuthLevel,
        gated: bool,
    ) -> Self {
        Self {
            action: AuthorizationAction::new(
                reverse_domain_id,
                description,
                message,
                required_level,
            ),
            gated,
        }
    }
}

/// A data-driven registry of privileged operations.
///
/// The catalog is the single place Checkpoint A is realized: which keys
/// exist, what authorization level each demands, and whether each is gated.
#[derive(Debug, Clone, Default)]
pub struct ActionCatalog {
    entries: BTreeMap<ActionId, CatalogEntry>,
}

impl ActionCatalog {
    /// Create an empty catalog.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Create a catalog seeded with the Checkpoint A recommended seam set.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut catalog = Self::new();
        for (id, entry) in seed_catalog() {
            catalog.entries.insert(id, entry);
        }
        catalog
    }

    /// Insert or replace an entry.
    pub fn insert(&mut self, id: ActionId, entry: CatalogEntry) {
        self.entries.insert(id, entry);
    }

    /// Look up an entry by catalog key.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&CatalogEntry> {
        self.entries.get(id)
    }

    /// Whether the operation identified by `id` is currently gated.
    ///
    /// Returns `None` if the catalog has no such key.
    #[must_use]
    pub fn is_gated(&self, id: &str) -> Option<bool> {
        self.entries.get(id).map(|e| e.gated)
    }

    /// Toggle the `gated` flag for an existing entry.
    ///
    /// Returns `true` if the entry existed and was updated. This is the
    /// runtime equivalent of editing the Checkpoint A data table.
    pub fn set_gated(&mut self, id: &str, gated: bool) -> bool {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.gated = gated;
            true
        } else {
            false
        }
    }

    /// Iterate over all catalog entries.
    pub fn entries(&self) -> impl Iterator<Item = (&ActionId, &CatalogEntry)> {
        self.entries.iter()
    }

    /// Number of registered operations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the catalog is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The Checkpoint A recommended seam set, expressed as data.
///
/// `gated = true` for destructive / system-state-changing operations;
/// `gated = false` for read-only / per-session-cosmetic operations.
#[must_use]
pub fn seed_catalog() -> Vec<(ActionId, CatalogEntry)> {
    // Required levels mirror the existing builtin convention: destructive power
    // ops (shutdown/reboot/hibernate) are AdminPassword — same real-credential
    // level as the other destructive system mutations — while the frequent,
    // recoverable suspend stays NoAuth (audited, never prompted). Account/admin
    // mutations are AdminPassword; routine config changes are UserPassword.
    vec![
        // ── accounts ────────────────────────────────────────────────
        (
            "accounts.create_user",
            CatalogEntry::new(
                "org.liquide.accounts.create_user",
                "Create a user account",
                "Authentication is required to create a user account.",
                AuthLevel::AdminPassword,
                true,
            ),
        ),
        (
            "accounts.delete_user",
            CatalogEntry::new(
                "org.liquide.accounts.delete_user",
                "Delete a user account",
                "Authentication is required to delete a user account.",
                AuthLevel::AdminPassword,
                true,
            ),
        ),
        (
            "accounts.change_password",
            CatalogEntry::new(
                "org.liquide.accounts.change_password",
                "Change an account password",
                "Authentication is required to change a password.",
                AuthLevel::AdminPassword,
                true,
            ),
        ),
        (
            "accounts.set_display_name",
            CatalogEntry::new(
                "org.liquide.accounts.set_display_name",
                "Change an account display name",
                "Authentication is required to change a display name.",
                AuthLevel::UserPassword,
                // cosmetic → ungated by default
                false,
            ),
        ),
        (
            "accounts.set_avatar",
            CatalogEntry::new(
                "org.liquide.accounts.set_avatar",
                "Change an account avatar",
                "Authentication is required to change an avatar.",
                AuthLevel::UserPassword,
                // cosmetic → ungated by default
                false,
            ),
        ),
        // ── firewall ────────────────────────────────────────────────
        (
            "firewall.add_rule",
            CatalogEntry::new(
                "org.liquide.firewall.add_rule",
                "Add a firewall rule",
                "Authentication is required to add a firewall rule.",
                AuthLevel::AdminPassword,
                true,
            ),
        ),
        (
            "firewall.remove_rule",
            CatalogEntry::new(
                "org.liquide.firewall.remove_rule",
                "Remove a firewall rule",
                "Authentication is required to remove a firewall rule.",
                AuthLevel::AdminPassword,
                true,
            ),
        ),
        (
            "firewall.enable_rule",
            CatalogEntry::new(
                "org.liquide.firewall.enable_rule",
                "Enable a firewall rule",
                "Authentication is required to enable a firewall rule.",
                AuthLevel::AdminPassword,
                true,
            ),
        ),
        (
            "firewall.disable_rule",
            CatalogEntry::new(
                "org.liquide.firewall.disable_rule",
                "Disable a firewall rule",
                "Authentication is required to disable a firewall rule.",
                AuthLevel::AdminPassword,
                true,
            ),
        ),
        (
            "firewall.set_profile",
            CatalogEntry::new(
                "org.liquide.firewall.set_profile",
                "Change the firewall profile",
                "Authentication is required to change the firewall profile.",
                AuthLevel::AdminPassword,
                true,
            ),
        ),
        // ── network ─────────────────────────────────────────────────
        (
            "network.connect_wifi",
            CatalogEntry::new(
                "org.liquide.network.connect_wifi",
                "Connect to a Wi-Fi network",
                "Authentication is required to connect to a Wi-Fi network.",
                AuthLevel::UserPassword,
                true,
            ),
        ),
        (
            "network.forget_wifi",
            CatalogEntry::new(
                "org.liquide.network.forget_wifi",
                "Forget a Wi-Fi network",
                "Authentication is required to forget a Wi-Fi network.",
                AuthLevel::UserPassword,
                true,
            ),
        ),
        (
            "network.connect_vpn",
            CatalogEntry::new(
                "org.liquide.network.connect_vpn",
                "Connect to a VPN",
                "Authentication is required to connect to a VPN.",
                AuthLevel::UserPassword,
                true,
            ),
        ),
        (
            "network.set_airplane_mode",
            CatalogEntry::new(
                "org.liquide.network.set_airplane_mode",
                "Toggle airplane mode",
                "Authentication is required to toggle airplane mode.",
                AuthLevel::UserPassword,
                true,
            ),
        ),
        (
            "network.enable_interface",
            CatalogEntry::new(
                "org.liquide.network.enable_interface",
                "Enable a network interface",
                "Authentication is required to enable a network interface.",
                AuthLevel::UserPassword,
                // interface toggle → ungated by default
                false,
            ),
        ),
        (
            "network.disable_interface",
            CatalogEntry::new(
                "org.liquide.network.disable_interface",
                "Disable a network interface",
                "Authentication is required to disable a network interface.",
                AuthLevel::UserPassword,
                // interface toggle → ungated by default
                false,
            ),
        ),
        // ── power ───────────────────────────────────────────────────
        (
            "power.shutdown",
            CatalogEntry::new(
                "org.liquide.system.shutdown",
                "Shut down the system",
                "The system will shut down. Unsaved work may be lost.",
                // Destructive: demand a real credential, matching the other
                // gated system mutations (accounts/firewall).
                AuthLevel::AdminPassword,
                true,
            ),
        ),
        (
            "power.reboot",
            CatalogEntry::new(
                "org.liquide.system.reboot",
                "Restart the system",
                "The system will restart. Unsaved work may be lost.",
                // Destructive: demand a real credential, matching the other
                // gated system mutations (accounts/firewall).
                AuthLevel::AdminPassword,
                true,
            ),
        ),
        (
            "power.suspend",
            CatalogEntry::new(
                "org.liquide.system.suspend",
                "Suspend the system",
                "The system will be suspended to RAM.",
                // Frequent and fully recoverable: stays NoAuth (audited via the
                // gated flag, but never prompts for a credential).
                AuthLevel::NoAuth,
                true,
            ),
        ),
        (
            "power.hibernate",
            CatalogEntry::new(
                "org.liquide.system.hibernate",
                "Hibernate the system",
                "The system will be hibernated to disk.",
                // Destructive: demand a real credential, matching the other
                // gated system mutations (accounts/firewall).
                AuthLevel::AdminPassword,
                true,
            ),
        ),
    ]
}
