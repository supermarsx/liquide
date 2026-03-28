//! Page layout computation: printable area and N-up layout.

use crate::paper::PaperSize;
use crate::settings::{Margins, Orientation};

/// The printable area within a page after margins are applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrintableArea {
    /// X offset from the left edge of the paper, in mm.
    pub x_mm: f32,
    /// Y offset from the top edge of the paper, in mm.
    pub y_mm: f32,
    /// Width of the printable area, in mm.
    pub width_mm: f32,
    /// Height of the printable area, in mm.
    pub height_mm: f32,
}

impl PrintableArea {
    /// Area of the printable region in square millimeters.
    pub fn area_mm2(&self) -> f32 {
        self.width_mm * self.height_mm
    }

    /// Aspect ratio (width / height).
    pub fn aspect_ratio(&self) -> f32 {
        if self.height_mm == 0.0 {
            return 0.0;
        }
        self.width_mm / self.height_mm
    }
}

/// A rectangle representing one logical page position within a sheet (for N-up layouts).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageRect {
    /// X offset within the printable area, in mm.
    pub x_mm: f32,
    /// Y offset within the printable area, in mm.
    pub y_mm: f32,
    /// Width of this page slot, in mm.
    pub width_mm: f32,
    /// Height of this page slot, in mm.
    pub height_mm: f32,
}

/// Compute the printable area for a given paper size, margins, and orientation.
///
/// For landscape orientation, the paper dimensions are swapped (width becomes height
/// and vice versa) before applying margins.
pub fn compute_printable_area(
    paper: &PaperSize,
    margins: &Margins,
    orientation: Orientation,
) -> PrintableArea {
    let (paper_w, paper_h) = match orientation {
        Orientation::Portrait => (paper.width_mm, paper.height_mm),
        Orientation::Landscape => (paper.height_mm, paper.width_mm),
    };

    let w = (paper_w - margins.left_mm - margins.right_mm).max(0.0);
    let h = (paper_h - margins.top_mm - margins.bottom_mm).max(0.0);

    PrintableArea {
        x_mm: margins.left_mm,
        y_mm: margins.top_mm,
        width_mm: w,
        height_mm: h,
    }
}

/// Compute N-up page slot positions within a printable area.
///
/// Supported values for `pages_per_sheet`: 1, 2, 4, 6, 9.
/// Other values fall back to 1-up.
///
/// Pages are arranged left-to-right, top-to-bottom in a grid. A small gap
/// (1mm) is placed between slots.
pub fn n_up_layout(printable: &PrintableArea, pages_per_sheet: u32) -> Vec<PageRect> {
    let (cols, rows) = grid_dimensions(pages_per_sheet);
    let gap_mm = if cols * rows > 1 { 1.0 } else { 0.0 };

    let total_gap_x = gap_mm * (cols as f32 - 1.0).max(0.0);
    let total_gap_y = gap_mm * (rows as f32 - 1.0).max(0.0);

    let slot_w = (printable.width_mm - total_gap_x) / cols as f32;
    let slot_h = (printable.height_mm - total_gap_y) / rows as f32;

    let count = cols * rows;
    let mut rects = Vec::with_capacity(count as usize);

    for i in 0..count {
        let col = i % cols;
        let row = i / cols;

        let x = printable.x_mm + col as f32 * (slot_w + gap_mm);
        let y = printable.y_mm + row as f32 * (slot_h + gap_mm);

        rects.push(PageRect {
            x_mm: x,
            y_mm: y,
            width_mm: slot_w.max(0.0),
            height_mm: slot_h.max(0.0),
        });
    }

    rects
}

/// Returns (columns, rows) for a given N-up value.
fn grid_dimensions(pages_per_sheet: u32) -> (u32, u32) {
    match pages_per_sheet {
        1 => (1, 1),
        2 => (2, 1),
        4 => (2, 2),
        6 => (3, 2),
        9 => (3, 3),
        _ => (1, 1),
    }
}
