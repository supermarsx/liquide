//! Resource pool for managing host-side allocations on behalf of plugins.

use std::collections::HashMap;
use std::fmt;

use liquide_plugin_abi::types::ResourceHandle;
use serde::{Deserialize, Serialize};

use crate::plugin::PluginId;
use crate::{PluginHostError, Result};

/// Metadata for a single resource allocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    /// The handle exposed to the plugin.
    pub handle: ResourceHandle,
    /// Allocation size in bytes.
    pub size: u64,
    /// The plugin that owns this allocation.
    pub owner: PluginId,
}

impl fmt::Display for ResourceAllocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Alloc(handle={}, size={}B, owner={})",
            self.handle.0, self.size, self.owner,
        )
    }
}

/// A pool of host-side resources shared across all loaded plugins.
pub struct ResourcePool {
    next_handle_id: u64,
    allocations: HashMap<ResourceHandle, ResourceAllocation>,
    total_allocated: u64,
    max_capacity: u64,
}

impl ResourcePool {
    /// Create a new resource pool with the given maximum capacity in bytes.
    #[must_use]
    pub fn new(max_capacity: u64) -> Self {
        Self {
            next_handle_id: 1,
            allocations: HashMap::new(),
            total_allocated: 0,
            max_capacity,
        }
    }

    /// Allocate a resource of the given size for the specified plugin.
    ///
    /// Returns the new [`ResourceHandle`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`PluginHostError::ResourceExhausted`] if there is insufficient
    /// capacity remaining in the pool.
    pub fn allocate(&mut self, size: u64, owner: PluginId) -> Result<ResourceHandle> {
        let available = self.max_capacity.saturating_sub(self.total_allocated);
        if size > available {
            return Err(PluginHostError::ResourceExhausted {
                requested: size,
                available,
            });
        }

        let handle = ResourceHandle(self.next_handle_id);
        self.next_handle_id += 1;

        let alloc = ResourceAllocation {
            handle,
            size,
            owner,
        };
        self.allocations.insert(handle, alloc);
        self.total_allocated += size;

        tracing::debug!(
            handle = handle.0,
            size,
            owner = owner.0,
            "resource allocated"
        );

        Ok(handle)
    }

    /// Free a previously allocated resource.
    ///
    /// Returns the freed [`ResourceAllocation`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`PluginHostError::Internal`] if the handle is not found.
    pub fn free(&mut self, handle: ResourceHandle) -> Result<ResourceAllocation> {
        let alloc = self
            .allocations
            .remove(&handle)
            .ok_or_else(|| PluginHostError::Internal(format!("unknown handle: {}", handle.0)))?;
        self.total_allocated = self.total_allocated.saturating_sub(alloc.size);

        tracing::debug!(
            handle = handle.0,
            size = alloc.size,
            "resource freed"
        );

        Ok(alloc)
    }

    /// Free all resources owned by a given plugin.
    ///
    /// Returns the number of allocations freed.
    pub fn free_all_for_plugin(&mut self, owner: PluginId) -> usize {
        let handles: Vec<ResourceHandle> = self
            .allocations
            .iter()
            .filter(|(_, a)| a.owner == owner)
            .map(|(h, _)| *h)
            .collect();

        let count = handles.len();
        for handle in handles {
            if let Some(alloc) = self.allocations.remove(&handle) {
                self.total_allocated = self.total_allocated.saturating_sub(alloc.size);
            }
        }

        if count > 0 {
            tracing::debug!(owner = owner.0, count, "freed all resources for plugin");
        }

        count
    }

    /// Look up an allocation by handle.
    #[must_use]
    pub fn get(&self, handle: ResourceHandle) -> Option<&ResourceAllocation> {
        self.allocations.get(&handle)
    }

    /// Total bytes currently allocated.
    #[must_use]
    pub fn total_allocated(&self) -> u64 {
        self.total_allocated
    }

    /// Maximum pool capacity in bytes.
    #[must_use]
    pub fn max_capacity(&self) -> u64 {
        self.max_capacity
    }

    /// Remaining available capacity in bytes.
    #[must_use]
    pub fn available(&self) -> u64 {
        self.max_capacity.saturating_sub(self.total_allocated)
    }

    /// Number of active allocations.
    #[must_use]
    pub fn allocation_count(&self) -> usize {
        self.allocations.len()
    }

    /// Get all allocations owned by a given plugin.
    #[must_use]
    pub fn allocations_for_plugin(&self, owner: PluginId) -> Vec<&ResourceAllocation> {
        self.allocations
            .values()
            .filter(|a| a.owner == owner)
            .collect()
    }
}

impl fmt::Display for ResourcePool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ResourcePool({}/{}B used, {} allocs)",
            self.total_allocated,
            self.max_capacity,
            self.allocations.len(),
        )
    }
}
