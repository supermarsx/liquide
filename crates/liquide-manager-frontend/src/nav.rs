//! Navigation model — sections, items, breadcrumbs, and history.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::auth::AuthRole;

/// A navigable section of the management UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NavSection {
    /// Real-time overview dashboard.
    Dashboard,
    /// Managed server list.
    Servers,
    /// Remote-desktop session list.
    Sessions,
    /// Admin user list.
    Users,
    /// Policy editor.
    Policies,
    /// Gateway list.
    Gateways,
    /// System metrics / charts.
    Metrics,
    /// Audit log viewer.
    Audit,
    /// Plugin management.
    Plugins,
}

impl NavSection {
    /// All sections in sidebar display order.
    pub const ALL: &'static [NavSection] = &[
        NavSection::Dashboard,
        NavSection::Servers,
        NavSection::Sessions,
        NavSection::Users,
        NavSection::Policies,
        NavSection::Gateways,
        NavSection::Metrics,
        NavSection::Audit,
        NavSection::Plugins,
    ];

    /// Human-readable label for the section.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Servers => "Servers",
            Self::Sessions => "Sessions",
            Self::Users => "Users",
            Self::Policies => "Policies",
            Self::Gateways => "Gateways",
            Self::Metrics => "Metrics",
            Self::Audit => "Audit Log",
            Self::Plugins => "Plugins",
        }
    }

    /// Icon identifier for the section (compatible with icon font names).
    #[must_use]
    pub fn icon(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Servers => "dns",
            Self::Sessions => "desktop_windows",
            Self::Users => "people",
            Self::Policies => "policy",
            Self::Gateways => "router",
            Self::Metrics => "bar_chart",
            Self::Audit => "history",
            Self::Plugins => "extension",
        }
    }

    /// Minimum role required to view this section.
    #[must_use]
    pub fn min_role(self) -> AuthRole {
        match self {
            Self::Dashboard | Self::Servers | Self::Sessions | Self::Metrics | Self::Gateways => {
                AuthRole::Viewer
            }
            Self::Users | Self::Audit => AuthRole::Operator,
            Self::Policies | Self::Plugins => AuthRole::Admin,
        }
    }
}

impl fmt::Display for NavSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// A single navigation item rendered in the sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavItem {
    /// Unique identifier.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Icon identifier.
    pub icon: String,
    /// Optional badge text (e.g. alert count).
    pub badge: Option<String>,
    /// Minimum role to display this item.
    pub requires_role: AuthRole,
}

impl NavItem {
    /// Build a nav item from a section.
    #[must_use]
    pub fn from_section(section: NavSection) -> Self {
        Self {
            id: format!("{:?}", section).to_lowercase(),
            label: section.label().to_string(),
            icon: section.icon().to_string(),
            badge: None,
            requires_role: section.min_role(),
        }
    }

    /// Set a badge value.
    #[must_use]
    pub fn with_badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }
}

/// Navigation state tracking current location, history, and breadcrumbs.
#[derive(Debug, Clone)]
pub struct NavState {
    /// Currently active section.
    current: NavSection,
    /// Navigation history (most recent last).
    history: Vec<NavSection>,
    /// Maximum history depth.
    max_history: usize,
}

impl NavState {
    /// Create a new navigation state starting at the dashboard.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: NavSection::Dashboard,
            history: Vec::new(),
            max_history: 50,
        }
    }

    /// Currently active section.
    #[must_use]
    pub fn current(&self) -> NavSection {
        self.current
    }

    /// Navigate to a new section.
    pub fn navigate(&mut self, section: NavSection) {
        if section != self.current {
            self.history.push(self.current);
            if self.history.len() > self.max_history {
                self.history.remove(0);
            }
            self.current = section;
        }
    }

    /// Go back to the previous section. Returns the section navigated to,
    /// or `None` if history is empty.
    pub fn go_back(&mut self) -> Option<NavSection> {
        if let Some(prev) = self.history.pop() {
            self.current = prev;
            Some(prev)
        } else {
            None
        }
    }

    /// Build breadcrumb trail from history root to current section.
    #[must_use]
    pub fn breadcrumbs(&self) -> Vec<NavSection> {
        let mut crumbs = Vec::new();
        for section in &self.history {
            if crumbs.last() != Some(section) {
                crumbs.push(*section);
            }
        }
        if crumbs.last() != Some(&self.current) {
            crumbs.push(self.current);
        }
        crumbs
    }

    /// Navigation history length.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Build the sidebar nav items, filtered by the given role.
    #[must_use]
    pub fn sidebar_items(&self, role: AuthRole) -> Vec<NavItem> {
        NavSection::ALL
            .iter()
            .filter(|s| role.has_permission(s.min_role()))
            .map(|s| NavItem::from_section(*s))
            .collect()
    }
}

impl Default for NavState {
    fn default() -> Self {
        Self::new()
    }
}
