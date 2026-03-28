//! Window matching rules engine.
//!
//! Provides glob-based pattern matching for window properties and a rule engine
//! that evaluates rules in order, collecting all matching actions.

/// Position within a tiling layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TilePosition {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

/// Window type hint (mirrors freedesktop `_NET_WM_WINDOW_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowType {
    Normal,
    Dialog,
    Splash,
    Utility,
    Toolbar,
    Menu,
    DropdownMenu,
    PopupMenu,
    Tooltip,
    Notification,
    Dock,
    Desktop,
}

/// An action to apply when a rule matches.
#[derive(Debug, Clone, PartialEq)]
pub enum RuleAction {
    /// Move the window to the specified workspace.
    MoveToWorkspace(u32),
    /// Set the window geometry (position and size).
    SetGeometry { x: i32, y: i32, w: u32, h: u32 },
    /// Maximize the window.
    Maximize,
    /// Minimize the window.
    Minimize,
    /// Set the window opacity (0.0 = transparent, 1.0 = opaque).
    SetOpacity(f32),
    /// Keep the window above all others.
    AlwaysOnTop,
    /// Remove window border/frame.
    NoBorder,
    /// Remove the title bar.
    NoTitleBar,
    /// Pin the window to all workspaces.
    PinToAllWorkspaces,
    /// Pin the window to a specific workspace (prevent moving).
    PinToWorkspace(u32),
    /// Skip this window in the taskbar.
    SkipTaskbar,
    /// Place the window below other windows.
    Below,
    /// Make the window fullscreen.
    Fullscreen,
    /// Tile the window at the given position.
    Tile(TilePosition),
    /// Set the initial size.
    SetSize { w: u32, h: u32 },
    /// Center the window on screen.
    Center,
    /// Assign the window to a specific group by name.
    AssignGroup(String),
}

/// Glob-based pattern matcher for window properties.
#[derive(Debug, Clone)]
pub struct WindowMatcher {
    /// Glob pattern for the application ID (e.g., "org.mozilla.*", "firefox").
    /// `None` means match any app_id.
    pub app_id_pattern: Option<String>,
    /// Glob pattern for the window title (e.g., "*- Mozilla Firefox").
    /// `None` means match any title.
    pub title_pattern: Option<String>,
    /// Match a specific window type. `None` means match any type.
    pub window_type: Option<WindowType>,
    /// If true, all specified criteria must match (AND logic).
    /// If false, any specified criterion matching is sufficient (OR logic).
    pub match_all: bool,
}

impl WindowMatcher {
    /// Create a matcher that matches any window.
    pub fn any() -> Self {
        Self {
            app_id_pattern: None,
            title_pattern: None,
            window_type: None,
            match_all: true,
        }
    }

    /// Create a matcher for a specific app ID pattern.
    pub fn app_id(pattern: impl Into<String>) -> Self {
        Self {
            app_id_pattern: Some(pattern.into()),
            title_pattern: None,
            window_type: None,
            match_all: true,
        }
    }

    /// Create a matcher for a specific title pattern.
    pub fn title(pattern: impl Into<String>) -> Self {
        Self {
            app_id_pattern: None,
            title_pattern: Some(pattern.into()),
            window_type: None,
            match_all: true,
        }
    }

    /// Create a matcher for a specific window type.
    pub fn window_type(wt: WindowType) -> Self {
        Self {
            app_id_pattern: None,
            title_pattern: None,
            window_type: Some(wt),
            match_all: true,
        }
    }

    /// Builder: set app_id pattern.
    pub fn with_app_id(mut self, pattern: impl Into<String>) -> Self {
        self.app_id_pattern = Some(pattern.into());
        self
    }

    /// Builder: set title pattern.
    pub fn with_title(mut self, pattern: impl Into<String>) -> Self {
        self.title_pattern = Some(pattern.into());
        self
    }

    /// Builder: set window type.
    pub fn with_window_type(mut self, wt: WindowType) -> Self {
        self.window_type = Some(wt);
        self
    }

    /// Builder: set match mode to OR (any criterion matches).
    pub fn match_any(mut self) -> Self {
        self.match_all = false;
        self
    }

    /// Check if this matcher matches the given window info.
    pub fn matches(&self, info: &WindowInfo) -> bool {
        let app_match = match &self.app_id_pattern {
            Some(pattern) => match &info.app_id {
                Some(app_id) => glob_match(pattern, app_id),
                None => false,
            },
            None => true,
        };

        let title_match = match &self.title_pattern {
            Some(pattern) => glob_match(pattern, &info.title),
            None => true,
        };

        let type_match = match self.window_type {
            Some(wt) => info.window_type == wt,
            None => true,
        };

        // Count how many criteria are specified.
        let has_app = self.app_id_pattern.is_some();
        let has_title = self.title_pattern.is_some();
        let has_type = self.window_type.is_some();

        // If no criteria are specified, match everything.
        if !has_app && !has_title && !has_type {
            return true;
        }

        if self.match_all {
            // AND: all specified criteria must match.
            (!has_app || app_match) && (!has_title || title_match) && (!has_type || type_match)
        } else {
            // OR: at least one specified criterion must match.
            (has_app && app_match) || (has_title && title_match) || (has_type && type_match)
        }
    }
}

/// Information about a window, used for rule matching.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// The window's unique ID.
    pub window_id: u64,
    /// Application identifier (e.g., "org.mozilla.firefox").
    pub app_id: Option<String>,
    /// Window title.
    pub title: String,
    /// Window type.
    pub window_type: WindowType,
    /// Requested width.
    pub width: u32,
    /// Requested height.
    pub height: u32,
}

impl WindowInfo {
    /// Create a new WindowInfo with the given parameters.
    pub fn new(
        window_id: u64,
        app_id: Option<String>,
        title: impl Into<String>,
        window_type: WindowType,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            window_id,
            app_id,
            title: title.into(),
            window_type,
            width,
            height,
        }
    }
}

/// A single rule: a matcher paired with actions to apply.
#[derive(Debug, Clone)]
pub struct WindowRule {
    /// Human-readable description of this rule.
    pub description: String,
    /// The matcher that determines if this rule applies.
    pub matcher: WindowMatcher,
    /// Actions to take when the rule matches.
    pub actions: Vec<RuleAction>,
    /// Whether this rule is currently enabled.
    pub enabled: bool,
    /// If true, stop evaluating further rules after this one matches.
    pub stop_processing: bool,
}

impl WindowRule {
    /// Create a new enabled rule with the given matcher and actions.
    pub fn new(
        description: impl Into<String>,
        matcher: WindowMatcher,
        actions: Vec<RuleAction>,
    ) -> Self {
        Self {
            description: description.into(),
            matcher,
            actions,
            enabled: true,
            stop_processing: false,
        }
    }

    /// Builder: mark this rule as "stop processing" (no further rules checked).
    pub fn stop_after(mut self) -> Self {
        self.stop_processing = true;
        self
    }

    /// Builder: disable this rule.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Rule engine that evaluates an ordered list of rules against window info.
///
/// Rules are evaluated in order. All matching rules contribute their actions
/// unless a rule has `stop_processing = true`, which halts further evaluation.
#[derive(Debug, Clone)]
pub struct RuleEngine {
    /// Ordered list of rules.
    rules: Vec<WindowRule>,
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleEngine {
    /// Create an empty rule engine.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a rule at the end of the list.
    pub fn add_rule(&mut self, rule: WindowRule) {
        self.rules.push(rule);
    }

    /// Insert a rule at the specified index.
    pub fn insert_rule(&mut self, index: usize, rule: WindowRule) {
        let idx = index.min(self.rules.len());
        self.rules.insert(idx, rule);
    }

    /// Remove the rule at the specified index. Returns it if valid.
    pub fn remove_rule(&mut self, index: usize) -> Option<WindowRule> {
        if index < self.rules.len() {
            Some(self.rules.remove(index))
        } else {
            None
        }
    }

    /// Move a rule from one position to another.
    pub fn reorder_rule(&mut self, from: usize, to: usize) -> bool {
        if from >= self.rules.len() || to >= self.rules.len() {
            return false;
        }
        if from == to {
            return true;
        }
        let rule = self.rules.remove(from);
        self.rules.insert(to, rule);
        true
    }

    /// Returns the number of rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Get a reference to a rule by index.
    pub fn get_rule(&self, index: usize) -> Option<&WindowRule> {
        self.rules.get(index)
    }

    /// Get a mutable reference to a rule by index.
    pub fn get_rule_mut(&mut self, index: usize) -> Option<&mut WindowRule> {
        self.rules.get_mut(index)
    }

    /// Evaluate all rules against the given window info.
    /// Returns the collected actions from all matching rules (in rule order).
    pub fn evaluate(&self, info: &WindowInfo) -> Vec<RuleAction> {
        let mut actions = Vec::new();
        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }
            if rule.matcher.matches(info) {
                actions.extend(rule.actions.iter().cloned());
                if rule.stop_processing {
                    break;
                }
            }
        }
        actions
    }

    /// Returns an iterator over all rules.
    pub fn rules(&self) -> impl Iterator<Item = &WindowRule> {
        self.rules.iter()
    }

    /// Clear all rules.
    pub fn clear(&mut self) {
        self.rules.clear();
    }
}

/// Simple glob matching supporting `*` (match any sequence) and `?` (match single char).
///
/// Case-insensitive matching is used.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.to_lowercase();
    let text = text.to_lowercase();
    glob_match_impl(pattern.as_bytes(), text.as_bytes())
}

fn glob_match_impl(pattern: &[u8], text: &[u8]) -> bool {
    let mut pi = 0;
    let mut ti = 0;
    let mut star_pi = usize::MAX;
    let mut star_ti = 0;

    while ti < text.len() {
        if pi < pattern.len() && (pattern[pi] == b'?' || pattern[pi] == text[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }

    pi == pattern.len()
}
