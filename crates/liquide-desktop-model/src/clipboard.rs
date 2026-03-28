//! Per-window-station clipboard with multi-format data exchange.
//!
//! The clipboard belongs to a [`WindowStation`](crate::station::WindowStation)
//! and supports multiple data formats, an owner window, and a viewer chain
//! for clipboard change notifications.

use crate::error::DesktopError;
use crate::types::WindowId;
use std::collections::HashMap;

/// Standard clipboard format IDs (matching CF_* constants).
pub mod formats {
    pub const TEXT: u32 = 1;
    pub const UNICODE_TEXT: u32 = 13;
    pub const BITMAP: u32 = 2;
    pub const DIB: u32 = 8;
    pub const HTML: u32 = 0xC001;
    pub const RTF: u32 = 0xC002;
    pub const PNG: u32 = 0xC003;
    pub const FILE_LIST: u32 = 15;
}

/// Per-station clipboard data store.
#[derive(Debug, Clone)]
pub struct ClipboardData {
    /// The window that currently owns the clipboard (set when it places data).
    owner: Option<WindowId>,
    /// The window that currently has the clipboard open, or `None`.
    opened_by: Option<WindowId>,
    /// Chain of windows that want to be notified of clipboard changes.
    viewer_chain: Vec<WindowId>,
    /// Format ID -> raw data bytes.
    data: HashMap<u32, Vec<u8>>,
    /// Sequence number, incremented on every `set_data` call.
    sequence_number: u32,
}

impl ClipboardData {
    /// Creates an empty clipboard.
    pub fn new() -> Self {
        Self {
            owner: None,
            opened_by: None,
            viewer_chain: Vec::new(),
            data: HashMap::new(),
            sequence_number: 0,
        }
    }

    /// Returns the current clipboard owner, if any.
    pub fn owner(&self) -> Option<WindowId> {
        self.owner
    }

    /// Returns the window that has the clipboard open, if any.
    pub fn opened_by(&self) -> Option<WindowId> {
        self.opened_by
    }

    /// Returns the current sequence number.
    pub fn sequence_number(&self) -> u32 {
        self.sequence_number
    }

    /// Returns the list of viewer windows.
    pub fn viewer_chain(&self) -> &[WindowId] {
        &self.viewer_chain
    }

    /// Returns the set of format IDs currently on the clipboard.
    pub fn available_formats(&self) -> Vec<u32> {
        self.data.keys().copied().collect()
    }

    /// Opens the clipboard for a given window. Only one window can have it
    /// open at a time.
    pub fn open(&mut self, owner: WindowId) -> Result<(), DesktopError> {
        if let Some(current) = self.opened_by {
            if current != owner {
                return Err(DesktopError::ClipboardAlreadyOpen {
                    current_owner: current,
                });
            }
            // Already open by same window — no-op.
            return Ok(());
        }
        self.opened_by = Some(owner);
        Ok(())
    }

    /// Closes the clipboard. The window that opened it becomes the owner.
    pub fn close(&mut self) -> Result<(), DesktopError> {
        let opener = self.opened_by.ok_or(DesktopError::ClipboardNotOpen)?;
        self.owner = Some(opener);
        self.opened_by = None;
        Ok(())
    }

    /// Empties all clipboard data. Must be called while the clipboard is open.
    pub fn empty(&mut self) -> Result<(), DesktopError> {
        if self.opened_by.is_none() {
            return Err(DesktopError::ClipboardNotOpen);
        }
        self.data.clear();
        self.sequence_number += 1;
        Ok(())
    }

    /// Sets data for a given format. The clipboard must be open.
    pub fn set_data(&mut self, format: u32, data: Vec<u8>) -> Result<(), DesktopError> {
        if self.opened_by.is_none() {
            return Err(DesktopError::ClipboardNotOpen);
        }
        self.data.insert(format, data);
        self.sequence_number += 1;
        Ok(())
    }

    /// Gets data for a given format, if present.
    pub fn get_data(&self, format: u32) -> Option<&[u8]> {
        self.data.get(&format).map(|v| v.as_slice())
    }

    /// Returns `true` if the clipboard has data in the given format.
    pub fn has_format(&self, format: u32) -> bool {
        self.data.contains_key(&format)
    }

    /// Adds a window to the clipboard viewer chain. The viewer will be
    /// notified of clipboard changes (notification is the caller's
    /// responsibility — this just maintains the chain).
    pub fn add_viewer(&mut self, window_id: WindowId) {
        if !self.viewer_chain.contains(&window_id) {
            self.viewer_chain.push(window_id);
        }
    }

    /// Removes a window from the clipboard viewer chain.
    pub fn remove_viewer(&mut self, window_id: WindowId) {
        self.viewer_chain.retain(|&id| id != window_id);
    }

    /// Removes all references to a window (as owner, opener, and viewer).
    /// Called when a window is destroyed.
    pub fn remove_window(&mut self, window_id: WindowId) {
        if self.owner == Some(window_id) {
            self.owner = None;
        }
        if self.opened_by == Some(window_id) {
            self.opened_by = None;
        }
        self.remove_viewer(window_id);
    }
}

impl Default for ClipboardData {
    fn default() -> Self {
        Self::new()
    }
}
