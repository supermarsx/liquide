//! Workspace persistence: snapshots, serialization, and window placement
//! rules.
//!
//! [`WorkspaceSnapshot`] captures the full workspace manager state for
//! session save/restore. [`WindowRule`] and [`WindowRuleEngine`] allow
//! automatic window placement based on app_id / title patterns.

use crate::manager::WorkspaceManager;
use crate::workspace::{Workspace, WorkspaceId};
use serde::{Deserialize, Serialize};

// ── WorkspaceSnapshot ────────────────────────────────────────────────

/// A serializable snapshot of the entire workspace manager state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    /// All workspaces in index order.
    pub workspaces: Vec<Workspace>,
    /// The active workspace ID.
    pub active_id: WorkspaceId,
    /// Next ID counter.
    pub next_id: u32,
}

impl WorkspaceSnapshot {
    /// Capture the current state of a workspace manager.
    pub fn capture(manager: &WorkspaceManager) -> Self {
        Self {
            workspaces: manager.all_workspaces().to_vec(),
            active_id: manager.active_workspace(),
            next_id: manager.next_id_raw(),
        }
    }

    /// Restore workspace state into a manager. The manager's workspace list
    /// is replaced entirely.
    pub fn restore(self, manager: &mut WorkspaceManager) {
        manager.set_next_id(self.next_id);
        manager.replace_workspaces(self.workspaces, self.active_id);
    }

    /// Serialize to JSON.
    pub fn serialize(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize from JSON.
    pub fn deserialize(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ── WindowRule ───────────────────────────────────────────────────────

/// A rule that matches windows by app_id and/or title pattern and assigns
/// them to a workspace with optional position and size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowRule {
    /// Optional glob-like pattern for the application ID (e.g.
    /// "org.mozilla.Firefox"). Supports `*` wildcard.
    pub app_id_pattern: Option<String>,
    /// Optional substring match for the window title.
    pub title_contains: Option<String>,
    /// Target workspace index (0-based). If `None`, no workspace
    /// assignment.
    pub target_workspace_index: Option<usize>,
    /// Optional fixed position.
    pub position: Option<(i32, i32)>,
    /// Optional fixed size.
    pub size: Option<(u32, u32)>,
    /// If true, the window starts maximized.
    pub start_maximized: bool,
    /// If true, the window starts minimized.
    pub start_minimized: bool,
}

impl WindowRule {
    /// Create a simple rule that assigns windows matching `app_id` to the
    /// given workspace index.
    pub fn for_app(app_id: impl Into<String>, workspace_index: usize) -> Self {
        Self {
            app_id_pattern: Some(app_id.into()),
            title_contains: None,
            target_workspace_index: Some(workspace_index),
            position: None,
            size: None,
            start_maximized: false,
            start_minimized: false,
        }
    }

    /// Check if this rule matches a window with the given app_id and title.
    pub fn matches(&self, app_id: &str, title: &str) -> bool {
        let app_match = match &self.app_id_pattern {
            None => true,
            Some(pattern) => glob_match(pattern, app_id),
        };
        let title_match = match &self.title_contains {
            None => true,
            Some(substr) => title.contains(substr.as_str()),
        };
        app_match && title_match
    }
}

/// Minimal glob matching: supports `*` as "match anything" wildcard.
fn glob_match(pattern: &str, input: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == input;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0usize;

    // First part must match at the start (unless empty from leading *).
    if !parts[0].is_empty() {
        if !input.starts_with(parts[0]) {
            return false;
        }
        pos = parts[0].len();
    }

    // Middle parts must appear in order.
    for part in &parts[1..parts.len().saturating_sub(1)] {
        if part.is_empty() {
            continue;
        }
        match input[pos..].find(part) {
            Some(idx) => pos += idx + part.len(),
            None => return false,
        }
    }

    // Last part must match at the end (unless empty from trailing *).
    if let Some(last) = parts.last() {
        if !last.is_empty() {
            return input[pos..].ends_with(last);
        }
    }

    true
}

// ── WindowRuleResult ─────────────────────────────────────────────────

/// The result of evaluating window rules: the first matching rule's
/// assignments.
#[derive(Debug, Clone, Default)]
pub struct WindowRuleResult {
    /// Target workspace index, if any.
    pub target_workspace_index: Option<usize>,
    /// Position override, if any.
    pub position: Option<(i32, i32)>,
    /// Size override, if any.
    pub size: Option<(u32, u32)>,
    /// Whether to start maximized.
    pub start_maximized: bool,
    /// Whether to start minimized.
    pub start_minimized: bool,
}

// ── WindowRuleEngine ─────────────────────────────────────────────────

/// Evaluates window rules against incoming windows. Rules are checked in
/// order; the first match wins.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowRuleEngine {
    rules: Vec<WindowRule>,
}

impl WindowRuleEngine {
    /// Create an empty rule engine.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a rule to the end of the list.
    pub fn add_rule(&mut self, rule: WindowRule) {
        self.rules.push(rule);
    }

    /// Remove all rules.
    pub fn clear(&mut self) {
        self.rules.clear();
    }

    /// Return the number of rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Return a reference to all rules.
    pub fn rules(&self) -> &[WindowRule] {
        &self.rules
    }

    /// Evaluate all rules against a window. Returns the first match, or
    /// `None` if no rule matches.
    pub fn evaluate(&self, app_id: &str, title: &str) -> Option<WindowRuleResult> {
        for rule in &self.rules {
            if rule.matches(app_id, title) {
                return Some(WindowRuleResult {
                    target_workspace_index: rule.target_workspace_index,
                    position: rule.position,
                    size: rule.size,
                    start_maximized: rule.start_maximized,
                    start_minimized: rule.start_minimized,
                });
            }
        }
        None
    }

    /// Serialize the rule set to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.rules)
    }

    /// Deserialize a rule set from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let rules: Vec<WindowRule> = serde_json::from_str(json)?;
        Ok(Self { rules })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::WorkspaceManager;

    // ── WorkspaceSnapshot ───────────────────────────────────────────

    #[test]
    fn snapshot_capture_and_restore() {
        let mut mgr = WorkspaceManager::new();
        let id2 = mgr.create_workspace(Some("Code".into())).unwrap();
        mgr.workspace_mut(mgr.active_workspace())
            .unwrap()
            .add_window(1);
        mgr.workspace_mut(id2).unwrap().add_window(2);
        mgr.switch_to(id2);

        let snap = WorkspaceSnapshot::capture(&mgr);
        assert_eq!(snap.workspaces.len(), 2);
        assert_eq!(snap.active_id, id2);

        let mut mgr2 = WorkspaceManager::new();
        snap.restore(&mut mgr2);
        assert_eq!(mgr2.workspace_count(), 2);
        assert_eq!(mgr2.active_workspace(), id2);
        assert!(mgr2.workspace(id2).unwrap().has_window(2));
    }

    #[test]
    fn snapshot_json_roundtrip() {
        let mut mgr = WorkspaceManager::new();
        mgr.create_workspace(Some("Music".into()));
        let snap = WorkspaceSnapshot::capture(&mgr);
        let json = snap.serialize().unwrap();
        let snap2 = WorkspaceSnapshot::deserialize(&json).unwrap();
        assert_eq!(snap2.workspaces.len(), snap.workspaces.len());
        assert_eq!(snap2.active_id, snap.active_id);
    }

    #[test]
    fn snapshot_preserves_wallpaper() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.active_workspace();
        mgr.workspace_mut(id)
            .unwrap()
            .set_wallpaper(Some("/usr/share/wallpapers/beach.jpg".into()));
        let snap = WorkspaceSnapshot::capture(&mgr);
        let json = snap.serialize().unwrap();
        let snap2 = WorkspaceSnapshot::deserialize(&json).unwrap();
        assert_eq!(
            snap2.workspaces[0].wallpaper_override.as_deref(),
            Some("/usr/share/wallpapers/beach.jpg")
        );
    }

    // ── glob_match ──────────────────────────────────────────────────

    #[test]
    fn glob_exact() {
        assert!(glob_match("firefox", "firefox"));
        assert!(!glob_match("firefox", "chrome"));
    }

    #[test]
    fn glob_star_only() {
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn glob_prefix_star() {
        assert!(glob_match("org.*", "org.mozilla.Firefox"));
        assert!(!glob_match("org.*", "com.google.Chrome"));
    }

    #[test]
    fn glob_suffix_star() {
        assert!(glob_match("*Firefox", "org.mozilla.Firefox"));
        assert!(!glob_match("*Firefox", "org.mozilla.Thunderbird"));
    }

    #[test]
    fn glob_middle_star() {
        assert!(glob_match("org.*.Firefox", "org.mozilla.Firefox"));
        assert!(!glob_match("org.*.Firefox", "org.mozilla.Thunderbird"));
    }

    #[test]
    fn glob_multiple_stars() {
        assert!(glob_match("*mozilla*", "org.mozilla.Firefox"));
        assert!(!glob_match("*mozilla*", "com.google.Chrome"));
    }

    // ── WindowRule ──────────────────────────────────────────────────

    #[test]
    fn rule_matches_app_id() {
        let rule = WindowRule::for_app("org.mozilla.Firefox", 1);
        assert!(rule.matches("org.mozilla.Firefox", "anything"));
        assert!(!rule.matches("com.google.Chrome", "anything"));
    }

    #[test]
    fn rule_matches_title_contains() {
        let rule = WindowRule {
            app_id_pattern: None,
            title_contains: Some("Untitled".into()),
            target_workspace_index: Some(0),
            position: None,
            size: None,
            start_maximized: false,
            start_minimized: false,
        };
        assert!(rule.matches("any.app", "Untitled Document"));
        assert!(!rule.matches("any.app", "My Project"));
    }

    #[test]
    fn rule_matches_both_app_and_title() {
        let rule = WindowRule {
            app_id_pattern: Some("org.gnome.Terminal".into()),
            title_contains: Some("root@".into()),
            target_workspace_index: Some(2),
            position: None,
            size: None,
            start_maximized: false,
            start_minimized: false,
        };
        assert!(rule.matches("org.gnome.Terminal", "root@server: /home"));
        assert!(!rule.matches("org.gnome.Terminal", "user@laptop: ~"));
        assert!(!rule.matches("com.other.App", "root@server: /home"));
    }

    #[test]
    fn rule_no_patterns_matches_everything() {
        let rule = WindowRule {
            app_id_pattern: None,
            title_contains: None,
            target_workspace_index: Some(0),
            position: None,
            size: None,
            start_maximized: false,
            start_minimized: false,
        };
        assert!(rule.matches("any.app", "any title"));
    }

    // ── WindowRuleEngine ────────────────────────────────────────────

    #[test]
    fn engine_first_match_wins() {
        let mut engine = WindowRuleEngine::new();
        engine.add_rule(WindowRule::for_app("org.mozilla.Firefox", 1));
        engine.add_rule(WindowRule::for_app("*", 0)); // catch-all

        let result = engine.evaluate("org.mozilla.Firefox", "Home").unwrap();
        assert_eq!(result.target_workspace_index, Some(1));
    }

    #[test]
    fn engine_no_match_returns_none() {
        let mut engine = WindowRuleEngine::new();
        engine.add_rule(WindowRule::for_app("org.mozilla.Firefox", 1));
        assert!(engine.evaluate("com.google.Chrome", "").is_none());
    }

    #[test]
    fn engine_clear_removes_all() {
        let mut engine = WindowRuleEngine::new();
        engine.add_rule(WindowRule::for_app("*", 0));
        assert_eq!(engine.rule_count(), 1);
        engine.clear();
        assert_eq!(engine.rule_count(), 0);
    }

    #[test]
    fn engine_json_roundtrip() {
        let mut engine = WindowRuleEngine::new();
        engine.add_rule(WindowRule::for_app("org.mozilla.Firefox", 1));
        engine.add_rule(WindowRule::for_app("org.gnome.Terminal", 2));
        let json = engine.to_json().unwrap();
        let engine2 = WindowRuleEngine::from_json(&json).unwrap();
        assert_eq!(engine2.rule_count(), 2);
    }

    #[test]
    fn engine_result_with_position_and_size() {
        let mut engine = WindowRuleEngine::new();
        engine.add_rule(WindowRule {
            app_id_pattern: Some("org.gnome.Calculator".into()),
            title_contains: None,
            target_workspace_index: None,
            position: Some((100, 200)),
            size: Some((300, 400)),
            start_maximized: false,
            start_minimized: false,
        });
        let result = engine.evaluate("org.gnome.Calculator", "Calculator").unwrap();
        assert_eq!(result.position, Some((100, 200)));
        assert_eq!(result.size, Some((300, 400)));
        assert!(result.target_workspace_index.is_none());
    }

    #[test]
    fn engine_result_start_maximized() {
        let mut engine = WindowRuleEngine::new();
        engine.add_rule(WindowRule {
            app_id_pattern: Some("org.gnome.Nautilus".into()),
            title_contains: None,
            target_workspace_index: Some(0),
            position: None,
            size: None,
            start_maximized: true,
            start_minimized: false,
        });
        let result = engine.evaluate("org.gnome.Nautilus", "Files").unwrap();
        assert!(result.start_maximized);
        assert!(!result.start_minimized);
    }
}
