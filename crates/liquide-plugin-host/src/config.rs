//! Plugin host configuration.

use liquide_plugin_abi::ExtensionPoint;
use serde::{Deserialize, Serialize};

/// Configuration for the plugin host subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHostConfig {
    /// Maximum number of concurrently loaded plugins.
    pub max_plugins: usize,
    /// Maximum memory (in bytes) a single plugin may request.
    pub max_memory_per_plugin: u64,
    /// Default timeout in milliseconds for plugin operations.
    pub default_timeout_ms: u64,
    /// Optional allowlist of extension points.  When `Some`, only these
    /// extension points may be registered.  When `None`, all are permitted.
    pub allowed_extension_points: Option<Vec<ExtensionPoint>>,
    /// Total resource pool capacity in bytes shared across all plugins.
    pub resource_pool_capacity: u64,
}

impl Default for PluginHostConfig {
    fn default() -> Self {
        Self {
            max_plugins: 64,
            max_memory_per_plugin: 64 * 1024 * 1024, // 64 MiB
            default_timeout_ms: 5000,
            allowed_extension_points: None,
            resource_pool_capacity: 512 * 1024 * 1024, // 512 MiB
        }
    }
}

impl PluginHostConfig {
    /// Create a new configuration with the given plugin limit and memory cap.
    #[must_use]
    pub fn new(max_plugins: usize, max_memory_per_plugin: u64) -> Self {
        Self {
            max_plugins,
            max_memory_per_plugin,
            ..Self::default()
        }
    }

    /// Check whether an extension point is allowed by the current config.
    #[must_use]
    pub fn is_extension_point_allowed(&self, point: ExtensionPoint) -> bool {
        match &self.allowed_extension_points {
            None => true,
            Some(allowed) => allowed.contains(&point),
        }
    }
}

impl std::fmt::Display for PluginHostConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PluginHostConfig(max_plugins={}, max_mem={}B, timeout={}ms, pool={}B)",
            self.max_plugins,
            self.max_memory_per_plugin,
            self.default_timeout_ms,
            self.resource_pool_capacity,
        )
    }
}
