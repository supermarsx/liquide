//! Logical and physical coordinate types with DPI-aware conversions.
//!
//! **Logical** coordinates are density-independent (CSS-like) — they stay the same
//! regardless of the display's DPI.
//!
//! **Physical** coordinates are in actual device pixels — what the GPU rasterises.

use crate::scale::DpiScale;

// ── Size ──────────────────────────────────────────────────────────────

/// A size in logical (density-independent) pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicalSize {
    pub width: f32,
    pub height: f32,
}

/// A size in physical (device) pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

impl LogicalSize {
    #[inline]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Zero-sized.
    #[inline]
    pub const fn zero() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
        }
    }

    /// Area in logical pixels squared.
    #[inline]
    pub fn area(self) -> f32 {
        self.width * self.height
    }

    /// Whether both dimensions are positive.
    #[inline]
    pub fn is_positive(self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }

    /// Convert to physical size using the given scale.
    #[inline]
    pub fn to_physical(self, scale: DpiScale) -> PhysicalSize {
        PhysicalSize {
            width: (self.width * scale.factor()).round() as u32,
            height: (self.height * scale.factor()).round() as u32,
        }
    }

    /// Scale both dimensions uniformly.
    #[inline]
    pub fn scale(self, factor: f32) -> Self {
        Self {
            width: self.width * factor,
            height: self.height * factor,
        }
    }
}

impl std::fmt::Display for LogicalSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{} (logical)", self.width, self.height)
    }
}

impl PhysicalSize {
    #[inline]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Zero-sized.
    #[inline]
    pub const fn zero() -> Self {
        Self {
            width: 0,
            height: 0,
        }
    }

    /// Area in physical pixels.
    #[inline]
    pub fn area(self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Whether both dimensions are positive.
    #[inline]
    pub fn is_positive(self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Convert to logical size using the given scale.
    #[inline]
    pub fn to_logical(self, scale: DpiScale) -> LogicalSize {
        LogicalSize {
            width: self.width as f32 / scale.factor(),
            height: self.height as f32 / scale.factor(),
        }
    }

    /// Total byte count for an RGBA framebuffer at this size.
    #[inline]
    pub fn framebuffer_size_bytes(self, bpp: u32) -> usize {
        (self.width * self.height * bpp) as usize
    }
}

impl std::fmt::Display for PhysicalSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{} (physical)", self.width, self.height)
    }
}

// ── Point ─────────────────────────────────────────────────────────────

/// A point in logical (density-independent) pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicalPoint {
    pub x: f32,
    pub y: f32,
}

/// A point in physical (device) pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PhysicalPoint {
    pub x: i32,
    pub y: i32,
}

impl LogicalPoint {
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub const fn origin() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Convert to physical point using the given scale.
    #[inline]
    pub fn to_physical(self, scale: DpiScale) -> PhysicalPoint {
        PhysicalPoint {
            x: (self.x * scale.factor()).round() as i32,
            y: (self.y * scale.factor()).round() as i32,
        }
    }

    /// Offset this point by a delta.
    #[inline]
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    /// Distance to another point.
    #[inline]
    pub fn distance_to(self, other: LogicalPoint) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

impl std::fmt::Display for LogicalPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}) logical", self.x, self.y)
    }
}

impl PhysicalPoint {
    #[inline]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub const fn origin() -> Self {
        Self { x: 0, y: 0 }
    }

    /// Convert to logical point using the given scale.
    #[inline]
    pub fn to_logical(self, scale: DpiScale) -> LogicalPoint {
        LogicalPoint {
            x: self.x as f32 / scale.factor(),
            y: self.y as f32 / scale.factor(),
        }
    }

    /// Offset this point by a delta.
    #[inline]
    pub fn offset(self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

impl std::fmt::Display for PhysicalPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}) physical", self.x, self.y)
    }
}

// ── Rect ──────────────────────────────────────────────────────────────

/// A rectangle in logical (density-independent) pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicalRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A rectangle in physical (device) pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl LogicalRect {
    #[inline]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create from origin point and size.
    #[inline]
    pub fn from_point_size(point: LogicalPoint, size: LogicalSize) -> Self {
        Self {
            x: point.x,
            y: point.y,
            width: size.width,
            height: size.height,
        }
    }

    /// The origin (top-left) point.
    #[inline]
    pub fn origin(self) -> LogicalPoint {
        LogicalPoint::new(self.x, self.y)
    }

    /// The size.
    #[inline]
    pub fn size(self) -> LogicalSize {
        LogicalSize::new(self.width, self.height)
    }

    /// Right edge (x + width).
    #[inline]
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge (y + height).
    #[inline]
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    /// Center point.
    #[inline]
    pub fn center(self) -> LogicalPoint {
        LogicalPoint::new(self.x + self.width * 0.5, self.y + self.height * 0.5)
    }

    /// Area.
    #[inline]
    pub fn area(self) -> f32 {
        self.width * self.height
    }

    /// Whether a point is inside this rectangle.
    #[inline]
    pub fn contains_point(self, point: LogicalPoint) -> bool {
        point.x >= self.x && point.x < self.right() && point.y >= self.y && point.y < self.bottom()
    }

    /// Whether this rectangle intersects another.
    #[inline]
    pub fn intersects(self, other: LogicalRect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Compute the intersection of two rectangles (returns None if disjoint).
    pub fn intersection(self, other: LogicalRect) -> Option<LogicalRect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right > x && bottom > y {
            Some(LogicalRect::new(x, y, right - x, bottom - y))
        } else {
            None
        }
    }

    /// Convert to physical rectangle using the given scale.
    ///
    /// The origin is rounded and the far edges are independently rounded to
    /// ensure pixel-perfect coverage (no sub-pixel gaps or overlaps).
    pub fn to_physical(self, scale: DpiScale) -> PhysicalRect {
        let s = scale.factor();
        let phys_x = (self.x * s).round() as i32;
        let phys_y = (self.y * s).round() as i32;
        let phys_right = ((self.x + self.width) * s).round() as i32;
        let phys_bottom = ((self.y + self.height) * s).round() as i32;
        PhysicalRect {
            x: phys_x,
            y: phys_y,
            width: (phys_right - phys_x).max(0) as u32,
            height: (phys_bottom - phys_y).max(0) as u32,
        }
    }
}

impl std::fmt::Display for LogicalRect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[({}, {}), {}x{}] logical",
            self.x, self.y, self.width, self.height
        )
    }
}

impl PhysicalRect {
    #[inline]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create from origin point and size.
    #[inline]
    pub fn from_point_size(point: PhysicalPoint, size: PhysicalSize) -> Self {
        Self {
            x: point.x,
            y: point.y,
            width: size.width,
            height: size.height,
        }
    }

    /// The origin (top-left) point.
    #[inline]
    pub fn origin(self) -> PhysicalPoint {
        PhysicalPoint::new(self.x, self.y)
    }

    /// The size.
    #[inline]
    pub fn size(self) -> PhysicalSize {
        PhysicalSize::new(self.width, self.height)
    }

    /// Right edge.
    #[inline]
    pub fn right(self) -> i32 {
        self.x + self.width as i32
    }

    /// Bottom edge.
    #[inline]
    pub fn bottom(self) -> i32 {
        self.y + self.height as i32
    }

    /// Area in physical pixels.
    #[inline]
    pub fn area(self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Whether a physical point is inside this rectangle.
    #[inline]
    pub fn contains_point(self, point: PhysicalPoint) -> bool {
        point.x >= self.x && point.x < self.right() && point.y >= self.y && point.y < self.bottom()
    }

    /// Whether this rectangle intersects another.
    #[inline]
    pub fn intersects(self, other: PhysicalRect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Convert to logical rectangle using the given scale.
    #[inline]
    pub fn to_logical(self, scale: DpiScale) -> LogicalRect {
        let s = scale.factor();
        LogicalRect {
            x: self.x as f32 / s,
            y: self.y as f32 / s,
            width: self.width as f32 / s,
            height: self.height as f32 / s,
        }
    }
}

impl std::fmt::Display for PhysicalRect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[({}, {}), {}x{}] physical",
            self.x, self.y, self.width, self.height
        )
    }
}
