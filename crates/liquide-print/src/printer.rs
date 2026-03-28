//! Printer representation and capabilities.

use crate::paper::PaperSize;

/// Unique identifier for a discovered printer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrinterId(pub u64);

/// Status of a printer.
#[derive(Debug, Clone, PartialEq)]
pub enum PrinterStatus {
    /// Printer is idle and ready to accept jobs.
    Idle,
    /// Printer is currently printing.
    Printing,
    /// Printer has encountered an error.
    Error(String),
    /// Printer is offline / unreachable.
    Offline,
    /// Printer has a paper jam.
    PaperJam,
    /// Printer toner or ink is low.
    LowToner,
}

impl PrinterStatus {
    /// Returns `true` if the printer can accept new jobs.
    pub fn is_ready(&self) -> bool {
        matches!(self, PrinterStatus::Idle)
    }

    /// Returns a human-readable label for this status.
    pub fn label(&self) -> &str {
        match self {
            PrinterStatus::Idle => "Idle",
            PrinterStatus::Printing => "Printing",
            PrinterStatus::Error(_) => "Error",
            PrinterStatus::Offline => "Offline",
            PrinterStatus::PaperJam => "Paper Jam",
            PrinterStatus::LowToner => "Low Toner",
        }
    }
}

/// Capabilities reported by a printer.
#[derive(Debug, Clone)]
pub struct PrinterCapabilities {
    /// Paper sizes this printer supports.
    pub paper_sizes: Vec<PaperSize>,
    /// Whether the printer supports duplex (two-sided) printing.
    pub supports_duplex: bool,
    /// Whether the printer supports color output.
    pub supports_color: bool,
    /// Maximum resolution in DPI.
    pub max_dpi: u32,
    /// Maximum number of copies per job.
    pub max_copies: u32,
}

impl Default for PrinterCapabilities {
    fn default() -> Self {
        Self {
            paper_sizes: vec![
                crate::paper::PAPER_A4.clone(),
                crate::paper::PAPER_LETTER.clone(),
            ],
            supports_duplex: false,
            supports_color: true,
            max_dpi: 600,
            max_copies: 99,
        }
    }
}

impl PrinterCapabilities {
    /// Check if the printer supports a given paper size.
    pub fn supports_paper(&self, paper: &PaperSize) -> bool {
        self.paper_sizes.iter().any(|p| p == paper)
    }

    /// Check if a requested DPI is within the printer's capability.
    pub fn supports_dpi(&self, dpi: u32) -> bool {
        dpi <= self.max_dpi
    }
}

/// A discovered printer.
#[derive(Debug, Clone)]
pub struct Printer {
    /// Unique printer identifier.
    pub id: PrinterId,
    /// Human-readable printer name.
    pub name: String,
    /// Physical or network location description.
    pub location: Option<String>,
    /// Driver or backend name.
    pub driver: String,
    /// Current printer status.
    pub status: PrinterStatus,
    /// Printer capabilities.
    pub capabilities: PrinterCapabilities,
    /// Whether this is the system default printer.
    pub is_default: bool,
    /// Whether this printer is accessed over a network.
    pub is_network: bool,
}

impl Printer {
    /// Returns `true` if the printer is ready to accept jobs.
    pub fn is_ready(&self) -> bool {
        self.status.is_ready()
    }
}
