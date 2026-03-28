/// Mouse wheel scroll configuration and handling.
#[derive(Debug, Clone)]
pub struct WheelConfig {
    /// Number of lines to scroll per wheel tick.
    pub lines_per_tick: f32,
    /// Pixels per "line" of scrolling.
    pub line_height: f32,
    /// If true, Shift+wheel scrolls by page instead of lines.
    pub page_scroll: bool,
    /// If true, wheel scrolling is animated (smooth).
    pub smooth_wheel: bool,
    /// If true, invert scroll direction (macOS natural scrolling).
    pub natural_scrolling: bool,
}

impl WheelConfig {
    pub fn new() -> Self {
        Self {
            lines_per_tick: 3.0,
            line_height: 20.0,
            page_scroll: true,
            smooth_wheel: true,
            natural_scrolling: false,
        }
    }

    /// Compute the pixel delta for a wheel event.
    ///
    /// `ticks` is the number of wheel ticks (positive = scroll down/right, negative = up/left).
    /// `shift_held` indicates whether Shift is held (for page scrolling).
    /// `viewport_size` is used for page scrolling calculation.
    ///
    /// Returns the pixel delta to scroll.
    pub fn compute_delta(&self, ticks: f32, shift_held: bool, viewport_size: f32) -> f32 {
        let mut delta = if self.page_scroll && shift_held {
            // Page scroll: scroll by viewport height minus a small overlap.
            ticks.signum() * (viewport_size - self.line_height * 2.0).max(self.line_height)
        } else {
            ticks * self.lines_per_tick * self.line_height
        };

        if self.natural_scrolling {
            delta = -delta;
        }

        delta
    }

    /// Default smooth scroll duration for wheel events (ms).
    pub fn smooth_duration_ms(&self) -> u32 {
        if self.smooth_wheel { 200 } else { 0 }
    }
}

impl Default for WheelConfig {
    fn default() -> Self {
        Self::new()
    }
}
