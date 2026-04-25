//! Authorization rules engine with wildcard matching and first-match semantics.
//!
//! While the existing [`crate::policy`] module maps action patterns to
//! auth levels, this module provides a more expressive rule system that
//! can match on both the action **and** the subject, producing a full
//! [`AuthDecision`].
//!
//! Rules are evaluated in order; the first matching rule wins. This
//! allows layered overrides: specific rules at the top, broad defaults
//! at the bottom.

use serde::{Deserialize, Serialize};

use crate::policy_db::AuthDecision;
use crate::subject::{self, Subject};

// ── SubjectMatch ────────────────────────────────────────────────────

/// Criteria for matching a [`Subject`] in a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubjectMatch {
    /// Matches any subject.
    Any,
    /// Matches only subjects whose uid equals the given value.
    Uid(u32),
    /// Matches subjects that belong to the named group.
    InGroup(String),
    /// Matches subjects that are administrators (root or admin group).
    IsAdmin,
    /// Matches subjects on a local session.
    IsLocal,
    /// Logical AND of two sub-matches.
    All(Vec<SubjectMatch>),
    /// Logical OR of two sub-matches.
    OneOf(Vec<SubjectMatch>),
}

impl SubjectMatch {
    /// Test whether a subject satisfies this match criterion.
    #[must_use]
    pub fn matches(&self, subject: &Subject) -> bool {
        match self {
            Self::Any => true,
            Self::Uid(uid) => subject.uid == *uid,
            Self::InGroup(group) => subject.in_group(group),
            Self::IsAdmin => subject::is_admin(subject),
            Self::IsLocal => subject.is_local_session(),
            Self::All(subs) => subs.iter().all(|s| s.matches(subject)),
            Self::OneOf(subs) => subs.iter().any(|s| s.matches(subject)),
        }
    }
}

// ── Rule ────────────────────────────────────────────────────────────

/// A single authorization rule: if the action matches `action_pattern`
/// and the subject matches `subject_match`, produce `result`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// Glob pattern for matching action IDs.
    /// Supports trailing `*` (e.g., `"org.liquide.desktop.*"`)
    /// and the universal wildcard `"*"`.
    pub action_pattern: String,

    /// Criterion the subject must satisfy.
    pub subject_match: SubjectMatch,

    /// The authorization decision if this rule matches.
    pub result: AuthDecision,

    /// Optional human-readable description of why this rule exists.
    pub description: Option<String>,
}

impl Rule {
    /// Create a new rule.
    #[must_use]
    pub fn new(
        action_pattern: impl Into<String>,
        subject_match: SubjectMatch,
        result: AuthDecision,
    ) -> Self {
        Self {
            action_pattern: action_pattern.into(),
            subject_match,
            result,
            description: None,
        }
    }

    /// Attach a description.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Test whether this rule matches the given action ID and subject.
    #[must_use]
    pub fn matches(&self, action_id: &str, subject: &Subject) -> bool {
        action_pattern_matches(&self.action_pattern, action_id)
            && self.subject_match.matches(subject)
    }
}

/// Check whether an action pattern matches an action ID.
/// Reuses the same conventions as `crate::policy::PolicyRule::matches`.
fn action_pattern_matches(pattern: &str, action_id: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if pattern.ends_with(".*") {
        let prefix = &pattern[..pattern.len() - 1];
        action_id.starts_with(prefix) || action_id == &pattern[..pattern.len() - 2]
    } else if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        action_id.starts_with(prefix)
    } else {
        pattern == action_id
    }
}

// ── RuleSet ─────────────────────────────────────────────────────────

/// An ordered collection of authorization rules.
///
/// Evaluation is first-match-wins: the first rule whose action pattern
/// and subject match produce the decision. If no rule matches, the
/// evaluation returns `None`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleSet {
    rules: Vec<Rule>,
}

impl RuleSet {
    /// Create an empty rule set.
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Create a rule set pre-loaded with built-in desktop defaults.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut rs = Self::new();
        for rule in builtin_rules() {
            rs.add_rule(rule);
        }
        rs
    }

    /// Append a rule to the end of the set (lowest priority).
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Insert a rule at the given index (0 = highest priority).
    pub fn insert_rule(&mut self, index: usize, rule: Rule) {
        let idx = index.min(self.rules.len());
        self.rules.insert(idx, rule);
    }

    /// Remove a rule at the given index.
    ///
    /// Returns the removed rule, or `None` if the index is out of bounds.
    pub fn remove_rule(&mut self, index: usize) -> Option<Rule> {
        if index < self.rules.len() {
            Some(self.rules.remove(index))
        } else {
            None
        }
    }

    /// Remove all rules whose action pattern matches the given string.
    pub fn remove_by_pattern(&mut self, pattern: &str) {
        self.rules.retain(|r| r.action_pattern != pattern);
    }

    /// Evaluate the rules against an action and subject.
    ///
    /// Returns the decision from the first matching rule, or `None` if
    /// no rule matches.
    #[must_use]
    pub fn evaluate(&self, action_id: &str, subject: &Subject) -> Option<AuthDecision> {
        for rule in &self.rules {
            if rule.matches(action_id, subject) {
                return Some(rule.result.clone());
            }
        }
        None
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

    /// Return an immutable view of all rules.
    #[must_use]
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
}

// ── Built-in rules ──────────────────────────────────────────────────

/// Default rules for common desktop authorization scenarios.
#[must_use]
fn builtin_rules() -> Vec<Rule> {
    use crate::policy_db::AuthType;

    vec![
        // ── Desktop personalization — always allow ──────────────────
        Rule::new(
            "org.liquide.desktop.change-wallpaper",
            SubjectMatch::Any,
            AuthDecision::Allow,
        )
        .with_description("Changing wallpaper requires no authentication"),
        Rule::new(
            "org.liquide.desktop.change-theme",
            SubjectMatch::Any,
            AuthDecision::Allow,
        )
        .with_description("Changing theme requires no authentication"),
        // ── Settings — user auth for writes ─────────────────────────
        Rule::new(
            "org.liquide.settings.read",
            SubjectMatch::Any,
            AuthDecision::Allow,
        )
        .with_description("Reading settings is always allowed"),
        Rule::new(
            "org.liquide.settings.write",
            SubjectMatch::IsAdmin,
            AuthDecision::Allow,
        )
        .with_description("Admins can write settings without prompt"),
        Rule::new(
            "org.liquide.settings.write",
            SubjectMatch::Any,
            AuthDecision::AuthRequired(AuthType::UserPassword),
        )
        .with_description("Non-admin users must authenticate to write settings"),
        // ── Package management — admin auth ─────────────────────────
        Rule::new(
            "org.liquide.package.*",
            SubjectMatch::IsAdmin,
            AuthDecision::Allow,
        )
        .with_description("Admins can manage packages without prompt"),
        Rule::new(
            "org.liquide.package.*",
            SubjectMatch::Any,
            AuthDecision::AuthRequired(AuthType::AdminPassword),
        )
        .with_description("Non-admin users need admin auth for package management"),
        // ── System power — local users allowed ──────────────────────
        Rule::new(
            "org.liquide.system.*",
            SubjectMatch::IsLocal,
            AuthDecision::Allow,
        )
        .with_description("Local console users can power-manage the system"),
        Rule::new(
            "org.liquide.system.*",
            SubjectMatch::Any,
            AuthDecision::AuthRequired(AuthType::UserPassword),
        )
        .with_description("Remote users must authenticate for power actions"),
        // ── Service management — admin only ─────────────────────────
        Rule::new(
            "org.liquide.service.*",
            SubjectMatch::IsAdmin,
            AuthDecision::Allow,
        )
        .with_description("Admins can manage services"),
        Rule::new(
            "org.liquide.service.*",
            SubjectMatch::Any,
            AuthDecision::AuthRequired(AuthType::AdminPassword),
        )
        .with_description("Non-admins need admin auth for service management"),
        // ── Catch-all — deny unknown actions ────────────────────────
        Rule::new("*", SubjectMatch::Any, AuthDecision::Deny)
            .with_description("Deny all unrecognized actions by default"),
    ]
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_db::AuthType;

    fn regular_user() -> Subject {
        Subject::new(1000, 1, "session-1").with_group("users")
    }

    fn admin_user() -> Subject {
        Subject::new(0, 1, "session-1")
            .with_group("admin")
            .with_group("users")
    }

    fn local_user() -> Subject {
        Subject::new(1000, 1, "session-1")
            .with_group("users")
            .as_local()
    }

    // ── SubjectMatch tests ──────────────────────────────────────────

    #[test]
    fn subject_match_any() {
        assert!(SubjectMatch::Any.matches(&regular_user()));
        assert!(SubjectMatch::Any.matches(&admin_user()));
    }

    #[test]
    fn subject_match_uid() {
        assert!(SubjectMatch::Uid(1000).matches(&regular_user()));
        assert!(!SubjectMatch::Uid(999).matches(&regular_user()));
    }

    #[test]
    fn subject_match_in_group() {
        assert!(SubjectMatch::InGroup("users".into()).matches(&regular_user()));
        assert!(!SubjectMatch::InGroup("admin".into()).matches(&regular_user()));
    }

    #[test]
    fn subject_match_is_admin() {
        assert!(SubjectMatch::IsAdmin.matches(&admin_user()));
        assert!(!SubjectMatch::IsAdmin.matches(&regular_user()));
    }

    #[test]
    fn subject_match_is_local() {
        assert!(SubjectMatch::IsLocal.matches(&local_user()));
        assert!(!SubjectMatch::IsLocal.matches(&regular_user()));
    }

    #[test]
    fn subject_match_all() {
        let matcher = SubjectMatch::All(vec![
            SubjectMatch::InGroup("users".into()),
            SubjectMatch::IsLocal,
        ]);
        assert!(matcher.matches(&local_user()));
        assert!(!matcher.matches(&regular_user())); // not local
    }

    #[test]
    fn subject_match_one_of() {
        let matcher = SubjectMatch::OneOf(vec![SubjectMatch::IsAdmin, SubjectMatch::IsLocal]);
        assert!(matcher.matches(&admin_user()));
        assert!(matcher.matches(&local_user()));
        assert!(!matcher.matches(&regular_user()));
    }

    #[test]
    fn subject_match_all_empty() {
        // Empty All = vacuously true
        assert!(SubjectMatch::All(vec![]).matches(&regular_user()));
    }

    #[test]
    fn subject_match_one_of_empty() {
        // Empty OneOf = nothing matches
        assert!(!SubjectMatch::OneOf(vec![]).matches(&regular_user()));
    }

    // ── action_pattern_matches tests ────────────────────────────────

    #[test]
    fn pattern_exact() {
        assert!(action_pattern_matches(
            "org.liquide.test",
            "org.liquide.test"
        ));
        assert!(!action_pattern_matches(
            "org.liquide.test",
            "org.liquide.other"
        ));
    }

    #[test]
    fn pattern_trailing_wildcard() {
        assert!(action_pattern_matches(
            "org.liquide.system.*",
            "org.liquide.system.shutdown"
        ));
        assert!(action_pattern_matches(
            "org.liquide.system.*",
            "org.liquide.system"
        ));
        assert!(!action_pattern_matches(
            "org.liquide.system.*",
            "org.liquide.package.install"
        ));
    }

    #[test]
    fn pattern_universal() {
        assert!(action_pattern_matches("*", "anything.at.all"));
        assert!(action_pattern_matches("*", "x"));
    }

    // ── Rule tests ──────────────────────────────────────────────────

    #[test]
    fn rule_matches_action_and_subject() {
        let rule = Rule::new(
            "org.liquide.desktop.*",
            SubjectMatch::Any,
            AuthDecision::Allow,
        );
        assert!(rule.matches("org.liquide.desktop.change-wallpaper", &regular_user()));
        assert!(!rule.matches("org.liquide.package.install", &regular_user()));
    }

    #[test]
    fn rule_subject_must_also_match() {
        let rule = Rule::new(
            "org.liquide.service.*",
            SubjectMatch::IsAdmin,
            AuthDecision::Allow,
        );
        assert!(rule.matches("org.liquide.service.start", &admin_user()));
        assert!(!rule.matches("org.liquide.service.start", &regular_user()));
    }

    #[test]
    fn rule_with_description() {
        let rule = Rule::new("*", SubjectMatch::Any, AuthDecision::Deny)
            .with_description("catch-all deny");
        assert_eq!(rule.description.as_deref(), Some("catch-all deny"));
    }

    // ── RuleSet tests ───────────────────────────────────────────────

    #[test]
    fn ruleset_empty() {
        let rs = RuleSet::new();
        assert!(rs.is_empty());
        assert_eq!(rs.len(), 0);
        assert!(
            rs.evaluate("org.liquide.anything", &regular_user())
                .is_none()
        );
    }

    #[test]
    fn ruleset_first_match_wins() {
        let mut rs = RuleSet::new();
        rs.add_rule(Rule::new(
            "org.liquide.test",
            SubjectMatch::Any,
            AuthDecision::Allow,
        ));
        rs.add_rule(Rule::new(
            "org.liquide.test",
            SubjectMatch::Any,
            AuthDecision::Deny,
        ));

        // First rule wins
        assert_eq!(
            rs.evaluate("org.liquide.test", &regular_user()),
            Some(AuthDecision::Allow)
        );
    }

    #[test]
    fn ruleset_insert_at_front() {
        let mut rs = RuleSet::new();
        rs.add_rule(Rule::new(
            "org.liquide.test",
            SubjectMatch::Any,
            AuthDecision::Allow,
        ));
        // Insert deny at front
        rs.insert_rule(
            0,
            Rule::new("org.liquide.test", SubjectMatch::Any, AuthDecision::Deny),
        );
        assert_eq!(
            rs.evaluate("org.liquide.test", &regular_user()),
            Some(AuthDecision::Deny)
        );
    }

    #[test]
    fn ruleset_remove_rule() {
        let mut rs = RuleSet::new();
        rs.add_rule(Rule::new(
            "org.liquide.test",
            SubjectMatch::Any,
            AuthDecision::Allow,
        ));
        assert_eq!(rs.len(), 1);

        let removed = rs.remove_rule(0);
        assert!(removed.is_some());
        assert!(rs.is_empty());
    }

    #[test]
    fn ruleset_remove_out_of_bounds() {
        let mut rs = RuleSet::new();
        assert!(rs.remove_rule(0).is_none());
        assert!(rs.remove_rule(999).is_none());
    }

    #[test]
    fn ruleset_remove_by_pattern() {
        let mut rs = RuleSet::new();
        rs.add_rule(Rule::new(
            "org.liquide.desktop.*",
            SubjectMatch::Any,
            AuthDecision::Allow,
        ));
        rs.add_rule(Rule::new(
            "org.liquide.package.*",
            SubjectMatch::Any,
            AuthDecision::Deny,
        ));
        assert_eq!(rs.len(), 2);

        rs.remove_by_pattern("org.liquide.desktop.*");
        assert_eq!(rs.len(), 1);
        assert_eq!(rs.rules()[0].action_pattern, "org.liquide.package.*");
    }

    #[test]
    fn ruleset_with_defaults() {
        let rs = RuleSet::with_defaults();
        assert!(!rs.is_empty());
        // Wallpaper should be allowed for anyone
        assert_eq!(
            rs.evaluate("org.liquide.desktop.change-wallpaper", &regular_user()),
            Some(AuthDecision::Allow)
        );
    }

    #[test]
    fn ruleset_defaults_admin_packages_allowed() {
        let rs = RuleSet::with_defaults();
        assert_eq!(
            rs.evaluate("org.liquide.package.install", &admin_user()),
            Some(AuthDecision::Allow)
        );
    }

    #[test]
    fn ruleset_defaults_regular_user_packages_need_auth() {
        let rs = RuleSet::with_defaults();
        assert_eq!(
            rs.evaluate("org.liquide.package.install", &regular_user()),
            Some(AuthDecision::AuthRequired(AuthType::AdminPassword))
        );
    }

    #[test]
    fn ruleset_defaults_local_user_shutdown_allowed() {
        let rs = RuleSet::with_defaults();
        assert_eq!(
            rs.evaluate("org.liquide.system.shutdown", &local_user()),
            Some(AuthDecision::Allow)
        );
    }

    #[test]
    fn ruleset_defaults_remote_user_shutdown_needs_auth() {
        let rs = RuleSet::with_defaults();
        assert_eq!(
            rs.evaluate("org.liquide.system.shutdown", &regular_user()),
            Some(AuthDecision::AuthRequired(AuthType::UserPassword))
        );
    }

    #[test]
    fn ruleset_defaults_unknown_action_denied() {
        let rs = RuleSet::with_defaults();
        assert_eq!(
            rs.evaluate("com.unknown.action", &regular_user()),
            Some(AuthDecision::Deny)
        );
    }

    #[test]
    fn ruleset_defaults_settings_read_allowed() {
        let rs = RuleSet::with_defaults();
        assert_eq!(
            rs.evaluate("org.liquide.settings.read", &regular_user()),
            Some(AuthDecision::Allow)
        );
    }

    #[test]
    fn ruleset_defaults_settings_write_admin_allowed() {
        let rs = RuleSet::with_defaults();
        assert_eq!(
            rs.evaluate("org.liquide.settings.write", &admin_user()),
            Some(AuthDecision::Allow)
        );
    }

    #[test]
    fn ruleset_defaults_settings_write_user_needs_auth() {
        let rs = RuleSet::with_defaults();
        assert_eq!(
            rs.evaluate("org.liquide.settings.write", &regular_user()),
            Some(AuthDecision::AuthRequired(AuthType::UserPassword))
        );
    }

    #[test]
    fn ruleset_serde_roundtrip() {
        let rs = RuleSet::with_defaults();
        let json = serde_json::to_string(&rs).unwrap();
        let back: RuleSet = serde_json::from_str(&json).unwrap();
        assert_eq!(rs.len(), back.len());
    }

    #[test]
    fn rule_serde_roundtrip() {
        let rule = Rule::new(
            "org.liquide.test.*",
            SubjectMatch::All(vec![
                SubjectMatch::InGroup("users".into()),
                SubjectMatch::IsLocal,
            ]),
            AuthDecision::AuthRequired(AuthType::UserPassword),
        )
        .with_description("test rule");
        let json = serde_json::to_string(&rule).unwrap();
        let back: Rule = serde_json::from_str(&json).unwrap();
        assert_eq!(rule, back);
    }

    #[test]
    fn insert_beyond_end_clamps() {
        let mut rs = RuleSet::new();
        rs.insert_rule(100, Rule::new("*", SubjectMatch::Any, AuthDecision::Deny));
        assert_eq!(rs.len(), 1);
    }
}
