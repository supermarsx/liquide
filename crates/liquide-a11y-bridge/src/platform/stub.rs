use liquide_a11y::AccessibilityTree;

use crate::{A11yBridgeBackend, A11yBridgeEvent, BridgeError};

/// Stub bridge for unsupported platforms.
///
/// Returns `false` for all queries, no-ops for events.
pub struct AccessibilityBridge {
    connected: bool,
}

impl AccessibilityBridge {
    #[must_use]
    pub fn new() -> Self {
        Self { connected: false }
    }
}

impl Default for AccessibilityBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl A11yBridgeBackend for AccessibilityBridge {
    fn init(&mut self) -> Result<(), BridgeError> {
        self.connected = true;
        Ok(())
    }

    fn shutdown(&mut self) {
        self.connected = false;
    }

    fn push_events(&mut self, _events: &[A11yBridgeEvent]) -> Result<(), BridgeError> {
        if !self.connected {
            return Err(BridgeError::ConnectionFailed("not initialized".into()));
        }
        Ok(())
    }

    fn sync_tree(&mut self, _tree: &AccessibilityTree) -> Result<(), BridgeError> {
        Ok(())
    }

    fn is_screen_reader_active(&self) -> bool {
        false
    }

    fn prefers_reduced_motion(&self) -> bool {
        false
    }

    fn prefers_high_contrast(&self) -> bool {
        false
    }

    fn font_scale(&self) -> f32 {
        1.0
    }
}
