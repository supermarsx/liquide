use std::fmt;

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
