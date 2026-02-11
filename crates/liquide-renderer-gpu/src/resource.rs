//! VRAM budget management and allocation tracking.
//!
//! Enforces per-session VRAM budgets (default 256 MB) and tracks individual
//! allocations by purpose so that resource usage can be monitored and
//! reported via audit events.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The purpose of a VRAM allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AllocationPurpose {
    /// Texture atlas for surface content.
    TextureAtlas,
    /// Intermediate or final render target.
    RenderTarget,
    /// Staging buffer for CPU ↔ GPU transfers.
    StagingBuffer,
    /// Compute shader storage buffer.
    ComputeBuffer,
    /// Glyph cache texture.
    GlyphCache,
}

impl std::fmt::Display for AllocationPurpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TextureAtlas => write!(f, "texture-atlas"),
            Self::RenderTarget => write!(f, "render-target"),
            Self::StagingBuffer => write!(f, "staging-buffer"),
            Self::ComputeBuffer => write!(f, "compute-buffer"),
            Self::GlyphCache => write!(f, "glyph-cache"),
        }
    }
}

/// VRAM budget configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VramBudget {
    /// Total VRAM on the device in megabytes.
    pub total_mb: u64,
    /// Currently allocated VRAM in megabytes.
    pub allocated_mb: u64,
    /// Per-session VRAM budget in megabytes (default 256).
    pub session_budget_mb: u64,
}

impl Default for VramBudget {
    fn default() -> Self {
        Self {
            total_mb: 0,
            allocated_mb: 0,
            session_budget_mb: 256,
        }
    }
}

/// A tracked VRAM allocation.
#[derive(Debug, Clone)]
pub struct VramAllocation {
    /// Unique identifier for this allocation.
    pub id: String,
    /// Size of the allocation in bytes.
    pub size_bytes: u64,
    /// What this memory is used for.
    pub purpose: AllocationPurpose,
}

/// VRAM allocation manager that enforces the session budget.
#[derive(Debug)]
pub struct VramAllocator {
    /// Budget configuration.
    budget: VramBudget,
    /// Active allocations keyed by ID.
    allocations: HashMap<String, VramAllocation>,
    /// Counter for generating unique allocation IDs.
    next_id: u64,
}

impl VramAllocator {
    /// Create a new allocator with the given budget.
    #[must_use]
    pub fn new(budget: VramBudget) -> Self {
        Self {
            budget,
            allocations: HashMap::new(),
            next_id: 0,
        }
    }

    /// Allocate VRAM for the given purpose.
    ///
    /// Returns the allocation ID on success, or an error if the budget
    /// would be exceeded.
    pub fn allocate(
        &mut self,
        purpose: AllocationPurpose,
        size_bytes: u64,
    ) -> crate::Result<String> {
        let size_mb = size_bytes / (1024 * 1024);
        let new_total = self.budget.allocated_mb + size_mb;

        if new_total > self.budget.session_budget_mb {
            return Err(crate::GpuRendererError::OutOfVram {
                allocated_mb: new_total,
                budget_mb: self.budget.session_budget_mb,
            });
        }

        let id = format!("vram-{}", self.next_id);
        self.next_id += 1;

        let alloc = VramAllocation {
            id: id.clone(),
            size_bytes,
            purpose,
        };

        self.budget.allocated_mb = new_total;
        self.allocations.insert(id.clone(), alloc);

        tracing::debug!(
            id = %id,
            size_bytes,
            purpose = %purpose,
            allocated_mb = new_total,
            "VRAM allocated"
        );

        Ok(id)
    }

    /// Free a previously allocated VRAM region.
    ///
    /// Returns `true` if the allocation was found and freed.
    pub fn free(&mut self, id: &str) -> bool {
        if let Some(alloc) = self.allocations.remove(id) {
            let freed_mb = alloc.size_bytes / (1024 * 1024);
            self.budget.allocated_mb = self.budget.allocated_mb.saturating_sub(freed_mb);
            tracing::debug!(id = %id, freed_mb, "VRAM freed");
            true
        } else {
            false
        }
    }

    /// Current VRAM usage as a percentage of the session budget.
    #[must_use]
    pub fn usage_pct(&self) -> f64 {
        if self.budget.session_budget_mb == 0 {
            return 0.0;
        }
        self.budget.allocated_mb as f64 / self.budget.session_budget_mb as f64 * 100.0
    }

    /// Available VRAM in bytes within the session budget.
    #[must_use]
    pub fn available_bytes(&self) -> u64 {
        let available_mb = self
            .budget
            .session_budget_mb
            .saturating_sub(self.budget.allocated_mb);
        available_mb * 1024 * 1024
    }

    /// Access the current budget state.
    #[must_use]
    pub fn budget(&self) -> &VramBudget {
        &self.budget
    }

    /// Number of active allocations.
    #[must_use]
    pub fn allocation_count(&self) -> usize {
        self.allocations.len()
    }
}
