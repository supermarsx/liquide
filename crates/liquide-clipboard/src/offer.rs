//! Clipboard offer and request types.

use serde::{Deserialize, Serialize};

use crate::format::ClipboardFormat;

/// A clipboard offer — announces available formats from a source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardOffer {
    pub source_id: u64,
    pub formats: Vec<ClipboardFormat>,
    pub timestamp_us: u64,
    pub serial: u64,
}

impl ClipboardOffer {
    /// Create a new clipboard offer.
    #[must_use]
    pub fn new(
        source_id: u64,
        formats: Vec<ClipboardFormat>,
        timestamp_us: u64,
        serial: u64,
    ) -> Self {
        Self {
            source_id,
            formats,
            timestamp_us,
            serial,
        }
    }

    /// Check if a specific format is available.
    #[must_use]
    pub fn has_format(&self, fmt: &ClipboardFormat) -> bool {
        self.formats.contains(fmt)
    }

    /// Get the preferred text format from the offer (PlainText > Html > RichText).
    #[must_use]
    pub fn preferred_text_format(&self) -> Option<&ClipboardFormat> {
        let priority = [
            ClipboardFormat::PlainText,
            ClipboardFormat::Html,
            ClipboardFormat::RichText,
        ];
        for p in &priority {
            if let Some(f) = self.formats.iter().find(|f| *f == p) {
                return Some(f);
            }
        }
        None
    }

    /// Get the preferred image format from the offer (Png > Jpeg > Svg).
    #[must_use]
    pub fn preferred_image_format(&self) -> Option<&ClipboardFormat> {
        let priority = [
            ClipboardFormat::Png,
            ClipboardFormat::Jpeg,
            ClipboardFormat::Svg,
        ];
        for p in &priority {
            if let Some(f) = self.formats.iter().find(|f| *f == p) {
                return Some(f);
            }
        }
        None
    }
}

/// A clipboard data request for a specific format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardRequest {
    pub target_format: ClipboardFormat,
    pub serial: u64,
}

impl ClipboardRequest {
    /// Create a new clipboard request.
    #[must_use]
    pub fn new(target_format: ClipboardFormat, serial: u64) -> Self {
        Self {
            target_format,
            serial,
        }
    }
}
