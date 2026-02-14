//! Theme manager — runtime theme switching, persistence, and cursor synchronisation.
//!
//! The `ThemeManager` owns the active [`UiTheme`], provides methods to cycle
//! through the built-in themes (Liquid Glass, Night, Sunset, Midday), and
//! serialises the current preference to a TOML config file so it persists
//! across sessions.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::theme::{ThemeMode, UiTheme};

/// Identifies a built-in theme by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemePreset {
    LiquidGlass,
    Night,
    Sunset,
    Midday,
}

impl ThemePreset {
    /// All available presets, in cycle order.
    pub const ALL: [ThemePreset; 4] = [
        Self::LiquidGlass,
        Self::Night,
        Self::Sunset,
        Self::Midday,
    ];

    /// Build the full `UiTheme` for this preset.
    pub fn to_theme(self) -> UiTheme {
        match self {
            Self::LiquidGlass => UiTheme::liquid_glass(),
            Self::Night => UiTheme::night(),
            Self::Sunset => UiTheme::sunset(),
            Self::Midday => UiTheme::midday(),
        }
    }

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::LiquidGlass => "Liquid Glass",
            Self::Night => "Night",
            Self::Sunset => "Sunset",
            Self::Midday => "Midday",
        }
    }

    /// Whether the preset is a dark theme.
    pub fn is_dark(self) -> bool {
        !matches!(self, Self::Midday)
    }

    /// Resolve from a string name (case-insensitive).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "liquid glass" | "liquid_glass" | "liquidglass" => Some(Self::LiquidGlass),
            "night" => Some(Self::Night),
            "sunset" => Some(Self::Sunset),
            "midday" => Some(Self::Midday),
            _ => None,
        }
    }
}

/// Persistent theme configuration saved to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Preferred theme mode (Dark/Light/System).
    pub mode: ThemeMode,
    /// Active preset name.
    pub preset: ThemePreset,
    /// Default preset for dark mode (when mode == System and system is dark).
    pub dark_preset: ThemePreset,
    /// Default preset for light mode (when mode == System and system is light).
    pub light_preset: ThemePreset,
    /// Custom CSS overrides (applied on top of the preset).
    pub custom_css: Option<String>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Dark,
            preset: ThemePreset::LiquidGlass,
            dark_preset: ThemePreset::LiquidGlass,
            light_preset: ThemePreset::Midday,
            custom_css: None,
        }
    }
}

/// Runtime theme manager.
///
/// Holds the active theme and configuration, provides methods for switching,
/// cycling, and persisting theme preferences.
pub struct ThemeManager {
    config: ThemeConfig,
    active: UiTheme,
    config_path: Option<PathBuf>,
    /// Callback invoked whenever the theme changes.
    on_change: Option<Box<dyn Fn(&UiTheme) + Send + Sync>>,
}

impl ThemeManager {
    /// Create a new theme manager with default configuration.
    pub fn new() -> Self {
        let config = ThemeConfig::default();
        let active = config.preset.to_theme();
        Self {
            config,
            active,
            config_path: None,
            on_change: None,
        }
    }

    /// Create from a specific configuration.
    pub fn from_config(config: ThemeConfig) -> Self {
        let active = config.preset.to_theme();
        Self {
            config,
            active,
            config_path: None,
            on_change: None,
        }
    }

    /// Set the path where the config will be saved/loaded.
    pub fn set_config_path(&mut self, path: PathBuf) {
        self.config_path = Some(path);
    }

    /// Register a callback for theme changes.
    pub fn on_change<F: Fn(&UiTheme) + Send + Sync + 'static>(&mut self, f: F) {
        self.on_change = Some(Box::new(f));
    }

    // ── Accessors ───────────────────────────────────────────────────────

    /// Get the current active theme.
    pub fn active_theme(&self) -> &UiTheme {
        &self.active
    }

    /// Get the current configuration.
    pub fn config(&self) -> &ThemeConfig {
        &self.config
    }

    /// Get the current preset.
    pub fn current_preset(&self) -> ThemePreset {
        self.config.preset
    }

    /// Get the current mode.
    pub fn mode(&self) -> ThemeMode {
        self.config.mode
    }

    // ── Mutation ────────────────────────────────────────────────────────

    /// Switch to a specific preset.
    pub fn set_preset(&mut self, preset: ThemePreset) {
        self.config.preset = preset;
        self.apply();
    }

    /// Set the theme mode (Dark/Light/System).
    pub fn set_mode(&mut self, mode: ThemeMode) {
        self.config.mode = mode;
        match mode {
            ThemeMode::Dark => self.config.preset = self.config.dark_preset,
            ThemeMode::Light => self.config.preset = self.config.light_preset,
            ThemeMode::System => {
                // In a real implementation, query the OS preference.
                // For now, default to dark.
                self.config.preset = self.config.dark_preset;
            }
        }
        self.apply();
    }

    /// Respond to system dark/light mode change (for ThemeMode::System).
    pub fn system_appearance_changed(&mut self, is_dark: bool) {
        if self.config.mode == ThemeMode::System {
            self.config.preset = if is_dark {
                self.config.dark_preset
            } else {
                self.config.light_preset
            };
            self.apply();
        }
    }

    /// Cycle to the next preset in order.
    pub fn cycle_next(&mut self) {
        let all = ThemePreset::ALL;
        let idx = all.iter().position(|p| *p == self.config.preset).unwrap_or(0);
        let next = (idx + 1) % all.len();
        self.set_preset(all[next]);
    }

    /// Cycle to the previous preset.
    pub fn cycle_prev(&mut self) {
        let all = ThemePreset::ALL;
        let idx = all.iter().position(|p| *p == self.config.preset).unwrap_or(0);
        let prev = if idx == 0 { all.len() - 1 } else { idx - 1 };
        self.set_preset(all[prev]);
    }

    // ── Internal ────────────────────────────────────────────────────────

    fn apply(&mut self) {
        self.active = self.config.preset.to_theme();
        if let Some(ref cb) = self.on_change {
            cb(&self.active);
        }
        self.save_config();
    }

    // ── Persistence ─────────────────────────────────────────────────────

    /// Load configuration from disk.
    pub fn load_config(&mut self) -> Result<(), String> {
        let path = self.config_path.as_ref().ok_or("no config path set")?;
        if !path.exists() {
            return Ok(()); // Use defaults
        }
        let contents = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read theme config: {e}"))?;
        let config: ThemeConfig =
            toml::from_str(&contents).map_err(|e| format!("failed to parse theme config: {e}"))?;
        self.config = config;
        self.active = self.config.preset.to_theme();
        Ok(())
    }

    /// Save configuration to disk (fire-and-forget: logs errors).
    fn save_config(&self) {
        let Some(ref path) = self.config_path else {
            return;
        };
        match toml::to_string_pretty(&self.config) {
            Ok(toml_str) => {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = std::fs::write(path, toml_str) {
                    tracing::warn!("failed to save theme config: {e}");
                }
            }
            Err(e) => {
                tracing::warn!("failed to serialise theme config: {e}");
            }
        }
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_liquid_glass_dark() {
        let mgr = ThemeManager::new();
        assert_eq!(mgr.current_preset(), ThemePreset::LiquidGlass);
        assert_eq!(mgr.mode(), ThemeMode::Dark);
        assert!(mgr.active_theme().is_dark());
    }

    #[test]
    fn test_set_mode_light() {
        let mut mgr = ThemeManager::new();
        mgr.set_mode(ThemeMode::Light);
        assert_eq!(mgr.current_preset(), ThemePreset::Midday);
        assert!(!mgr.active_theme().is_dark());
    }

    #[test]
    fn test_cycle() {
        let mut mgr = ThemeManager::new();
        assert_eq!(mgr.current_preset(), ThemePreset::LiquidGlass);
        mgr.cycle_next();
        assert_eq!(mgr.current_preset(), ThemePreset::Night);
        mgr.cycle_next();
        assert_eq!(mgr.current_preset(), ThemePreset::Sunset);
        mgr.cycle_next();
        assert_eq!(mgr.current_preset(), ThemePreset::Midday);
        mgr.cycle_next();
        assert_eq!(mgr.current_preset(), ThemePreset::LiquidGlass);
    }

    #[test]
    fn test_cycle_prev() {
        let mut mgr = ThemeManager::new();
        mgr.cycle_prev();
        assert_eq!(mgr.current_preset(), ThemePreset::Midday);
    }

    #[test]
    fn test_system_appearance_changed() {
        let mut mgr = ThemeManager::new();
        mgr.set_mode(ThemeMode::System);
        mgr.system_appearance_changed(false);
        assert_eq!(mgr.current_preset(), ThemePreset::Midday);
        mgr.system_appearance_changed(true);
        assert_eq!(mgr.current_preset(), ThemePreset::LiquidGlass);
    }

    #[test]
    fn test_preset_from_name() {
        assert_eq!(ThemePreset::from_name("Night"), Some(ThemePreset::Night));
        assert_eq!(ThemePreset::from_name("liquid_glass"), Some(ThemePreset::LiquidGlass));
        assert_eq!(ThemePreset::from_name("unknown"), None);
    }

    #[test]
    fn test_all_presets_produce_themes() {
        for preset in ThemePreset::ALL {
            let theme = preset.to_theme();
            assert!(!theme.name.is_empty());
        }
    }

    #[test]
    fn test_on_change_callback() {
        use std::sync::{Arc, Mutex};
        let called = Arc::new(Mutex::new(false));
        let called_clone = called.clone();
        let mut mgr = ThemeManager::new();
        mgr.on_change(move |_| {
            *called_clone.lock().unwrap() = true;
        });
        mgr.set_preset(ThemePreset::Night);
        assert!(*called.lock().unwrap());
    }
}
