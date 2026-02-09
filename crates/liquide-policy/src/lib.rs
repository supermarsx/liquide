#![doc = "Policy engine for the Liquide session server."]
#![doc = ""]
#![doc = "Implements a hierarchical policy model where rules cascade from"]
#![doc = "server-wide defaults, through group policies, down to per-user and"]
#![doc = "per-session overrides.  The engine evaluates the effective policy for"]
#![doc = "any given request."]

pub mod engine;
pub mod evaluation;
pub mod hierarchy;
pub mod rule;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The source (priority level) of a policy entry.
///
/// Policies are resolved from lowest to highest priority; a `Session`-level
/// policy overrides a `User`-level policy, which overrides `Group`, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PolicySource {
    /// Server-wide default.
    Server = 0,
    /// Group-level policy.
    Group = 1,
    /// Per-user policy.
    User = 2,
    /// Per-session override.
    Session = 3,
}

/// The computed effective policy for a particular session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EffectivePolicy {
    /// Whether clipboard sharing is allowed.
    pub clipboard_enabled: bool,
    /// Whether USB redirection is allowed.
    pub usb_redirect_enabled: bool,
    /// Whether audio playback is allowed.
    pub audio_playback_enabled: bool,
    /// Whether audio capture (microphone) is allowed.
    pub audio_capture_enabled: bool,
    /// Whether file transfer is allowed.
    pub file_transfer_enabled: bool,
    /// Whether printing is allowed.
    pub printing_enabled: bool,
    /// Maximum display resolution width.
    pub max_resolution_w: u32,
    /// Maximum display resolution height.
    pub max_resolution_h: u32,
    /// Session idle timeout in seconds (0 = disabled).
    pub idle_timeout_secs: u64,
}

/// The policy engine that resolves effective policies.
#[derive(Debug)]
pub struct PolicyEngine {
    /// Ordered list of policy layers, lowest priority first.
    layers: Vec<(PolicySource, rule::RuleSet)>,
}

impl PolicyEngine {
    /// Create an empty policy engine.
    #[must_use]
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Add a set of rules at the given source level.
    pub fn add_layer(&mut self, source: PolicySource, rules: rule::RuleSet) {
        self.layers.push((source, rules));
        self.layers.sort_by_key(|(s, _)| *s);
    }

    /// Compute the effective policy for the current set of layers.
    #[must_use]
    pub fn evaluate(&self) -> EffectivePolicy {
        evaluation::evaluate(&self.layers)
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors from the policy subsystem.
#[derive(Debug, Error)]
pub enum PolicyError {
    /// A policy file could not be parsed.
    #[error("policy parse error: {0}")]
    Parse(String),

    /// A referenced policy source was not found.
    #[error("policy source not found: {0}")]
    NotFound(String),

    /// The evaluated policy explicitly denies the requested action.
    #[error("action denied by policy: {0}")]
    Denied(String),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, PolicyError>;
