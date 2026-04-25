//! Top-level plugin host — orchestrates runtime, dispatcher, and host functions.

use std::fmt;

use liquide_plugin_abi::host_functions::{HOST_FUNCTIONS, HostFunction};
use liquide_plugin_abi::types::{PluginResult, ResourceHandle};
use liquide_plugin_abi::{ExtensionPoint, PluginManifest};

use crate::config::PluginHostConfig;
use crate::dispatcher::{DispatchResult, Dispatcher};
use crate::plugin::{LoadedPlugin, PluginId};
use crate::resources::ResourcePool;
use crate::runtime::PluginRuntime;
use crate::{PluginHostError, Result};

/// The top-level plugin host managing the full lifecycle.
pub struct PluginHost {
    runtime: PluginRuntime,
    dispatcher: Dispatcher,
    config: PluginHostConfig,
}

impl PluginHost {
    /// Create a new plugin host with the given configuration.
    #[must_use]
    pub fn new(config: PluginHostConfig) -> Self {
        let runtime = PluginRuntime::new(config.clone());
        Self {
            runtime,
            dispatcher: Dispatcher::new(),
            config,
        }
    }

    /// Create a new plugin host with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(PluginHostConfig::default())
    }

    /// Load a plugin from its manifest, register its extension points,
    /// and activate it.
    ///
    /// # Errors
    ///
    /// Propagates errors from the runtime or dispatcher.
    pub fn load_plugin(&mut self, manifest: PluginManifest) -> Result<PluginId> {
        let extension_points: Vec<ExtensionPoint> = manifest.extension_points.clone();

        let id = self.runtime.load_plugin(manifest)?;

        // Register all declared extension points.
        for point in extension_points {
            if let Err(e) = self.dispatcher.register(point, id, Some(&self.config)) {
                // Roll back: unload the plugin if registration fails.
                tracing::warn!(
                    plugin_id = id.0,
                    error = %e,
                    "extension point registration failed, unloading plugin"
                );
                let _ = self.runtime.unload_plugin(id);
                self.dispatcher.unregister_all(id);
                return Err(e);
            }
        }

        tracing::info!(plugin_id = id.0, "plugin fully loaded and registered");

        Ok(id)
    }

    /// Load a plugin from raw JSON manifest bytes.
    ///
    /// # Errors
    ///
    /// Returns [`PluginHostError::ManifestParse`] if parsing fails.
    pub fn load_plugin_from_json(&mut self, json: &[u8]) -> Result<PluginId> {
        let manifest = liquide_plugin_abi::plugin_manifest::parse_manifest(json)
            .map_err(PluginHostError::ManifestParse)?;
        self.load_plugin(manifest)
    }

    /// Unload a plugin, removing all its registrations and freeing resources.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin is not found or already unloaded.
    pub fn unload_plugin(&mut self, id: PluginId) -> Result<()> {
        self.dispatcher.unregister_all(id);
        self.runtime.unload_plugin(id)?;

        tracing::info!(plugin_id = id.0, "plugin fully unloaded");

        Ok(())
    }

    /// Suspend a plugin (it remains registered but will not receive dispatches
    /// until resumed).
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin is not found or not active.
    pub fn suspend_plugin(&mut self, id: PluginId) -> Result<()> {
        self.runtime.suspend_plugin(id)
    }

    /// Resume a previously suspended plugin.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin is not found or not suspended.
    pub fn resume_plugin(&mut self, id: PluginId) -> Result<()> {
        self.runtime.resume_plugin(id)
    }

    /// Dispatch an extension-point call to all registered and active plugins.
    ///
    /// Only plugins in the [`PluginState::Active`] state will be invoked.
    #[must_use]
    pub fn dispatch(&self, point: ExtensionPoint) -> Vec<DispatchResult> {
        let all = self.dispatcher.dispatch(point);
        all.into_iter()
            .filter(|r| {
                self.runtime
                    .plugin(r.plugin_id)
                    .is_some_and(|p| p.is_active())
            })
            .collect()
    }

    /// Allocate a resource for a plugin.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin is not found, not active, or resources
    /// are exhausted.
    pub fn allocate_resource(&mut self, id: PluginId, size: u64) -> Result<ResourceHandle> {
        self.runtime.allocate_resource(id, size)
    }

    /// Free a resource by handle.
    ///
    /// # Errors
    ///
    /// Returns an error if the handle is not found.
    pub fn free_resource(&mut self, handle: ResourceHandle) -> Result<()> {
        self.runtime.free_resource(handle)
    }

    /// Look up a plugin by its runtime ID.
    #[must_use]
    pub fn plugin(&self, id: PluginId) -> Option<&LoadedPlugin> {
        self.runtime.plugin(id)
    }

    /// Look up a plugin by its manifest ID.
    #[must_use]
    pub fn find_by_manifest_id(&self, manifest_id: &str) -> Option<&LoadedPlugin> {
        self.runtime.find_by_manifest_id(manifest_id)
    }

    /// Get all currently active (non-unloaded) plugins.
    #[must_use]
    pub fn active_plugins(&self) -> Vec<&LoadedPlugin> {
        self.runtime.active_plugins()
    }

    /// Number of non-unloaded plugins.
    #[must_use]
    pub fn active_plugin_count(&self) -> usize {
        self.runtime.active_plugin_count()
    }

    /// Get the dispatcher.
    #[must_use]
    pub fn dispatcher(&self) -> &Dispatcher {
        &self.dispatcher
    }

    /// Get the runtime.
    #[must_use]
    pub fn runtime(&self) -> &PluginRuntime {
        &self.runtime
    }

    /// Get the resource pool.
    #[must_use]
    pub fn resources(&self) -> &ResourcePool {
        self.runtime.resources()
    }

    /// Get the host configuration.
    #[must_use]
    pub fn config(&self) -> &PluginHostConfig {
        &self.config
    }

    /// Get the list of available host functions.
    #[must_use]
    pub fn host_functions(&self) -> &'static [HostFunction] {
        HOST_FUNCTIONS
    }

    /// Simulate invoking a host function by index.
    ///
    /// Returns `PluginResult::Ok` for known functions and
    /// `PluginResult::Error` for unknown indices.
    #[must_use]
    pub fn invoke_host_function(&self, function_index: u32) -> PluginResult {
        if HOST_FUNCTIONS.iter().any(|f| f.index == function_index) {
            tracing::trace!(function_index, "host function invoked");
            PluginResult::Ok
        } else {
            tracing::warn!(function_index, "unknown host function");
            PluginResult::Error
        }
    }
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl fmt::Display for PluginHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PluginHost({} active plugins, {})",
            self.active_plugin_count(),
            self.dispatcher,
        )
    }
}
