//! Tray visual layout — computes icon positions and hit-testing.
//!
//! The renderer module handles the spatial arrangement of tray items within
//! the available status bar area. It supports horizontal and vertical
//! orientations, multi-row layouts, and overflow indicators.

use serde::{Deserialize, Serialize};

/// Orientation of the tray layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrayOrientation {
    /// Icons arranged left-to-right.
    Horizontal,
    /// Icons arranged top-to-bottom.
    Vertical,
}

/// Layout parameters for the tray.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TrayLayout {
    /// Width and height of each icon cell in logical pixels.
    pub item_size: f32,
    /// Spacing between icon cells in logical pixels.
    pub padding: f32,
    /// Direction of the primary layout axis.
    pub orientation: TrayOrientation,
    /// Maximum number of rows (for horizontal) or columns (for vertical).
    /// When items exceed the capacity of max_rows * available_length, overflow
    /// is triggered.
    pub max_rows: u32,
}

impl TrayLayout {
    /// Create a default horizontal tray layout.
    pub fn new() -> Self {
        Self {
            item_size: 22.0,
            padding: 4.0,
            orientation: TrayOrientation::Horizontal,
            max_rows: 1,
        }
    }

    /// Builder: set item size.
    pub fn with_item_size(mut self, size: f32) -> Self {
        self.item_size = size;
        self
    }

    /// Builder: set padding.
    pub fn with_padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Builder: set orientation.
    pub fn with_orientation(mut self, orientation: TrayOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Builder: set max rows.
    pub fn with_max_rows(mut self, rows: u32) -> Self {
        self.max_rows = rows.max(1);
        self
    }

    /// The advance (cell size + spacing) along the primary axis.
    pub fn cell_advance(&self) -> f32 {
        self.item_size + self.padding
    }
}

impl Default for TrayLayout {
    fn default() -> Self {
        Self::new()
    }
}

/// A positioned rectangle for a single tray item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemRect {
    /// X position in logical pixels (relative to the tray origin).
    pub x: f32,
    /// Y position in logical pixels (relative to the tray origin).
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl ItemRect {
    /// Returns `true` if the point `(px, py)` falls inside this rect.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    /// Returns the center point.
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width * 0.5, self.y + self.height * 0.5)
    }
}

/// The result of a tray layout computation.
#[derive(Debug, Clone)]
pub struct TrayBounds {
    /// Positioned rectangles for each visible item (in order).
    pub item_rects: Vec<ItemRect>,
    /// Total width of the tray area.
    pub total_width: f32,
    /// Total height of the tray area.
    pub total_height: f32,
    /// Whether overflow was triggered.
    pub has_overflow: bool,
    /// Number of items that did not fit.
    pub overflow_count: usize,
    /// The rectangle for the overflow indicator, if any.
    pub overflow_indicator: Option<ItemRect>,
}

/// Compute the layout bounds for `item_count` tray items.
///
/// For a horizontal layout, items fill left-to-right, wrapping to a new row
/// when the current row is full. If more rows would be needed than `max_rows`,
/// the last slot is replaced by an overflow indicator.
///
/// For a vertical layout, the same logic applies top-to-bottom.
///
/// # Parameters
/// - `item_count` — number of items to lay out.
/// - `available_extent` — available width (horizontal) or height (vertical) in
///   logical pixels.
/// - `layout` — layout parameters.
pub fn compute_tray_bounds(
    item_count: usize,
    available_extent: f32,
    layout: &TrayLayout,
) -> TrayBounds {
    if item_count == 0 {
        return TrayBounds {
            item_rects: Vec::new(),
            total_width: 0.0,
            total_height: 0.0,
            has_overflow: false,
            overflow_count: 0,
            overflow_indicator: None,
        };
    }

    let advance = layout.cell_advance();
    let size = layout.item_size;
    let max_rows = layout.max_rows.max(1) as usize;

    // How many items fit along the primary axis?
    let items_per_row = if available_extent < size {
        1usize
    } else {
        1 + ((available_extent - size) / advance).floor() as usize
    };

    let capacity = items_per_row * max_rows;

    let (visible_count, has_overflow) = if item_count <= capacity {
        (item_count, false)
    } else {
        // Reserve one slot for the overflow indicator.
        let reserved = if capacity > 0 { capacity - 1 } else { 0 };
        (reserved, true)
    };

    let mut rects = Vec::with_capacity(visible_count);

    for i in 0..visible_count {
        let (primary_idx, secondary_idx) = (i % items_per_row, i / items_per_row);
        let primary_pos = primary_idx as f32 * advance;
        let secondary_pos = secondary_idx as f32 * (size + layout.padding);

        let (x, y) = match layout.orientation {
            TrayOrientation::Horizontal => (primary_pos, secondary_pos),
            TrayOrientation::Vertical => (secondary_pos, primary_pos),
        };

        rects.push(ItemRect {
            x,
            y,
            width: size,
            height: size,
        });
    }

    // Overflow indicator.
    let overflow_indicator = if has_overflow {
        let primary_idx = visible_count % items_per_row;
        let secondary_idx = visible_count / items_per_row;
        let primary_pos = primary_idx as f32 * advance;
        let secondary_pos = secondary_idx as f32 * (size + layout.padding);

        let (x, y) = match layout.orientation {
            TrayOrientation::Horizontal => (primary_pos, secondary_pos),
            TrayOrientation::Vertical => (secondary_pos, primary_pos),
        };

        Some(ItemRect {
            x,
            y,
            width: size,
            height: size,
        })
    } else {
        None
    };

    // Compute total bounds.
    let used_rows = if visible_count == 0 {
        0
    } else {
        ((visible_count - 1) / items_per_row) + 1
    };
    let used_cols = visible_count.min(items_per_row);

    // Account for overflow indicator in bounds.
    let (total_cols, total_rows) = if has_overflow {
        let ov_col = (visible_count % items_per_row) + 1;
        let ov_row = (visible_count / items_per_row) + 1;
        (used_cols.max(ov_col), used_rows.max(ov_row))
    } else {
        (used_cols, used_rows)
    };

    let primary_extent = if total_cols > 0 {
        total_cols as f32 * size + (total_cols - 1).max(0) as f32 * layout.padding
    } else {
        0.0
    };
    let secondary_extent = if total_rows > 0 {
        total_rows as f32 * size + (total_rows - 1).max(0) as f32 * layout.padding
    } else {
        0.0
    };

    let (total_width, total_height) = match layout.orientation {
        TrayOrientation::Horizontal => (primary_extent, secondary_extent),
        TrayOrientation::Vertical => (secondary_extent, primary_extent),
    };

    TrayBounds {
        item_rects: rects,
        total_width,
        total_height,
        has_overflow,
        overflow_count: item_count - visible_count,
        overflow_indicator,
    }
}

/// Hit-test a point against the tray bounds. Returns the index of the item
/// at that point, or `None`. If the overflow indicator is hit, returns
/// `Some(usize::MAX)`.
pub fn item_at_point(bounds: &TrayBounds, x: f32, y: f32) -> Option<usize> {
    for (i, rect) in bounds.item_rects.iter().enumerate() {
        if rect.contains(x, y) {
            return Some(i);
        }
    }
    if let Some(ref indicator) = bounds.overflow_indicator {
        if indicator.contains(x, y) {
            return Some(usize::MAX);
        }
    }
    None
}
