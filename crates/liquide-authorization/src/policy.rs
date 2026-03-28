use serde::{Deserialize, Serialize};

use crate::level::AuthLevel;

/// A single policy rule that maps an action pattern to an authorization level.
///
/// Patterns support trailing wildcards: `"org.liquide.system.*"` matches
/// `"org.liquide.system.shutdown"`, `"org.liquide.system.reboot"`, etc.
/// An exact match always takes priority over a wildcard match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Glob-style pattern for matching action IDs.
    /// Supports trailing `*` wildcard (e.g., `"org.liquide.system.*"`).
    pub action_pattern: String,

    /// The auth level required for actions matching this pattern.
    pub level: AuthLevel,

    /// Whether a granted authorization can be kept alive for a period
    /// of time, avoiding repeated prompts for the same action.
    pub allow_keep_alive: bool,

    /// How long (in seconds) a keep-alive grant lasts.
    /// Only meaningful when `allow_keep_alive` is true.
    pub keep_alive_seconds: u32,
}

impl PolicyRule {
    /// Create a new policy rule.
    #[must_use]
    pub fn new(action_pattern: impl Into<String>, level: AuthLevel) -> Self {
        Self {
            action_pattern: action_pattern.into(),
            level,
            allow_keep_alive: false,
            keep_alive_seconds: 0,
        }
    }

    /// Enable keep-alive for this rule with the given duration.
    #[must_use]
    pub fn with_keep_alive(mut self, seconds: u32) -> Self {
        self.allow_keep_alive = true;
        self.keep_alive_seconds = seconds;
        self
    }

    /// Check whether this rule's pattern matches the given action ID.
    ///
    /// Matching rules:
    /// - Exact match: `"org.liquide.system.shutdown"` matches only that ID.
    /// - Trailing wildcard: `"org.liquide.system.*"` matches any ID starting
    ///   with `"org.liquide.system."`.
    /// - Universal wildcard: `"*"` matches everything.
    #[must_use]
    pub fn matches(&self, action_id: &str) -> bool {
        pattern_matches(&self.action_pattern, action_id)
    }

    /// Returns the specificity of the pattern — more segments = more specific.
    /// An exact match (no wildcard) gets a bonus to always beat a wildcard.
    #[must_use]
    pub fn specificity(&self) -> u32 {
        let segments = self.action_pattern.matches('.').count() as u32 + 1;
        if self.action_pattern.ends_with('*') {
            segments
        } else {
            // Exact matches get a large bonus so they always win.
            segments + 1000
        }
    }
}

/// Check whether a pattern matches an action ID.
fn pattern_matches(pattern: &str, action_id: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if pattern.ends_with(".*") {
        let prefix = &pattern[..pattern.len() - 1]; // include the trailing dot
        action_id.starts_with(prefix) || action_id == &pattern[..pattern.len() - 2]
    } else if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        action_id.starts_with(prefix)
    } else {
        pattern == action_id
    }
}

/// A collection of policy rules that determine authorization requirements.
///
/// When multiple rules match an action, the most specific rule wins
/// (exact match beats wildcard, longer prefix beats shorter).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthorizationPolicy {
    rules: Vec<PolicyRule>,
}

impl AuthorizationPolicy {
    /// Create an empty policy.
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Create a policy pre-loaded with sensible defaults.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self {
            rules: default_policies(),
        }
    }

    /// Add a rule to this policy.
    pub fn add_rule(&mut self, rule: PolicyRule) {
        self.rules.push(rule);
    }

    /// Return an immutable view of all rules.
    #[must_use]
    pub fn rules(&self) -> &[PolicyRule] {
        &self.rules
    }

    /// Find the most specific matching rule for an action ID.
    ///
    /// Returns `None` if no rule matches.
    #[must_use]
    pub fn find_matching_rule(&self, action_id: &str) -> Option<&PolicyRule> {
        self.rules
            .iter()
            .filter(|r| r.matches(action_id))
            .max_by_key(|r| r.specificity())
    }

    /// Determine the required auth level for an action.
    ///
    /// Returns the level from the most specific matching rule, or `None`
    /// if no rule matches.
    #[must_use]
    pub fn required_level(&self, action_id: &str) -> Option<AuthLevel> {
        self.find_matching_rule(action_id).map(|r| r.level)
    }

    /// Remove all rules matching a given pattern string.
    pub fn remove_rules(&mut self, pattern: &str) {
        self.rules.retain(|r| r.action_pattern != pattern);
    }

    /// Return the number of rules.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Return true if there are no rules.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Returns a set of sensible default policy rules.
#[must_use]
pub fn default_policies() -> Vec<PolicyRule> {
    vec![
        // System power actions — no auth needed, user is at the console
        PolicyRule::new("org.liquide.system.shutdown", AuthLevel::NoAuth),
        PolicyRule::new("org.liquide.system.reboot", AuthLevel::NoAuth),
        PolicyRule::new("org.liquide.system.suspend", AuthLevel::NoAuth),
        // Package management — needs admin
        PolicyRule::new("org.liquide.package.install", AuthLevel::AdminPassword)
            .with_keep_alive(300),
        PolicyRule::new("org.liquide.package.remove", AuthLevel::AdminPassword)
            .with_keep_alive(300),
        PolicyRule::new("org.liquide.package.update", AuthLevel::AdminPassword)
            .with_keep_alive(300),
        // System settings — user password
        PolicyRule::new("org.liquide.settings.system.*", AuthLevel::UserPassword)
            .with_keep_alive(120),
        // Device mounting — user password
        PolicyRule::new("org.liquide.device.mount", AuthLevel::UserPassword),
        PolicyRule::new("org.liquide.device.unmount", AuthLevel::UserPassword),
        // Service management — admin
        PolicyRule::new("org.liquide.service.*", AuthLevel::AdminPassword)
            .with_keep_alive(60),
        // Catch-all wildcard — require admin by default for unknown actions
        PolicyRule::new("*", AuthLevel::AdminPassword),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        let rule = PolicyRule::new("org.liquide.system.shutdown", AuthLevel::NoAuth);
        assert!(rule.matches("org.liquide.system.shutdown"));
        assert!(!rule.matches("org.liquide.system.reboot"));
        assert!(!rule.matches("org.liquide.system"));
    }

    #[test]
    fn wildcard_match() {
        let rule = PolicyRule::new("org.liquide.system.*", AuthLevel::UserPassword);
        assert!(rule.matches("org.liquide.system.shutdown"));
        assert!(rule.matches("org.liquide.system.reboot"));
        assert!(rule.matches("org.liquide.system.suspend"));
        // The bare prefix without trailing segment should also match
        assert!(rule.matches("org.liquide.system"));
        assert!(!rule.matches("org.liquide.package.install"));
    }

    #[test]
    fn universal_wildcard() {
        let rule = PolicyRule::new("*", AuthLevel::AdminPassword);
        assert!(rule.matches("org.liquide.anything"));
        assert!(rule.matches("com.example.other"));
        assert!(rule.matches("x"));
    }

    #[test]
    fn specificity_ordering() {
        let exact = PolicyRule::new("org.liquide.system.shutdown", AuthLevel::NoAuth);
        let wild = PolicyRule::new("org.liquide.system.*", AuthLevel::AdminPassword);
        let universal = PolicyRule::new("*", AuthLevel::AdminPassword);
        assert!(exact.specificity() > wild.specificity());
        assert!(wild.specificity() > universal.specificity());
    }

    #[test]
    fn most_specific_rule_wins() {
        let mut policy = AuthorizationPolicy::new();
        policy.add_rule(PolicyRule::new("*", AuthLevel::AdminPassword));
        policy.add_rule(PolicyRule::new(
            "org.liquide.system.*",
            AuthLevel::UserPassword,
        ));
        policy.add_rule(PolicyRule::new(
            "org.liquide.system.shutdown",
            AuthLevel::NoAuth,
        ));

        assert_eq!(
            policy.required_level("org.liquide.system.shutdown"),
            Some(AuthLevel::NoAuth)
        );
        assert_eq!(
            policy.required_level("org.liquide.system.reboot"),
            Some(AuthLevel::UserPassword)
        );
        assert_eq!(
            policy.required_level("org.liquide.package.install"),
            Some(AuthLevel::AdminPassword)
        );
    }

    #[test]
    fn default_policies_are_populated() {
        let defaults = default_policies();
        assert!(defaults.len() >= 10);
        // shutdown should be NoAuth
        let shutdown = defaults
            .iter()
            .find(|r| r.action_pattern == "org.liquide.system.shutdown")
            .unwrap();
        assert_eq!(shutdown.level, AuthLevel::NoAuth);
        // package install should be AdminPassword with keep-alive
        let install = defaults
            .iter()
            .find(|r| r.action_pattern == "org.liquide.package.install")
            .unwrap();
        assert_eq!(install.level, AuthLevel::AdminPassword);
        assert!(install.allow_keep_alive);
        assert_eq!(install.keep_alive_seconds, 300);
    }

    #[test]
    fn with_defaults_constructor() {
        let policy = AuthorizationPolicy::with_defaults();
        assert!(!policy.is_empty());
        assert!(policy.len() >= 10);
    }

    #[test]
    fn remove_rules() {
        let mut policy = AuthorizationPolicy::with_defaults();
        let before = policy.len();
        policy.remove_rules("org.liquide.system.shutdown");
        assert_eq!(policy.len(), before - 1);
        assert!(policy
            .find_matching_rule("org.liquide.system.shutdown")
            .is_some()); // still matches wildcard
    }

    #[test]
    fn keep_alive_builder() {
        let rule = PolicyRule::new("org.liquide.test", AuthLevel::UserPassword)
            .with_keep_alive(600);
        assert!(rule.allow_keep_alive);
        assert_eq!(rule.keep_alive_seconds, 600);
    }

    #[test]
    fn no_match_returns_none() {
        let policy = AuthorizationPolicy::new(); // empty, no catch-all
        assert!(policy.required_level("org.liquide.test").is_none());
    }

    #[test]
    fn serde_roundtrip() {
        let policy = AuthorizationPolicy::with_defaults();
        let json = serde_json::to_string(&policy).unwrap();
        let back: AuthorizationPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy.len(), back.len());
        for (a, b) in policy.rules().iter().zip(back.rules().iter()) {
            assert_eq!(a, b);
        }
    }
}
