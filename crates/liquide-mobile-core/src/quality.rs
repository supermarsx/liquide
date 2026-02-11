//! Adaptive quality control and network-aware profile management.

use serde::{Deserialize, Serialize};

/// A quality profile describing encoding parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityProfile {
    /// Maximum frames per second.
    pub max_fps: u32,
    /// Target bitrate in kilobits per second.
    pub target_bitrate_kbps: u32,
    /// Resolution scale relative to native (0.0--1.0).
    pub resolution_scale: f32,
    /// Preferred codec name.
    pub codec_preference: String,
    /// Color depth in bits per pixel.
    pub color_depth: u32,
}

/// Preset quality levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityPreset {
    /// Low quality for constrained networks.
    Low,
    /// Balanced quality.
    Medium,
    /// Best quality for fast networks.
    High,
    /// Automatically adjusted based on network conditions.
    Auto,
}

impl std::fmt::Display for QualityPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Auto => write!(f, "auto"),
        }
    }
}

impl QualityPreset {
    /// Convert this preset into a concrete quality profile.
    #[must_use]
    pub fn to_profile(self) -> QualityProfile {
        match self {
            Self::Low => QualityProfile {
                max_fps: 24,
                target_bitrate_kbps: 1_000,
                resolution_scale: 0.5,
                codec_preference: "h264".to_string(),
                color_depth: 24,
            },
            Self::Medium => QualityProfile {
                max_fps: 30,
                target_bitrate_kbps: 4_000,
                resolution_scale: 0.75,
                codec_preference: "h264".to_string(),
                color_depth: 24,
            },
            Self::High => QualityProfile {
                max_fps: 60,
                target_bitrate_kbps: 10_000,
                resolution_scale: 1.0,
                codec_preference: "h265".to_string(),
                color_depth: 30,
            },
            Self::Auto => Self::Medium.to_profile(),
        }
    }
}

/// Assessed network condition based on measured metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum NetworkCondition {
    /// Outstanding network quality.
    Excellent,
    /// Good for most workloads.
    Good,
    /// Acceptable but may require quality reduction.
    Fair,
    /// Degraded, significant quality reduction needed.
    Poor,
    /// Nearly unusable.
    Critical,
}

impl std::fmt::Display for NetworkCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Excellent => write!(f, "excellent"),
            Self::Good => write!(f, "good"),
            Self::Fair => write!(f, "fair"),
            Self::Poor => write!(f, "poor"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

impl NetworkCondition {
    /// Classify network condition from raw metrics.
    #[must_use]
    pub fn from_metrics(latency_ms: f32, packet_loss: f32, bandwidth_kbps: u32) -> Self {
        if latency_ms > 200.0 || packet_loss > 10.0 || bandwidth_kbps < 500 {
            return Self::Critical;
        }
        if latency_ms > 100.0 || packet_loss > 5.0 || bandwidth_kbps < 1_000 {
            return Self::Poor;
        }
        if latency_ms > 50.0 || packet_loss > 2.0 || bandwidth_kbps < 3_000 {
            return Self::Fair;
        }
        if latency_ms > 20.0 || packet_loss > 0.5 || bandwidth_kbps < 8_000 {
            return Self::Good;
        }
        Self::Excellent
    }
}

/// Adaptively adjusts quality based on measured network conditions.
pub struct AdaptiveQuality {
    current_preset: QualityPreset,
    condition: NetworkCondition,
    latency_ms: f32,
    packet_loss: f32,
    bandwidth_kbps: u32,
}

impl AdaptiveQuality {
    /// Create a new adaptive quality controller starting at the given preset.
    #[must_use]
    pub fn new(initial_preset: QualityPreset) -> Self {
        Self {
            current_preset: initial_preset,
            condition: NetworkCondition::Good,
            latency_ms: 0.0,
            packet_loss: 0.0,
            bandwidth_kbps: 0,
        }
    }

    /// Update measured network metrics and re-assess condition.
    pub fn update_metrics(&mut self, latency_ms: f32, packet_loss: f32, bandwidth_kbps: u32) {
        self.latency_ms = latency_ms;
        self.packet_loss = packet_loss;
        self.bandwidth_kbps = bandwidth_kbps;
        self.condition = NetworkCondition::from_metrics(latency_ms, packet_loss, bandwidth_kbps);
    }

    /// Current network condition.
    #[must_use]
    pub fn condition(&self) -> NetworkCondition {
        self.condition
    }

    /// Current quality profile after adaptive adjustments.
    #[must_use]
    pub fn current_profile(&self) -> QualityProfile {
        self.current_preset.to_profile()
    }

    /// Current quality preset.
    #[must_use]
    pub fn current_preset(&self) -> QualityPreset {
        self.current_preset
    }

    /// Adjust quality up or down based on the current network condition.
    /// Returns `true` if the quality level changed.
    pub fn adjust(&mut self) -> bool {
        let new_preset = match self.condition {
            NetworkCondition::Excellent => QualityPreset::High,
            NetworkCondition::Good => QualityPreset::Medium,
            NetworkCondition::Fair => QualityPreset::Medium,
            NetworkCondition::Poor => QualityPreset::Low,
            NetworkCondition::Critical => QualityPreset::Low,
        };

        if new_preset != self.current_preset {
            self.current_preset = new_preset;
            true
        } else {
            false
        }
    }

    /// Last measured latency in milliseconds.
    #[must_use]
    pub fn latency_ms(&self) -> f32 {
        self.latency_ms
    }

    /// Last measured packet loss percentage.
    #[must_use]
    pub fn packet_loss(&self) -> f32 {
        self.packet_loss
    }

    /// Last measured bandwidth in kbps.
    #[must_use]
    pub fn bandwidth_kbps(&self) -> u32 {
        self.bandwidth_kbps
    }
}

impl Default for AdaptiveQuality {
    fn default() -> Self {
        Self::new(QualityPreset::Auto)
    }
}
