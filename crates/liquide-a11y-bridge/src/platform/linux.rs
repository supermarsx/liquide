use std::process::Command;

use liquide_a11y::AccessibilityTree;

use crate::{A11yBridgeBackend, A11yBridgeEvent, AnnouncePriority, BridgeError};

/// AT-SPI2 bridge for Linux.
///
/// Initial implementation uses CLI tools (`pgrep`, `gsettings`, `spd-say`) as
/// a bridge to the desktop accessibility stack.  A future version will speak
/// AT-SPI2 / D-Bus directly via the `atspi` crate.
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
        // Check if Orca is running.
        Command::new("pgrep")
            .arg("orca")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn check_reduced_motion(&self) -> bool {
        Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "enable-animations"])
            .output()
            .map(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                s.trim() == "false"
            })
            .unwrap_or(false)
    }

    fn check_high_contrast(&self) -> bool {
        Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "high-contrast"])
            .output()
            .map(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                s.trim() == "true"
            })
            .unwrap_or(false)
    }

    fn get_font_scale(&self) -> f32 {
        Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "text-scaling-factor"])
            .output()
            .map(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                s.trim().parse::<f32>().unwrap_or(1.0)
            })
            .unwrap_or(1.0)
    }

    fn speak(&self, text: &str, priority: AnnouncePriority) {
        let urgency = match priority {
            AnnouncePriority::Polite => "text",
            AnnouncePriority::Assertive => "important",
        };
        let _ = Command::new("spd-say")
            .args(["-P", urgency, text])
            .spawn();
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
                    // AT-SPI would emit focus:object event.
                    // Full implementation will use atspi crate.
                }
                A11yBridgeEvent::NodeCreated { .. }
                | A11yBridgeEvent::NodeDestroyed { .. }
                | A11yBridgeEvent::NodeChanged { .. }
                | A11yBridgeEvent::ValueChanged { .. } => {
                    // Full AT-SPI integration would emit D-Bus signals
                    // for each event type.
                }
            }
        }
        Ok(())
    }

    fn sync_tree(&mut self, _tree: &AccessibilityTree) -> Result<(), BridgeError> {
        // Full implementation would register all nodes with AT-SPI
        // via the atspi crate's object registration API.
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
