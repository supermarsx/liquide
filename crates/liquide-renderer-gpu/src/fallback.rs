//! CPU fallback handling for GPU renderer failures.
//!
//! When the GPU is unavailable, the device is lost, or VRAM is exhausted,
//! the renderer transparently falls back to the CPU path.  This module
//! tracks the fallback state and the reason it was activated.

use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Reason why the GPU renderer fell back to CPU rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FallbackReason {
    /// No GPU device was found during probing.
    NoGpu,
    /// The Vulkan device was lost (VK_ERROR_DEVICE_LOST).
    DeviceLost,
    /// VRAM budget was exhausted.
    OutOfVram,
    /// The required pixel format is not supported by the GPU.
    UnsupportedFormat,
    /// A GPU driver error occurred.
    DriverError(String),
}

impl std::fmt::Display for FallbackReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoGpu => write!(f, "no GPU available"),
            Self::DeviceLost => write!(f, "Vulkan device lost"),
            Self::OutOfVram => write!(f, "VRAM budget exhausted"),
            Self::UnsupportedFormat => write!(f, "unsupported pixel format"),
            Self::DriverError(msg) => write!(f, "driver error: {msg}"),
        }
    }
}

/// Current state of the CPU fallback.
#[derive(Debug)]
pub struct FallbackState {
    /// Whether fallback is currently active.
    pub active: bool,
    /// The reason fallback was activated, if any.
    pub reason: Option<FallbackReason>,
    /// When fallback was activated.
    pub since: Option<Instant>,
}

impl Default for FallbackState {
    fn default() -> Self {
        Self {
            active: false,
            reason: None,
            since: None,
        }
    }
}

/// Manager for GPU-to-CPU fallback transitions.
///
/// Tracks whether the renderer is currently in fallback mode and
/// provides methods to activate/deactivate it.
#[derive(Debug)]
pub struct FallbackManager {
    /// Current fallback state.
    state: FallbackState,
}

impl FallbackManager {
    /// Create a new fallback manager in the inactive state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: FallbackState::default(),
        }
    }

    /// Activate CPU fallback for the given reason.
    pub fn activate(&mut self, reason: FallbackReason) {
        tracing::warn!(reason = %reason, "GPU fallback activated");
        self.state.active = true;
        self.state.reason = Some(reason);
        self.state.since = Some(Instant::now());
    }

    /// Deactivate CPU fallback and return to GPU rendering.
    pub fn deactivate(&mut self) {
        if self.state.active {
            tracing::info!("GPU fallback deactivated, returning to GPU rendering");
        }
        self.state.active = false;
        self.state.reason = None;
        self.state.since = None;
    }

    /// Whether fallback is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state.active
    }

    /// The reason fallback was activated, if active.
    #[must_use]
    pub fn reason(&self) -> Option<&FallbackReason> {
        self.state.reason.as_ref()
    }

    /// When fallback was activated, if active.
    #[must_use]
    pub fn since(&self) -> Option<Instant> {
        self.state.since
    }
}

impl Default for FallbackManager {
    fn default() -> Self {
        Self::new()
    }
}
