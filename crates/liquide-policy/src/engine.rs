//! Policy engine core: loading, caching, and querying policies.

use std::path::Path;

use serde::Deserialize;

use crate::rule::{Rule, RuleAction, RuleSet};
use crate::{PolicyEngine, PolicyError, PolicySource, Result};

/// On-disk representation of a single TOML policy file.
#[derive(Debug, Deserialize)]
struct PolicyFile {
    /// The source level for the rules in this file.
    source: String,
    /// The rules contained in this policy file.
    rules: Vec<RuleEntry>,
}

/// On-disk representation of a single rule inside a TOML policy file.
#[derive(Debug, Deserialize)]
struct RuleEntry {
    /// The policy key (e.g. `"clipboard.enabled"`).
    key: String,
    /// The action: `"allow"`, `"deny"`, or `"set:<value>"`.
    action: String,
}

/// Parse a [`PolicySource`] from a string.
fn parse_source(s: &str) -> Result<PolicySource> {
    match s.to_lowercase().as_str() {
        "server" => Ok(PolicySource::Server),
        "group" => Ok(PolicySource::Group),
        "user" => Ok(PolicySource::User),
        "session" => Ok(PolicySource::Session),
        other => Err(PolicyError::Parse(format!("unknown policy source: {other}"))),
    }
}

/// Parse a [`RuleAction`] from a string.
fn parse_action(s: &str) -> Result<RuleAction> {
    match s.to_lowercase().as_str() {
        "allow" => Ok(RuleAction::Allow),
        "deny" => Ok(RuleAction::Deny),
        other => {
            if let Some(value) = other.strip_prefix("set:") {
                Ok(RuleAction::Set(value.to_string()))
            } else {
                Err(PolicyError::Parse(format!(
                    "unknown rule action: {other} (expected \"allow\", \"deny\", or \"set:<value>\")"
                )))
            }
        }
    }
}

/// Load a [`PolicyEngine`] from a TOML configuration directory.
///
/// Reads every `*.toml` file in `dir`, parses it as a [`PolicyFile`], and
/// adds the resulting rules to the engine at the declared source level.
///
/// Returns an error if any file cannot be read or contains invalid TOML /
/// missing required fields.
pub fn load_from_dir(dir: &Path) -> Result<PolicyEngine> {
    let mut engine = PolicyEngine::new();

    let entries = std::fs::read_dir(dir).map_err(|e| {
        PolicyError::NotFound(format!("cannot read policy directory {}: {e}", dir.display()))
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            PolicyError::Parse(format!("error reading directory entry: {e}"))
        })?;
        let path = entry.path();

        // Only process .toml files.
        let is_toml = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("toml"))
            .unwrap_or(false);
        if !is_toml {
            continue;
        }

        let contents = std::fs::read_to_string(&path).map_err(|e| {
            PolicyError::Parse(format!("cannot read {}: {e}", path.display()))
        })?;

        let policy_file: PolicyFile = toml::from_str(&contents).map_err(|e| {
            PolicyError::Parse(format!("{}: {e}", path.display()))
        })?;

        let source = parse_source(&policy_file.source).map_err(|e| {
            PolicyError::Parse(format!("{}: {e}", path.display()))
        })?;

        let mut ruleset = RuleSet::new();
        for rule_entry in policy_file.rules {
            let action = parse_action(&rule_entry.action).map_err(|e| {
                PolicyError::Parse(format!("{}: {e}", path.display()))
            })?;
            ruleset.push(Rule {
                key: rule_entry.key,
                action,
            });
        }

        engine.add_layer(source, ruleset);
    }

    Ok(engine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_dir() -> TempDir {
        TempDir::new().expect("failed to create temp dir")
    }

    #[test]
    fn load_empty_dir() {
        let dir = make_dir();
        let engine = load_from_dir(dir.path()).unwrap();
        let policy = engine.evaluate();
        // Default effective policy values.
        assert!(policy.clipboard_enabled);
        assert!(!policy.usb_redirect_enabled);
    }

    #[test]
    fn load_single_policy_file() {
        let dir = make_dir();
        let toml = r#"
source = "server"

[[rules]]
key = "clipboard.enabled"
action = "deny"

[[rules]]
key = "display.max_width"
action = "set:1920"
"#;
        fs::write(dir.path().join("server.toml"), toml).unwrap();

        let engine = load_from_dir(dir.path()).unwrap();
        let policy = engine.evaluate();
        assert!(!policy.clipboard_enabled);
        assert_eq!(policy.max_resolution_w, 1920);
    }

    #[test]
    fn load_multiple_files_with_hierarchy() {
        let dir = make_dir();

        // Server-level: deny clipboard.
        let server = r#"
source = "server"
[[rules]]
key = "clipboard.enabled"
action = "deny"
"#;
        // User-level: re-allow clipboard.
        let user = r#"
source = "user"
[[rules]]
key = "clipboard.enabled"
action = "allow"
"#;
        fs::write(dir.path().join("server.toml"), server).unwrap();
        fs::write(dir.path().join("user.toml"), user).unwrap();

        let engine = load_from_dir(dir.path()).unwrap();
        let policy = engine.evaluate();
        // User-level override should win.
        assert!(policy.clipboard_enabled);
    }

    #[test]
    fn malformed_toml_returns_error() {
        let dir = make_dir();
        fs::write(dir.path().join("bad.toml"), "this is not valid toml {{{").unwrap();

        let result = load_from_dir(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("bad.toml"), "error should mention filename: {err}");
    }

    #[test]
    fn missing_required_source_field() {
        let dir = make_dir();
        let toml = r#"
[[rules]]
key = "clipboard.enabled"
action = "allow"
"#;
        fs::write(dir.path().join("no_source.toml"), toml).unwrap();

        let result = load_from_dir(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn invalid_source_value() {
        let dir = make_dir();
        let toml = r#"
source = "galaxy"
[[rules]]
key = "clipboard.enabled"
action = "allow"
"#;
        fs::write(dir.path().join("bad_source.toml"), toml).unwrap();

        let result = load_from_dir(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown policy source"));
    }

    #[test]
    fn invalid_action_value() {
        let dir = make_dir();
        let toml = r#"
source = "server"
[[rules]]
key = "clipboard.enabled"
action = "maybe"
"#;
        fs::write(dir.path().join("bad_action.toml"), toml).unwrap();

        let result = load_from_dir(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown rule action"));
    }

    #[test]
    fn nonexistent_dir_returns_error() {
        let result = load_from_dir(Path::new("/nonexistent/policy/dir"));
        assert!(result.is_err());
    }

    #[test]
    fn non_toml_files_are_ignored() {
        let dir = make_dir();
        fs::write(dir.path().join("readme.md"), "# not a policy").unwrap();
        fs::write(dir.path().join("notes.txt"), "some notes").unwrap();

        let engine = load_from_dir(dir.path()).unwrap();
        let policy = engine.evaluate();
        // Should behave like an empty dir — all defaults.
        assert!(policy.clipboard_enabled);
    }

    #[test]
    fn set_action_parses_correctly() {
        let dir = make_dir();
        let toml = r#"
source = "session"
[[rules]]
key = "session.idle_timeout"
action = "set:300"

[[rules]]
key = "display.max_height"
action = "set:1080"
"#;
        fs::write(dir.path().join("session.toml"), toml).unwrap();

        let engine = load_from_dir(dir.path()).unwrap();
        let policy = engine.evaluate();
        assert_eq!(policy.idle_timeout_secs, 300);
        assert_eq!(policy.max_resolution_h, 1080);
    }
}
