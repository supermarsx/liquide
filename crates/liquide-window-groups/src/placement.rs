//! Smart window placement algorithms.
//!
//! Provides several strategies for determining where to place new windows
//! on screen, including overlap minimization, cascading, and gap-finding.

/// A rectangle representing a screen region or window bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// Create a new rectangle.
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns the right edge (exclusive).
    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    /// Returns the bottom edge (exclusive).
    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }

    /// Returns the area of this rectangle.
    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Returns true if this rectangle intersects with another.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Returns the overlap area between this rectangle and another.
    pub fn overlap_area(&self, other: &Rect) -> u64 {
        let ox = (self.right().min(other.right()) - self.x.max(other.x)).max(0) as u64;
        let oy = (self.bottom().min(other.bottom()) - self.y.max(other.y)).max(0) as u64;
        ox * oy
    }

    /// Returns true if this rectangle is fully contained within another.
    pub fn contained_in(&self, outer: &Rect) -> bool {
        self.x >= outer.x
            && self.y >= outer.y
            && self.right() <= outer.right()
            && self.bottom() <= outer.bottom()
    }
}

/// A reserved screen area (panel, dock, etc.) that windows should avoid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strut {
    /// Which edge of the screen this strut is on.
    pub edge: StrutEdge,
    /// Thickness of the strut in pixels.
    pub size: u32,
}

/// Which screen edge a strut occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrutEdge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Strut {
    /// Create a new strut.
    pub fn new(edge: StrutEdge, size: u32) -> Self {
        Self { edge, size }
    }
}

/// Compute the usable (work) area of a screen after subtracting struts.
pub fn work_area(screen: &Rect, struts: &[Strut]) -> Rect {
    let mut x = screen.x;
    let mut y = screen.y;
    let mut w = screen.width;
    let mut h = screen.height;

    for strut in struts {
        match strut.edge {
            StrutEdge::Top => {
                let s = strut.size.min(h);
                y += s as i32;
                h -= s;
            }
            StrutEdge::Bottom => {
                h = h.saturating_sub(strut.size);
            }
            StrutEdge::Left => {
                let s = strut.size.min(w);
                x += s as i32;
                w -= s;
            }
            StrutEdge::Right => {
                w = w.saturating_sub(strut.size);
            }
        }
    }

    Rect::new(x, y, w, h)
}

/// Window placement strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementStrategy {
    /// Minimize overlap with existing windows using a grid scan.
    Smart,
    /// Cascade windows at a fixed offset from each other.
    Cascade,
    /// Center the window on screen.
    Center,
    /// Random position within the work area.
    Random,
    /// Place the window near the mouse cursor.
    UnderMouse,
    /// Find the first gap large enough for the window.
    FirstAvailable,
}

/// Configuration for window placement.
#[derive(Debug, Clone)]
pub struct PlacementConfig {
    /// Which strategy to use.
    pub strategy: PlacementStrategy,
    /// Offset for cascade placement (dx, dy).
    pub cascade_offset: (i32, i32),
    /// Whether to respect struts (reserved panel areas).
    pub respect_struts: bool,
    /// Minimum gap between windows for smart placement.
    pub min_gap: i32,
    /// Grid cell size for the smart placement scan (smaller = more precise but slower).
    pub grid_step: u32,
}

impl Default for PlacementConfig {
    fn default() -> Self {
        Self {
            strategy: PlacementStrategy::Smart,
            cascade_offset: (30, 30),
            respect_struts: true,
            min_gap: 8,
            grid_step: 16,
        }
    }
}

/// Place a window using the smart overlap-minimization algorithm.
///
/// Scans a grid over the work area and finds the position where the window
/// overlaps least with existing windows, preferring positions closer to
/// the top-left.
pub fn smart_place(
    window_size: (u32, u32),
    existing: &[Rect],
    screen: &Rect,
    struts: &[Strut],
    config: &PlacementConfig,
) -> (i32, i32) {
    let area = if config.respect_struts {
        work_area(screen, struts)
    } else {
        *screen
    };

    let (ww, wh) = window_size;

    // If the window doesn't fit at all, just center it.
    if ww as i32 > area.width as i32 || wh as i32 > area.height as i32 {
        return center_place(window_size, screen, struts, config);
    }

    let step = config.grid_step.max(1) as i32;
    let max_x = area.x + area.width as i32 - ww as i32;
    let max_y = area.y + area.height as i32 - wh as i32;

    let mut best_x = area.x;
    let mut best_y = area.y;
    let mut best_overlap: u64 = u64::MAX;

    let mut cy = area.y;
    while cy <= max_y {
        let mut cx = area.x;
        while cx <= max_x {
            let candidate = Rect::new(cx, cy, ww, wh);
            let total_overlap: u64 = existing.iter().map(|r| candidate.overlap_area(r)).sum();

            if total_overlap < best_overlap {
                best_overlap = total_overlap;
                best_x = cx;
                best_y = cy;
                if total_overlap == 0 {
                    return (best_x, best_y);
                }
            }
            cx += step;
        }
        cy += step;
    }

    (best_x, best_y)
}

/// Place a window using cascading: each successive window is offset by
/// `cascade_offset` from the previous one.
pub fn cascade_place(
    index: usize,
    window_size: (u32, u32),
    screen: &Rect,
    struts: &[Strut],
    config: &PlacementConfig,
) -> (i32, i32) {
    let area = if config.respect_struts {
        work_area(screen, struts)
    } else {
        *screen
    };

    let (dx, dy) = config.cascade_offset;
    let offset_x = (index as i32) * dx;
    let offset_y = (index as i32) * dy;

    let max_x = area.x + area.width as i32 - window_size.0 as i32;
    let max_y = area.y + area.height as i32 - window_size.1 as i32;

    // Wrap around if we exceed the work area.
    let wrap_x = if max_x > area.x {
        (max_x - area.x) + 1
    } else {
        1
    };
    let wrap_y = if max_y > area.y {
        (max_y - area.y) + 1
    } else {
        1
    };

    let x = area.x + (offset_x % wrap_x);
    let y = area.y + (offset_y % wrap_y);

    (x.min(max_x).max(area.x), y.min(max_y).max(area.y))
}

/// Center a window on the screen (respecting struts if configured).
pub fn center_place(
    window_size: (u32, u32),
    screen: &Rect,
    struts: &[Strut],
    config: &PlacementConfig,
) -> (i32, i32) {
    let area = if config.respect_struts {
        work_area(screen, struts)
    } else {
        *screen
    };

    let x = area.x + (area.width as i32 - window_size.0 as i32) / 2;
    let y = area.y + (area.height as i32 - window_size.1 as i32) / 2;
    (x, y)
}

/// Place a window near the mouse cursor, adjusting to keep it on-screen.
pub fn under_mouse_place(
    window_size: (u32, u32),
    mouse: (i32, i32),
    screen: &Rect,
    struts: &[Strut],
    config: &PlacementConfig,
) -> (i32, i32) {
    let area = if config.respect_struts {
        work_area(screen, struts)
    } else {
        *screen
    };

    let (mx, my) = mouse;
    let (ww, wh) = window_size;

    // Try to center the window on the cursor.
    let mut x = mx - ww as i32 / 2;
    let mut y = my - wh as i32 / 2;

    // Clamp to work area.
    let max_x = area.x + area.width as i32 - ww as i32;
    let max_y = area.y + area.height as i32 - wh as i32;
    x = x.clamp(area.x, max_x.max(area.x));
    y = y.clamp(area.y, max_y.max(area.y));

    (x, y)
}

/// Find the first gap large enough for the window, scanning left-to-right,
/// top-to-bottom.
pub fn first_available_place(
    window_size: (u32, u32),
    existing: &[Rect],
    screen: &Rect,
    struts: &[Strut],
    config: &PlacementConfig,
) -> (i32, i32) {
    let area = if config.respect_struts {
        work_area(screen, struts)
    } else {
        *screen
    };

    let (ww, wh) = window_size;
    let step = config.grid_step.max(1) as i32;
    let gap = config.min_gap;
    let max_x = area.x + area.width as i32 - ww as i32;
    let max_y = area.y + area.height as i32 - wh as i32;

    let mut cy = area.y;
    while cy <= max_y {
        let mut cx = area.x;
        while cx <= max_x {
            let candidate = Rect::new(cx, cy, ww + gap as u32 * 2, wh + gap as u32 * 2);
            let fits = !existing.iter().any(|r| candidate.intersects(r));
            if fits {
                return (cx + gap, cy + gap);
            }
            cx += step;
        }
        cy += step;
    }

    // Fallback: use smart placement if no clear gap found.
    smart_place(window_size, existing, screen, struts, config)
}

/// Place a window using the configured strategy.
pub fn place_window(
    window_size: (u32, u32),
    existing: &[Rect],
    screen: &Rect,
    struts: &[Strut],
    config: &PlacementConfig,
    cascade_index: usize,
    mouse_pos: Option<(i32, i32)>,
) -> (i32, i32) {
    match config.strategy {
        PlacementStrategy::Smart => smart_place(window_size, existing, screen, struts, config),
        PlacementStrategy::Cascade => {
            cascade_place(cascade_index, window_size, screen, struts, config)
        }
        PlacementStrategy::Center => center_place(window_size, screen, struts, config),
        PlacementStrategy::Random => {
            // Deterministic "random" based on cascade_index for testability.
            let area = if config.respect_struts {
                work_area(screen, struts)
            } else {
                *screen
            };
            let max_x = (area.width as i32 - window_size.0 as i32).max(0);
            let max_y = (area.height as i32 - window_size.1 as i32).max(0);
            let hash = (cascade_index.wrapping_mul(2654435761)) as u32;
            let x = area.x + (hash % (max_x as u32 + 1)) as i32;
            let y = area.y + ((hash >> 16) % (max_y as u32 + 1)) as i32;
            (x, y)
        }
        PlacementStrategy::UnderMouse => {
            let mouse = mouse_pos.unwrap_or_else(|| {
                // Fallback to center if no mouse position.
                let area = if config.respect_struts {
                    work_area(screen, struts)
                } else {
                    *screen
                };
                (
                    area.x + area.width as i32 / 2,
                    area.y + area.height as i32 / 2,
                )
            });
            under_mouse_place(window_size, mouse, screen, struts, config)
        }
        PlacementStrategy::FirstAvailable => {
            first_available_place(window_size, existing, screen, struts, config)
        }
    }
}
