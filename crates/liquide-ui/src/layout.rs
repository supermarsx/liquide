//! Layout engine for arranging widgets.

use crate::geometry::Rect;

/// Direction of layout flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayoutDirection {
    /// Children are arranged left to right.
    Horizontal,
    /// Children are arranged top to bottom.
    Vertical,
}

impl Default for LayoutDirection {
    fn default() -> Self {
        Self::Vertical
    }
}

/// Alignment of children within a layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayoutAlign {
    /// Align to the start (left or top).
    Start,
    /// Center children.
    Center,
    /// Align to the end (right or bottom).
    End,
    /// Stretch children to fill available space.
    Stretch,
    /// Distribute space between children.
    SpaceBetween,
    /// Distribute space around children.
    SpaceAround,
    /// Distribute space evenly.
    SpaceEvenly,
}

impl Default for LayoutAlign {
    fn default() -> Self {
        Self::Start
    }
}

/// Size constraints for a layout child.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutConstraints {
    /// Minimum width.
    pub min_width: f32,
    /// Minimum height.
    pub min_height: f32,
    /// Maximum width.
    pub max_width: f32,
    /// Maximum height.
    pub max_height: f32,
    /// Preferred width.
    pub preferred_width: f32,
    /// Preferred height.
    pub preferred_height: f32,
}

impl LayoutConstraints {
    /// Create constraints with preferred size only.
    #[must_use]
    pub fn with_preferred(width: f32, height: f32) -> Self {
        Self {
            min_width: 0.0,
            min_height: 0.0,
            max_width: f32::MAX,
            max_height: f32::MAX,
            preferred_width: width,
            preferred_height: height,
        }
    }
}

/// Padding around a layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Padding {
    /// Top padding.
    pub top: f32,
    /// Right padding.
    pub right: f32,
    /// Bottom padding.
    pub bottom: f32,
    /// Left padding.
    pub left: f32,
}

impl Padding {
    /// Create padding with individual values.
    #[must_use]
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Create uniform padding on all sides.
    #[must_use]
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create symmetric padding (vertical, horizontal).
    #[must_use]
    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Total horizontal padding (left + right).
    #[must_use]
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Total vertical padding (top + bottom).
    #[must_use]
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

/// Margin around a widget.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Margin {
    /// Top margin.
    pub top: f32,
    /// Right margin.
    pub right: f32,
    /// Bottom margin.
    pub bottom: f32,
    /// Left margin.
    pub left: f32,
}

impl Margin {
    /// Create margins with individual values.
    #[must_use]
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Create uniform margins on all sides.
    #[must_use]
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create symmetric margins (vertical, horizontal).
    #[must_use]
    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Total horizontal margin (left + right).
    #[must_use]
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Total vertical margin (top + bottom).
    #[must_use]
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

/// A box layout that arranges children in a single direction.
#[derive(Debug, Clone)]
pub struct BoxLayout {
    /// Direction of layout flow.
    pub direction: LayoutDirection,
    /// Alignment of children along the cross axis.
    pub align: LayoutAlign,
    /// Spacing between children in pixels.
    pub gap: f32,
    /// Padding inside the layout container.
    pub padding: Padding,
    /// Whether children should wrap to the next line.
    pub wrap: bool,
}

impl Default for BoxLayout {
    fn default() -> Self {
        Self {
            direction: LayoutDirection::Vertical,
            align: LayoutAlign::Start,
            gap: 0.0,
            padding: Padding::default(),
            wrap: false,
        }
    }
}

impl BoxLayout {
    /// Create a new box layout.
    #[must_use]
    pub fn new(direction: LayoutDirection) -> Self {
        Self {
            direction,
            ..Default::default()
        }
    }

    /// Lay out children within the available rectangle.
    ///
    /// Returns a rectangle for each child based on its constraints
    /// and the layout direction/alignment.
    #[must_use]
    pub fn layout(&self, children_constraints: &[LayoutConstraints], available: Rect) -> Vec<Rect> {
        if children_constraints.is_empty() {
            return Vec::new();
        }

        let content_x = available.x + self.padding.left;
        let content_y = available.y + self.padding.top;
        let content_width = (available.width - self.padding.horizontal()).max(0.0);
        let content_height = (available.height - self.padding.vertical()).max(0.0);

        let count = children_constraints.len();
        let total_gap = if count > 1 {
            self.gap * (count as f32 - 1.0)
        } else {
            0.0
        };

        match self.direction {
            LayoutDirection::Horizontal => {
                self.layout_horizontal(children_constraints, content_x, content_y, content_width, content_height, total_gap)
            }
            LayoutDirection::Vertical => {
                self.layout_vertical(children_constraints, content_x, content_y, content_width, content_height, total_gap)
            }
        }
    }

    fn layout_horizontal(
        &self,
        constraints: &[LayoutConstraints],
        content_x: f32,
        content_y: f32,
        content_width: f32,
        content_height: f32,
        total_gap: f32,
    ) -> Vec<Rect> {
        let count = constraints.len();
        let total_preferred: f32 = constraints.iter().map(|c| c.preferred_width).sum();
        let remaining = (content_width - total_gap - total_preferred).max(0.0);

        // Calculate spacing for alignment
        let (start_offset, between_extra) = match self.align {
            LayoutAlign::Center => ((remaining / 2.0), 0.0),
            LayoutAlign::End => (remaining, 0.0),
            LayoutAlign::SpaceBetween if count > 1 => {
                (0.0, remaining / (count as f32 - 1.0))
            }
            LayoutAlign::SpaceAround if count > 0 => {
                let space = remaining / count as f32;
                (space / 2.0, space)
            }
            LayoutAlign::SpaceEvenly if count > 0 => {
                let space = remaining / (count as f32 + 1.0);
                (space, space)
            }
            _ => (0.0, 0.0),
        };

        let mut result = Vec::with_capacity(count);
        let mut x = content_x + start_offset;
        for constraint in constraints {
            let w = constraint
                .preferred_width
                .max(constraint.min_width)
                .min(if constraint.max_width > 0.0 {
                    constraint.max_width
                } else {
                    f32::MAX
                });
            let h = if self.align == LayoutAlign::Stretch {
                content_height
            } else {
                constraint
                    .preferred_height
                    .max(constraint.min_height)
                    .min(content_height)
            };
            result.push(Rect::new(x, content_y, w, h));
            x += w + self.gap + between_extra;
        }
        result
    }

    fn layout_vertical(
        &self,
        constraints: &[LayoutConstraints],
        content_x: f32,
        content_y: f32,
        content_width: f32,
        content_height: f32,
        total_gap: f32,
    ) -> Vec<Rect> {
        let count = constraints.len();
        let total_preferred: f32 = constraints.iter().map(|c| c.preferred_height).sum();
        let remaining = (content_height - total_gap - total_preferred).max(0.0);

        let (start_offset, between_extra) = match self.align {
            LayoutAlign::Center => ((remaining / 2.0), 0.0),
            LayoutAlign::End => (remaining, 0.0),
            LayoutAlign::SpaceBetween if count > 1 => {
                (0.0, remaining / (count as f32 - 1.0))
            }
            LayoutAlign::SpaceAround if count > 0 => {
                let space = remaining / count as f32;
                (space / 2.0, space)
            }
            LayoutAlign::SpaceEvenly if count > 0 => {
                let space = remaining / (count as f32 + 1.0);
                (space, space)
            }
            _ => (0.0, 0.0),
        };

        let mut result = Vec::with_capacity(count);
        let mut y = content_y + start_offset;
        for constraint in constraints {
            let h = constraint
                .preferred_height
                .max(constraint.min_height)
                .min(if constraint.max_height > 0.0 {
                    constraint.max_height
                } else {
                    f32::MAX
                });
            let w = if self.align == LayoutAlign::Stretch {
                content_width
            } else {
                constraint
                    .preferred_width
                    .max(constraint.min_width)
                    .min(content_width)
            };
            result.push(Rect::new(content_x, y, w, h));
            y += h + self.gap + between_extra;
        }
        result
    }
}

/// A stack layout that layers children on top of each other.
///
/// All children share the same available rectangle.
#[derive(Debug, Clone, Default)]
pub struct StackLayout {
    /// Padding inside the stack container.
    pub padding: Padding,
}

impl StackLayout {
    /// Create a new stack layout.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Lay out children within the available rectangle.
    ///
    /// All children receive the same content area.
    #[must_use]
    pub fn layout(&self, child_count: usize, available: Rect) -> Vec<Rect> {
        let content = Rect::new(
            available.x + self.padding.left,
            available.y + self.padding.top,
            (available.width - self.padding.horizontal()).max(0.0),
            (available.height - self.padding.vertical()).max(0.0),
        );
        vec![content; child_count]
    }
}

/// A grid layout that arranges children in rows and columns.
#[derive(Debug, Clone)]
pub struct GridLayout {
    /// Number of columns.
    pub columns: u32,
    /// Number of rows.
    pub rows: u32,
    /// Gap between cells in pixels.
    pub gap: f32,
}

impl Default for GridLayout {
    fn default() -> Self {
        Self {
            columns: 1,
            rows: 1,
            gap: 0.0,
        }
    }
}

impl GridLayout {
    /// Create a new grid layout.
    #[must_use]
    pub fn new(columns: u32, rows: u32) -> Self {
        Self {
            columns,
            rows,
            gap: 0.0,
        }
    }

    /// Lay out children in a grid within the available rectangle.
    ///
    /// Children are placed left-to-right, top-to-bottom.
    #[must_use]
    pub fn layout(&self, child_count: usize, available: Rect) -> Vec<Rect> {
        if child_count == 0 || self.columns == 0 || self.rows == 0 {
            return Vec::new();
        }

        let cols = self.columns as f32;
        let rows = self.rows as f32;

        let total_h_gap = if self.columns > 1 {
            self.gap * (cols - 1.0)
        } else {
            0.0
        };
        let total_v_gap = if self.rows > 1 {
            self.gap * (rows - 1.0)
        } else {
            0.0
        };

        let cell_width = ((available.width - total_h_gap) / cols).max(0.0);
        let cell_height = ((available.height - total_v_gap) / rows).max(0.0);

        let mut result = Vec::with_capacity(child_count);
        for i in 0..child_count {
            let col = (i as u32) % self.columns;
            let row = (i as u32) / self.columns;

            if row >= self.rows {
                break;
            }

            let x = available.x + col as f32 * (cell_width + self.gap);
            let y = available.y + row as f32 * (cell_height + self.gap);
            result.push(Rect::new(x, y, cell_width, cell_height));
        }
        result
    }
}
