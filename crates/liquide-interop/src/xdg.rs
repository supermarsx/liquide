use std::fmt;

use serde::{Deserialize, Serialize};

/// XDG Base Directory paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XdgDirs {
    pub data_home: String,
    pub config_home: String,
    pub cache_home: String,
    pub state_home: String,
    pub runtime_dir: Option<String>,
    pub data_dirs: Vec<String>,
    pub config_dirs: Vec<String>,
}

impl XdgDirs {
    /// Create XDG dirs with default paths under `/home/<user>`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_home("/home/user")
    }

    /// Create XDG dirs based on a specific home directory.
    #[must_use]
    pub fn with_home(home: &str) -> Self {
        Self {
            data_home: format!("{home}/.local/share"),
            config_home: format!("{home}/.config"),
            cache_home: format!("{home}/.cache"),
            state_home: format!("{home}/.local/state"),
            runtime_dir: None,
            data_dirs: vec![
                "/usr/local/share".to_string(),
                "/usr/share".to_string(),
            ],
            config_dirs: vec!["/etc/xdg".to_string()],
        }
    }

    /// Resolve a data file name against data_home and data_dirs.
    #[must_use]
    pub fn find_data_file(&self, name: &str) -> Vec<String> {
        let mut paths = vec![format!("{}/{name}", self.data_home)];
        for dir in &self.data_dirs {
            paths.push(format!("{dir}/{name}"));
        }
        paths
    }

    /// Resolve a config file name against config_home and config_dirs.
    #[must_use]
    pub fn find_config_file(&self, name: &str) -> Vec<String> {
        let mut paths = vec![format!("{}/{name}", self.config_home)];
        for dir in &self.config_dirs {
            paths.push(format!("{dir}/{name}"));
        }
        paths
    }
}

impl Default for XdgDirs {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for XdgDirs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "XdgDirs(data={}, config={}, cache={})",
            self.data_home, self.config_home, self.cache_home
        )
    }
}
