//! Individual setting entries with typed values.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::category::Category;

/// The type and constraints of a setting value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SettingKind {
    /// Boolean on/off toggle.
    Toggle,
    /// Numeric slider with min, max, step.
    Slider { min: f64, max: f64, step: f64 },
    /// Choice from a list of options.
    Choice { options: Vec<String> },
    /// Free-form text field.
    Text { max_length: usize },
    /// Color picker (stored as #RRGGBB).
    Color,
    /// Key binding (stored as string like "Ctrl+Shift+T").
    KeyBind,
    /// Percentage (0..=100).
    Percentage,
}

/// A concrete value stored for a setting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SettingValue {
    Bool(bool),
    Number(f64),
    Text(String),
}

impl fmt::Display for SettingValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Number(n) => write!(f, "{n}"),
            Self::Text(s) => f.write_str(s),
        }
    }
}

/// A single setting entry.
#[derive(Debug, Clone)]
pub struct SettingEntry {
    /// Unique key, e.g. "display.resolution".
    pub key: String,
    /// Human-readable label.
    pub label: String,
    /// Description shown as tooltip/subtitle.
    pub description: String,
    /// Category this setting belongs to.
    pub category: Category,
    /// Section within the category (for grouping).
    pub section: String,
    /// Type and constraints.
    pub kind: SettingKind,
    /// Current value.
    pub value: SettingValue,
    /// Default value.
    pub default: SettingValue,
    /// Whether this is an advanced setting.
    pub advanced: bool,
    /// Search keywords beyond label/description.
    pub keywords: Vec<String>,
}

impl SettingEntry {
    /// Create a new toggle setting.
    #[must_use]
    pub fn toggle(
        key: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        category: Category,
        section: impl Into<String>,
        default: bool,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            description: description.into(),
            category,
            section: section.into(),
            kind: SettingKind::Toggle,
            value: SettingValue::Bool(default),
            default: SettingValue::Bool(default),
            advanced: false,
            keywords: Vec::new(),
        }
    }

    /// Create a new slider setting.
    #[must_use]
    pub fn slider(
        key: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        category: Category,
        section: impl Into<String>,
        min: f64,
        max: f64,
        step: f64,
        default: f64,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            description: description.into(),
            category,
            section: section.into(),
            kind: SettingKind::Slider { min, max, step },
            value: SettingValue::Number(default),
            default: SettingValue::Number(default),
            advanced: false,
            keywords: Vec::new(),
        }
    }

    /// Create a new choice setting.
    #[must_use]
    pub fn choice(
        key: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        category: Category,
        section: impl Into<String>,
        options: Vec<String>,
        default: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            description: description.into(),
            category,
            section: section.into(),
            kind: SettingKind::Choice { options },
            value: SettingValue::Text(default.into()),
            default: SettingValue::Text(String::new()),
            advanced: false,
            keywords: Vec::new(),
        }
    }

    /// Create a new text setting.
    #[must_use]
    pub fn text(
        key: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        category: Category,
        section: impl Into<String>,
        max_length: usize,
        default: impl Into<String>,
    ) -> Self {
        let d: String = default.into();
        Self {
            key: key.into(),
            label: label.into(),
            description: description.into(),
            category,
            section: section.into(),
            kind: SettingKind::Text { max_length },
            value: SettingValue::Text(d.clone()),
            default: SettingValue::Text(d),
            advanced: false,
            keywords: Vec::new(),
        }
    }

    /// Whether the current value differs from the default.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.value != self.default
    }

    /// Reset to the default value.
    pub fn reset(&mut self) {
        self.value = self.default.clone();
    }

    /// Validate a proposed value against this entry's constraints.
    pub fn validate(&self, value: &SettingValue) -> crate::Result<()> {
        match (&self.kind, value) {
            (SettingKind::Toggle, SettingValue::Bool(_)) => Ok(()),
            (SettingKind::Slider { min, max, .. }, SettingValue::Number(n)) => {
                if *n < *min || *n > *max {
                    Err(crate::SettingsError::InvalidValue {
                        key: self.key.clone(),
                        reason: format!("value {n} out of range [{min}, {max}]"),
                    })
                } else {
                    Ok(())
                }
            }
            (SettingKind::Choice { options }, SettingValue::Text(t)) => {
                if options.contains(t) {
                    Ok(())
                } else {
                    Err(crate::SettingsError::InvalidValue {
                        key: self.key.clone(),
                        reason: format!("'{t}' is not a valid option"),
                    })
                }
            }
            (SettingKind::Text { max_length }, SettingValue::Text(t)) => {
                if t.len() > *max_length {
                    Err(crate::SettingsError::InvalidValue {
                        key: self.key.clone(),
                        reason: format!("text length {} exceeds max {max_length}", t.len()),
                    })
                } else {
                    Ok(())
                }
            }
            (SettingKind::Color, SettingValue::Text(t)) => {
                if t.starts_with('#')
                    && t.len() == 7
                    && t[1..].chars().all(|c| c.is_ascii_hexdigit())
                {
                    Ok(())
                } else {
                    Err(crate::SettingsError::InvalidValue {
                        key: self.key.clone(),
                        reason: format!("'{t}' is not a valid #RRGGBB color"),
                    })
                }
            }
            (SettingKind::Percentage, SettingValue::Number(n)) => {
                if *n < 0.0 || *n > 100.0 {
                    Err(crate::SettingsError::InvalidValue {
                        key: self.key.clone(),
                        reason: format!("percentage {n} out of range [0, 100]"),
                    })
                } else {
                    Ok(())
                }
            }
            (SettingKind::KeyBind, SettingValue::Text(_)) => Ok(()),
            _ => Err(crate::SettingsError::InvalidValue {
                key: self.key.clone(),
                reason: "type mismatch".into(),
            }),
        }
    }
}
