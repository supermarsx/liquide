//! Frontend runtime coordinator.
//!
//! Ties together configuration, authentication, navigation, page registry,
//! and theming into a single application shell.

use crate::auth::{AuthManager, AuthRole};
use crate::config::FrontendConfig;
use crate::nav::{NavSection, NavState};
use crate::page::{PageKind, PageRegistry};
use crate::theme::{Theme, ThemePreset};

/// Central coordinator for the management frontend application.
#[derive(Debug, Clone)]
pub struct FrontendRuntime {
    config: FrontendConfig,
    auth: AuthManager,
    nav: NavState,
    pages: PageRegistry,
    theme: Theme,
}

impl FrontendRuntime {
    /// Create a new runtime with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(FrontendConfig::default())
    }

    /// Create a new runtime with the given configuration.
    #[must_use]
    pub fn with_config(config: FrontendConfig) -> Self {
        let theme = ThemePreset::LiquidGlass.to_theme();
        Self {
            config,
            auth: AuthManager::new(),
            nav: NavState::new(),
            pages: PageRegistry::new(),
            theme,
        }
    }

    // -- Accessors --

    /// Get the current configuration.
    #[must_use]
    pub fn config(&self) -> &FrontendConfig {
        &self.config
    }

    /// Get the authentication manager.
    #[must_use]
    pub fn auth(&self) -> &AuthManager {
        &self.auth
    }

    /// Get a mutable reference to the authentication manager.
    pub fn auth_mut(&mut self) -> &mut AuthManager {
        &mut self.auth
    }

    /// Get the navigation state.
    #[must_use]
    pub fn nav(&self) -> &NavState {
        &self.nav
    }

    /// Get a mutable reference to the navigation state.
    pub fn nav_mut(&mut self) -> &mut NavState {
        &mut self.nav
    }

    /// Get the page registry.
    #[must_use]
    pub fn pages(&self) -> &PageRegistry {
        &self.pages
    }

    /// Get a mutable reference to the page registry.
    pub fn pages_mut(&mut self) -> &mut PageRegistry {
        &mut self.pages
    }

    /// Get the current theme.
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Total number of registered pages.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.pages.count()
    }

    // -- Navigation helpers --

    /// Navigate to a section, updating both nav state and page registry.
    pub fn navigate_to(&mut self, section: NavSection) {
        self.nav.navigate(section);
        let kind = PageKind::from_section(section);
        if let Some(page) = self.pages.get_mut(kind) {
            page.mark_refreshing();
        }
    }

    /// Get the current page kind based on navigation state.
    #[must_use]
    pub fn current_page(&self) -> PageKind {
        PageKind::from_section(self.nav.current())
    }

    // -- Authorization helpers --

    /// Whether the current session's role has at least the given permission.
    #[must_use]
    pub fn is_authorized(&self, required: AuthRole) -> bool {
        match self.auth.current_session() {
            Some(session) => session.role.has_permission(required),
            None => false,
        }
    }

    // -- Theme helpers --

    /// Switch the active theme to a preset.
    pub fn set_theme(&mut self, preset: ThemePreset) {
        self.theme = preset.to_theme();
    }

    // -- Refresh --

    /// Trigger a refresh on the current page.
    pub fn refresh_current_page(&mut self) {
        let kind = self.current_page();
        if let Some(page) = self.pages.get_mut(kind) {
            page.mark_refreshing();
        }
    }
}

impl Default for FrontendRuntime {
    fn default() -> Self {
        Self::new()
    }
}
