//! Page model — page kinds, lifecycle states, and a page registry.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::nav::NavSection;

/// The kind of page being displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PageKind {
    /// Login / authentication page.
    Login,
    /// Real-time overview dashboard.
    Dashboard,
    /// Managed server list / detail.
    Servers,
    /// Session list / detail.
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
    /// Path not found.
    NotFound,
    /// Unrecoverable error.
    Error,
}

impl PageKind {
    /// Convert from a navigation section.
    #[must_use]
    pub fn from_section(section: NavSection) -> Self {
        match section {
            NavSection::Dashboard => Self::Dashboard,
            NavSection::Servers => Self::Servers,
            NavSection::Sessions => Self::Sessions,
            NavSection::Users => Self::Users,
            NavSection::Policies => Self::Policies,
            NavSection::Gateways => Self::Gateways,
            NavSection::Metrics => Self::Metrics,
            NavSection::Audit => Self::Audit,
            NavSection::Plugins => Self::Plugins,
        }
    }
}

impl fmt::Display for PageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login => write!(f, "login"),
            Self::Dashboard => write!(f, "dashboard"),
            Self::Servers => write!(f, "servers"),
            Self::Sessions => write!(f, "sessions"),
            Self::Users => write!(f, "users"),
            Self::Policies => write!(f, "policies"),
            Self::Gateways => write!(f, "gateways"),
            Self::Metrics => write!(f, "metrics"),
            Self::Audit => write!(f, "audit"),
            Self::Plugins => write!(f, "plugins"),
            Self::NotFound => write!(f, "not-found"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Lifecycle state of a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageState {
    /// Initial data is being loaded.
    Loading,
    /// Page data is ready for display.
    Ready,
    /// An error occurred while loading or refreshing.
    Error { message: String },
    /// The page is auto-refreshing in the background.
    Refreshing,
}

impl Default for PageState {
    fn default() -> Self {
        Self::Loading
    }
}

impl fmt::Display for PageState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Loading => write!(f, "loading"),
            Self::Ready => write!(f, "ready"),
            Self::Error { message } => write!(f, "error: {message}"),
            Self::Refreshing => write!(f, "refreshing"),
        }
    }
}

/// A page in the management UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    /// What kind of page this is.
    pub kind: PageKind,
    /// Display title.
    pub title: String,
    /// Current lifecycle state.
    pub state: PageState,
    /// Epoch-seconds of the last successful data refresh.
    pub last_refreshed: Option<u64>,
}

impl Page {
    /// Create a new page.
    #[must_use]
    pub fn new(kind: PageKind, title: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            state: PageState::Loading,
            last_refreshed: None,
        }
    }

    /// Transition to the ready state and record a refresh timestamp.
    pub fn mark_ready(&mut self, now: u64) {
        self.state = PageState::Ready;
        self.last_refreshed = Some(now);
    }

    /// Transition to the error state.
    pub fn mark_error(&mut self, message: impl Into<String>) {
        self.state = PageState::Error {
            message: message.into(),
        };
    }

    /// Transition to the refreshing state.
    pub fn mark_refreshing(&mut self) {
        self.state = PageState::Refreshing;
    }

    /// Whether the page is in a ready state.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.state == PageState::Ready
    }
}

/// Registry of all pages known to the application.
#[derive(Debug, Clone)]
pub struct PageRegistry {
    pages: Vec<Page>,
}

impl PageRegistry {
    /// Create a new registry pre-populated with the default pages.
    #[must_use]
    pub fn new() -> Self {
        let pages = vec![
            Page::new(PageKind::Login, "Login"),
            Page::new(PageKind::Dashboard, "Dashboard"),
            Page::new(PageKind::Servers, "Servers"),
            Page::new(PageKind::Sessions, "Sessions"),
            Page::new(PageKind::Users, "Users"),
            Page::new(PageKind::Policies, "Policies"),
            Page::new(PageKind::Gateways, "Gateways"),
            Page::new(PageKind::Metrics, "Metrics"),
            Page::new(PageKind::Audit, "Audit Log"),
            Page::new(PageKind::Plugins, "Plugins"),
            Page::new(PageKind::NotFound, "Not Found"),
            Page::new(PageKind::Error, "Error"),
        ];
        Self { pages }
    }

    /// Look up a page by kind.
    #[must_use]
    pub fn get(&self, kind: PageKind) -> Option<&Page> {
        self.pages.iter().find(|p| p.kind == kind)
    }

    /// Look up a page by kind (mutable).
    pub fn get_mut(&mut self, kind: PageKind) -> Option<&mut Page> {
        self.pages.iter_mut().find(|p| p.kind == kind)
    }

    /// Total number of registered pages.
    #[must_use]
    pub fn count(&self) -> usize {
        self.pages.len()
    }

    /// All registered pages.
    #[must_use]
    pub fn all(&self) -> &[Page] {
        &self.pages
    }
}

impl Default for PageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
