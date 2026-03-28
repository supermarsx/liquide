//! Sound event system — map desktop events to sound files and play them.
//!
//! Follows the freedesktop Sound Theme specification: events are mapped
//! to sound file paths via a [`SoundTheme`], and playback is governed
//! by a [`SoundConfig`] that controls global volume and per-event toggles.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Well-known desktop sound events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundEvent {
    /// A notification popup appeared.
    NotificationPopup,
    /// A new message was received (chat, email).
    MessageReceived,
    /// An incoming call is ringing.
    CallIncoming,
    /// A call ended (hang up).
    CallEnded,
    /// The system volume was changed.
    VolumeChange,
    /// An audio device was connected (USB headset, bluetooth speaker).
    DeviceConnected,
    /// An audio device was disconnected.
    DeviceDisconnected,
    /// The screen was locked.
    ScreenLock,
    /// The screen was unlocked.
    ScreenUnlock,
    /// A user logged in.
    Login,
    /// A user logged out.
    Logout,
    /// An error occurred.
    Error,
    /// A warning was displayed.
    Warning,
    /// An item was moved to the trash.
    Trash,
    /// A screenshot was taken.
    Screenshot,
}

impl SoundEvent {
    /// All sound event variants.
    pub const ALL: &'static [SoundEvent] = &[
        SoundEvent::NotificationPopup,
        SoundEvent::MessageReceived,
        SoundEvent::CallIncoming,
        SoundEvent::CallEnded,
        SoundEvent::VolumeChange,
        SoundEvent::DeviceConnected,
        SoundEvent::DeviceDisconnected,
        SoundEvent::ScreenLock,
        SoundEvent::ScreenUnlock,
        SoundEvent::Login,
        SoundEvent::Logout,
        SoundEvent::Error,
        SoundEvent::Warning,
        SoundEvent::Trash,
        SoundEvent::Screenshot,
    ];

    /// The freedesktop sound-naming-spec name for this event.
    #[must_use]
    pub fn freedesktop_name(&self) -> &'static str {
        match self {
            Self::NotificationPopup => "message-new-instant",
            Self::MessageReceived => "message-new-email",
            Self::CallIncoming => "phone-incoming-call",
            Self::CallEnded => "phone-hangup",
            Self::VolumeChange => "audio-volume-change",
            Self::DeviceConnected => "device-added",
            Self::DeviceDisconnected => "device-removed",
            Self::ScreenLock => "screen-capture",
            Self::ScreenUnlock => "service-login",
            Self::Login => "service-login",
            Self::Logout => "service-logout",
            Self::Error => "dialog-error",
            Self::Warning => "dialog-warning",
            Self::Trash => "trash-empty",
            Self::Screenshot => "screen-capture",
        }
    }
}

impl fmt::Display for SoundEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.freedesktop_name())
    }
}

/// A sound theme that maps events to sound file paths.
///
/// Follows the freedesktop Sound Theme specification layout:
/// `{base_dir}/{theme_name}/stereo/{event_name}.oga`
pub struct SoundTheme {
    /// Theme name (e.g. "freedesktop", "Yaru", "Adwaita").
    name: String,
    /// Base directory for sound themes (e.g. "/usr/share/sounds").
    base_dir: PathBuf,
    /// Override mappings: event -> custom file path.
    overrides: HashMap<SoundEvent, PathBuf>,
}

impl SoundTheme {
    /// Create a new sound theme with the given name and base directory.
    #[must_use]
    pub fn new(name: String, base_dir: PathBuf) -> Self {
        Self {
            name,
            base_dir,
            overrides: HashMap::new(),
        }
    }

    /// Create the default freedesktop sound theme.
    #[must_use]
    pub fn freedesktop() -> Self {
        Self::new(
            "freedesktop".to_string(),
            PathBuf::from("/usr/share/sounds"),
        )
    }

    /// Set a custom sound file for a specific event.
    pub fn set_override(&mut self, event: SoundEvent, path: PathBuf) {
        self.overrides.insert(event, path);
    }

    /// Remove a custom override for a specific event.
    pub fn remove_override(&mut self, event: SoundEvent) {
        self.overrides.remove(&event);
    }

    /// Look up the sound file path for an event.
    ///
    /// Checks overrides first, then falls back to the theme directory.
    #[must_use]
    pub fn lookup(&self, event: SoundEvent) -> PathBuf {
        if let Some(path) = self.overrides.get(&event) {
            return path.clone();
        }

        self.base_dir
            .join(&self.name)
            .join("stereo")
            .join(format!("{}.oga", event.freedesktop_name()))
    }

    /// Check whether the sound file for an event exists on disk.
    #[must_use]
    pub fn sound_exists(&self, event: SoundEvent) -> bool {
        self.lookup(event).exists()
    }

    /// The theme name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The base directory.
    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

impl fmt::Display for SoundTheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SoundTheme(\"{}\", {}, {} overrides)",
            self.name,
            self.base_dir.display(),
            self.overrides.len(),
        )
    }
}

/// Configuration for the sound event system.
///
/// Controls whether sounds are played, the global volume, and
/// per-event enable/disable toggles.
pub struct SoundConfig {
    /// Global sound volume (0.0 = silence, 1.0 = full).
    pub global_volume: f32,
    /// Whether sound events are globally enabled.
    pub events_enabled: bool,
    /// Per-event enable/disable overrides (default is enabled).
    per_event: HashMap<SoundEvent, bool>,
}

impl SoundConfig {
    /// Create a new config with sounds enabled at full volume.
    #[must_use]
    pub fn new() -> Self {
        Self {
            global_volume: 1.0,
            events_enabled: true,
            per_event: HashMap::new(),
        }
    }

    /// Set whether a specific event is enabled.
    pub fn set_event_enabled(&mut self, event: SoundEvent, enabled: bool) {
        self.per_event.insert(event, enabled);
    }

    /// Check whether a specific event is enabled.
    ///
    /// Returns `false` if events are globally disabled, or if the
    /// specific event has been disabled.
    #[must_use]
    pub fn is_event_enabled(&self, event: SoundEvent) -> bool {
        if !self.events_enabled {
            return false;
        }
        *self.per_event.get(&event).unwrap_or(&true)
    }

    /// Set the global volume, clamped to 0.0..=1.0.
    pub fn set_global_volume(&mut self, volume: f32) {
        self.global_volume = volume.clamp(0.0, 1.0);
    }

    /// Enable all events.
    pub fn enable_all(&mut self) {
        self.events_enabled = true;
        self.per_event.clear();
    }

    /// Disable all events (mute).
    pub fn disable_all(&mut self) {
        self.events_enabled = false;
    }
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SoundConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SoundConfig(vol={:.0}%, {}{})",
            self.global_volume * 100.0,
            if self.events_enabled { "enabled" } else { "disabled" },
            if self.per_event.is_empty() {
                String::new()
            } else {
                format!(", {} overrides", self.per_event.len())
            },
        )
    }
}

/// Sound event player that combines a theme and config.
///
/// Call [`play_sound_event`](SoundPlayer::play_sound_event) to look up
/// the sound file and enqueue it for playback.
pub struct SoundPlayer {
    theme: SoundTheme,
    config: SoundConfig,
    /// Log of played events (for testing and debugging).
    play_log: Vec<(SoundEvent, PathBuf)>,
}

impl SoundPlayer {
    /// Create a new sound player with the given theme and config.
    #[must_use]
    pub fn new(theme: SoundTheme, config: SoundConfig) -> Self {
        Self {
            theme,
            config,
            play_log: Vec::new(),
        }
    }

    /// Create a sound player with the default freedesktop theme.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(SoundTheme::freedesktop(), SoundConfig::new())
    }

    /// Play a sound event. Returns the file path that would be played,
    /// or `None` if the event is disabled or volume is zero.
    pub fn play_sound_event(&mut self, event: SoundEvent) -> Option<PathBuf> {
        if !self.config.is_event_enabled(event) {
            return None;
        }
        if self.config.global_volume <= 0.0 {
            return None;
        }

        let path = self.theme.lookup(event);
        self.play_log.push((event, path.clone()));
        Some(path)
    }

    /// Get a reference to the theme.
    #[must_use]
    pub fn theme(&self) -> &SoundTheme {
        &self.theme
    }

    /// Get a mutable reference to the theme.
    pub fn theme_mut(&mut self) -> &mut SoundTheme {
        &mut self.theme
    }

    /// Get a reference to the config.
    #[must_use]
    pub fn config(&self) -> &SoundConfig {
        &self.config
    }

    /// Get a mutable reference to the config.
    pub fn config_mut(&mut self) -> &mut SoundConfig {
        &mut self.config
    }

    /// Get the play log (events played since creation).
    #[must_use]
    pub fn play_log(&self) -> &[(SoundEvent, PathBuf)] {
        &self.play_log
    }

    /// Clear the play log.
    pub fn clear_log(&mut self) {
        self.play_log.clear();
    }
}

impl fmt::Display for SoundPlayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SoundPlayer({}, {}, {} played)",
            self.theme, self.config, self.play_log.len(),
        )
    }
}
