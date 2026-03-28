use std::collections::HashMap;

use crate::event::SoundEvent;
use crate::format::SoundFile;
use crate::playback;
use crate::theme::{self, SoundTheme};
use crate::wav;

/// Central sound management for desktop event sounds.
///
/// Maintains a registry of sound themes, tracks the active theme,
/// handles theme inheritance for sound lookups, and dispatches
/// playback through the platform backend.
pub struct SoundManager {
    /// All registered themes, keyed by theme ID.
    themes: HashMap<String, SoundTheme>,
    /// ID of the currently active theme.
    active_theme_id: String,
    /// Master enable/disable toggle for event sounds.
    enabled: bool,
    /// Master volume (0.0 = silent, 1.0 = full).
    volume: f32,
}

impl SoundManager {
    /// Create a new SoundManager with built-in themes registered.
    /// The "default" theme is active initially.
    pub fn new() -> Self {
        let mut themes = HashMap::new();

        let default = theme::default_theme();
        let silent = theme::silent_theme();
        let minimal = theme::minimal_theme();
        let retro = theme::retro_theme();

        themes.insert(default.id.clone(), default);
        themes.insert(silent.id.clone(), silent);
        themes.insert(minimal.id.clone(), minimal);
        themes.insert(retro.id.clone(), retro);

        SoundManager {
            themes,
            active_theme_id: "default".to_owned(),
            enabled: true,
            volume: 1.0,
        }
    }

    /// Play the sound associated with a desktop event.
    ///
    /// Does nothing if sounds are disabled, volume is zero, or no sound
    /// is mapped for this event (after inheritance resolution).
    pub fn play_event(&self, event: SoundEvent) {
        if !self.enabled || self.volume <= 0.0 {
            return;
        }
        if let Some(sound) = self.resolve_sound(event) {
            // In a real system this would check if the file exists on disk.
            // For now, fire-and-forget async playback.
            let path = sound.path.clone();
            playback::play_wav_file_async(&path);
        }
    }

    /// Play an arbitrary sound file by path.
    pub fn play_file(&self, path: &str) {
        if !self.enabled || self.volume <= 0.0 {
            return;
        }
        playback::play_wav_file_async(path);
    }

    /// Play in-memory WAV data (e.g. from `wav::generate_beep`).
    pub fn play_bytes(&self, data: Vec<u8>) {
        if !self.enabled || self.volume <= 0.0 {
            return;
        }
        playback::play_wav_bytes_async(data);
    }

    /// Set the active theme by ID. Returns `true` if the theme was found.
    pub fn set_theme(&mut self, theme_id: &str) -> bool {
        if self.themes.contains_key(theme_id) {
            self.active_theme_id = theme_id.to_owned();
            true
        } else {
            false
        }
    }

    /// Returns a reference to the currently active sound theme.
    pub fn active_theme(&self) -> &SoundTheme {
        self.themes
            .get(&self.active_theme_id)
            .expect("active theme must be registered")
    }

    /// Returns the ID of the currently active theme.
    pub fn active_theme_id(&self) -> &str {
        &self.active_theme_id
    }

    /// Enable or disable event sounds globally.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Returns whether event sounds are enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the master volume (clamped to 0.0..=1.0).
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Returns the current master volume.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Register a custom theme. If a theme with the same ID already exists,
    /// it is replaced.
    pub fn register_theme(&mut self, theme: SoundTheme) {
        self.themes.insert(theme.id.clone(), theme);
    }

    /// Remove a theme by ID. Cannot remove the active theme.
    /// Returns the removed theme, or None.
    pub fn remove_theme(&mut self, theme_id: &str) -> Option<SoundTheme> {
        if theme_id == self.active_theme_id {
            return None;
        }
        self.themes.remove(theme_id)
    }

    /// List all registered theme IDs.
    pub fn theme_ids(&self) -> Vec<&str> {
        self.themes.keys().map(|s| s.as_str()).collect()
    }

    /// Returns the number of registered themes.
    pub fn theme_count(&self) -> usize {
        self.themes.len()
    }

    /// Resolve the sound file for an event, walking the theme inheritance
    /// chain. Returns `None` if no theme in the chain has a mapping.
    pub fn resolve_sound(&self, event: SoundEvent) -> Option<&SoundFile> {
        self.resolve_sound_in_theme(&self.active_theme_id, event, 0)
    }

    /// Internal recursive resolver with depth limit to prevent infinite loops.
    fn resolve_sound_in_theme(
        &self,
        theme_id: &str,
        event: SoundEvent,
        depth: usize,
    ) -> Option<&SoundFile> {
        const MAX_INHERITANCE_DEPTH: usize = 8;
        if depth > MAX_INHERITANCE_DEPTH {
            return None;
        }

        let theme = self.themes.get(theme_id)?;

        // Check this theme first.
        if let Some(file) = theme.get(event) {
            return Some(file);
        }

        // Walk up the inheritance chain.
        if let Some(ref parent_id) = theme.parent {
            return self.resolve_sound_in_theme(parent_id, event, depth + 1);
        }

        None
    }

    /// Resolve a sound and return the file path as a String, or None.
    /// Convenience method for callers that just need the path.
    pub fn resolve_path(&self, event: SoundEvent) -> Option<&str> {
        self.resolve_sound(event).map(|f| f.path.as_str())
    }

    /// Generate and play a simple beep (useful as a fallback when no
    /// sound file is available).
    pub fn play_fallback_beep(&self) {
        if !self.enabled || self.volume <= 0.0 {
            return;
        }
        let data = wav::generate_beep(800.0, 100, self.volume);
        playback::play_wav_bytes_async(data);
    }
}

impl Default for SoundManager {
    fn default() -> Self {
        SoundManager::new()
    }
}
