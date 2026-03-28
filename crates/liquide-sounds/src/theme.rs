use std::collections::HashMap;

use crate::event::SoundEvent;
use crate::format::{SoundFile, SoundFormat};

/// A sound theme containing event-to-sound mappings.
///
/// Follows the freedesktop.org sound theme specification pattern:
/// themes have an ID, display name, optional parent for inheritance,
/// and a mapping from events to sound files.
#[derive(Debug, Clone)]
pub struct SoundTheme {
    /// Unique theme identifier (e.g. "default", "silent", "retro").
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Optional parent theme ID for sound inheritance.
    pub parent: Option<String>,
    /// Event to sound file mappings.
    pub sounds: HashMap<SoundEvent, SoundFile>,
    /// Parent theme ID this theme inherits from (alias for `parent`).
    pub inherits_from: Option<String>,
}

impl SoundTheme {
    /// Create a new empty theme.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        SoundTheme {
            id: id.into(),
            name: name.into(),
            parent: None,
            sounds: HashMap::new(),
            inherits_from: None,
        }
    }

    /// Set the parent theme ID (for inheritance).
    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        let p = parent.into();
        self.parent = Some(p.clone());
        self.inherits_from = Some(p);
        self
    }

    /// Insert a sound mapping for an event.
    pub fn insert(&mut self, event: SoundEvent, file: SoundFile) {
        self.sounds.insert(event, file);
    }

    /// Look up the sound file for an event in this theme (no inheritance).
    pub fn get(&self, event: SoundEvent) -> Option<&SoundFile> {
        self.sounds.get(&event)
    }

    /// Returns true if this theme has a mapping for the given event.
    pub fn has_sound(&self, event: SoundEvent) -> bool {
        self.sounds.contains_key(&event)
    }

    /// Number of event mappings in this theme.
    pub fn len(&self) -> usize {
        self.sounds.len()
    }

    /// Returns true if this theme has no sound mappings.
    pub fn is_empty(&self) -> bool {
        self.sounds.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Built-in themes
// ---------------------------------------------------------------------------

/// Helper to create a SoundFile pointing to `assets/sounds/{subdir}/{event}.wav`.
fn asset_wav(subdir: &str, event: SoundEvent) -> SoundFile {
    SoundFile::with_format(
        format!("assets/sounds/{}/{}.wav", subdir, event.as_str()),
        SoundFormat::Wav,
    )
}

/// The default sound theme. Maps all events to WAV files under
/// `assets/sounds/default/`.
pub fn default_theme() -> SoundTheme {
    let mut theme = SoundTheme::new("default", "LiquiDE Default");
    for &event in SoundEvent::all() {
        theme.insert(event, asset_wav("default", event));
    }
    theme
}

/// A silent theme with no sound mappings. Useful as a "mute all event
/// sounds" option without disabling the sound subsystem entirely.
pub fn silent_theme() -> SoundTheme {
    SoundTheme::new("silent", "Silent")
}

/// A minimal theme with only essential sounds (notifications, errors,
/// session events). Inherits remaining sounds from the default theme.
pub fn minimal_theme() -> SoundTheme {
    let mut theme = SoundTheme::new("minimal", "Minimal").with_parent("default");
    let essential = [
        SoundEvent::Login,
        SoundEvent::Logout,
        SoundEvent::NotificationDefault,
        SoundEvent::NotificationUrgent,
        SoundEvent::Error,
        SoundEvent::Warning,
        SoundEvent::BatteryLow,
        SoundEvent::DeviceConnect,
        SoundEvent::DeviceDisconnect,
        SoundEvent::DesktopLogin,
        SoundEvent::SessionStart,
    ];
    for event in essential {
        theme.insert(event, asset_wav("minimal", event));
    }
    theme
}

/// A retro/8-bit style theme. Maps all events to OGG files under
/// `assets/sounds/retro/`. Inherits from "default" for any missing sounds.
pub fn retro_theme() -> SoundTheme {
    let mut theme = SoundTheme::new("retro", "Retro 8-bit").with_parent("default");
    for &event in SoundEvent::all() {
        theme.insert(
            event,
            SoundFile::with_format(
                format!("assets/sounds/retro/{}.ogg", event.as_str()),
                SoundFormat::Ogg,
            ),
        );
    }
    theme
}
