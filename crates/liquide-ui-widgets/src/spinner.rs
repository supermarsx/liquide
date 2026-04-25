//! Spinner widget: loading/activity indicator.
//!
//! Shows an animated spinning indicator. Available in two modes:
//! - Indeterminate: continuous spin for unknown-duration operations
//! - Determinate: fills based on a progress value

use liquide_ui_core::WidgetId;
use serde::{Deserialize, Serialize};

/// Spinner style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpinnerStyle {
    /// Circular arc spinner (typical loading indicator).
    Circular,
    /// Dotted spinner (series of dots).
    Dots,
    /// Pulsing ring.
    Pulse,
}

impl Default for SpinnerStyle {
    fn default() -> Self {
        Self::Circular
    }
}

/// The spinner widget.
#[derive(Debug)]
pub struct Spinner {
    pub id: WidgetId,
    /// Whether the spinner is currently spinning.
    pub active: bool,
    /// Diameter in pixels.
    pub size: f32,
    /// Stroke width in pixels.
    pub stroke_width: f32,
    /// Visual style.
    pub style: SpinnerStyle,
    /// Current rotation angle in radians (for animation).
    rotation: f32,
    /// Animation speed (radians per second).
    pub speed: f32,
    /// Optional progress value (0.0–1.0) for determinate mode.
    progress: Option<f32>,
    /// Optional label text shown below/beside the spinner.
    pub label: Option<String>,
}

impl Spinner {
    #[must_use]
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            active: true,
            size: 24.0,
            stroke_width: 3.0,
            style: SpinnerStyle::default(),
            rotation: 0.0,
            speed: std::f32::consts::TAU, // One revolution per second
            progress: None,
            label: None,
        }
    }

    #[must_use]
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn with_style(mut self, style: SpinnerStyle) -> Self {
        self.style = style;
        self
    }

    /// Set determinate progress (0.0 to 1.0).
    pub fn set_progress(&mut self, progress: f32) {
        self.progress = Some(progress.clamp(0.0, 1.0));
    }

    /// Clear progress (back to indeterminate mode).
    pub fn set_indeterminate(&mut self) {
        self.progress = None;
    }

    /// Get current progress (None = indeterminate).
    #[must_use]
    pub fn progress(&self) -> Option<f32> {
        self.progress
    }

    /// Is this spinner in determinate mode?
    #[must_use]
    pub fn is_determinate(&self) -> bool {
        self.progress.is_some()
    }

    /// Update the spinner animation by a time delta (in seconds).
    pub fn tick(&mut self, dt: f32) {
        if self.active {
            self.rotation += self.speed * dt;
            if self.rotation > std::f32::consts::TAU {
                self.rotation -= std::f32::consts::TAU;
            }
        }
    }

    /// Current rotation angle in radians.
    #[must_use]
    pub fn rotation(&self) -> f32 {
        self.rotation
    }

    /// Start the spinner.
    pub fn start(&mut self) {
        self.active = true;
    }

    /// Stop the spinner.
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// The arc sweep angle for rendering (varies between indeterminate/determinate).
    #[must_use]
    pub fn sweep_angle(&self) -> f32 {
        match self.progress {
            Some(p) => p * std::f32::consts::TAU,
            None => {
                // Indeterminate: variable-length arc
                let phase = (self.rotation * 0.5).sin();
                let min_sweep = std::f32::consts::FRAC_PI_4;
                let max_sweep = std::f32::consts::PI * 1.5;
                min_sweep + (max_sweep - min_sweep) * (phase * 0.5 + 0.5)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_creation() {
        let sp = Spinner::new(WidgetId::from_raw(1));
        assert!(sp.active);
        assert!(!sp.is_determinate());
        assert_eq!(sp.size, 24.0);
    }

    #[test]
    fn test_spinner_tick() {
        let mut sp = Spinner::new(WidgetId::from_raw(1));
        assert_eq!(sp.rotation(), 0.0);
        sp.tick(0.5); // Half second
        assert!(sp.rotation() > 0.0);
    }

    #[test]
    fn test_determinate_progress() {
        let mut sp = Spinner::new(WidgetId::from_raw(1));
        sp.set_progress(0.5);
        assert!(sp.is_determinate());
        assert_eq!(sp.progress(), Some(0.5));
    }

    #[test]
    fn test_progress_clamping() {
        let mut sp = Spinner::new(WidgetId::from_raw(1));
        sp.set_progress(1.5);
        assert_eq!(sp.progress(), Some(1.0));
        sp.set_progress(-0.5);
        assert_eq!(sp.progress(), Some(0.0));
    }

    #[test]
    fn test_indeterminate_sweep() {
        let sp = Spinner::new(WidgetId::from_raw(1));
        let sweep = sp.sweep_angle();
        assert!(sweep > 0.0);
    }

    #[test]
    fn test_determinate_sweep() {
        let mut sp = Spinner::new(WidgetId::from_raw(1));
        sp.set_progress(0.5);
        let sweep = sp.sweep_angle();
        assert!((sweep - std::f32::consts::PI).abs() < 0.01);
    }

    #[test]
    fn test_start_stop() {
        let mut sp = Spinner::new(WidgetId::from_raw(1));
        sp.stop();
        let r = sp.rotation();
        sp.tick(1.0);
        assert_eq!(sp.rotation(), r); // doesn't advance when stopped
    }
}
