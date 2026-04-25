//! Level of Detail (LOD) management for CPU rendering.
//!
//! Automatically selects appropriate detail levels for rendered objects
//! based on distance, size, and performance constraints.

use liquide_compositor::geometry::Rect;

/// Level of detail for rendering an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LodLevel {
    /// Full detail rendering.
    High,
    /// Medium detail (some simplifications).
    Medium,
    /// Low detail (highly simplified).
    Low,
    /// Minimal detail (flat rendering, no effects).
    Minimal,
}

impl LodLevel {
    /// Get the quality factor for this LOD level (0.0 = minimal, 1.0 = high).
    #[must_use]
    pub fn quality_factor(&self) -> f32 {
        match self {
            Self::High => 1.0,
            Self::Medium => 0.6,
            Self::Low => 0.3,
            Self::Minimal => 0.1,
        }
    }

    /// Get the pixel size threshold for this LOD level.
    #[must_use]
    pub fn pixel_threshold(&self) -> f32 {
        match self {
            Self::High => 256.0,   // > 256px on screen
            Self::Medium => 128.0, // > 128px on screen
            Self::Low => 32.0,     // > 32px on screen
            Self::Minimal => 0.0,  // < 32px on screen
        }
    }
}

/// Criteria for determining appropriate LOD level.
#[derive(Debug, Clone, Copy)]
pub struct LodCriteria {
    /// Screen-space bounding box of the object.
    pub screen_bounds: Rect,
    /// Distance from camera/viewport center (normalized 0.0-1.0).
    pub distance: f32,
    /// Whether the object is currently visible on screen.
    pub visible: bool,
    /// Performance mode (affects LOD thresholds).
    pub performance_mode: PerformanceMode,
}

/// Performance mode affects LOD selection thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceMode {
    /// Quality preferred over performance.
    Quality,
    /// Balanced quality and performance.
    Balanced,
    /// Performance preferred over quality.
    Performance,
}

impl PerformanceMode {
    /// Get the LOD bias for this performance mode.
    /// Higher bias = prefer lower LOD levels.
    #[must_use]
    fn lod_bias(&self) -> f32 {
        match self {
            Self::Quality => 0.0,
            Self::Balanced => 0.5,
            Self::Performance => 1.0,
        }
    }
}

/// Manages LOD selection for rendered objects.
pub struct LodManager {
    /// Current performance mode.
    performance_mode: PerformanceMode,
    /// Viewport dimensions for screen-space calculations.
    viewport_width: f32,
    viewport_height: f32,
    /// Adaptive LOD: automatically downgrades quality under high load.
    adaptive_enabled: bool,
    /// Current adaptive bias (0.0 = no bias, 1.0 = maximum downgrade).
    adaptive_bias: f32,
}

impl LodManager {
    /// Create a new LOD manager.
    #[must_use]
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            performance_mode: PerformanceMode::Balanced,
            viewport_width,
            viewport_height,
            adaptive_enabled: true,
            adaptive_bias: 0.0,
        }
    }

    /// Set the performance mode.
    pub fn set_performance_mode(&mut self, mode: PerformanceMode) {
        self.performance_mode = mode;
    }

    /// Get the current performance mode.
    #[must_use]
    pub fn get_performance_mode(&self) -> PerformanceMode {
        self.performance_mode
    }

    /// Update viewport dimensions.
    pub fn resize(&mut self, width: f32, height: f32) {
        self.viewport_width = width;
        self.viewport_height = height;
    }

    /// Enable or disable adaptive LOD.
    pub fn set_adaptive_enabled(&mut self, enabled: bool) {
        self.adaptive_enabled = enabled;
        if !enabled {
            self.adaptive_bias = 0.0;
        }
    }

    /// Update adaptive bias based on current frame time.
    /// Higher frame times increase bias (lower quality).
    pub fn update_adaptive_bias(&mut self, frame_time_ms: f64, target_time_ms: f64) {
        if !self.adaptive_enabled {
            return;
        }

        let ratio = frame_time_ms / target_time_ms;

        if ratio > 1.5 {
            // Frame time significantly over budget — increase bias
            self.adaptive_bias = (self.adaptive_bias + 0.1).min(1.0);
        } else if ratio < 0.8 {
            // Frame time well under budget — decrease bias
            self.adaptive_bias = (self.adaptive_bias - 0.05).max(0.0);
        }
    }

    /// Determine the appropriate LOD level for an object.
    #[must_use]
    pub fn select_lod(&self, criteria: &LodCriteria) -> LodLevel {
        if !criteria.visible {
            return LodLevel::Minimal;
        }

        // Calculate screen-space size
        let screen_size = criteria
            .screen_bounds
            .width
            .max(criteria.screen_bounds.height);

        // Apply performance mode bias
        let mode_bias = self.performance_mode.lod_bias();
        let total_bias = mode_bias + self.adaptive_bias;

        // Adjust thresholds based on bias (higher bias = lower thresholds = lower quality)
        let bias_factor = 1.0 / (1.0 + total_bias);
        let high_threshold = 256.0 * bias_factor;
        let medium_threshold = 128.0 * bias_factor;
        let low_threshold = 32.0 * bias_factor; // Lowered from 64 to ensure small UI elements render

        // Select LOD based on screen size and distance
        if screen_size >= high_threshold && criteria.distance < 0.3 {
            LodLevel::High
        } else if screen_size >= medium_threshold && criteria.distance < 0.6 {
            LodLevel::Medium
        } else if screen_size >= low_threshold {
            LodLevel::Low
        } else {
            LodLevel::Minimal
        }
    }

    /// Calculate distance from viewport center (normalized 0.0-1.0).
    #[must_use]
    pub fn calculate_distance_from_center(&self, bounds: &Rect) -> f32 {
        let center_x = self.viewport_width / 2.0;
        let center_y = self.viewport_height / 2.0;

        let obj_center_x = bounds.x + bounds.width / 2.0;
        let obj_center_y = bounds.y + bounds.height / 2.0;

        let dx = obj_center_x - center_x;
        let dy = obj_center_y - center_y;

        let distance = (dx * dx + dy * dy).sqrt();
        let max_distance = (self.viewport_width * self.viewport_width
            + self.viewport_height * self.viewport_height)
            .sqrt()
            / 2.0;

        (distance / max_distance).min(1.0)
    }

    /// Get statistics about LOD management.
    #[must_use]
    pub fn stats(&self) -> LodStats {
        LodStats {
            performance_mode: self.performance_mode,
            adaptive_enabled: self.adaptive_enabled,
            adaptive_bias: self.adaptive_bias,
            viewport_width: self.viewport_width,
            viewport_height: self.viewport_height,
        }
    }
}

/// Statistics about LOD management.
#[derive(Debug, Clone, Copy)]
pub struct LodStats {
    pub performance_mode: PerformanceMode,
    pub adaptive_enabled: bool,
    pub adaptive_bias: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lod_selection_by_size() {
        let manager = LodManager::new(1920.0, 1080.0);

        // Large object close to center
        let large_criteria = LodCriteria {
            screen_bounds: Rect::new(100.0, 100.0, 300.0, 300.0),
            distance: 0.2,
            visible: true,
            performance_mode: PerformanceMode::Balanced,
        };
        assert_eq!(manager.select_lod(&large_criteria), LodLevel::High);

        // Medium object
        let medium_criteria = LodCriteria {
            screen_bounds: Rect::new(100.0, 100.0, 150.0, 150.0),
            distance: 0.4,
            visible: true,
            performance_mode: PerformanceMode::Balanced,
        };
        assert_eq!(manager.select_lod(&medium_criteria), LodLevel::Medium);

        // Small object
        let small_criteria = LodCriteria {
            screen_bounds: Rect::new(100.0, 100.0, 50.0, 50.0),
            distance: 0.7,
            visible: true,
            performance_mode: PerformanceMode::Balanced,
        };
        assert_eq!(manager.select_lod(&small_criteria), LodLevel::Low);
    }

    #[test]
    fn test_lod_culling_invisible() {
        let manager = LodManager::new(1920.0, 1080.0);

        let criteria = LodCriteria {
            screen_bounds: Rect::new(100.0, 100.0, 300.0, 300.0),
            distance: 0.2,
            visible: false,
            performance_mode: PerformanceMode::Balanced,
        };

        assert_eq!(manager.select_lod(&criteria), LodLevel::Minimal);
    }

    #[test]
    fn test_lod_performance_mode_bias() {
        let mut manager = LodManager::new(1920.0, 1080.0);

        let criteria = LodCriteria {
            screen_bounds: Rect::new(100.0, 100.0, 200.0, 200.0),
            distance: 0.4,
            visible: true,
            performance_mode: PerformanceMode::Balanced,
        };

        manager.set_performance_mode(PerformanceMode::Quality);
        let quality_lod = manager.select_lod(&criteria);

        manager.set_performance_mode(PerformanceMode::Performance);
        let perf_lod = manager.select_lod(&criteria);

        // Performance mode should select lower LOD
        assert!(quality_lod as u8 <= perf_lod as u8);
    }

    #[test]
    fn test_lod_adaptive_bias() {
        let mut manager = LodManager::new(1920.0, 1080.0);
        manager.set_adaptive_enabled(true);

        // Simulate high frame time (over budget)
        manager.update_adaptive_bias(25.0, 16.0);

        assert!(manager.adaptive_bias > 0.0);

        // Simulate low frame time (under budget)
        for _ in 0..10 {
            manager.update_adaptive_bias(10.0, 16.0);
        }

        assert!(manager.adaptive_bias < 0.1);
    }

    #[test]
    fn test_distance_calculation() {
        let manager = LodManager::new(1920.0, 1080.0);

        // Object at viewport center
        let center_bounds = Rect::new(910.0, 520.0, 100.0, 40.0);
        let distance = manager.calculate_distance_from_center(&center_bounds);
        assert!(distance < 0.1);

        // Object at corner
        let corner_bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let distance = manager.calculate_distance_from_center(&corner_bounds);
        assert!(distance > 0.5);
    }
}
