use std::fmt;

/// Categories for settings, shown in the settings UI sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SettingCategory {
    Appearance,
    Desktop,
    Display,
    WindowManagement,
    Input,
    Power,
    Notifications,
    Accessibility,
    Privacy,
    Sound,
    Network,
    Region,
    Users,
    About,
    Custom(String),
}

impl SettingCategory {
    /// Returns all 14 standard categories in display order
    /// (does not include Custom variants).
    pub fn all() -> Vec<Self> {
        vec![
            Self::Appearance,
            Self::Desktop,
            Self::Display,
            Self::WindowManagement,
            Self::Input,
            Self::Power,
            Self::Notifications,
            Self::Accessibility,
            Self::Privacy,
            Self::Sound,
            Self::Network,
            Self::Region,
            Self::Users,
            Self::About,
        ]
    }

    /// Human-readable label for this category.
    pub fn label(&self) -> &str {
        match self {
            Self::Appearance => "Appearance",
            Self::Desktop => "Desktop",
            Self::Display => "Display",
            Self::WindowManagement => "Window Management",
            Self::Input => "Input",
            Self::Power => "Power",
            Self::Notifications => "Notifications",
            Self::Accessibility => "Accessibility",
            Self::Privacy => "Privacy & Security",
            Self::Sound => "Sound",
            Self::Network => "Network",
            Self::Region => "Region & Language",
            Self::Users => "Users",
            Self::About => "About",
            Self::Custom(name) => name.as_str(),
        }
    }

    /// Icon name for this category (freedesktop icon naming convention).
    pub fn icon(&self) -> &str {
        match self {
            Self::Appearance => "preferences-desktop-theme",
            Self::Desktop => "preferences-desktop-wallpaper",
            Self::Display => "preferences-desktop-display",
            Self::WindowManagement => "preferences-system-windows",
            Self::Input => "preferences-desktop-peripherals",
            Self::Power => "preferences-system-power",
            Self::Notifications => "preferences-desktop-notifications",
            Self::Accessibility => "preferences-desktop-accessibility",
            Self::Privacy => "preferences-system-privacy",
            Self::Sound => "preferences-desktop-sound",
            Self::Network => "preferences-system-network",
            Self::Region => "preferences-desktop-locale",
            Self::Users => "system-users",
            Self::About => "help-about",
            Self::Custom(_) => "preferences-other",
        }
    }

    /// Map a key category prefix string to a SettingCategory.
    pub fn from_prefix(prefix: &str) -> Self {
        match prefix {
            "appearance" => Self::Appearance,
            "desktop" => Self::Desktop,
            "display" => Self::Display,
            "wm" => Self::WindowManagement,
            "input" => Self::Input,
            "power" => Self::Power,
            "notifications" => Self::Notifications,
            "a11y" => Self::Accessibility,
            "privacy" => Self::Privacy,
            "sound" => Self::Sound,
            "network" => Self::Network,
            "region" => Self::Region,
            "users" => Self::Users,
            "about" => Self::About,
            other => Self::Custom(other.to_string()),
        }
    }
}

/// A setting key wrapping a dotted string like "appearance.theme".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SettingKey(String);

impl SettingKey {
    pub fn new(s: &str) -> Self {
        Self(s.to_string())
    }

    /// Returns the category prefix (part before the first '.'), or the
    /// whole key if there is no '.'.
    pub fn category(&self) -> &str {
        match self.0.find('.') {
            Some(idx) => &self.0[..idx],
            None => &self.0,
        }
    }

    /// Returns the full key string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SettingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A typed setting value.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Choice(String),
    Color { r: u8, g: u8, b: u8, a: u8 },
    KeyBinding(String),
    FilePath(String),
}

impl SettingValue {
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        if let Self::Int(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        if let Self::Float(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(v) | Self::Choice(v) | Self::KeyBinding(v) | Self::FilePath(v) => {
                Some(v.as_str())
            }
            _ => None,
        }
    }

    /// Returns the name of the value type.
    pub fn type_name(&self) -> &str {
        match self {
            Self::Bool(_) => "Bool",
            Self::Int(_) => "Int",
            Self::Float(_) => "Float",
            Self::String(_) => "String",
            Self::Choice(_) => "Choice",
            Self::Color { .. } => "Color",
            Self::KeyBinding(_) => "KeyBinding",
            Self::FilePath(_) => "FilePath",
        }
    }

    /// Serialize to a string representation for config file persistence.
    pub fn serialize(&self) -> String {
        match self {
            Self::Bool(v) => format!("bool:{}", v),
            Self::Int(v) => format!("int:{}", v),
            Self::Float(v) => format!("float:{}", v),
            Self::String(v) => format!("string:{}", v),
            Self::Choice(v) => format!("choice:{}", v),
            Self::Color { r, g, b, a } => format!("color:{},{},{},{}", r, g, b, a),
            Self::KeyBinding(v) => format!("keybinding:{}", v),
            Self::FilePath(v) => format!("filepath:{}", v),
        }
    }

    /// Deserialize from the string representation used in config files.
    pub fn deserialize(s: &str) -> Option<Self> {
        let (tag, rest) = s.split_once(':')?;
        match tag {
            "bool" => match rest {
                "true" => Some(Self::Bool(true)),
                "false" => Some(Self::Bool(false)),
                _ => None,
            },
            "int" => rest.parse::<i64>().ok().map(Self::Int),
            "float" => rest.parse::<f64>().ok().map(Self::Float),
            "string" => Some(Self::String(rest.to_string())),
            "choice" => Some(Self::Choice(rest.to_string())),
            "color" => {
                let parts: Vec<&str> = rest.split(',').collect();
                if parts.len() == 4 {
                    let r = parts[0].parse().ok()?;
                    let g = parts[1].parse().ok()?;
                    let b = parts[2].parse().ok()?;
                    let a = parts[3].parse().ok()?;
                    Some(Self::Color { r, g, b, a })
                } else {
                    None
                }
            }
            "keybinding" => Some(Self::KeyBinding(rest.to_string())),
            "filepath" => Some(Self::FilePath(rest.to_string())),
            _ => None,
        }
    }

    /// Returns true if both values are the same variant (ignoring inner data).
    pub(crate) fn same_type(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl fmt::Display for SettingValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(v) => write!(f, "{}", v),
            Self::Int(v) => write!(f, "{}", v),
            Self::Float(v) => write!(f, "{}", v),
            Self::String(v) => write!(f, "{}", v),
            Self::Choice(v) => write!(f, "{}", v),
            Self::Color { r, g, b, a } => write!(f, "#{:02x}{:02x}{:02x}{:02x}", r, g, b, a),
            Self::KeyBinding(v) => write!(f, "{}", v),
            Self::FilePath(v) => write!(f, "{}", v),
        }
    }
}

/// A registered setting with its default value and optional validation constraints.
#[derive(Debug, Clone)]
pub struct Setting {
    pub key: SettingKey,
    pub default: SettingValue,
    /// For Int: Some((min, max))
    pub int_range: Option<(i64, i64)>,
    /// For Float: Some((min, max))
    pub float_range: Option<(f64, f64)>,
    /// For Choice: allowed values
    pub choices: Option<Vec<String>>,
}

// ── Extended schema types ────────────────────────────────────────────

/// A schema entry describing a single setting with metadata, validation
/// constraints, and documentation. Used by the settings UI and tooling
/// for type-safe access and introspection.
#[derive(Debug, Clone)]
pub struct SchemaEntry {
    /// Dotted key (e.g. "appearance.theme").
    pub key: String,
    /// The value type expected.
    pub value_type: SchemaValueType,
    /// Default value.
    pub default: SettingValue,
    /// Short one-line summary.
    pub summary: String,
    /// Longer description (may be multi-line).
    pub description: String,
    /// For Int: optional (min, max) range constraint.
    pub range_constraint: Option<(i64, i64)>,
    /// For Float: optional (min, max) range constraint.
    pub float_range: Option<(f64, f64)>,
    /// For Choice: allowed enum values.
    pub enum_values: Option<Vec<String>>,
    /// Category path (e.g. "appearance", "desktop.background").
    pub category: String,
}

/// The type of a schema entry's value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaValueType {
    Bool,
    Int,
    Float,
    Str,
    Choice,
    Color,
    KeyBinding,
    FilePath,
}

/// Error from schema validation.
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Key not found in schema.
    UnknownKey(String),
    /// Value type does not match schema.
    TypeMismatch {
        key: String,
        expected: String,
        got: String,
    },
    /// Value is outside the allowed range.
    OutOfRange { key: String, message: String },
    /// Choice value is not in the allowed set.
    InvalidChoice {
        key: String,
        value: String,
        allowed: Vec<String>,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey(k) => write!(f, "unknown setting key: {}", k),
            Self::TypeMismatch { key, expected, got } => {
                write!(f, "{}: expected type {}, got {}", key, expected, got)
            }
            Self::OutOfRange { key, message } => {
                write!(f, "{}: {}", key, message)
            }
            Self::InvalidChoice {
                key,
                value,
                allowed,
            } => {
                write!(f, "{}: '{}' is not one of {:?}", key, value, allowed)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// A settings schema registry containing all known setting entries,
/// with nested categories and validation.
pub struct SettingsSchema {
    entries: std::collections::HashMap<String, SchemaEntry>,
}

impl SettingsSchema {
    /// Create a new empty schema.
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    /// Create a schema pre-populated with the built-in desktop settings.
    pub fn with_builtins() -> Self {
        let mut schema = Self::new();
        schema.register_builtins();
        schema
    }

    /// Register a schema entry.
    pub fn register(&mut self, entry: SchemaEntry) {
        self.entries.insert(entry.key.clone(), entry);
    }

    /// Look up a schema entry by key.
    pub fn get(&self, key: &str) -> Option<&SchemaEntry> {
        self.entries.get(key)
    }

    /// Return all registered keys (sorted).
    pub fn all_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.entries.keys().map(|k| k.as_str()).collect();
        keys.sort();
        keys
    }

    /// Return keys in a given category (exact prefix match on the category field).
    pub fn keys_in_category(&self, category: &str) -> Vec<&str> {
        let mut keys: Vec<&str> = self
            .entries
            .iter()
            .filter(|(_, e)| e.category == category)
            .map(|(k, _)| k.as_str())
            .collect();
        keys.sort();
        keys
    }

    /// Return all distinct categories.
    pub fn categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = self
            .entries
            .values()
            .map(|e| e.category.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        cats.sort();
        cats
    }

    /// Return the number of registered entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the schema is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Validate a value against the schema for a given key.
    pub fn validate(&self, key: &str, value: &SettingValue) -> Result<(), ValidationError> {
        let entry = self
            .entries
            .get(key)
            .ok_or_else(|| ValidationError::UnknownKey(key.to_string()))?;

        // Type check
        let expected_type = &entry.value_type;
        let actual_type = match value {
            SettingValue::Bool(_) => SchemaValueType::Bool,
            SettingValue::Int(_) => SchemaValueType::Int,
            SettingValue::Float(_) => SchemaValueType::Float,
            SettingValue::String(_) => SchemaValueType::Str,
            SettingValue::Choice(_) => SchemaValueType::Choice,
            SettingValue::Color { .. } => SchemaValueType::Color,
            SettingValue::KeyBinding(_) => SchemaValueType::KeyBinding,
            SettingValue::FilePath(_) => SchemaValueType::FilePath,
        };

        // Allow String and Choice to be interchangeable for validation
        let types_compatible = *expected_type == actual_type
            || (*expected_type == SchemaValueType::Choice && actual_type == SchemaValueType::Str)
            || (*expected_type == SchemaValueType::Str && actual_type == SchemaValueType::Choice);

        if !types_compatible {
            return Err(ValidationError::TypeMismatch {
                key: key.to_string(),
                expected: format!("{:?}", expected_type),
                got: format!("{:?}", actual_type),
            });
        }

        // Range validation for Int
        if let (SettingValue::Int(v), Some((min, max))) = (value, &entry.range_constraint) {
            if *v < *min || *v > *max {
                return Err(ValidationError::OutOfRange {
                    key: key.to_string(),
                    message: format!("{} is outside [{}, {}]", v, min, max),
                });
            }
        }

        // Range validation for Float
        if let (SettingValue::Float(v), Some((min, max))) = (value, &entry.float_range) {
            if *v < *min || *v > *max {
                return Err(ValidationError::OutOfRange {
                    key: key.to_string(),
                    message: format!("{} is outside [{}, {}]", v, min, max),
                });
            }
        }

        // Choice validation
        if let Some(ref allowed) = entry.enum_values {
            if let Some(s) = value.as_str() {
                if !allowed.contains(&s.to_string()) {
                    return Err(ValidationError::InvalidChoice {
                        key: key.to_string(),
                        value: s.to_string(),
                        allowed: allowed.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    // ── Built-in schema registration ─────────────────────────────────

    fn register_builtins(&mut self) {
        // ── appearance ──────────────────────────────────────────
        self.register(SchemaEntry {
            key: "appearance.theme".into(),
            value_type: SchemaValueType::Choice,
            default: SettingValue::Choice("liquid_glass".into()),
            summary: "Desktop theme".into(),
            description: "The visual theme applied to all shell elements".into(),
            range_constraint: None,
            float_range: None,
            enum_values: Some(vec![
                "liquid_glass".into(),
                "night".into(),
                "midday".into(),
                "sunset".into(),
            ]),
            category: "appearance".into(),
        });
        self.register(SchemaEntry {
            key: "appearance.font-family".into(),
            value_type: SchemaValueType::Str,
            default: SettingValue::String("Inter".into()),
            summary: "Default font family".into(),
            description: "The primary font family used for all UI text".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "appearance".into(),
        });
        self.register(SchemaEntry {
            key: "appearance.font-size".into(),
            value_type: SchemaValueType::Int,
            default: SettingValue::Int(14),
            summary: "Base font size".into(),
            description: "Base font size in pixels for UI text".into(),
            range_constraint: Some((6, 72)),
            float_range: None,
            enum_values: None,
            category: "appearance".into(),
        });
        self.register(SchemaEntry {
            key: "appearance.accent-color".into(),
            value_type: SchemaValueType::Color,
            default: SettingValue::Color {
                r: 0,
                g: 122,
                b: 255,
                a: 255,
            },
            summary: "Accent color".into(),
            description: "System accent color for highlights, focus rings, and selection".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "appearance".into(),
        });
        self.register(SchemaEntry {
            key: "appearance.icon-theme".into(),
            value_type: SchemaValueType::Str,
            default: SettingValue::String("default".into()),
            summary: "Icon theme".into(),
            description: "Icon theme for system and application icons".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "appearance".into(),
        });
        self.register(SchemaEntry {
            key: "appearance.cursor-theme".into(),
            value_type: SchemaValueType::Str,
            default: SettingValue::String("default".into()),
            summary: "Cursor theme".into(),
            description: "Mouse cursor theme".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "appearance".into(),
        });
        self.register(SchemaEntry {
            key: "appearance.cursor-size".into(),
            value_type: SchemaValueType::Int,
            default: SettingValue::Int(24),
            summary: "Cursor size".into(),
            description: "Mouse cursor size in pixels".into(),
            range_constraint: Some((16, 96)),
            float_range: None,
            enum_values: None,
            category: "appearance".into(),
        });

        // ── desktop ────────────────────────────────────────────
        self.register(SchemaEntry {
            key: "desktop.wallpaper".into(),
            value_type: SchemaValueType::FilePath,
            default: SettingValue::FilePath(String::new()),
            summary: "Wallpaper path".into(),
            description: "Path to the desktop wallpaper image file".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "desktop".into(),
        });
        self.register(SchemaEntry {
            key: "desktop.wallpaper-mode".into(),
            value_type: SchemaValueType::Choice,
            default: SettingValue::Choice("fill".into()),
            summary: "Wallpaper display mode".into(),
            description: "How the wallpaper is scaled and positioned".into(),
            range_constraint: None,
            float_range: None,
            enum_values: Some(vec![
                "fill".into(),
                "fit".into(),
                "stretch".into(),
                "tile".into(),
                "center".into(),
            ]),
            category: "desktop".into(),
        });
        self.register(SchemaEntry {
            key: "desktop.show-icons".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(true),
            summary: "Show desktop icons".into(),
            description: "Display file and folder icons on the desktop surface".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "desktop".into(),
        });
        self.register(SchemaEntry {
            key: "desktop.hot-corners".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Enable hot corners".into(),
            description: "Trigger actions when the cursor reaches screen corners".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "desktop".into(),
        });

        // ── dock ───────────────────────────────────────────────
        self.register(SchemaEntry {
            key: "dock.position".into(),
            value_type: SchemaValueType::Choice,
            default: SettingValue::Choice("bottom".into()),
            summary: "Dock position".into(),
            description: "Screen edge where the dock is placed".into(),
            range_constraint: None,
            float_range: None,
            enum_values: Some(vec!["bottom".into(), "left".into(), "right".into()]),
            category: "dock".into(),
        });
        self.register(SchemaEntry {
            key: "dock.auto-hide".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Auto-hide dock".into(),
            description: "Automatically hide the dock when not in use".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "dock".into(),
        });
        self.register(SchemaEntry {
            key: "dock.icon-size".into(),
            value_type: SchemaValueType::Int,
            default: SettingValue::Int(48),
            summary: "Dock icon size".into(),
            description: "Size of dock icons in pixels".into(),
            range_constraint: Some((24, 128)),
            float_range: None,
            enum_values: None,
            category: "dock".into(),
        });
        self.register(SchemaEntry {
            key: "dock.magnification".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Dock magnification".into(),
            description: "Magnify dock icons when hovered".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "dock".into(),
        });

        // ── window management ──────────────────────────────────
        self.register(SchemaEntry {
            key: "wm.focus-policy".into(),
            value_type: SchemaValueType::Choice,
            default: SettingValue::Choice("click".into()),
            summary: "Window focus policy".into(),
            description: "How windows receive keyboard focus".into(),
            range_constraint: None,
            float_range: None,
            enum_values: Some(vec![
                "click".into(),
                "sloppy".into(),
                "focus-follows-mouse".into(),
            ]),
            category: "wm".into(),
        });
        self.register(SchemaEntry {
            key: "wm.tiling-gap".into(),
            value_type: SchemaValueType::Int,
            default: SettingValue::Int(8),
            summary: "Tiling gap".into(),
            description: "Gap in pixels between tiled windows".into(),
            range_constraint: Some((0, 64)),
            float_range: None,
            enum_values: None,
            category: "wm".into(),
        });
        self.register(SchemaEntry {
            key: "wm.snap-enabled".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(true),
            summary: "Window snapping".into(),
            description: "Snap windows to screen edges when dragged".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "wm".into(),
        });
        self.register(SchemaEntry {
            key: "wm.titlebar-double-click".into(),
            value_type: SchemaValueType::Choice,
            default: SettingValue::Choice("maximize".into()),
            summary: "Titlebar double-click action".into(),
            description: "Action when double-clicking a window titlebar".into(),
            range_constraint: None,
            float_range: None,
            enum_values: Some(vec![
                "maximize".into(),
                "minimize".into(),
                "shade".into(),
                "nothing".into(),
            ]),
            category: "wm".into(),
        });

        // ── input ──────────────────────────────────────────────
        self.register(SchemaEntry {
            key: "input.mouse-speed".into(),
            value_type: SchemaValueType::Float,
            default: SettingValue::Float(1.0),
            summary: "Mouse speed".into(),
            description: "Mouse pointer acceleration multiplier".into(),
            range_constraint: None,
            float_range: Some((0.1, 3.0)),
            enum_values: None,
            category: "input".into(),
        });
        self.register(SchemaEntry {
            key: "input.natural-scroll".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Natural scrolling".into(),
            description: "Reverse scroll direction so content follows finger movement".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "input".into(),
        });
        self.register(SchemaEntry {
            key: "input.key-repeat-delay".into(),
            value_type: SchemaValueType::Int,
            default: SettingValue::Int(400),
            summary: "Key repeat delay".into(),
            description: "Delay in milliseconds before key repeat begins".into(),
            range_constraint: Some((100, 2000)),
            float_range: None,
            enum_values: None,
            category: "input".into(),
        });
        self.register(SchemaEntry {
            key: "input.key-repeat-rate".into(),
            value_type: SchemaValueType::Int,
            default: SettingValue::Int(30),
            summary: "Key repeat rate".into(),
            description: "Key repeats per second once repeat starts".into(),
            range_constraint: Some((1, 100)),
            float_range: None,
            enum_values: None,
            category: "input".into(),
        });
        self.register(SchemaEntry {
            key: "input.tap-to-click".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(true),
            summary: "Tap to click".into(),
            description: "Interpret touchpad taps as mouse clicks".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "input".into(),
        });
        self.register(SchemaEntry {
            key: "input.touchpad-speed".into(),
            value_type: SchemaValueType::Float,
            default: SettingValue::Float(1.0),
            summary: "Touchpad speed".into(),
            description: "Touchpad pointer acceleration multiplier".into(),
            range_constraint: None,
            float_range: Some((0.1, 3.0)),
            enum_values: None,
            category: "input".into(),
        });

        // ── display ────────────────────────────────────────────
        self.register(SchemaEntry {
            key: "display.dpi-scale".into(),
            value_type: SchemaValueType::Float,
            default: SettingValue::Float(1.0),
            summary: "Display scaling".into(),
            description: "UI scaling factor (1.0 = 100%)".into(),
            range_constraint: None,
            float_range: Some((0.5, 4.0)),
            enum_values: None,
            category: "display".into(),
        });
        self.register(SchemaEntry {
            key: "display.night-light".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Night light".into(),
            description: "Reduce blue light emission in the evening".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "display".into(),
        });
        self.register(SchemaEntry {
            key: "display.night-light-temperature".into(),
            value_type: SchemaValueType::Int,
            default: SettingValue::Int(4000),
            summary: "Night light temperature".into(),
            description: "Color temperature in Kelvin when night light is active".into(),
            range_constraint: Some((1700, 6500)),
            float_range: None,
            enum_values: None,
            category: "display".into(),
        });

        // ── power ──────────────────────────────────────────────
        self.register(SchemaEntry {
            key: "power.screen-blank-minutes".into(),
            value_type: SchemaValueType::Int,
            default: SettingValue::Int(5),
            summary: "Screen blank timeout".into(),
            description: "Minutes of inactivity before blanking the screen (0 = never)".into(),
            range_constraint: Some((0, 120)),
            float_range: None,
            enum_values: None,
            category: "power".into(),
        });
        self.register(SchemaEntry {
            key: "power.auto-suspend-minutes".into(),
            value_type: SchemaValueType::Int,
            default: SettingValue::Int(30),
            summary: "Auto-suspend timeout".into(),
            description: "Minutes of inactivity before automatic suspend (0 = never)".into(),
            range_constraint: Some((0, 480)),
            float_range: None,
            enum_values: None,
            category: "power".into(),
        });
        self.register(SchemaEntry {
            key: "power.lid-close-action".into(),
            value_type: SchemaValueType::Choice,
            default: SettingValue::Choice("suspend".into()),
            summary: "Lid close action".into(),
            description: "Action when the laptop lid is closed".into(),
            range_constraint: None,
            float_range: None,
            enum_values: Some(vec![
                "suspend".into(),
                "hibernate".into(),
                "shutdown".into(),
                "nothing".into(),
            ]),
            category: "power".into(),
        });
        self.register(SchemaEntry {
            key: "power.show-battery-percentage".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(true),
            summary: "Show battery percentage".into(),
            description: "Display battery charge percentage in the status bar".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "power".into(),
        });

        // ── notifications ──────────────────────────────────────
        self.register(SchemaEntry {
            key: "notifications.dnd".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Do not disturb".into(),
            description: "Suppress all notification popups".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "notifications".into(),
        });
        self.register(SchemaEntry {
            key: "notifications.show-on-lockscreen".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(true),
            summary: "Show on lock screen".into(),
            description: "Display notifications on the lock screen".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "notifications".into(),
        });
        self.register(SchemaEntry {
            key: "notifications.show-previews".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(true),
            summary: "Show previews".into(),
            description: "Show notification content in popup banners".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "notifications".into(),
        });
        self.register(SchemaEntry {
            key: "notifications.sound".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(true),
            summary: "Notification sound".into(),
            description: "Play a sound when notifications arrive".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "notifications".into(),
        });

        // ── accessibility ──────────────────────────────────────
        self.register(SchemaEntry {
            key: "a11y.high-contrast".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "High contrast".into(),
            description: "Increase contrast for better visibility".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "a11y".into(),
        });
        self.register(SchemaEntry {
            key: "a11y.large-text".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Large text".into(),
            description: "Increase text size throughout the interface".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "a11y".into(),
        });
        self.register(SchemaEntry {
            key: "a11y.reduce-motion".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Reduce motion".into(),
            description: "Minimize animations and transitions".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "a11y".into(),
        });
        self.register(SchemaEntry {
            key: "a11y.screen-reader".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Screen reader".into(),
            description: "Enable the built-in screen reader for accessibility".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "a11y".into(),
        });
        self.register(SchemaEntry {
            key: "a11y.sticky-keys".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Sticky keys".into(),
            description: "Modifier keys remain active after being pressed once".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "a11y".into(),
        });
        self.register(SchemaEntry {
            key: "a11y.slow-keys".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Slow keys".into(),
            description: "Keys must be held briefly before being accepted".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "a11y".into(),
        });

        // ── privacy ────────────────────────────────────────────
        self.register(SchemaEntry {
            key: "privacy.lock-on-suspend".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(true),
            summary: "Lock on suspend".into(),
            description: "Lock the screen when the system suspends".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "privacy".into(),
        });
        self.register(SchemaEntry {
            key: "privacy.auto-lock-minutes".into(),
            value_type: SchemaValueType::Int,
            default: SettingValue::Int(5),
            summary: "Auto-lock delay".into(),
            description: "Minutes of inactivity before the screen locks automatically".into(),
            range_constraint: Some((0, 120)),
            float_range: None,
            enum_values: None,
            category: "privacy".into(),
        });
        self.register(SchemaEntry {
            key: "privacy.location-services".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Location services".into(),
            description: "Allow applications to access location information".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "privacy".into(),
        });
        self.register(SchemaEntry {
            key: "privacy.usage-statistics".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Usage statistics".into(),
            description: "Collect anonymous usage statistics".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "privacy".into(),
        });

        // ── sound ──────────────────────────────────────────────
        self.register(SchemaEntry {
            key: "sound.output-volume".into(),
            value_type: SchemaValueType::Int,
            default: SettingValue::Int(75),
            summary: "Output volume".into(),
            description: "System audio output volume (0-100)".into(),
            range_constraint: Some((0, 100)),
            float_range: None,
            enum_values: None,
            category: "sound".into(),
        });
        self.register(SchemaEntry {
            key: "sound.input-volume".into(),
            value_type: SchemaValueType::Int,
            default: SettingValue::Int(50),
            summary: "Input volume".into(),
            description: "Microphone input volume (0-100)".into(),
            range_constraint: Some((0, 100)),
            float_range: None,
            enum_values: None,
            category: "sound".into(),
        });
        self.register(SchemaEntry {
            key: "sound.mute".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(false),
            summary: "Mute".into(),
            description: "Mute all audio output".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "sound".into(),
        });
        self.register(SchemaEntry {
            key: "sound.event-sounds".into(),
            value_type: SchemaValueType::Bool,
            default: SettingValue::Bool(true),
            summary: "Event sounds".into(),
            description: "Play sounds for system events (login, logout, etc.)".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "sound".into(),
        });

        // ── region ─────────────────────────────────────────────
        self.register(SchemaEntry {
            key: "region.language".into(),
            value_type: SchemaValueType::Str,
            default: SettingValue::String("en_US".into()),
            summary: "Language".into(),
            description: "System language and locale".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "region".into(),
        });
        self.register(SchemaEntry {
            key: "region.timezone".into(),
            value_type: SchemaValueType::Str,
            default: SettingValue::String("UTC".into()),
            summary: "Timezone".into(),
            description: "System timezone (IANA timezone identifier)".into(),
            range_constraint: None,
            float_range: None,
            enum_values: None,
            category: "region".into(),
        });
        self.register(SchemaEntry {
            key: "region.clock-format".into(),
            value_type: SchemaValueType::Choice,
            default: SettingValue::Choice("24h".into()),
            summary: "Clock format".into(),
            description: "Time display format in the status bar".into(),
            range_constraint: None,
            float_range: None,
            enum_values: Some(vec!["12h".into(), "24h".into()]),
            category: "region".into(),
        });
    }
}
