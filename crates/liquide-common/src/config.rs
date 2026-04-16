//! Configuration file loading and parsing utilities.

use serde::de::DeserializeOwned;
use std::path::Path;

use crate::error::Result;

/// Load a TOML configuration file from disk and deserialize it into `T`.
///
/// # Errors
///
/// Returns [`LiquideError::Io`] if the file cannot be read, or
/// [`LiquideError::Toml`] if deserialization fails.
pub fn load_toml<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let content = std::fs::read_to_string(path)?;
    let value: T = toml::from_str(&content)?;
    Ok(value)
}

/// Attempt to locate the default configuration directory.
///
/// Returns `$XDG_CONFIG_HOME/liquide` when set, otherwise `~/.config/liquide`.
pub fn default_config_dir() -> Result<std::path::PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let mut home = dirs_fallback();
            home.push(".config");
            home
        });
    Ok(base.join("liquide"))
}

/// Simple fallback for the home directory when the `dirs` crate is not available.
fn dirs_fallback() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/"))
}

/// Marker trait for types that represent a Liquide configuration section.
pub trait ConfigSection: DeserializeOwned + std::fmt::Debug {
    /// The section name inside the TOML file (e.g. `"server"`, `"client"`).
    fn section_name() -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct TestConfig {
        name: String,
        port: u16,
    }

    impl ConfigSection for TestConfig {
        fn section_name() -> &'static str {
            "test"
        }
    }

    #[test]
    fn test_load_toml_valid() {
        let dir = std::env::temp_dir().join("liquide_test_config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("valid.toml");
        std::fs::write(&path, "name = \"hello\"\nport = 8080\n").unwrap();
        let config: TestConfig = load_toml(&path).unwrap();
        assert_eq!(config.name, "hello");
        assert_eq!(config.port, 8080);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_toml_invalid_content() {
        let dir = std::env::temp_dir().join("liquide_test_config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("invalid.toml");
        std::fs::write(&path, "not valid {{{{ toml").unwrap();
        let result: crate::error::Result<TestConfig> = load_toml(&path);
        assert!(result.is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_toml_missing_file() {
        let path = std::path::PathBuf::from("nonexistent_liquide_file_98765.toml");
        let result: crate::error::Result<TestConfig> = load_toml(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_config_dir() {
        let dir = default_config_dir();
        assert!(dir.is_ok());
        let path = dir.unwrap();
        assert!(path.to_string_lossy().contains("liquide"));
    }

    #[test]
    fn test_config_section_trait() {
        assert_eq!(TestConfig::section_name(), "test");
    }
}
