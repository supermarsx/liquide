//! Extension-point dispatcher for routing calls to registered plugins.

use std::collections::HashMap;
use std::fmt;

use liquide_plugin_abi::ExtensionPoint;
use liquide_plugin_abi::types::PluginResult;

use crate::config::PluginHostConfig;
use crate::plugin::PluginId;
use crate::{ExtensionPointDisplay, PluginHostError, Result};

/// Result of dispatching a call to a single plugin.
#[derive(Debug, Clone)]
pub struct DispatchResult {
    /// The plugin that handled the call.
    pub plugin_id: PluginId,
    /// The result code returned by the plugin.
    pub result: PluginResult,
}

impl fmt::Display for DispatchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={:?}", self.plugin_id, self.result)
    }
}

/// Dispatcher routes extension-point calls to the plugins that registered for them.
pub struct Dispatcher {
    hooks: HashMap<ExtensionPoint, Vec<PluginId>>,
}

impl Dispatcher {
    /// Create an empty dispatcher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    /// Register a plugin for the given extension point.
    ///
    /// If `config` is provided, the extension point is checked against the
    /// allowlist before registration.
    ///
    /// # Errors
    ///
    /// Returns [`PluginHostError::ExtensionPointNotAllowed`] if the extension
    /// point is blocked by the config.
    pub fn register(
        &mut self,
        point: ExtensionPoint,
        plugin_id: PluginId,
        config: Option<&PluginHostConfig>,
    ) -> Result<()> {
        if let Some(cfg) = config {
            if !cfg.is_extension_point_allowed(point) {
                return Err(PluginHostError::ExtensionPointNotAllowed {
                    point: ExtensionPointDisplay(point),
                });
            }
        }

        let entry = self.hooks.entry(point).or_default();
        if !entry.contains(&plugin_id) {
            entry.push(plugin_id);
            tracing::debug!(
                plugin_id = plugin_id.0,
                point = ?point,
                "registered for extension point"
            );
        }
        Ok(())
    }

    /// Unregister a plugin from a specific extension point.
    ///
    /// Returns `true` if the plugin was previously registered.
    pub fn unregister(&mut self, point: ExtensionPoint, plugin_id: PluginId) -> bool {
        if let Some(plugins) = self.hooks.get_mut(&point) {
            let before = plugins.len();
            plugins.retain(|id| *id != plugin_id);
            let removed = plugins.len() < before;
            if removed {
                tracing::debug!(
                    plugin_id = plugin_id.0,
                    point = ?point,
                    "unregistered from extension point"
                );
            }
            removed
        } else {
            false
        }
    }

    /// Unregister a plugin from all extension points.
    ///
    /// Returns the number of extension points from which the plugin was removed.
    pub fn unregister_all(&mut self, plugin_id: PluginId) -> usize {
        let mut removed = 0;
        for plugins in self.hooks.values_mut() {
            let before = plugins.len();
            plugins.retain(|id| *id != plugin_id);
            if plugins.len() < before {
                removed += 1;
            }
        }
        if removed > 0 {
            tracing::debug!(
                plugin_id = plugin_id.0,
                removed,
                "unregistered from all extension points"
            );
        }
        removed
    }

    /// Get the list of plugin IDs registered for an extension point.
    #[must_use]
    pub fn plugins_for(&self, point: ExtensionPoint) -> &[PluginId] {
        self.hooks.get(&point).map_or(&[], Vec::as_slice)
    }

    /// Check whether any plugins are registered for the given extension point.
    #[must_use]
    pub fn has_handlers(&self, point: ExtensionPoint) -> bool {
        self.hooks.get(&point).is_some_and(|p| !p.is_empty())
    }

    /// Simulate dispatching a call to all plugins registered for an extension
    /// point.  In a real implementation this would invoke the WASM export;
    /// here it returns [`PluginResult::Ok`] for each registered plugin.
    #[must_use]
    pub fn dispatch(&self, point: ExtensionPoint) -> Vec<DispatchResult> {
        let plugins = self.plugins_for(point);
        plugins
            .iter()
            .map(|&plugin_id| DispatchResult {
                plugin_id,
                result: PluginResult::Ok,
            })
            .collect()
    }

    /// Total number of extension-point registrations across all hooks.
    #[must_use]
    pub fn total_registrations(&self) -> usize {
        self.hooks.values().map(Vec::len).sum()
    }

    /// Number of distinct extension points that have at least one plugin
    /// registered.
    #[must_use]
    pub fn active_extension_point_count(&self) -> usize {
        self.hooks.values().filter(|v| !v.is_empty()).count()
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Dispatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Dispatcher({} extension points, {} registrations)",
            self.active_extension_point_count(),
            self.total_registrations(),
        )
    }
}
