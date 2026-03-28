//! Desktop heap — memory budget tracking per desktop.
//!
//! Each desktop has a fixed memory budget. All window-related allocations on
//! that desktop (window objects, menus, hooks, etc.) are tracked against this
//! budget to prevent one desktop from consuming all system memory.

use crate::error::DesktopError;
use crate::types::DesktopId;

/// Default desktop heap budget: 48 MiB.
pub const DEFAULT_HEAP_BUDGET: usize = 48 * 1024 * 1024;

/// Default interactive desktop heap budget: 20 MiB.
pub const DEFAULT_INTERACTIVE_HEAP_BUDGET: usize = 20 * 1024 * 1024;

/// Tracks memory usage against a fixed budget for a single desktop.
#[derive(Debug, Clone)]
pub struct DesktopHeap {
    /// Which desktop this heap belongs to.
    desktop_id: DesktopId,
    /// Total budget in bytes.
    budget: usize,
    /// Currently allocated bytes.
    used: usize,
    /// High-water mark (peak usage).
    peak: usize,
    /// Number of allocation operations performed.
    alloc_count: u64,
}

impl DesktopHeap {
    /// Creates a new heap tracker for the given desktop with the specified budget.
    pub fn new(desktop_id: DesktopId, budget: usize) -> Self {
        Self {
            desktop_id,
            budget,
            used: 0,
            peak: 0,
            alloc_count: 0,
        }
    }

    /// Attempts to allocate `size` bytes from this desktop's budget.
    ///
    /// Returns `Ok(())` if there is sufficient space, or
    /// `Err(DesktopError::HeapExhausted)` if the allocation would exceed the
    /// budget.
    pub fn allocate(&mut self, size: usize) -> Result<(), DesktopError> {
        let new_used = self.used.checked_add(size).ok_or_else(|| {
            DesktopError::HeapExhausted {
                desktop: self.desktop_id,
                requested: size,
                available: self.budget.saturating_sub(self.used),
            }
        })?;

        if new_used > self.budget {
            return Err(DesktopError::HeapExhausted {
                desktop: self.desktop_id,
                requested: size,
                available: self.budget.saturating_sub(self.used),
            });
        }

        self.used = new_used;
        if self.used > self.peak {
            self.peak = self.used;
        }
        self.alloc_count += 1;
        Ok(())
    }

    /// Frees `size` bytes from this desktop's tracked usage.
    ///
    /// Saturates at zero (does not underflow).
    pub fn deallocate(&mut self, size: usize) {
        self.used = self.used.saturating_sub(size);
    }

    /// Returns the number of bytes currently allocated.
    pub fn used(&self) -> usize {
        self.used
    }

    /// Returns the total budget in bytes.
    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Returns the number of bytes still available.
    pub fn available(&self) -> usize {
        self.budget.saturating_sub(self.used)
    }

    /// Returns the peak usage (high-water mark) in bytes.
    pub fn peak(&self) -> usize {
        self.peak
    }

    /// Returns the total number of allocation operations performed.
    pub fn alloc_count(&self) -> u64 {
        self.alloc_count
    }

    /// Returns the desktop this heap belongs to.
    pub fn desktop_id(&self) -> DesktopId {
        self.desktop_id
    }

    /// Resets tracked usage to zero. The peak is not reset.
    pub fn reset(&mut self) {
        self.used = 0;
    }

    /// Returns usage as a fraction of the budget (0.0 .. 1.0).
    pub fn utilization(&self) -> f64 {
        if self.budget == 0 {
            return 0.0;
        }
        self.used as f64 / self.budget as f64
    }
}
