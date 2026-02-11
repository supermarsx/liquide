//! Software center configuration.

use serde::{Deserialize, Serialize};

/// Top-level configuration for the software center.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareCenterConfig {
    /// Whether to check for updates automatically.
    pub auto_check_updates: bool,
    /// Update check interval in hours.
    pub check_interval_hours: u32,
    /// Whether to show proprietary applications.
    pub show_proprietary: bool,
    /// Whether to show ratings and reviews.
    pub show_reviews: bool,
    /// Maximum concurrent downloads.
    pub max_concurrent_downloads: usize,
    /// Download cache directory.
    pub cache_dir: String,
}

impl Default for SoftwareCenterConfig {
    fn default() -> Self {
        Self {
            auto_check_updates: true,
            check_interval_hours: 24,
            show_proprietary: true,
            show_reviews: true,
            max_concurrent_downloads: 3,
            cache_dir: "/var/cache/liquide/software-center".into(),
        }
    }
}
