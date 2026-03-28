#![allow(dead_code)]

use crate::{FirewallBackend, FirewallError, FirewallProfile, FirewallRule};

/// Stub firewall backend for unsupported platforms.
/// Returns `NotSupported` for every operation.
pub struct StubFirewall;

impl StubFirewall {
    pub fn new() -> Self {
        Self
    }
}

impl FirewallBackend for StubFirewall {
    fn apply_profile(&mut self, _profile: &FirewallProfile) -> Result<(), FirewallError> {
        Err(FirewallError::NotSupported)
    }

    fn add_rule(&mut self, _rule: &FirewallRule) -> Result<(), FirewallError> {
        Err(FirewallError::NotSupported)
    }

    fn remove_rule(&mut self, _rule_name: &str) -> Result<(), FirewallError> {
        Err(FirewallError::NotSupported)
    }

    fn list_rules(&self) -> Result<Vec<String>, FirewallError> {
        Err(FirewallError::NotSupported)
    }

    fn is_enabled(&self) -> Result<bool, FirewallError> {
        Err(FirewallError::NotSupported)
    }

    fn set_enabled(&mut self, _enabled: bool) -> Result<(), FirewallError> {
        Err(FirewallError::NotSupported)
    }
}
