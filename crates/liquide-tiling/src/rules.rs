//! Per-window tiling rules: match windows by class/app_id and apply actions.

/// Action to apply when a window matches a rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TileAction {
    /// Normal tiling.
    Tile,
    /// Always float (never tile).
    Float,
    /// Fullscreen on its workspace.
    Fullscreen,
    /// Move to a specific workspace.
    Workspace(u32),
    /// Place in master position.
    Master,
}

/// A rule that matches windows by class and/or app_id and applies an action.
#[derive(Debug, Clone)]
pub struct TileRule {
    /// Substring match against the window class (e.g. "dialog", "splash").
    pub window_class: Option<String>,
    /// Substring match against the application identifier.
    pub app_id: Option<String>,
    /// Action to apply when matched.
    pub action: TileAction,
}

impl TileRule {
    /// Create a rule matching by window class.
    #[must_use]
    pub fn by_class(class: impl Into<String>, action: TileAction) -> Self {
        Self {
            window_class: Some(class.into()),
            app_id: None,
            action,
        }
    }

    /// Create a rule matching by app_id.
    #[must_use]
    pub fn by_app_id(app_id: impl Into<String>, action: TileAction) -> Self {
        Self {
            window_class: None,
            app_id: Some(app_id.into()),
            action,
        }
    }

    /// Check if this rule matches the given window class and app_id.
    #[must_use]
    pub fn matches(&self, window_class: Option<&str>, app_id: Option<&str>) -> bool {
        let class_ok = match &self.window_class {
            Some(pattern) => window_class.is_some_and(|c| {
                c.to_lowercase().contains(&pattern.to_lowercase())
            }),
            None => true,
        };

        let app_ok = match &self.app_id {
            Some(pattern) => app_id.is_some_and(|a| {
                a.to_lowercase().contains(&pattern.to_lowercase())
            }),
            None => true,
        };

        class_ok && app_ok
    }
}

/// Engine that evaluates tiling rules for incoming windows.
pub struct RuleEngine {
    rules: Vec<TileRule>,
}

impl RuleEngine {
    /// Create a new rule engine with default rules.
    ///
    /// Defaults: dialogs float, splash screens float, tooltips float.
    #[must_use]
    pub fn new() -> Self {
        let rules = vec![
            TileRule::by_class("dialog", TileAction::Float),
            TileRule::by_class("splash", TileAction::Float),
            TileRule::by_class("tooltip", TileAction::Float),
            TileRule::by_class("popup", TileAction::Float),
            TileRule::by_class("notification", TileAction::Float),
            TileRule::by_class("menu", TileAction::Float),
        ];
        Self { rules }
    }

    /// Create an empty rule engine (no defaults).
    #[must_use]
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a rule.
    pub fn add_rule(&mut self, rule: TileRule) {
        self.rules.push(rule);
    }

    /// Insert a rule at the front (highest priority).
    pub fn add_priority_rule(&mut self, rule: TileRule) {
        self.rules.insert(0, rule);
    }

    /// Remove all rules matching the given app_id pattern.
    pub fn remove_rules_for_app(&mut self, app_id: &str) {
        self.rules.retain(|r| {
            r.app_id.as_deref() != Some(app_id)
        });
    }

    /// Evaluate rules for a window. Returns the first matching action,
    /// or `TileAction::Tile` if no rule matches.
    #[must_use]
    pub fn evaluate(&self, window_class: Option<&str>, app_id: Option<&str>) -> TileAction {
        for rule in &self.rules {
            if rule.matches(window_class, app_id) {
                return rule.action.clone();
            }
        }
        TileAction::Tile
    }

    /// Number of rules.
    #[must_use]
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}
