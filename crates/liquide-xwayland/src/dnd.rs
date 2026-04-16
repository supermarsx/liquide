//! Drag-and-drop bridge between X11 (XDND) and Wayland DnD protocol.
//!
//! # Stub
//! This module provides the structural scaffolding for bridging X11
//! XDND drag-and-drop with the Wayland `wl_data_device` drag protocol.
//! The actual XDND event handling is not yet implemented.

use crate::error::Result;

/// Bridges X11 XDND drag-and-drop with the Wayland drag-and-drop protocol.
pub struct X11DndBridge {
    active: bool,
}

impl X11DndBridge {
    pub fn new() -> Self {
        Self { active: false }
    }

    /// Start listening for XDND events and bridging them.
    pub fn start(&mut self) -> Result<()> {
        self.active = true;
        Ok(())
    }

    /// Stop the DnD bridge.
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Whether the bridge is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Default for X11DndBridge {
    fn default() -> Self {
        Self::new()
    }
}
