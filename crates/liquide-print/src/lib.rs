//! Print system framework for LiquiDE.
//!
//! Provides printer discovery, job management, print settings, and page layout
//! computation for the desktop environment.

mod discovery;
mod job;
mod layout;
mod manager;
mod paper;
mod printer;
mod settings;

pub use job::{JobStatus, PrintJob};
pub use layout::{PageRect, PrintableArea, compute_printable_area, n_up_layout};
pub use manager::PrintManager;
pub use paper::{
    PAPER_A3, PAPER_A4, PAPER_A5, PAPER_B5, PAPER_LEGAL, PAPER_LETTER, PAPER_TABLOID, PaperSize,
};
pub use printer::{Printer, PrinterCapabilities, PrinterId, PrinterStatus};
pub use settings::{
    ColorMode, DuplexMode, Margins, Orientation, PageRange, PrintScale, PrintSettings,
};

#[cfg(test)]
mod tests;
