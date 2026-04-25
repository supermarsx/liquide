//! Plugin runtime — manages loaded plugins and their resource allocations.

use std::collections::HashMap;
use std::fmt;

use liquide_plugin_abi::plugin_manifest;
use liquide_plugin_abi::types::ResourceHandle;
use liquide_plugin_abi::{ABI_VERSION, PluginManifest};

use crate::config::PluginHostConfig;
use crate::plugin::{LoadedPlugin, PluginId};
use crate::resources::ResourcePool;
use crate::{PluginHostError, Result};

/// The plugin runtime manages plugin instances and their resource allocations.
pub struct PluginRuntime {
    plugins: HashMap<PluginId, LoadedPlugin>,
    resources: ResourcePool,
    next_plugin_id: u64,
    config: PluginHostConfig,
}

impl PluginRuntime {
    /// Create a new runtime with the given configuration.
    #[must_use]
    pub fn new(config: PluginHostConfig) -> Self {
        let pool_cap = config.resource_pool_capacity;
        Self {
            plugins: HashMap::new(),
            resources: ResourcePool::new(pool_cap),
            next_plugin_id: 1,
            config,
        }
    }

    /// Load a plugin from its manifest.
    ///
    /// The plugin is validated for ABI compatibility, checked for duplicate
    /// manifest IDs, and verified against plugin limits and memory caps before
    /// being placed into the [`PluginState::Active`] state.
    ///
    /// # Errors
    ///
    /// Returns an error if the ABI is incompatible, the manifest ID is a
    /// duplicate, the plugin limit is reached, or memory limits are exceeded.
    pub fn load_plugin(&mut self, manifest: PluginManifest) -> Result<PluginId> {
        // ABI compatibility check.
        if !plugin_manifest::is_compatible(&manifest) {
            return Err(PluginHostError::IncompatibleAbi {
                expected: ABI_VERSION,
                found: manifest.abi_version,
            });
        }

        // Duplicate check.
        for p in self.plugins.values() {
            if p.manifest.id == manifest.id && !p.is_unloaded() {
                return Err(PluginHostError::DuplicatePlugin {
                    manifest_id: manifest.id.clone(),
                });
            }
        }

        // Plugin limit.
        let active_count = self.plugins.values().filter(|p| !p.is_unloaded()).count();
        if active_count >= self.config.max_plugins {
            return Err(PluginHostError::PluginLimitReached {
                max: self.config.max_plugins,
            });
        }

        // Memory cap.
        if manifest.requested_memory_bytes > self.config.max_memory_per_plugin {
            return Err(PluginHostError::ResourceExhausted {
                requested: manifest.requested_memory_bytes,
                available: self.config.max_memory_per_plugin,
            });
        }

        let id = PluginId(self.next_plugin_id);
        self.next_plugin_id += 1;

        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        let mut plugin = LoadedPlugin::new(id, manifest, now_us);
        plugin.activate();

        tracing::info!(
            plugin_id = id.0,
            manifest_id = %plugin.manifest_id(),
            name = %plugin.name(),
            "plugin loaded"
        );

        self.plugins.insert(id, plugin);
        Ok(id)
    }

    /// Load a plugin from raw JSON manifest bytes.
    ///
    /// # Errors
    ///
    /// Returns [`PluginHostError::ManifestParse`] if parsing fails, or any
    /// error from [`load_plugin`](Self::load_plugin).
    pub fn load_plugin_from_json(&mut self, json: &[u8]) -> Result<PluginId> {
        let manifest =
            plugin_manifest::parse_manifest(json).map_err(PluginHostError::ManifestParse)?;
        self.load_plugin(manifest)
    }

    /// Unload a plugin, freeing all its resources.
    ///
    /// # Errors
    ///
    /// Returns [`PluginHostError::PluginNotFound`] if the ID is unknown, or
    /// [`PluginHostError::InvalidState`] if the plugin is already unloaded.
    pub fn unload_plugin(&mut self, id: PluginId) -> Result<()> {
        let plugin = self
            .plugins
            .get_mut(&id)
            .ok_or(PluginHostError::PluginNotFound { id })?;

        if plugin.is_unloaded() {
            return Err(PluginHostError::InvalidState {
                id,
                state: plugin.state.to_string(),
            });
        }

        plugin.unload();
        self.resources.free_all_for_plugin(id);

        tracing::info!(plugin_id = id.0, "plugin unloaded");

        Ok(())
    }

    /// Suspend a plugin.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin is not found or not in the Active state.
    pub fn suspend_plugin(&mut self, id: PluginId) -> Result<()> {
        let plugin = self
            .plugins
            .get_mut(&id)
            .ok_or(PluginHostError::PluginNotFound { id })?;

        if !plugin.is_active() {
            return Err(PluginHostError::InvalidState {
                id,
                state: plugin.state.to_string(),
            });
        }

        plugin.suspend();

        tracing::debug!(plugin_id = id.0, "plugin suspended");

        Ok(())
    }

    /// Resume a suspended plugin.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin is not found or not in the Suspended state.
    pub fn resume_plugin(&mut self, id: PluginId) -> Result<()> {
        let plugin = self
            .plugins
            .get_mut(&id)
            .ok_or(PluginHostError::PluginNotFound { id })?;

        if !plugin.is_suspended() {
            return Err(PluginHostError::InvalidState {
                id,
                state: plugin.state.to_string(),
            });
        }

        plugin.activate();

        tracing::debug!(plugin_id = id.0, "plugin resumed");

        Ok(())
    }

    /// Mark a plugin as failed with the given reason.
    ///
    /// # Errors
    ///
    /// Returns [`PluginHostError::PluginNotFound`] if the ID is unknown.
    pub fn fail_plugin(&mut self, id: PluginId, reason: impl Into<String>) -> Result<()> {
        let plugin = self
            .plugins
            .get_mut(&id)
            .ok_or(PluginHostError::PluginNotFound { id })?;

        let reason = reason.into();
        tracing::warn!(plugin_id = id.0, reason = %reason, "plugin failed");
        plugin.fail(reason);

        Ok(())
    }

    /// Allocate a resource for the specified plugin.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin is not found, not active, or the pool
    /// is exhausted.
    pub fn allocate_resource(&mut self, id: PluginId, size: u64) -> Result<ResourceHandle> {
        let plugin = self
            .plugins
            .get(&id)
            .ok_or(PluginHostError::PluginNotFound { id })?;

        if !plugin.is_active() {
            return Err(PluginHostError::InvalidState {
                id,
                state: plugin.state.to_string(),
            });
        }

        self.resources.allocate(size, id)
    }

    /// Free a resource by handle.
    ///
    /// # Errors
    ///
    /// Returns an error if the handle is not found.
    pub fn free_resource(&mut self, handle: ResourceHandle) -> Result<()> {
        self.resources.free(handle)?;
        Ok(())
    }

    /// Look up a plugin by ID.
    #[must_use]
    pub fn plugin(&self, id: PluginId) -> Option<&LoadedPlugin> {
        self.plugins.get(&id)
    }

    /// Look up a plugin mutably by ID.
    #[must_use]
    pub fn plugin_mut(&mut self, id: PluginId) -> Option<&mut LoadedPlugin> {
        self.plugins.get_mut(&id)
    }

    /// Get all loaded (non-unloaded) plugins.
    #[must_use]
    pub fn active_plugins(&self) -> Vec<&LoadedPlugin> {
        self.plugins.values().filter(|p| !p.is_unloaded()).collect()
    }

    /// Get the total number of plugin slots (including unloaded).
    #[must_use]
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get the number of non-unloaded plugins.
    #[must_use]
    pub fn active_plugin_count(&self) -> usize {
        self.plugins.values().filter(|p| !p.is_unloaded()).count()
    }

    /// Get a reference to the resource pool.
    #[must_use]
    pub fn resources(&self) -> &ResourcePool {
        &self.resources
    }

    /// Get a mutable reference to the resource pool.
    pub fn resources_mut(&mut self) -> &mut ResourcePool {
        &mut self.resources
    }

    /// Get the runtime configuration.
    #[must_use]
    pub fn config(&self) -> &PluginHostConfig {
        &self.config
    }

    /// Find a plugin by its manifest ID string.
    #[must_use]
    pub fn find_by_manifest_id(&self, manifest_id: &str) -> Option<&LoadedPlugin> {
        self.plugins
            .values()
            .find(|p| p.manifest_id() == manifest_id && !p.is_unloaded())
    }
}

impl fmt::Display for PluginRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let active = self.active_plugin_count();
        write!(
            f,
            "PluginRuntime({active} active plugins, {})",
            self.resources,
        )
    }
}
