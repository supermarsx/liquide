//! Policy rules and rule sets.

use serde::{Deserialize, Serialize};

/// A single policy rule — a key/value pair with an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// The policy key this rule affects (e.g. `"clipboard.enabled"`).
    pub key: String,
    /// The action: allow, deny, or set a value.
    pub action: RuleAction,
}

/// What a rule does when it matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleAction {
    /// Explicitly allow the action.
    Allow,
    /// Explicitly deny the action.
    Deny,
    /// Set a configuration value.
    Set(String),
}

/// An ordered collection of rules from a single policy source.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleSet {
    /// The rules in evaluation order.
    pub rules: Vec<Rule>,
}

impl RuleSet {
    /// Create an empty rule set.
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a rule to this set.
    pub fn push(&mut self, rule: Rule) {
        self.rules.push(rule);
    }
}
