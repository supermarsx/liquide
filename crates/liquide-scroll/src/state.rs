/// Per-scrollable-container scroll state.
#[derive(Debug, Clone)]
pub struct ScrollState {
    /// Current scroll offset (x, y). Always clamped to [0, max_scroll].
    pub offset: (f32, f32),
    /// Total content dimensions (width, height).
    pub content_size: (f32, f32),
    /// Visible viewport dimensions (width, height).
    pub viewport_size: (f32, f32),
}

impl ScrollState {
    /// Create a new scroll state with zero offset.
    pub fn new(content_size: (f32, f32), viewport_size: (f32, f32)) -> Self {
        Self {
            offset: (0.0, 0.0),
            content_size,
            viewport_size,
        }
    }

    /// Maximum scroll offset in each axis.
    /// Returns (0,0) if content fits within viewport.
    pub fn max_scroll(&self) -> (f32, f32) {
        (
            (self.content_size.0 - self.viewport_size.0).max(0.0),
            (self.content_size.1 - self.viewport_size.1).max(0.0),
        )
    }

    /// Whether scroll is at the start (top/left) edge.
    pub fn is_at_start(&self) -> (bool, bool) {
        (self.offset.0 <= 0.0, self.offset.1 <= 0.0)
    }

    /// Whether scroll is at the end (bottom/right) edge.
    pub fn is_at_end(&self) -> (bool, bool) {
        let max = self.max_scroll();
        (self.offset.0 >= max.0, self.offset.1 >= max.1)
    }

    /// Scroll progress as a fraction 0.0..=1.0 in each axis.
    /// Returns 0.0 if content fits within viewport (no scrolling possible).
    pub fn scroll_percent(&self) -> (f32, f32) {
        let max = self.max_scroll();
        let px = if max.0 > 0.0 {
            (self.offset.0 / max.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let py = if max.1 > 0.0 {
            (self.offset.1 / max.1).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (px, py)
    }

    /// Clamp current offset to valid range.
    pub fn clamp(&mut self) {
        let max = self.max_scroll();
        self.offset.0 = self.offset.0.clamp(0.0, max.0);
        self.offset.1 = self.offset.1.clamp(0.0, max.1);
    }

    /// Set offset, clamping to valid range.
    pub fn set_offset(&mut self, x: f32, y: f32) {
        self.offset = (x, y);
        self.clamp();
    }

    /// Add delta to offset, clamping to valid range.
    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.offset.0 += dx;
        self.offset.1 += dy;
        self.clamp();
    }

    /// Update content size and re-clamp offset.
    pub fn set_content_size(&mut self, w: f32, h: f32) {
        self.content_size = (w, h);
        self.clamp();
    }

    /// Update viewport size and re-clamp offset.
    pub fn set_viewport_size(&mut self, w: f32, h: f32) {
        self.viewport_size = (w, h);
        self.clamp();
    }

    /// Whether scrolling is possible in horizontal axis.
    pub fn can_scroll_x(&self) -> bool {
        self.content_size.0 > self.viewport_size.0
    }

    /// Whether scrolling is possible in vertical axis.
    pub fn can_scroll_y(&self) -> bool {
        self.content_size.1 > self.viewport_size.1
    }
}
