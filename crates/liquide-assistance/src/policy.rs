//! Assistance policy engine.

use crate::config::{AssistanceConfig, ModeConfig, PermissionsConfig, StealthConfig};
use crate::mode::AssistanceMode;
use crate::observer::Observer;

/// Policy that governs what assistance operations are permitted.
#[derive(Debug, Clone)]
pub struct AssistancePolicy {
    config: AssistanceConfig,
    mode_config: ModeConfig,
    #[allow(dead_code)]
    stealth_config: StealthConfig,
    permissions_config: PermissionsConfig,
}

impl AssistancePolicy {
    /// Create a policy from configuration.
    #[must_use]
    pub fn from_config(
        config: AssistanceConfig,
        mode_config: ModeConfig,
        stealth_config: StealthConfig,
        permissions_config: PermissionsConfig,
    ) -> Self {
        Self {
            config,
            mode_config,
            stealth_config,
            permissions_config,
        }
    }

    /// Whether the given mode is allowed by policy.
    #[must_use]
    pub fn is_mode_allowed(&self, mode: AssistanceMode) -> bool {
        if !self.config.enabled {
            return false;
        }
        match mode {
            AssistanceMode::ViewOnly => self.mode_config.view_only,
            AssistanceMode::Interactive => self.mode_config.interactive,
            AssistanceMode::Exclusive => self.mode_config.exclusive,
            AssistanceMode::Stealth => self.mode_config.stealth && self.stealth_config.enabled,
        }
    }

    /// Whether stealth mode is allowed for the given observer.
    #[must_use]
    pub fn is_stealth_allowed(&self, observer: &Observer) -> bool {
        self.stealth_config.enabled && observer.can_stealth()
    }

    /// Maximum number of observers for the given mode.
    #[must_use]
    pub fn max_observers(&self, mode: AssistanceMode) -> u32 {
        let mode_max = mode.capabilities().max_concurrent_observers;
        mode_max.min(self.config.max_concurrent_observers)
    }

    /// Whether the observer can request assistance.
    #[must_use]
    pub fn can_request(&self, observer: &Observer) -> bool {
        if !self.config.enabled {
            return false;
        }
        match observer.role {
            crate::observer::ObserverRole::HelpDesk => self.permissions_config.helpdesk_can_request,
            crate::observer::ObserverRole::Admin => true,
            crate::observer::ObserverRole::SecurityAdmin => true,
            crate::observer::ObserverRole::Peer => self.permissions_config.user_can_invite,
        }
    }

    /// Whether the current configuration allows creating invites.
    #[must_use]
    pub fn can_invite(&self) -> bool {
        self.config.enabled && self.permissions_config.user_can_invite
    }
}
