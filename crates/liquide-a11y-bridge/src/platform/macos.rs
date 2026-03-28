use std::process::Command;

use liquide_a11y::AccessibilityTree;

use crate::{A11yBridgeBackend, A11yBridgeEvent, AnnouncePriority, BridgeError};

/// NSAccessibility bridge for macOS.
///
/// Initial implementation uses CLI tools (`defaults`, `say`) as a bridge to
/// the macOS accessibility stack.  A future version will use the
/// `NSAccessibility` protocol via `objc2` bindings.
pub struct AccessibilityBridge {
    connected: bool,
    screen_reader_active: bool,
}

impl AccessibilityBridge {
    #[must_use]
    pub fn new() -> Self {
        Self {
            connected: false,
            screen_reader_active: false,
        }
    }

    fn check_screen_reader(&self) -> bool {
        // Check if VoiceOver is enabled.
        Command::new("defaults")
            .args(["read", "com.apple.universalaccess", "voiceOverOnOffKey"])
            .output()
            .map(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                s.trim() == "1"
            })
            .unwrap_or(false)
    }

    fn check_reduced_motion(&self) -> bool {
        Command::new("defaults")
            .args(["read", "com.apple.universalaccess", "reduceMotion"])
            .output()
            .map(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                s.trim() == "1"
            })
            .unwrap_or(false)
    }

    fn check_high_contrast(&self) -> bool {
        Command::new("defaults")
            .args(["read", "com.apple.universalaccess", "increaseContrast"])
            .output()
            .map(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                s.trim() == "1"
            })
            .unwrap_or(false)
    }

    fn get_font_scale(&self) -> f32 {
        // macOS doesn't expose a single text-scaling-factor registry value
        // the way Linux/Windows do.  AppleDisplayScaleFactor is per-app.
        // Default to 1.0 and refine when NSAccessibility integration lands.
        1.0
    }

    fn speak(&self, text: &str, _priority: AnnouncePriority) {
        // Use the macOS `say` command.  VoiceOver's NSAccessibility
        // announcement API will replace this in the full implementation.
        let _ = Command::new("say").arg(text).spawn();
    }
}

impl Default for AccessibilityBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl A11yBridgeBackend for AccessibilityBridge {
    fn init(&mut self) -> Result<(), BridgeError> {
        self.screen_reader_active = self.check_screen_reader();
        self.connected = true;
        Ok(())
    }

    fn shutdown(&mut self) {
        self.connected = false;
    }

    fn push_events(&mut self, events: &[A11yBridgeEvent]) -> Result<(), BridgeError> {
        if !self.connected {
            return Err(BridgeError::ConnectionFailed("not initialized".into()));
        }

        for event in events {
            match event {
                A11yBridgeEvent::Announce { text, priority } => {
                    if self.screen_reader_active {
                        self.speak(text, *priority);
                    }
                }
                A11yBridgeEvent::FocusChanged { id: _ } => {
                    // Full implementation: post NSAccessibilityFocusedUIElementChangedNotification.
                }
                A11yBridgeEvent::NodeCreated { .. }
                | A11yBridgeEvent::NodeDestroyed { .. }
                | A11yBridgeEvent::NodeChanged { .. }
                | A11yBridgeEvent::ValueChanged { .. } => {
                    // Full implementation: post NSAccessibility notifications.
                }
            }
        }
        Ok(())
    }

    fn sync_tree(&mut self, _tree: &AccessibilityTree) -> Result<(), BridgeError> {
        // Full implementation would expose the tree via NSAccessibility protocol.
        Ok(())
    }

    fn is_screen_reader_active(&self) -> bool {
        self.screen_reader_active
    }

    fn prefers_reduced_motion(&self) -> bool {
        self.check_reduced_motion()
    }

    fn prefers_high_contrast(&self) -> bool {
        self.check_high_contrast()
    }

    fn font_scale(&self) -> f32 {
        self.get_font_scale()
    }
}
