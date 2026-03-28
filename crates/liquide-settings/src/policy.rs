//! Desktop policy system for managed/enterprise configuration.
//!
//! Policies allow system administrators and organizations to enforce, restrict,
//! or provide defaults for desktop settings. Inspired by GNOME dconf locks and
//! enterprise management patterns.

use std::collections::HashMap;
use std::fmt;

/// A policy key using dotted path notation (e.g. "desktop.background.allow-change").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PolicyKey(String);

impl PolicyKey {
    /// Create a new policy key. The key must be a non-empty dotted path.
    pub fn new(s: &str) -> Option<Self> {
        if s.is_empty() || s.starts_with('.') || s.ends_with('.') || s.contains("..") {
            return None;
        }
        Some(Self(s.to_string()))
    }

    /// Create a policy key without validation (for internal/test use).
    pub fn unchecked(s: &str) -> Self {
        Self(s.to_string())
    }

    /// Return the full key string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return the top-level category (segment before the first dot).
    pub fn category(&self) -> &str {
        match self.0.find('.') {
            Some(idx) => &self.0[..idx],
            None => &self.0,
        }
    }

    /// Check whether this key is a prefix of another key.
    pub fn is_prefix_of(&self, other: &PolicyKey) -> bool {
        other.0.starts_with(&self.0) && other.0.len() > self.0.len()
            && other.0.as_bytes()[self.0.len()] == b'.'
    }
}

impl fmt::Display for PolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The resolved value of a policy for a given key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyValue {
    /// The setting may be freely changed by the user.
    Allow,
    /// The setting is denied / hidden from the user.
    Deny,
    /// The setting is forced to the given value and cannot be changed.
    Force(String),
    /// A default value is provided but the user may override it.
    Default(String),
}

impl PolicyValue {
    /// Returns true if this policy locks the setting (user cannot change it).
    pub fn is_locked(&self) -> bool {
        matches!(self, Self::Force(_) | Self::Deny)
    }
}

/// The source of a policy, determining its priority.
/// Higher-priority sources override lower ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PolicySource {
    /// User-level policy (lowest priority).
    User = 0,
    /// Organization-level policy (MDM, fleet management).
    Organization = 1,
    /// System-level policy (highest priority, set by the OS distributor).
    System = 2,
}

impl PolicySource {
    /// Returns all sources in priority order (lowest first).
    pub fn all() -> &'static [PolicySource] {
        &[Self::User, Self::Organization, Self::System]
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::User => "User",
            Self::Organization => "Organization",
            Self::System => "System",
        }
    }
}

/// A single policy entry from a given source.
#[derive(Debug, Clone)]
pub struct PolicyEntry {
    pub key: PolicyKey,
    pub value: PolicyValue,
    pub source: PolicySource,
    /// Optional description for admin tooling.
    pub description: Option<String>,
}

/// Database that merges policies from multiple sources and evaluates them.
pub struct PolicyDatabase {
    /// Policies indexed by key, with all source entries stored.
    entries: HashMap<String, Vec<PolicyEntry>>,
}

impl PolicyDatabase {
    /// Create an empty policy database.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Add a policy entry. Multiple entries for the same key from different
    /// sources are allowed; the highest-priority source wins at evaluation time.
    pub fn add(&mut self, entry: PolicyEntry) {
        let key = entry.key.as_str().to_string();
        self.entries.entry(key).or_default().push(entry);
    }

    /// Add a policy entry using a builder-style shorthand.
    pub fn set_policy(
        &mut self,
        key: &str,
        value: PolicyValue,
        source: PolicySource,
    ) {
        if let Some(pk) = PolicyKey::new(key) {
            self.add(PolicyEntry {
                key: pk,
                value,
                source,
                description: None,
            });
        }
    }

    /// Remove all policies for a given key.
    pub fn remove(&mut self, key: &str) {
        self.entries.remove(key);
    }

    /// Remove all policies from a specific source.
    pub fn remove_source(&mut self, source: PolicySource) {
        for entries in self.entries.values_mut() {
            entries.retain(|e| e.source != source);
        }
        self.entries.retain(|_, v| !v.is_empty());
    }

    /// Load policies from a text-based policy file format.
    /// Format: one entry per line, `source:key=action[:value]`
    /// Lines starting with `#` are comments, blank lines are skipped.
    pub fn load_from_text(&mut self, text: &str) -> Result<usize, PolicyError> {
        let mut count = 0;
        for (line_no, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let (source_str, rest) = line.split_once(':').ok_or_else(|| {
                PolicyError::ParseError(line_no + 1, "missing source prefix".into())
            })?;
            let source = match source_str {
                "system" => PolicySource::System,
                "org" | "organization" => PolicySource::Organization,
                "user" => PolicySource::User,
                _ => {
                    return Err(PolicyError::ParseError(
                        line_no + 1,
                        format!("unknown source '{}'", source_str),
                    ));
                }
            };

            let (key_str, action_str) = rest.split_once('=').ok_or_else(|| {
                PolicyError::ParseError(line_no + 1, "missing '=' separator".into())
            })?;

            let key = PolicyKey::new(key_str.trim()).ok_or_else(|| {
                PolicyError::ParseError(line_no + 1, format!("invalid key '{}'", key_str.trim()))
            })?;

            let action = action_str.trim();
            let value = if action == "allow" {
                PolicyValue::Allow
            } else if action == "deny" {
                PolicyValue::Deny
            } else if let Some(forced) = action.strip_prefix("force:") {
                PolicyValue::Force(forced.to_string())
            } else if let Some(default) = action.strip_prefix("default:") {
                PolicyValue::Default(default.to_string())
            } else {
                return Err(PolicyError::ParseError(
                    line_no + 1,
                    format!("unknown action '{}'", action),
                ));
            };

            self.add(PolicyEntry {
                key,
                value,
                source,
                description: None,
            });
            count += 1;
        }
        Ok(count)
    }

    /// Evaluate the effective policy for a key. Returns the value from the
    /// highest-priority source, or `PolicyValue::Allow` if no policy exists.
    pub fn evaluate(&self, key: &str) -> PolicyValue {
        if let Some(entries) = self.entries.get(key) {
            let mut best: Option<&PolicyEntry> = None;
            for entry in entries {
                if best.is_none() || entry.source > best.unwrap().source {
                    best = Some(entry);
                }
            }
            if let Some(entry) = best {
                return entry.value.clone();
            }
        }
        PolicyValue::Allow
    }

    /// Check whether a key is locked (cannot be changed by the user).
    /// A key is locked if a System or Organization source sets Force or Deny.
    pub fn is_locked(&self, key: &str) -> bool {
        if let Some(entries) = self.entries.get(key) {
            for entry in entries {
                if entry.source >= PolicySource::Organization && entry.value.is_locked() {
                    return true;
                }
            }
        }
        false
    }

    /// Given a user's preferred value for a setting, return the effective value
    /// after applying any policy overrides.
    pub fn effective_value(&self, key: &str, user_preference: &str) -> String {
        match self.evaluate(key) {
            PolicyValue::Allow => user_preference.to_string(),
            PolicyValue::Deny => String::new(),
            PolicyValue::Force(forced) => forced,
            PolicyValue::Default(default) => {
                if user_preference.is_empty() {
                    default
                } else {
                    user_preference.to_string()
                }
            }
        }
    }

    /// Return all keys that have at least one policy entry.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.keys().map(|k| k.as_str()).collect()
    }

    /// Return all entries for a given key, sorted by source priority (lowest first).
    pub fn entries_for(&self, key: &str) -> Vec<&PolicyEntry> {
        if let Some(entries) = self.entries.get(key) {
            let mut sorted: Vec<&PolicyEntry> = entries.iter().collect();
            sorted.sort_by_key(|e| e.source);
            sorted
        } else {
            Vec::new()
        }
    }

    /// Return the total number of policy entries across all keys.
    pub fn entry_count(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }

    /// Return the number of distinct keys with policies.
    pub fn key_count(&self) -> usize {
        self.entries.len()
    }
}

/// Errors from policy operations.
#[derive(Debug, Clone)]
pub enum PolicyError {
    /// Error parsing a policy file at a specific line.
    ParseError(usize, String),
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseError(line, msg) => write!(f, "policy parse error at line {}: {}", line, msg),
        }
    }
}

impl std::error::Error for PolicyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_key_valid() {
        assert!(PolicyKey::new("desktop.background.allow-change").is_some());
        assert!(PolicyKey::new("input.keyboard").is_some());
        assert!(PolicyKey::new("single").is_some());
    }

    #[test]
    fn policy_key_invalid() {
        assert!(PolicyKey::new("").is_none());
        assert!(PolicyKey::new(".leading").is_none());
        assert!(PolicyKey::new("trailing.").is_none());
        assert!(PolicyKey::new("double..dot").is_none());
    }

    #[test]
    fn policy_key_category() {
        let k = PolicyKey::unchecked("desktop.background.wallpaper");
        assert_eq!(k.category(), "desktop");
    }

    #[test]
    fn policy_key_prefix() {
        let parent = PolicyKey::unchecked("desktop");
        let child = PolicyKey::unchecked("desktop.background");
        let unrelated = PolicyKey::unchecked("input.keyboard");

        assert!(parent.is_prefix_of(&child));
        assert!(!parent.is_prefix_of(&unrelated));
        assert!(!parent.is_prefix_of(&parent));
    }

    #[test]
    fn policy_value_is_locked() {
        assert!(!PolicyValue::Allow.is_locked());
        assert!(PolicyValue::Deny.is_locked());
        assert!(PolicyValue::Force("x".into()).is_locked());
        assert!(!PolicyValue::Default("x".into()).is_locked());
    }

    #[test]
    fn policy_source_ordering() {
        assert!(PolicySource::System > PolicySource::Organization);
        assert!(PolicySource::Organization > PolicySource::User);
    }

    #[test]
    fn evaluate_no_policy_returns_allow() {
        let db = PolicyDatabase::new();
        assert_eq!(db.evaluate("nonexistent.key"), PolicyValue::Allow);
    }

    #[test]
    fn evaluate_single_source() {
        let mut db = PolicyDatabase::new();
        db.set_policy("desktop.wallpaper", PolicyValue::Deny, PolicySource::Organization);
        assert_eq!(db.evaluate("desktop.wallpaper"), PolicyValue::Deny);
    }

    #[test]
    fn evaluate_highest_priority_wins() {
        let mut db = PolicyDatabase::new();
        db.set_policy("desktop.theme", PolicyValue::Allow, PolicySource::User);
        db.set_policy("desktop.theme", PolicyValue::Force("night".into()), PolicySource::System);
        db.set_policy("desktop.theme", PolicyValue::Default("midday".into()), PolicySource::Organization);

        assert_eq!(db.evaluate("desktop.theme"), PolicyValue::Force("night".into()));
    }

    #[test]
    fn is_locked_by_org() {
        let mut db = PolicyDatabase::new();
        db.set_policy("desktop.wallpaper", PolicyValue::Force("/usr/share/bg.png".into()), PolicySource::Organization);
        assert!(db.is_locked("desktop.wallpaper"));
    }

    #[test]
    fn is_not_locked_by_user() {
        let mut db = PolicyDatabase::new();
        db.set_policy("desktop.wallpaper", PolicyValue::Force("/home/user/bg.png".into()), PolicySource::User);
        assert!(!db.is_locked("desktop.wallpaper"));
    }

    #[test]
    fn is_locked_deny() {
        let mut db = PolicyDatabase::new();
        db.set_policy("privacy.screen-share", PolicyValue::Deny, PolicySource::System);
        assert!(db.is_locked("privacy.screen-share"));
    }

    #[test]
    fn effective_value_allow() {
        let db = PolicyDatabase::new();
        assert_eq!(db.effective_value("desktop.theme", "night"), "night");
    }

    #[test]
    fn effective_value_force() {
        let mut db = PolicyDatabase::new();
        db.set_policy("desktop.theme", PolicyValue::Force("corporate".into()), PolicySource::Organization);
        assert_eq!(db.effective_value("desktop.theme", "night"), "corporate");
    }

    #[test]
    fn effective_value_deny() {
        let mut db = PolicyDatabase::new();
        db.set_policy("desktop.wallpaper", PolicyValue::Deny, PolicySource::System);
        assert_eq!(db.effective_value("desktop.wallpaper", "/home/user/bg.png"), "");
    }

    #[test]
    fn effective_value_default_with_user_pref() {
        let mut db = PolicyDatabase::new();
        db.set_policy("desktop.theme", PolicyValue::Default("corporate".into()), PolicySource::Organization);
        assert_eq!(db.effective_value("desktop.theme", "night"), "night");
    }

    #[test]
    fn effective_value_default_without_user_pref() {
        let mut db = PolicyDatabase::new();
        db.set_policy("desktop.theme", PolicyValue::Default("corporate".into()), PolicySource::Organization);
        assert_eq!(db.effective_value("desktop.theme", ""), "corporate");
    }

    #[test]
    fn remove_policy() {
        let mut db = PolicyDatabase::new();
        db.set_policy("desktop.theme", PolicyValue::Deny, PolicySource::System);
        assert_eq!(db.evaluate("desktop.theme"), PolicyValue::Deny);
        db.remove("desktop.theme");
        assert_eq!(db.evaluate("desktop.theme"), PolicyValue::Allow);
    }

    #[test]
    fn remove_source() {
        let mut db = PolicyDatabase::new();
        db.set_policy("desktop.theme", PolicyValue::Deny, PolicySource::Organization);
        db.set_policy("desktop.theme", PolicyValue::Allow, PolicySource::User);
        db.set_policy("input.speed", PolicyValue::Force("1.5".into()), PolicySource::Organization);

        db.remove_source(PolicySource::Organization);

        assert_eq!(db.evaluate("desktop.theme"), PolicyValue::Allow);
        assert_eq!(db.evaluate("input.speed"), PolicyValue::Allow);
    }

    #[test]
    fn load_from_text_basic() {
        let mut db = PolicyDatabase::new();
        let text = r#"
# Policy file
system:desktop.wallpaper=deny
org:desktop.theme=force:corporate-theme
user:input.speed=default:1.0
"#;
        let count = db.load_from_text(text).unwrap();
        assert_eq!(count, 3);
        assert_eq!(db.evaluate("desktop.wallpaper"), PolicyValue::Deny);
        assert_eq!(db.evaluate("desktop.theme"), PolicyValue::Force("corporate-theme".into()));
        assert_eq!(db.evaluate("input.speed"), PolicyValue::Default("1.0".into()));
    }

    #[test]
    fn load_from_text_allow() {
        let mut db = PolicyDatabase::new();
        let text = "system:desktop.wallpaper=allow\n";
        let count = db.load_from_text(text).unwrap();
        assert_eq!(count, 1);
        assert_eq!(db.evaluate("desktop.wallpaper"), PolicyValue::Allow);
    }

    #[test]
    fn load_from_text_invalid_source() {
        let mut db = PolicyDatabase::new();
        let text = "admin:desktop.wallpaper=deny\n";
        assert!(db.load_from_text(text).is_err());
    }

    #[test]
    fn load_from_text_invalid_action() {
        let mut db = PolicyDatabase::new();
        let text = "system:desktop.wallpaper=restrict\n";
        assert!(db.load_from_text(text).is_err());
    }

    #[test]
    fn load_from_text_missing_separator() {
        let mut db = PolicyDatabase::new();
        let text = "system desktop.wallpaper deny\n";
        assert!(db.load_from_text(text).is_err());
    }

    #[test]
    fn entries_for_sorted_by_priority() {
        let mut db = PolicyDatabase::new();
        db.set_policy("desktop.theme", PolicyValue::Allow, PolicySource::System);
        db.set_policy("desktop.theme", PolicyValue::Deny, PolicySource::User);
        db.set_policy("desktop.theme", PolicyValue::Force("x".into()), PolicySource::Organization);

        let entries = db.entries_for("desktop.theme");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].source, PolicySource::User);
        assert_eq!(entries[1].source, PolicySource::Organization);
        assert_eq!(entries[2].source, PolicySource::System);
    }

    #[test]
    fn key_count_and_entry_count() {
        let mut db = PolicyDatabase::new();
        db.set_policy("a.b", PolicyValue::Allow, PolicySource::User);
        db.set_policy("a.b", PolicyValue::Deny, PolicySource::System);
        db.set_policy("c.d", PolicyValue::Allow, PolicySource::User);

        assert_eq!(db.key_count(), 2);
        assert_eq!(db.entry_count(), 3);
    }

    #[test]
    fn policy_source_labels() {
        assert_eq!(PolicySource::User.label(), "User");
        assert_eq!(PolicySource::Organization.label(), "Organization");
        assert_eq!(PolicySource::System.label(), "System");
    }

    #[test]
    fn policy_source_all() {
        let all = PolicySource::all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], PolicySource::User);
        assert_eq!(all[2], PolicySource::System);
    }

    #[test]
    fn policy_key_display() {
        let k = PolicyKey::unchecked("desktop.background.allow-change");
        assert_eq!(format!("{}", k), "desktop.background.allow-change");
    }

    #[test]
    fn policy_error_display() {
        let err = PolicyError::ParseError(5, "bad token".into());
        let msg = format!("{}", err);
        assert!(msg.contains("line 5"));
        assert!(msg.contains("bad token"));
    }

    #[test]
    fn load_from_text_organization_alias() {
        let mut db = PolicyDatabase::new();
        let text = "organization:desktop.wallpaper=deny\n";
        let count = db.load_from_text(text).unwrap();
        assert_eq!(count, 1);
    }
}
