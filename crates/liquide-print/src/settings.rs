//! Print settings: copies, paper, orientation, duplex, color, page range, scale, margins.

use crate::paper::PaperSize;

/// Page orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// Duplex (two-sided) printing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplexMode {
    /// Single-sided printing.
    None,
    /// Two-sided, flipping on the long edge (standard for documents).
    LongEdge,
    /// Two-sided, flipping on the short edge (calendar-style).
    ShortEdge,
}

/// Color mode for printing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Full color output.
    Color,
    /// Grayscale (shades of gray).
    Grayscale,
    /// Monochrome (black and white only).
    Mono,
}

/// Which pages to print.
#[derive(Debug, Clone, PartialEq)]
pub enum PageRange {
    /// Print all pages.
    All,
    /// Print a contiguous range (1-indexed, inclusive).
    Range(u32, u32),
    /// Print specific page numbers (1-indexed).
    Pages(Vec<u32>),
}

impl PageRange {
    /// Returns the set of page indices that should be printed, given the total number of pages.
    /// Pages are 1-indexed. Invalid page numbers (0 or beyond total) are filtered out.
    pub fn resolve(&self, total_pages: u32) -> Vec<u32> {
        match self {
            PageRange::All => (1..=total_pages).collect(),
            PageRange::Range(start, end) => {
                let s = (*start).max(1);
                let e = (*end).min(total_pages);
                if s > e { Vec::new() } else { (s..=e).collect() }
            }
            PageRange::Pages(pages) => {
                let mut out: Vec<u32> = pages
                    .iter()
                    .copied()
                    .filter(|&p| p >= 1 && p <= total_pages)
                    .collect();
                out.sort_unstable();
                out.dedup();
                out
            }
        }
    }
}

/// Scaling mode for print output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrintScale {
    /// Scale content to fit the printable area.
    FitToPage,
    /// Print at actual (100%) size.
    ActualSize,
    /// Custom scale factor (e.g., 0.5 = 50%, 2.0 = 200%).
    Custom(f32),
}

impl PrintScale {
    /// The effective scale factor. `FitToPage` returns 1.0 as a placeholder
    /// (actual fitting depends on content and printable area at render time).
    pub fn factor(&self) -> f32 {
        match self {
            PrintScale::FitToPage => 1.0,
            PrintScale::ActualSize => 1.0,
            PrintScale::Custom(f) => *f,
        }
    }
}

/// Page margins in millimeters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top_mm: f32,
    pub bottom_mm: f32,
    pub left_mm: f32,
    pub right_mm: f32,
}

impl Default for Margins {
    /// Default margins: 25.4 mm (1 inch) on all sides.
    fn default() -> Self {
        Self {
            top_mm: 25.4,
            bottom_mm: 25.4,
            left_mm: 25.4,
            right_mm: 25.4,
        }
    }
}

impl Margins {
    /// Narrow margins: 12.7 mm (0.5 inch) on all sides.
    pub fn narrow() -> Self {
        Self {
            top_mm: 12.7,
            bottom_mm: 12.7,
            left_mm: 12.7,
            right_mm: 12.7,
        }
    }

    /// No margins (borderless printing).
    pub fn none() -> Self {
        Self {
            top_mm: 0.0,
            bottom_mm: 0.0,
            left_mm: 0.0,
            right_mm: 0.0,
        }
    }

    /// Total horizontal margin (left + right).
    pub fn horizontal(&self) -> f32 {
        self.left_mm + self.right_mm
    }

    /// Total vertical margin (top + bottom).
    pub fn vertical(&self) -> f32 {
        self.top_mm + self.bottom_mm
    }
}

/// Complete print settings for a job.
#[derive(Debug, Clone)]
pub struct PrintSettings {
    /// Number of copies to print.
    pub copies: u32,
    /// Paper size to use.
    pub paper_size: PaperSize,
    /// Page orientation.
    pub orientation: Orientation,
    /// Duplex mode.
    pub duplex: DuplexMode,
    /// Color mode.
    pub color_mode: ColorMode,
    /// Which pages to print.
    pub page_range: PageRange,
    /// Scale mode.
    pub scale: PrintScale,
    /// Page margins.
    pub margins: Margins,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            copies: 1,
            paper_size: crate::paper::PAPER_A4.clone(),
            orientation: Orientation::Portrait,
            duplex: DuplexMode::None,
            color_mode: ColorMode::Color,
            page_range: PageRange::All,
            scale: PrintScale::ActualSize,
            margins: Margins::default(),
        }
    }
}
