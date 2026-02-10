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
