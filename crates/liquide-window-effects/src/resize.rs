use crate::effects::Rect;

/// Which handle of a window is being dragged for resizing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHandle {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Constraints applied during an interactive resize.
#[derive(Debug, Clone)]
pub struct ResizeConstraints {
    pub min_width: f32,
    pub min_height: f32,
    pub max_width: f32,
    pub max_height: f32,
    /// If set, width/height ratio is locked.
    pub aspect_ratio: Option<f32>,
    /// If set, width and height snap to multiples of this value.
    pub snap_to_grid: Option<f32>,
}

impl Default for ResizeConstraints {
    fn default() -> Self {
        Self {
            min_width: 200.0,
            min_height: 150.0,
            max_width: f32::MAX,
            max_height: f32::MAX,
            aspect_ratio: None,
            snap_to_grid: None,
        }
    }
}

/// Compute a new window rectangle after a resize drag of `(dx, dy)` pixels
/// from a given `handle`, applying `constraints`.
pub fn constrain_resize(
    handle: ResizeHandle,
    dx: f32,
    dy: f32,
    current: Rect,
    constraints: &ResizeConstraints,
) -> Rect {
    let mut x = current.x;
    let mut y = current.y;
    let mut w = current.width;
    let mut h = current.height;

    // Apply delta based on handle
    match handle {
        ResizeHandle::Right => {
            w += dx;
        }
        ResizeHandle::Left => {
            x += dx;
            w -= dx;
        }
        ResizeHandle::Bottom => {
            h += dy;
        }
        ResizeHandle::Top => {
            y += dy;
            h -= dy;
        }
        ResizeHandle::TopLeft => {
            x += dx;
            w -= dx;
            y += dy;
            h -= dy;
        }
        ResizeHandle::TopRight => {
            w += dx;
            y += dy;
            h -= dy;
        }
        ResizeHandle::BottomLeft => {
            x += dx;
            w -= dx;
            h += dy;
        }
        ResizeHandle::BottomRight => {
            w += dx;
            h += dy;
        }
    }

    // Clamp to min/max
    if w < constraints.min_width {
        let diff = constraints.min_width - w;
        w = constraints.min_width;
        // If left edge was moving, push it back
        if matches!(
            handle,
            ResizeHandle::Left | ResizeHandle::TopLeft | ResizeHandle::BottomLeft
        ) {
            x -= diff;
        }
    }
    if h < constraints.min_height {
        let diff = constraints.min_height - h;
        h = constraints.min_height;
        if matches!(
            handle,
            ResizeHandle::Top | ResizeHandle::TopLeft | ResizeHandle::TopRight
        ) {
            y -= diff;
        }
    }
    if w > constraints.max_width {
        w = constraints.max_width;
    }
    if h > constraints.max_height {
        h = constraints.max_height;
    }

    // Aspect ratio enforcement
    if let Some(ratio) = constraints.aspect_ratio {
        // ratio = width / height
        let desired_h = w / ratio;
        if desired_h < h {
            // Width is the constraining dimension — adjust height
            h = desired_h;
        } else {
            // Height is the constraining dimension — adjust width
            w = h * ratio;
        }
        // Re-clamp after ratio adjustment
        w = w.clamp(constraints.min_width, constraints.max_width);
        h = h.clamp(constraints.min_height, constraints.max_height);
    }

    // Grid snapping
    if let Some(grid) = constraints.snap_to_grid {
        if grid > 0.0 {
            w = (w / grid).round() * grid;
            h = (h / grid).round() * grid;
            w = w.max(constraints.min_width);
            h = h.max(constraints.min_height);
        }
    }

    Rect::new(x, y, w, h)
}

/// Return the CSS cursor name appropriate for a resize handle.
pub fn resize_cursor(handle: ResizeHandle) -> &'static str {
    match handle {
        ResizeHandle::Top => "n-resize",
        ResizeHandle::Bottom => "s-resize",
        ResizeHandle::Left => "w-resize",
        ResizeHandle::Right => "e-resize",
        ResizeHandle::TopLeft => "nw-resize",
        ResizeHandle::TopRight => "ne-resize",
        ResizeHandle::BottomLeft => "sw-resize",
        ResizeHandle::BottomRight => "se-resize",
    }
}

/// Tracks state for an ongoing interactive resize.
pub struct LiveResize {
    active: bool,
    handle: ResizeHandle,
    initial_rect: Rect,
    accumulated_dx: f32,
    accumulated_dy: f32,
}

impl LiveResize {
    pub fn new() -> Self {
        Self {
            active: false,
            handle: ResizeHandle::BottomRight,
            initial_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            accumulated_dx: 0.0,
            accumulated_dy: 0.0,
        }
    }

    /// Begin a resize operation.
    pub fn begin(&mut self, handle: ResizeHandle, window: Rect) {
        self.active = true;
        self.handle = handle;
        self.initial_rect = window;
        self.accumulated_dx = 0.0;
        self.accumulated_dy = 0.0;
    }

    /// Accumulate drag deltas and return the constrained result.
    pub fn update(&mut self, dx: f32, dy: f32, constraints: &ResizeConstraints) -> Rect {
        self.accumulated_dx += dx;
        self.accumulated_dy += dy;
        constrain_resize(
            self.handle,
            self.accumulated_dx,
            self.accumulated_dy,
            self.initial_rect,
            constraints,
        )
    }

    /// End the resize and return the final rectangle, or `None` if not active.
    pub fn end(&mut self) -> Option<Rect> {
        if !self.active {
            return None;
        }
        self.active = false;
        Some(constrain_resize(
            self.handle,
            self.accumulated_dx,
            self.accumulated_dy,
            self.initial_rect,
            &ResizeConstraints::default(),
        ))
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Default for LiveResize {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_rect() -> Rect {
        Rect::new(100.0, 100.0, 800.0, 600.0)
    }

    // ── constrain_resize ────────────────────────────────────────────

    #[test]
    fn resize_right_increases_width() {
        let r = constrain_resize(
            ResizeHandle::Right,
            50.0,
            0.0,
            base_rect(),
            &ResizeConstraints::default(),
        );
        assert!((r.width - 850.0).abs() < 1e-3);
        assert!((r.x - 100.0).abs() < 1e-3); // x unchanged
    }

    #[test]
    fn resize_left_moves_x() {
        let r = constrain_resize(
            ResizeHandle::Left,
            -30.0,
            0.0,
            base_rect(),
            &ResizeConstraints::default(),
        );
        assert!((r.x - 70.0).abs() < 1e-3);
        assert!((r.width - 830.0).abs() < 1e-3);
    }

    #[test]
    fn resize_bottom_increases_height() {
        let r = constrain_resize(
            ResizeHandle::Bottom,
            0.0,
            40.0,
            base_rect(),
            &ResizeConstraints::default(),
        );
        assert!((r.height - 640.0).abs() < 1e-3);
    }

    #[test]
    fn resize_top_moves_y() {
        let r = constrain_resize(
            ResizeHandle::Top,
            0.0,
            -20.0,
            base_rect(),
            &ResizeConstraints::default(),
        );
        assert!((r.y - 80.0).abs() < 1e-3);
        assert!((r.height - 620.0).abs() < 1e-3);
    }

    #[test]
    fn resize_bottom_right() {
        let r = constrain_resize(
            ResizeHandle::BottomRight,
            50.0,
            30.0,
            base_rect(),
            &ResizeConstraints::default(),
        );
        assert!((r.width - 850.0).abs() < 1e-3);
        assert!((r.height - 630.0).abs() < 1e-3);
        assert!((r.x - 100.0).abs() < 1e-3);
        assert!((r.y - 100.0).abs() < 1e-3);
    }

    #[test]
    fn resize_top_left() {
        let r = constrain_resize(
            ResizeHandle::TopLeft,
            -20.0,
            -15.0,
            base_rect(),
            &ResizeConstraints::default(),
        );
        assert!((r.x - 80.0).abs() < 1e-3);
        assert!((r.y - 85.0).abs() < 1e-3);
        assert!((r.width - 820.0).abs() < 1e-3);
        assert!((r.height - 615.0).abs() < 1e-3);
    }

    #[test]
    fn resize_constrain_min_width() {
        let c = ResizeConstraints {
            min_width: 300.0,
            ..Default::default()
        };
        let r = constrain_resize(ResizeHandle::Right, -700.0, 0.0, base_rect(), &c);
        assert!((r.width - 300.0).abs() < 1e-3);
    }

    #[test]
    fn resize_constrain_min_height() {
        let c = ResizeConstraints {
            min_height: 200.0,
            ..Default::default()
        };
        let r = constrain_resize(ResizeHandle::Bottom, 0.0, -500.0, base_rect(), &c);
        assert!((r.height - 200.0).abs() < 1e-3);
    }

    #[test]
    fn resize_constrain_max_width() {
        let c = ResizeConstraints {
            max_width: 1000.0,
            ..Default::default()
        };
        let r = constrain_resize(ResizeHandle::Right, 500.0, 0.0, base_rect(), &c);
        assert!((r.width - 1000.0).abs() < 1e-3);
    }

    #[test]
    fn resize_aspect_ratio_lock() {
        let c = ResizeConstraints {
            aspect_ratio: Some(16.0 / 9.0),
            ..Default::default()
        };
        let r = constrain_resize(ResizeHandle::BottomRight, 100.0, 100.0, base_rect(), &c);
        let ratio = r.width / r.height;
        assert!((ratio - 16.0 / 9.0).abs() < 0.01);
    }

    #[test]
    fn resize_grid_snapping() {
        let c = ResizeConstraints {
            snap_to_grid: Some(50.0),
            ..Default::default()
        };
        let r = constrain_resize(ResizeHandle::BottomRight, 23.0, 17.0, base_rect(), &c);
        assert!((r.width % 50.0).abs() < 1e-3);
        assert!((r.height % 50.0).abs() < 1e-3);
    }

    #[test]
    fn resize_top_right_handle() {
        let r = constrain_resize(
            ResizeHandle::TopRight,
            40.0,
            -20.0,
            base_rect(),
            &ResizeConstraints::default(),
        );
        assert!((r.width - 840.0).abs() < 1e-3);
        assert!((r.y - 80.0).abs() < 1e-3);
        assert!((r.height - 620.0).abs() < 1e-3);
        assert!((r.x - 100.0).abs() < 1e-3); // x unchanged
    }

    #[test]
    fn resize_bottom_left_handle() {
        let r = constrain_resize(
            ResizeHandle::BottomLeft,
            -25.0,
            35.0,
            base_rect(),
            &ResizeConstraints::default(),
        );
        assert!((r.x - 75.0).abs() < 1e-3);
        assert!((r.width - 825.0).abs() < 1e-3);
        assert!((r.height - 635.0).abs() < 1e-3);
    }

    // ── resize_cursor ───────────────────────────────────────────────

    #[test]
    fn cursor_names_all_handles() {
        assert_eq!(resize_cursor(ResizeHandle::Top), "n-resize");
        assert_eq!(resize_cursor(ResizeHandle::Bottom), "s-resize");
        assert_eq!(resize_cursor(ResizeHandle::Left), "w-resize");
        assert_eq!(resize_cursor(ResizeHandle::Right), "e-resize");
        assert_eq!(resize_cursor(ResizeHandle::TopLeft), "nw-resize");
        assert_eq!(resize_cursor(ResizeHandle::TopRight), "ne-resize");
        assert_eq!(resize_cursor(ResizeHandle::BottomLeft), "sw-resize");
        assert_eq!(resize_cursor(ResizeHandle::BottomRight), "se-resize");
    }

    // ── LiveResize ──────────────────────────────────────────────────

    #[test]
    fn live_resize_workflow() {
        let mut lr = LiveResize::new();
        assert!(!lr.is_active());

        lr.begin(ResizeHandle::BottomRight, base_rect());
        assert!(lr.is_active());

        let r = lr.update(30.0, 20.0, &ResizeConstraints::default());
        assert!((r.width - 830.0).abs() < 1e-3);
        assert!((r.height - 620.0).abs() < 1e-3);

        // Accumulates
        let r = lr.update(10.0, 5.0, &ResizeConstraints::default());
        assert!((r.width - 840.0).abs() < 1e-3);
        assert!((r.height - 625.0).abs() < 1e-3);

        let final_r = lr.end().unwrap();
        assert!(!lr.is_active());
        assert!((final_r.width - 840.0).abs() < 1e-3);
    }

    #[test]
    fn live_resize_end_when_inactive() {
        let mut lr = LiveResize::new();
        assert!(lr.end().is_none());
    }

    #[test]
    fn live_resize_default() {
        let lr = LiveResize::default();
        assert!(!lr.is_active());
    }
}
