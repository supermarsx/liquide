//! Invitation codes for assistance sessions.

use std::collections::HashMap;

use crate::mode::AssistanceMode;
use crate::{AssistanceError, Result};

/// An invite code that grants access to an assistance session.
#[derive(Debug, Clone)]
pub struct InviteCode {
    /// The code string.
    pub code: String,
    /// Mode granted by this invite.
    pub mode: AssistanceMode,
    /// Who created the invite.
    pub created_by: String,
    /// Creation timestamp (unix seconds).
    pub created_at: u64,
    /// Expiration timestamp (unix seconds).
    pub expires_at: u64,
    /// Maximum number of uses.
    pub max_uses: u32,
    /// Current number of uses.
    pub uses: u32,
}

impl InviteCode {
    /// Generate a new invite code.
    #[must_use]
    pub fn generate(
        owner_id: String,
        mode: AssistanceMode,
        expiry_secs: u64,
        max_uses: u32,
    ) -> Self {
        // Deterministic code derived from a simple hash of inputs.
        let hash_input = format!("{}-{}-{}-{}", owner_id, mode, expiry_secs, max_uses);
        let hash = simple_hash(&hash_input);
        let code = format!("ASSIST-{:08X}", hash);
        Self {
            code,
            mode,
            created_by: owner_id,
            created_at: 0,
            expires_at: expiry_secs,
            max_uses,
            uses: 0,
        }
    }

    /// Whether the invite has expired.
    #[must_use]
    pub fn is_expired(&self, now: u64) -> bool {
        now >= self.expires_at
    }

    /// Whether the invite has been used the maximum number of times.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.uses >= self.max_uses
    }

    /// Whether the invite can still be used.
    #[must_use]
    pub fn is_valid(&self, now: u64) -> bool {
        !self.is_expired(now) && !self.is_exhausted()
    }

    /// Redeem the invite, incrementing the use count.
    pub fn redeem(&mut self) -> Result<()> {
        if self.is_exhausted() {
            return Err(AssistanceError::InvalidInviteCode);
        }
        self.uses += 1;
        Ok(())
    }
}

/// Registry of active invite codes.
pub struct InviteRegistry {
    codes: HashMap<String, InviteCode>,
}

impl InviteRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            codes: HashMap::new(),
        }
    }

    /// Register an invite code.
    pub fn register(&mut self, invite: InviteCode) {
        self.codes.insert(invite.code.clone(), invite);
    }

    /// Look up an invite by code.
    #[must_use]
    pub fn lookup(&self, code: &str) -> Option<&InviteCode> {
        self.codes.get(code)
    }

    /// Redeem an invite code.
    pub fn redeem(&mut self, code: &str) -> Result<&InviteCode> {
        let invite = self
            .codes
            .get_mut(code)
            .ok_or(AssistanceError::InvalidInviteCode)?;
        invite.redeem()?;
        // Re-borrow as immutable.
        Ok(&self.codes[code])
    }

    /// Remove all expired invites.
    pub fn cleanup_expired(&mut self, now: u64) {
        self.codes.retain(|_, invite| !invite.is_expired(now));
    }
}

impl Default for InviteRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple deterministic hash for code generation.
fn simple_hash(input: &str) -> u32 {
    let mut hash: u32 = 5381;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    hash
}
