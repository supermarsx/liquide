//! Layout policies for arranging windows.

use liquide_compositor::geometry::Rect;

use crate::window::Window;

/// Trait for window layout policies.
pub trait LayoutPolicy {
    /// Arrange the given windows within the screen rectangle.
    fn arrange(&self, windows: &mut [Window], screen: Rect);

    /// Name of this layout policy.
    fn name(&self) -> &str;
}

/// Floating layout — windows keep their positions (no-op).
pub struct FloatingLayout;

impl LayoutPolicy for FloatingLayout {
    fn arrange(&self, _windows: &mut [Window], _screen: Rect) {}

    fn name(&self) -> &str {
        "floating"
    }
}

/// Tiling layout — arranges windows in a grid.
pub struct TilingLayout {
    pub gap: f32,
    pub max_columns: u32,
}

impl TilingLayout {
    /// Create a new tiling layout.
    #[must_use]
    pub fn new(gap: f32, max_columns: u32) -> Self {
        Self { gap, max_columns }
    }
}

impl LayoutPolicy for TilingLayout {
    fn arrange(&self, windows: &mut [Window], screen: Rect) {
        let count = windows.len();
        if count == 0 {
            return;
        }

        let cols = (count as u32).min(self.max_columns).max(1);
        let rows = (count as u32).div_ceil(cols);

        let total_gap_x = self.gap * (cols as f32 + 1.0);
        let total_gap_y = self.gap * (rows as f32 + 1.0);
        let cell_w = (screen.width - total_gap_x) / cols as f32;
        let cell_h = (screen.height - total_gap_y) / rows as f32;

        for (i, win) in windows.iter_mut().enumerate() {
            let col = (i as u32) % cols;
            let row = (i as u32) / cols;
            let x = screen.x + self.gap + col as f32 * (cell_w + self.gap);
            let y = screen.y + self.gap + row as f32 * (cell_h + self.gap);
            win.bounds = Rect::new(x, y, cell_w, cell_h);
        }
    }

    fn name(&self) -> &str {
        "tiling"
    }
}

/// Stacked/cascading layout — windows overlap with an offset.
pub struct StackedLayout {
    pub offset_x: f32,
    pub offset_y: f32,
    pub initial_x: f32,
    pub initial_y: f32,
    pub default_width: f32,
    pub default_height: f32,
}

impl StackedLayout {
    /// Create with default cascade settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            offset_x: 30.0,
            offset_y: 30.0,
            initial_x: 50.0,
            initial_y: 50.0,
            default_width: 400.0,
            default_height: 300.0,
        }
    }
}

impl Default for StackedLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutPolicy for StackedLayout {
    fn arrange(&self, windows: &mut [Window], _screen: Rect) {
        for (i, win) in windows.iter_mut().enumerate() {
            let x = self.initial_x + i as f32 * self.offset_x;
            let y = self.initial_y + i as f32 * self.offset_y;
            win.bounds = Rect::new(x, y, self.default_width, self.default_height);
        }
    }

    fn name(&self) -> &str {
        "stacked"
    }
}
