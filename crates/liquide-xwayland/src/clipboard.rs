//! Clipboard bridge between X11 selections and Wayland data offers.

use crate::error::Result;

/// Bridges X11 clipboard (CLIPBOARD / PRIMARY selections) with the
/// Wayland data device protocol.
pub struct X11ClipboardBridge {
    active: bool,
}

impl X11ClipboardBridge {
    pub fn new() -> Self {
        Self { active: false }
    }

    /// Start listening for X11 selection events and bridging them.
    pub fn start(&mut self) -> Result<()> {
        self.active = true;
        Ok(())
    }

    /// Stop the clipboard bridge.
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Whether the bridge is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Default for X11ClipboardBridge {
    fn default() -> Self {
        Self::new()
    }
}
