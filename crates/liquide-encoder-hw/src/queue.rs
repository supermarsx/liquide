//! Multi-GPU session queue manager.

use crate::api::HwEncoderApi;

/// Tracks resource usage for a single GPU.
#[derive(Debug, Clone)]
pub struct GpuSlot {
    /// Index of this GPU.
    pub gpu_index: usize,
    /// Which API this GPU uses.
    pub api: HwEncoderApi,
    /// Maximum concurrent sessions for this GPU.
    pub max_sessions: u32,
    /// Currently active sessions.
    pub active_sessions: u32,
    /// Total VRAM in megabytes.
    pub vram_total_mb: u64,
    /// Currently used VRAM in megabytes.
    pub vram_used_mb: u64,
}

/// Manages encoder session allocation across multiple GPUs.
pub struct EncoderQueueManager {
    slots: Vec<GpuSlot>,
}

impl EncoderQueueManager {
    /// Create an empty queue manager.
    #[must_use]
    pub fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// Register a GPU with the queue manager.
    pub fn register_gpu(
        &mut self,
        gpu_index: usize,
        api: HwEncoderApi,
        max_sessions: u32,
        vram_mb: u64,
    ) {
        self.slots.push(GpuSlot {
            gpu_index,
            api,
            max_sessions,
            active_sessions: 0,
            vram_total_mb: vram_mb,
            vram_used_mb: 0,
        });
    }

    /// Allocate a session on the given GPU, consuming VRAM.
    pub fn allocate_session(&mut self, gpu_index: usize, vram_needed: u64) -> crate::Result<()> {
        let slot = self
            .slots
            .iter_mut()
            .find(|s| s.gpu_index == gpu_index)
            .ok_or_else(|| crate::HwEncoderError::Internal(format!("GPU {gpu_index} not found")))?;

        if slot.active_sessions >= slot.max_sessions {
            return Err(crate::HwEncoderError::SessionLimitReached {
                api: slot.api.to_string(),
                max: slot.max_sessions,
            });
        }

        if slot.vram_used_mb + vram_needed > slot.vram_total_mb {
            return Err(crate::HwEncoderError::VramExhausted {
                used_mb: slot.vram_used_mb + vram_needed,
                budget_mb: slot.vram_total_mb,
            });
        }

        slot.active_sessions += 1;
        slot.vram_used_mb += vram_needed;
        Ok(())
    }

    /// Release a session from the given GPU.
    pub fn release_session(&mut self, gpu_index: usize, vram_freed: u64) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.gpu_index == gpu_index) {
            slot.active_sessions = slot.active_sessions.saturating_sub(1);
            slot.vram_used_mb = slot.vram_used_mb.saturating_sub(vram_freed);
        }
    }

    /// Find the least-loaded GPU (fewest active sessions).
    #[must_use]
    pub fn best_gpu(&self) -> Option<usize> {
        self.slots
            .iter()
            .filter(|s| s.active_sessions < s.max_sessions)
            .min_by_key(|s| s.active_sessions)
            .map(|s| s.gpu_index)
    }

    /// Whether the given GPU has reached its session limit.
    #[must_use]
    pub fn is_full(&self, gpu_index: usize) -> bool {
        self.slots
            .iter()
            .find(|s| s.gpu_index == gpu_index)
            .map_or(true, |s| s.active_sessions >= s.max_sessions)
    }

    /// Total active sessions across all GPUs.
    #[must_use]
    pub fn total_active_sessions(&self) -> u32 {
        self.slots.iter().map(|s| s.active_sessions).sum()
    }

    /// Access the GPU slots.
    #[must_use]
    pub fn slots(&self) -> &[GpuSlot] {
        &self.slots
    }
}

impl Default for EncoderQueueManager {
    fn default() -> Self {
        Self::new()
    }
}
