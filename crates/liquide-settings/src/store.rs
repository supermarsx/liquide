use crate::schema::{Setting, SettingCategory, SettingKey, SettingValue};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

/// Error type for settings operations.
#[derive(Debug, Clone)]
pub enum SettingsError {
    NotFound(SettingKey),
    TypeMismatch,
    OutOfRange(String),
    InvalidChoice(String),
    IoError(String),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(key) => write!(f, "setting not found: {}", key),
            Self::TypeMismatch => write!(f, "type mismatch"),
            Self::OutOfRange(msg) => write!(f, "out of range: {}", msg),
            Self::InvalidChoice(msg) => write!(f, "invalid choice: {}", msg),
            Self::IoError(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for SettingsError {}

/// Central settings store with typed key-value storage and schema validation.
pub struct SettingsStore {
    /// Registered settings (key string -> Setting with default + constraints).
    settings: HashMap<String, Setting>,
    /// Current overridden values (only non-default entries are stored here).
    overrides: HashMap<String, SettingValue>,
    /// Optional config file path for save/load.
    config_path: Option<PathBuf>,
    /// Whether the store has unsaved changes.
    dirty: bool,
}

impl SettingsStore {
    /// Create a new store with all built-in defaults registered.
    pub fn new() -> Self {
        let mut store = Self {
            settings: HashMap::new(),
            overrides: HashMap::new(),
            config_path: None,
            dirty: false,
        };
        store.register_defaults();
        store
    }

    /// Builder method: set the config file path for save/load.
    pub fn with_config_path(mut self, path: PathBuf) -> Self {
        self.config_path = Some(path);
        self
    }

    /// Get the current value for a setting key. Returns the override if present,
    /// the registered default if known, or None if the key is unregistered.
    pub fn get(&self, key: &SettingKey) -> Option<&SettingValue> {
        let k = key.as_str();
        if let Some(val) = self.overrides.get(k) {
            return Some(val);
        }
        self.settings.get(k).map(|s| &s.default)
    }

    /// Convenience: get a bool value by key string.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(&SettingKey::new(key)).and_then(|v| v.as_bool())
    }

    /// Convenience: get an int value by key string.
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(&SettingKey::new(key)).and_then(|v| v.as_int())
    }

    /// Convenience: get a float value by key string.
    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.get(&SettingKey::new(key)).and_then(|v| v.as_float())
    }

    /// Convenience: get a string value by key string.
    /// Works for String, Choice, KeyBinding, and FilePath variants.
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.get(&SettingKey::new(key)).and_then(|v| v.as_str())
    }

    /// Set a value, validating type compatibility and constraints.
    pub fn set(&mut self, key: &SettingKey, value: SettingValue) -> Result<(), SettingsError> {
        let k = key.as_str();
        let setting = self
            .settings
            .get(k)
            .ok_or_else(|| SettingsError::NotFound(key.clone()))?;

        // Type check: the new value must be the same variant as the default.
        if !setting.default.same_type(&value) {
            return Err(SettingsError::TypeMismatch);
        }

        // Range validation for Int.
        if let (SettingValue::Int(v), Some((min, max))) = (&value, &setting.int_range) {
            if *v < *min || *v > *max {
                return Err(SettingsError::OutOfRange(format!(
                    "{} is outside [{}, {}]",
                    v, min, max
                )));
            }
        }

        // Range validation for Float.
        if let (SettingValue::Float(v), Some((min, max))) = (&value, &setting.float_range) {
            if *v < *min || *v > *max {
                return Err(SettingsError::OutOfRange(format!(
                    "{} is outside [{}, {}]",
                    v, min, max
                )));
            }
        }

        // Choice validation.
        if let SettingValue::Choice(v) = &value {
            if let Some(choices) = &setting.choices {
                if !choices.contains(v) {
                    return Err(SettingsError::InvalidChoice(format!(
                        "'{}' is not one of {:?}",
                        v, choices
                    )));
                }
            }
        }

        self.overrides.insert(k.to_string(), value);
        self.dirty = true;
        Ok(())
    }

    /// Reset a single key to its default (removes override).
    pub fn reset(&mut self, key: &SettingKey) -> Result<(), SettingsError> {
        let k = key.as_str();
        if !self.settings.contains_key(k) {
            return Err(SettingsError::NotFound(key.clone()));
        }
        self.overrides.remove(k);
        self.dirty = true;
        Ok(())
    }

    /// Reset all keys to their defaults.
    pub fn reset_all(&mut self) {
        self.overrides.clear();
        self.dirty = false;
    }

    /// Whether the store has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Total number of registered settings.
    pub fn setting_count(&self) -> usize {
        self.settings.len()
    }

    /// Return setting keys for a given category.
    pub fn category_settings(&self, category: &SettingCategory) -> Vec<SettingKey> {
        self.settings
            .keys()
            .filter(|k| {
                let prefix = match k.find('.') {
                    Some(idx) => &k[..idx],
                    None => k.as_str(),
                };
                SettingCategory::from_prefix(prefix) == *category
            })
            .map(|k| SettingKey::new(k))
            .collect()
    }

    /// Return all categories that have at least one registered setting.
    pub fn categories(&self) -> Vec<SettingCategory> {
        let mut seen = Vec::new();
        for k in self.settings.keys() {
            let prefix = match k.find('.') {
                Some(idx) => &k[..idx],
                None => k.as_str(),
            };
            let cat = SettingCategory::from_prefix(prefix);
            if !seen.contains(&cat) {
                seen.push(cat);
            }
        }
        seen
    }

    /// Save overridden (non-default) values to the config file.
    pub fn save(&mut self) -> Result<(), SettingsError> {
        let path = self
            .config_path
            .as_ref()
            .ok_or_else(|| SettingsError::IoError("no config path set".into()))?
            .clone();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SettingsError::IoError(e.to_string()))?;
        }

        let mut lines = Vec::new();
        for (key, value) in &self.overrides {
            lines.push(format!("{}={}", key, value.serialize()));
        }
        lines.sort(); // deterministic output

        let content = lines.join("\n");
        std::fs::write(&path, content).map_err(|e| SettingsError::IoError(e.to_string()))?;
        self.dirty = false;
        Ok(())
    }

    /// Load overrides from the config file. Missing file is not an error.
    pub fn load(&mut self) -> Result<(), SettingsError> {
        let path = self
            .config_path
            .as_ref()
            .ok_or_else(|| SettingsError::IoError("no config path set".into()))?
            .clone();

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(SettingsError::IoError(e.to_string())),
        };

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, serialized)) = line.split_once('=') {
                let key = key.trim();
                let serialized = serialized.trim();
                // Only import if we know this key.
                if self.settings.contains_key(key) {
                    if let Some(value) = SettingValue::deserialize(serialized) {
                        // Validate before importing.
                        let sk = SettingKey::new(key);
                        if self.validate_value(&sk, &value).is_ok() {
                            self.overrides.insert(key.to_string(), value);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // ── Private helpers ────────────────────────────────────────────

    /// Validate a value against the setting's constraints without storing it.
    fn validate_value(&self, key: &SettingKey, value: &SettingValue) -> Result<(), SettingsError> {
        let k = key.as_str();
        let setting = self
            .settings
            .get(k)
            .ok_or_else(|| SettingsError::NotFound(key.clone()))?;

        if !setting.default.same_type(value) {
            return Err(SettingsError::TypeMismatch);
        }

        if let (SettingValue::Int(v), Some((min, max))) = (value, &setting.int_range) {
            if *v < *min || *v > *max {
                return Err(SettingsError::OutOfRange(format!(
                    "{} is outside [{}, {}]",
                    v, min, max
                )));
            }
        }

        if let (SettingValue::Float(v), Some((min, max))) = (value, &setting.float_range) {
            if *v < *min || *v > *max {
                return Err(SettingsError::OutOfRange(format!(
                    "{} is outside [{}, {}]",
                    v, min, max
                )));
            }
        }

        if let SettingValue::Choice(v) = value {
            if let Some(choices) = &setting.choices {
                if !choices.contains(v) {
                    return Err(SettingsError::InvalidChoice(format!(
                        "'{}' is not one of {:?}",
                        v, choices
                    )));
                }
            }
        }

        Ok(())
    }

    /// Register a single setting.
    fn register(&mut self, setting: Setting) {
        self.settings.insert(setting.key.as_str().to_string(), setting);
    }

    /// Register all built-in default settings.
    fn register_defaults(&mut self) {
        // ── Appearance ───────────────────────────────────────────
        self.register(Setting {
            key: SettingKey::new("appearance.theme"),
            default: SettingValue::Choice("liquid_glass".into()),
            int_range: None,
            float_range: None,
            choices: Some(vec![
                "liquid_glass".into(),
                "night".into(),
                "midday".into(),
                "sunset".into(),
            ]),
        });
        self.register(Setting {
            key: SettingKey::new("appearance.font_size"),
            default: SettingValue::Int(14),
            int_range: Some((6, 72)),
            float_range: None,
            choices: None,
        });
        self.register(Setting {
            key: SettingKey::new("appearance.accent_color"),
            default: SettingValue::Color {
                r: 0,
                g: 122,
                b: 255,
                a: 255,
            },
            int_range: None,
            float_range: None,
            choices: None,
        });

        // ── Desktop ─────────────────────────────────────────────
        self.register(Setting {
            key: SettingKey::new("desktop.wallpaper"),
            default: SettingValue::FilePath(String::new()),
            int_range: None,
            float_range: None,
            choices: None,
        });
        self.register(Setting {
            key: SettingKey::new("desktop.show_icons"),
            default: SettingValue::Bool(true),
            int_range: None,
            float_range: None,
            choices: None,
        });

        // ── Window Management ───────────────────────────────────
        self.register(Setting {
            key: SettingKey::new("wm.focus_policy"),
            default: SettingValue::Choice("click".into()),
            int_range: None,
            float_range: None,
            choices: Some(vec![
                "click".into(),
                "sloppy".into(),
                "focus-follows-mouse".into(),
            ]),
        });
        self.register(Setting {
            key: SettingKey::new("wm.tiling_gap"),
            default: SettingValue::Int(8),
            int_range: Some((0, 64)),
            float_range: None,
            choices: None,
        });
        self.register(Setting {
            key: SettingKey::new("wm.snap_enabled"),
            default: SettingValue::Bool(true),
            int_range: None,
            float_range: None,
            choices: None,
        });

        // ── Input ───────────────────────────────────────────────
        self.register(Setting {
            key: SettingKey::new("input.mouse_speed"),
            default: SettingValue::Float(1.0),
            int_range: None,
            float_range: Some((0.1, 3.0)),
            choices: None,
        });
        self.register(Setting {
            key: SettingKey::new("input.natural_scroll"),
            default: SettingValue::Bool(false),
            int_range: None,
            float_range: None,
            choices: None,
        });
        self.register(Setting {
            key: SettingKey::new("input.key_repeat_delay"),
            default: SettingValue::Int(400),
            int_range: Some((100, 2000)),
            float_range: None,
            choices: None,
        });

        // ── Display ─────────────────────────────────────────────
        self.register(Setting {
            key: SettingKey::new("display.dpi_scale"),
            default: SettingValue::Float(1.0),
            int_range: None,
            float_range: Some((0.5, 4.0)),
            choices: None,
        });

        // ── Power ───────────────────────────────────────────────
        self.register(Setting {
            key: SettingKey::new("power.screen_blank_minutes"),
            default: SettingValue::Int(5),
            int_range: Some((0, 120)),
            float_range: None,
            choices: None,
        });
        self.register(Setting {
            key: SettingKey::new("power.auto_suspend_minutes"),
            default: SettingValue::Int(30),
            int_range: Some((0, 480)),
            float_range: None,
            choices: None,
        });

        // ── Notifications ───────────────────────────────────────
        self.register(Setting {
            key: SettingKey::new("notifications.dnd"),
            default: SettingValue::Bool(false),
            int_range: None,
            float_range: None,
            choices: None,
        });
        self.register(Setting {
            key: SettingKey::new("notifications.show_on_lockscreen"),
            default: SettingValue::Bool(true),
            int_range: None,
            float_range: None,
            choices: None,
        });

        // ── Accessibility ───────────────────────────────────────
        self.register(Setting {
            key: SettingKey::new("a11y.high_contrast"),
            default: SettingValue::Bool(false),
            int_range: None,
            float_range: None,
            choices: None,
        });
        self.register(Setting {
            key: SettingKey::new("a11y.large_text"),
            default: SettingValue::Bool(false),
            int_range: None,
            float_range: None,
            choices: None,
        });
        self.register(Setting {
            key: SettingKey::new("a11y.reduce_motion"),
            default: SettingValue::Bool(false),
            int_range: None,
            float_range: None,
            choices: None,
        });
        self.register(Setting {
            key: SettingKey::new("a11y.screen_reader"),
            default: SettingValue::Bool(false),
            int_range: None,
            float_range: None,
            choices: None,
        });

        // ── Privacy ─────────────────────────────────────────────
        self.register(Setting {
            key: SettingKey::new("privacy.lock_on_suspend"),
            default: SettingValue::Bool(true),
            int_range: None,
            float_range: None,
            choices: None,
        });
        self.register(Setting {
            key: SettingKey::new("privacy.auto_lock_minutes"),
            default: SettingValue::Int(5),
            int_range: Some((0, 120)),
            float_range: None,
            choices: None,
        });
    }
}
