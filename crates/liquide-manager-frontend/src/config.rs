//! Frontend configuration types.

use serde::{Deserialize, Serialize};

/// Frontend application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendConfig {
    /// Base URL in the manager REST API (e.g. `https://mgr:8443/api/v1`).
    pub api_base_url: String,
    /// Active theme name.
    pub theme: String,
    /// Locale / language tag (e.g. `en-US`).
    pub locale: String,
    /// Automatic refresh interval in seconds.
    pub auto_refresh_sec: u32,
    /// Default page size for list views.
    pub items_per_page: u32,
    /// Whether the sidebar starts collapsed.
    pub sidebar_collapsed: bool,
}

impl Default for FrontendConfig {
    fn default() -> Self {
        Self {
            api_base_url: "https://localhost:8443/api/v1".to_string(),
            theme: "liquid-glass".to_string(),
            locale: "en-US".to_string(),
            auto_refresh_sec: 5,
            items_per_page: 25,
            sidebar_collapsed: false,
        }
    }
}

impl FrontendConfig {
    /// Create a new configuration with the given API base URL.
    #[must_use]
    pub fn new(api_base_url: impl Into<String>) -> Self {
        Self {
            api_base_url: api_base_url.into(),
            ..Self::default()
        }
    }

    /// Set the theme.
    #[must_use]
    pub fn with_theme(mut self, theme: impl Into<String>) -> Self {
        self.theme = theme.into();
        self
    }

    /// Set the locale.
    #[must_use]
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = locale.into();
        self
    }

    /// Set the auto-refresh interval.
    #[must_use]
    pub fn with_auto_refresh_sec(mut self, seconds: u32) -> Self {
        self.auto_refresh_sec = seconds;
        self
    }

    /// Set the items-per-page default.
    #[must_use]
    pub fn with_items_per_page(mut self, count: u32) -> Self {
        self.items_per_page = count;
        self
    }

    /// Set sidebar collapsed state.
    #[must_use]
    pub fn with_sidebar_collapsed(mut self, collapsed: bool) -> Self {
        self.sidebar_collapsed = collapsed;
        self
    }
}
