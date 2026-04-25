//! Smart card reader emulation and APDU exchange.

use crate::{Result, UsbError};
use serde::{Deserialize, Serialize};

/// An APDU command sent to the smart card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApduCommand {
    pub cla: u8,
    pub ins: u8,
    pub p1: u8,
    pub p2: u8,
    pub data: Vec<u8>,
    pub le: Option<u16>,
}

/// An APDU response from the smart card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApduResponse {
    pub data: Vec<u8>,
    pub sw1: u8,
    pub sw2: u8,
}

/// State of a smart card reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartCardReaderState {
    Idle,
    CardInserted,
    Processing,
    Error,
}

/// A virtual smart card reader.
pub struct SmartCardReader {
    name: String,
    state: SmartCardReaderState,
    atr: Option<Vec<u8>>,
}

impl SmartCardReader {
    /// Create a new smart card reader with the given name.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            state: SmartCardReaderState::Idle,
            atr: None,
        }
    }

    /// Get the reader name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the current reader state.
    #[must_use]
    pub fn state(&self) -> SmartCardReaderState {
        self.state
    }

    /// Get the ATR (Answer To Reset) if a card is inserted.
    #[must_use]
    pub fn atr(&self) -> Option<&[u8]> {
        self.atr.as_deref()
    }

    /// Insert a card with the given ATR.
    pub fn insert_card(&mut self, atr: Vec<u8>) -> Result<()> {
        match self.state {
            SmartCardReaderState::Idle => {
                self.atr = Some(atr);
                self.state = SmartCardReaderState::CardInserted;
                Ok(())
            }
            _ => Err(UsbError::Internal(
                "card already inserted or reader in error state".to_string(),
            )),
        }
    }

    /// Remove the card from the reader.
    pub fn remove_card(&mut self) -> Result<()> {
        match self.state {
            SmartCardReaderState::CardInserted => {
                self.atr = None;
                self.state = SmartCardReaderState::Idle;
                Ok(())
            }
            _ => Err(UsbError::Internal(
                "no card to remove or reader in error state".to_string(),
            )),
        }
    }

    /// Exchange an APDU command with the card.
    ///
    /// This is a stub implementation that returns a success status (0x90, 0x00).
    pub fn exchange_apdu(&mut self, _cmd: &ApduCommand) -> Result<ApduResponse> {
        match self.state {
            SmartCardReaderState::CardInserted => {
                self.state = SmartCardReaderState::Processing;
                let response = ApduResponse {
                    data: Vec::new(),
                    sw1: 0x90,
                    sw2: 0x00,
                };
                self.state = SmartCardReaderState::CardInserted;
                Ok(response)
            }
            SmartCardReaderState::Processing => {
                Err(UsbError::Internal("reader is busy processing".to_string()))
            }
            _ => Err(UsbError::Internal("no card inserted".to_string())),
        }
    }

    /// Reset the reader to idle state.
    pub fn reset(&mut self) {
        self.state = SmartCardReaderState::Idle;
        self.atr = None;
    }
}
