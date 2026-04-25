use std::path::PathBuf;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

/// liquidctl client-side config file (§6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtlConfig {
    #[serde(default)]
    pub default: DefaultProfile,

    #[serde(default)]
    pub remote: std::collections::HashMap<String, RemoteProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultProfile {
    #[serde(default = "default_server")]
    pub server: String,

    #[serde(default = "default_format")]
    pub format: String,

    #[serde(default = "default_color")]
    pub color: String,
}

impl Default for DefaultProfile {
    fn default() -> Self {
        Self {
            server: default_server(),
            format: default_format(),
            color: default_color(),
        }
    }
}

fn default_server() -> String {
    "unix:///run/liquide/ctl.sock".to_string()
}

fn default_format() -> String {
    "text".to_string()
}

fn default_color() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteProfile {
    pub server: String,
    pub api_key: Option<String>,
}

impl CtlConfig {
    /// Load config from the default path. Returns default config if file doesn't exist.
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let config: Self =
            toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
        Ok(config)
    }

    /// Resolve the server address. If it starts with `@`, look up a remote profile.
    pub fn resolve_server(&self, server: Option<&str>) -> anyhow::Result<String> {
        match server {
            Some(s) if s.starts_with('@') => {
                let profile_name = &s[1..];
                let profile = self
                    .remote
                    .get(profile_name)
                    .ok_or_else(|| anyhow::anyhow!("Unknown remote profile: {profile_name}"))?;
                Ok(profile.server.clone())
            }
            Some(s) => Ok(s.to_string()),
            None => Ok(self.default.server.clone()),
        }
    }

    /// Resolve the API key. Prefers explicit --api-key, then falls back to remote profile.
    pub fn resolve_api_key(&self, api_key: Option<&str>, server: Option<&str>) -> Option<String> {
        if let Some(key) = api_key {
            return Some(key.to_string());
        }
        if let Some(s) = server {
            if s.starts_with('@') {
                let profile_name = &s[1..];
                if let Some(profile) = self.remote.get(profile_name) {
                    return profile.api_key.clone();
                }
            }
        }
        None
    }

    /// Config file path per spec §6.
    fn config_path() -> PathBuf {
        if cfg!(windows) {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("liquidctl")
                .join("config.toml")
        } else {
            dirs::config_dir()
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join(".config")
                })
                .join("liquidctl")
                .join("config.toml")
        }
    }
}

impl Default for CtlConfig {
    fn default() -> Self {
        Self {
            default: DefaultProfile::default(),
            remote: std::collections::HashMap::new(),
        }
    }
}
