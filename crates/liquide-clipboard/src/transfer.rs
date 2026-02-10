//! Clipboard data transfer state machine.

use crate::format::ClipboardFormat;
use crate::offer::ClipboardOffer;
use crate::{ClipboardError, Result};

/// State of a clipboard transfer.
#[derive(Debug, Clone)]
pub enum TransferState {
    Idle,
    Offered { offer: ClipboardOffer },
    Requested { format: ClipboardFormat },
    Transferring { received: usize, total: Option<usize> },
    Complete,
    Failed(String),
}

/// Manages a single clipboard data transfer.
pub struct ClipboardTransfer {
    state: TransferState,
    data: Vec<u8>,
    format: ClipboardFormat,
    max_size: usize,
}

impl ClipboardTransfer {
    /// Create a new transfer with a max payload size.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            state: TransferState::Idle,
            data: Vec::new(),
            format: ClipboardFormat::PlainText,
            max_size,
        }
    }

    /// Begin an offer phase.
    pub fn begin_offer(&mut self, offer: ClipboardOffer) {
        self.data.clear();
        self.state = TransferState::Offered { offer };
    }

    /// Request a specific format from the current offer.
    pub fn request_format(&mut self, format: ClipboardFormat) -> Result<()> {
        match &self.state {
            TransferState::Offered { offer } => {
                if !offer.has_format(&format) {
                    return Err(ClipboardError::FormatNotAvailable);
                }
                self.format = format.clone();
                self.state = TransferState::Requested { format };
                Ok(())
            }
            _ => Err(ClipboardError::TransferFailed(
                "not in offered state".to_string(),
            )),
        }
    }

    /// Receive a chunk of data.
    pub fn receive_chunk(&mut self, chunk: &[u8]) -> Result<()> {
        let new_size = self.data.len() + chunk.len();
        if new_size > self.max_size {
            self.state = TransferState::Failed("payload too large".to_string());
            return Err(ClipboardError::PayloadTooLarge {
                size: new_size,
                max: self.max_size,
            });
        }
        self.data.extend_from_slice(chunk);
        self.state = TransferState::Transferring {
            received: self.data.len(),
            total: None,
        };
        Ok(())
    }

    /// Complete the transfer and return the data.
    pub fn complete(&mut self) -> Result<Vec<u8>> {
        let data = std::mem::take(&mut self.data);
        self.state = TransferState::Complete;
        Ok(data)
    }

    /// Get the current transfer state.
    #[must_use]
    pub fn state(&self) -> &TransferState {
        &self.state
    }

    /// Check if the transfer is complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        matches!(self.state, TransferState::Complete)
    }

    /// Get the number of received bytes so far.
    #[must_use]
    pub fn received_bytes(&self) -> usize {
        self.data.len()
    }

    /// Reset the transfer to idle.
    pub fn reset(&mut self) {
        self.state = TransferState::Idle;
        self.data.clear();
        self.format = ClipboardFormat::PlainText;
    }

    /// Get the current transfer format.
    #[must_use]
    pub fn format(&self) -> &ClipboardFormat {
        &self.format
    }

    /// Abort the current transfer with a reason.
    pub fn abort(&mut self, reason: impl Into<String>) {
        self.state = TransferState::Failed(reason.into());
        self.data.clear();
    }
}

impl std::fmt::Display for TransferState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Offered { offer } => write!(f, "Offered(formats={})", offer.formats.len()),
            Self::Requested { format } => write!(f, "Requested({format})"),
            Self::Transferring { received, total } => {
                match total {
                    Some(t) => write!(f, "Transferring({received}/{t})"),
                    None => write!(f, "Transferring({received}/?)"),
                }
            }
            Self::Complete => write!(f, "Complete"),
            Self::Failed(reason) => write!(f, "Failed({reason})"),
        }
    }
}
