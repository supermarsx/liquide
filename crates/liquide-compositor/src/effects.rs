//! Effect budget system and degradation ladder.
//!
//! The compositor tracks effect rendering costs and automatically degrades
//! visual quality when the frame budget is exceeded, following a deterministic
//! 14-level ladder (L0 = full quality, L13 = emergency).

use serde::{Deserialize, Serialize};

/// Degradation level (L0 = full quality, L13 = emergency minimal rendering).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum DegradationLevel {
    #[default]
    L0 = 0,
    L1 = 1,
    L2 = 2,
    L3 = 3,
    L4 = 4,
    L5 = 5,
    L6 = 6,
    L7 = 7,
    L8 = 8,
    L9 = 9,
    L10 = 10,
    L11 = 11,
    L12 = 12,
    L13 = 13,
}

impl DegradationLevel {
    /// Convert to raw byte.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Convert from a raw byte.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::L0),
            1 => Some(Self::L1),
            2 => Some(Self::L2),
            3 => Some(Self::L3),
            4 => Some(Self::L4),
            5 => Some(Self::L5),
            6 => Some(Self::L6),
            7 => Some(Self::L7),
            8 => Some(Self::L8),
            9 => Some(Self::L9),
            10 => Some(Self::L10),
            11 => Some(Self::L11),
            12 => Some(Self::L12),
            13 => Some(Self::L13),
            _ => None,
        }
    }

    /// Step down one level (towards lower quality). Returns same level if
    /// already at L13.
    #[must_use]
    pub fn step_down(self) -> Self {
        DegradationLevel::from_u8(self.as_u8().saturating_add(1).min(13)).unwrap()
    }

    /// Step up one level (towards higher quality). Returns same level if
    /// already at L0.
    #[must_use]
    pub fn step_up(self) -> Self {
        DegradationLevel::from_u8(self.as_u8().saturating_sub(1)).unwrap()
    }
}

/// Effect quality profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum QualityProfile {
    Quality,
    #[default]
    Balanced,
    Performance,
    Minimal,
}

/// Current effect budget state.
#[derive(Debug, Clone)]
pub struct EffectBudget {
    pub profile: QualityProfile,
    /// Total budget for all effects combined (ms).
    pub total_effects_budget_ms: f64,
    /// Total budget for the entire frame (ms).
    pub total_frame_budget_ms: f64,
    /// Target frames per second.
    pub target_fps: u32,
    /// Budget for backdrop blur effects (ms).
    pub blur_budget_ms: f64,
    /// Budget for box shadow effects (ms).
    pub shadow_budget_ms: f64,
    /// Budget for text rendering (ms). Never reduced by degradation.
    pub text_budget_ms: f64,
}

impl EffectBudget {
    fn default_target_fps(profile: QualityProfile) -> u32 {
        match profile {
            QualityProfile::Minimal => 30,
            QualityProfile::Quality | QualityProfile::Balanced | QualityProfile::Performance => 60,
        }
    }

    fn with_scaled_budgets(
        profile: QualityProfile,
        base_target_fps: u32,
        base_total_effects_budget_ms: f64,
        base_total_frame_budget_ms: f64,
        base_blur_budget_ms: f64,
        base_shadow_budget_ms: f64,
        base_text_budget_ms: f64,
        target_fps: u32,
    ) -> Self {
        let target_fps = if target_fps == 0 {
            base_target_fps
        } else {
            target_fps
        };
        let scale = base_target_fps as f64 / target_fps as f64;

        Self {
            profile,
            total_effects_budget_ms: base_total_effects_budget_ms * scale,
            total_frame_budget_ms: base_total_frame_budget_ms * scale,
            target_fps,
            blur_budget_ms: base_blur_budget_ms * scale,
            shadow_budget_ms: base_shadow_budget_ms * scale,
            text_budget_ms: base_text_budget_ms * scale,
        }
    }

    /// Return the budget for a given quality profile at L0 (no degradation).
    #[must_use]
    pub fn for_profile(profile: QualityProfile) -> Self {
        Self::for_profile_with_target_fps(profile, Self::default_target_fps(profile))
    }

    /// Return the budget for a given quality profile retargeted to a specific fps.
    #[must_use]
    pub fn for_profile_with_target_fps(profile: QualityProfile, target_fps: u32) -> Self {
        match profile {
            QualityProfile::Quality => {
                Self::with_scaled_budgets(profile, 60, 10.0, 16.67, 4.0, 1.0, 3.0, target_fps)
            }
            QualityProfile::Balanced => {
                Self::with_scaled_budgets(profile, 60, 6.0, 12.0, 3.0, 0.8, 3.0, target_fps)
            }
            QualityProfile::Performance => {
                Self::with_scaled_budgets(profile, 60, 3.0, 8.0, 1.5, 0.5, 3.0, target_fps)
            }
            QualityProfile::Minimal => {
                Self::with_scaled_budgets(profile, 30, 1.0, 5.0, 0.0, 0.0, 3.0, target_fps)
            }
        }
    }

    /// Retarget this budget to a different fps while preserving the profile ratios.
    pub fn set_target_fps(&mut self, target_fps: u32) {
        *self = Self::for_profile_with_target_fps(self.profile, target_fps);
    }

    /// Remaining budget given elapsed time.
    #[must_use]
    pub fn remaining_ms(&self, elapsed_ms: f64) -> f64 {
        (self.total_frame_budget_ms - elapsed_ms).max(0.0)
    }
}

/// Blur algorithm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlurAlgorithm {
    /// Separable Gaussian blur (higher quality).
    #[default]
    Gaussian,
    /// Box blur (faster, lower quality).
    Box,
}

/// Runtime-tunable effect parameters, derived from profile + degradation level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectParams {
    /// Backdrop blur radius in pixels.
    pub blur_radius: u32,
    /// Downsample factor for blur (e.g. 4 = quarter resolution).
    pub blur_downsample: u32,
    /// Which blur algorithm to use.
    pub blur_algorithm: BlurAlgorithm,
    /// Shadow blur radius in pixels.
    pub shadow_blur_radius: u32,
    /// Shadow spread in pixels.
    pub shadow_spread: u32,
    /// Maximum number of concurrent backdrop blurs per frame.
    pub max_backdrop_blurs: u32,
    /// Inner glow width in pixels.
    pub inner_glow_width: f32,
    /// Whether parallax effect is enabled.
    pub parallax_enabled: bool,
    /// Animation time scale (1.0 = normal, 0.0 = disabled).
    pub animation_scale: f32,
}

impl EffectParams {
    /// Default params for a quality profile at L0 (no degradation).
    #[must_use]
    pub fn for_profile(profile: QualityProfile) -> Self {
        match profile {
            QualityProfile::Quality => Self {
                blur_radius: 20,
                blur_downsample: 4,
                blur_algorithm: BlurAlgorithm::Gaussian,
                shadow_blur_radius: 16,
                shadow_spread: 8,
                max_backdrop_blurs: 8,
                inner_glow_width: 1.5,
                parallax_enabled: true,
                animation_scale: 1.0,
            },
            QualityProfile::Balanced => Self {
                blur_radius: 16,
                blur_downsample: 4,
                blur_algorithm: BlurAlgorithm::Gaussian,
                shadow_blur_radius: 12,
                shadow_spread: 6,
                max_backdrop_blurs: 6,
                inner_glow_width: 1.0,
                parallax_enabled: false,
                animation_scale: 1.0,
            },
            QualityProfile::Performance => Self {
                blur_radius: 10,
                blur_downsample: 4,
                blur_algorithm: BlurAlgorithm::Box,
                shadow_blur_radius: 8,
                shadow_spread: 4,
                max_backdrop_blurs: 4,
                inner_glow_width: 1.0,
                parallax_enabled: false,
                animation_scale: 0.5,
            },
            QualityProfile::Minimal => Self {
                blur_radius: 0,
                blur_downsample: 8,
                blur_algorithm: BlurAlgorithm::Box,
                shadow_blur_radius: 0,
                shadow_spread: 0,
                max_backdrop_blurs: 0,
                inner_glow_width: 0.0,
                parallax_enabled: false,
                animation_scale: 0.0,
            },
        }
    }

    /// Apply a degradation level to modify the current parameters.
    #[must_use]
    pub fn apply_degradation(&self, level: DegradationLevel) -> Self {
        let mut p = self.clone();
        match level {
            DegradationLevel::L0 => {} // No change
            DegradationLevel::L1 => {
                // Increase downsample
                p.blur_downsample = (p.blur_downsample + 1).min(8);
                p.parallax_enabled = false;
            }
            DegradationLevel::L2 => {
                p.blur_downsample = (p.blur_downsample + 2).min(8);
                p.parallax_enabled = false;
                p.shadow_blur_radius = p.shadow_blur_radius.saturating_sub(4);
            }
            DegradationLevel::L3 => {
                p.blur_downsample = 8;
                p.parallax_enabled = false;
                p.shadow_blur_radius = p.shadow_blur_radius.saturating_sub(8);
                p.inner_glow_width = 0.0;
            }
            DegradationLevel::L4 => {
                p.blur_downsample = 8;
                p.max_backdrop_blurs = p.max_backdrop_blurs.min(4);
                p.shadow_blur_radius = 0;
                p.inner_glow_width = 0.0;
                p.parallax_enabled = false;
            }
            DegradationLevel::L5 => {
                p.max_backdrop_blurs = p.max_backdrop_blurs.min(2);
                p.blur_algorithm = BlurAlgorithm::Box;
                p.shadow_blur_radius = 0;
                p.shadow_spread = 0;
                p.inner_glow_width = 0.0;
                p.parallax_enabled = false;
            }
            DegradationLevel::L6 => {
                p.max_backdrop_blurs = 1;
                p.blur_algorithm = BlurAlgorithm::Box;
                p.blur_radius = p.blur_radius.min(8);
                p.shadow_blur_radius = 0;
                p.shadow_spread = 0;
                p.inner_glow_width = 0.0;
                p.parallax_enabled = false;
            }
            DegradationLevel::L7 => {
                p.max_backdrop_blurs = 0;
                p.blur_radius = 0;
                p.shadow_blur_radius = 0;
                p.shadow_spread = 0;
                p.inner_glow_width = 0.0;
                p.parallax_enabled = false;
                p.animation_scale = 0.0;
            }
            _ => {
                // L8-L13: all effects disabled, FPS reduction handled elsewhere
                p.max_backdrop_blurs = 0;
                p.blur_radius = 0;
                p.shadow_blur_radius = 0;
                p.shadow_spread = 0;
                p.inner_glow_width = 0.0;
                p.parallax_enabled = false;
                p.animation_scale = 0.0;
            }
        }
        p
    }
}

/// Manages the degradation ladder with hysteresis.
///
/// The controller descends one level after `descend_threshold` consecutive
/// over-budget frames, and ascends one level after `ascend_threshold`
/// consecutive under-budget frames (at <70% of budget).
pub struct DegradationController {
    current_level: DegradationLevel,
    over_budget_count: u32,
    under_budget_count: u32,
    /// Number of consecutive over-budget frames before descending (default 3).
    descend_threshold: u32,
    /// Number of consecutive under-budget frames at <70% before ascending (default 10).
    ascend_threshold: u32,
}

impl DegradationController {
    /// Create a new controller starting at L0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_level: DegradationLevel::L0,
            over_budget_count: 0,
            under_budget_count: 0,
            descend_threshold: 3,
            ascend_threshold: 10,
        }
    }

    /// Create a controller with custom thresholds.
    #[must_use]
    pub fn with_thresholds(descend: u32, ascend: u32) -> Self {
        Self {
            descend_threshold: descend,
            ascend_threshold: ascend,
            ..Self::new()
        }
    }

    /// Report a frame's rendering time. Returns `true` if the level changed.
    pub fn report_frame_time(&mut self, frame_ms: f64, budget_ms: f64) -> bool {
        let prev = self.current_level;

        if frame_ms > budget_ms {
            // Over budget
            self.under_budget_count = 0;
            self.over_budget_count += 1;

            if self.over_budget_count >= self.descend_threshold {
                self.current_level = self.current_level.step_down();
                self.over_budget_count = 0;
            }
        } else if frame_ms < budget_ms * 0.7 {
            // Well under budget (<70%)
            self.over_budget_count = 0;
            self.under_budget_count += 1;

            if self.under_budget_count >= self.ascend_threshold {
                self.current_level = self.current_level.step_up();
                self.under_budget_count = 0;
            }
        } else {
            // Within budget but not far under — reset both counters
            self.over_budget_count = 0;
            self.under_budget_count = 0;
        }

        self.current_level != prev
    }

    /// Get the current degradation level.
    #[must_use]
    pub fn current_level(&self) -> DegradationLevel {
        self.current_level
    }

    /// Compute the current [`EffectParams`] for a given profile.
    #[must_use]
    pub fn current_params(&self, profile: QualityProfile) -> EffectParams {
        EffectParams::for_profile(profile).apply_degradation(self.current_level)
    }

    /// Force a specific degradation level.
    pub fn set_level(&mut self, level: DegradationLevel) {
        self.current_level = level;
        self.over_budget_count = 0;
        self.under_budget_count = 0;
    }
}

impl Default for DegradationController {
    fn default() -> Self {
        Self::new()
    }
}
