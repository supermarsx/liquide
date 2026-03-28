//! Tiling layout types, tile zones, and normalized rectangles.

/// A rectangle expressed as fractions of the workspace (0.0 to 1.0 on each axis).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl NormalizedRect {
    /// Create a new normalized rectangle.
    #[must_use]
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// Full workspace (0,0,1,1).
    pub const FULL: Self = Self {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };

    /// Left half.
    pub const LEFT_HALF: Self = Self {
        x: 0.0,
        y: 0.0,
        w: 0.5,
        h: 1.0,
    };

    /// Right half.
    pub const RIGHT_HALF: Self = Self {
        x: 0.5,
        y: 0.0,
        w: 0.5,
        h: 1.0,
    };

    /// Clamp all values into the 0.0..=1.0 range.
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            x: self.x.clamp(0.0, 1.0),
            y: self.y.clamp(0.0, 1.0),
            w: self.w.clamp(0.0, 1.0),
            h: self.h.clamp(0.0, 1.0),
        }
    }
}

/// A rectangular zone in the workspace that windows can be placed into.
#[derive(Debug, Clone, PartialEq)]
pub struct TileZone {
    /// Position as fraction of workspace (0.0-1.0).
    pub rect: NormalizedRect,
    /// Optional label for the zone.
    pub name: Option<String>,
    /// Maximum number of windows in this zone (None = unlimited).
    pub max_windows: Option<u32>,
}

impl TileZone {
    /// Create a new tile zone with the given normalized rect.
    #[must_use]
    pub fn new(rect: NormalizedRect) -> Self {
        Self {
            rect,
            name: None,
            max_windows: None,
        }
    }

    /// Set the zone name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the max window count.
    #[must_use]
    pub fn with_max_windows(mut self, max: u32) -> Self {
        self.max_windows = Some(max);
        self
    }
}

/// Available tiling layout algorithms.
#[derive(Debug, Clone, PartialEq)]
pub enum TilingLayout {
    /// Master-stack: master on left, stack on right. Configurable master width ratio.
    Columns,
    /// Master on top, stack on bottom.
    Rows,
    /// Equal-sized grid (auto rows/cols based on window count).
    Grid,
    /// Left stack, center master, right stack.
    ThreeColumn,
    /// Fibonacci spiral layout (alternating horizontal/vertical splits).
    Spiral,
    /// All windows stacked full-screen; only the active one is visible.
    Monocle,
    /// Traditional floating (no tiling).
    Float,
    /// User-defined zones.
    Custom(Vec<TileZone>),
}

impl TilingLayout {
    /// The canonical ordering of non-custom layouts for cycling.
    const CYCLE_ORDER: &[TilingLayout] = &[
        TilingLayout::Columns,
        TilingLayout::Rows,
        TilingLayout::Grid,
        TilingLayout::ThreeColumn,
        TilingLayout::Spiral,
        TilingLayout::Monocle,
    ];

    /// Return the next layout in the cycle (skips Float and Custom).
    /// If the current layout is not in the cycle, returns the first layout.
    #[must_use]
    pub fn next_in_cycle(&self) -> TilingLayout {
        match Self::CYCLE_ORDER.iter().position(|l| l == self) {
            Some(idx) => {
                let next = (idx + 1) % Self::CYCLE_ORDER.len();
                Self::CYCLE_ORDER[next].clone()
            }
            None => Self::CYCLE_ORDER[0].clone(),
        }
    }

    /// Whether this layout performs actual tiling (as opposed to floating).
    #[must_use]
    pub fn is_tiling(&self) -> bool {
        !matches!(self, TilingLayout::Float)
    }
}

/// Direction for focus/swap navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Direction for rotating windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RotateDir {
    /// Cycle windows forward (each window takes the next window's position).
    Forward,
    /// Cycle windows backward.
    Backward,
}
