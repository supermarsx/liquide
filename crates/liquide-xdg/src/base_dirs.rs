//! XDG Base Directory Specification (v0.8).
//!
//! Provides platform-aware directory resolution following the freedesktop.org
//! Base Directory Specification. On Linux, the standard `$XDG_*` environment
//! variables are respected with well-known fallbacks. On other platforms,
//! sensible defaults are chosen that mirror the same organisational intent.

use std::env;
use std::fmt;
use std::path::{Path, PathBuf};

/// XDG base directory paths for a single user session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XdgDirs {
    /// User-specific data files (`$XDG_DATA_HOME`).
    pub data_home: PathBuf,
    /// User-specific configuration files (`$XDG_CONFIG_HOME`).
    pub config_home: PathBuf,
    /// User-specific non-essential cached data (`$XDG_CACHE_HOME`).
    pub cache_home: PathBuf,
    /// User-specific state data (`$XDG_STATE_HOME`).
    pub state_home: PathBuf,
    /// User-specific runtime files (`$XDG_RUNTIME_DIR`).
    /// This may be `None` when a secure runtime directory cannot be determined.
    pub runtime_dir: Option<PathBuf>,
}

/// Errors that can occur when resolving XDG directories.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XdgError {
    /// The user home directory could not be determined.
    NoHomeDirectory,
    /// A required directory could not be created.
    CreateFailed(String),
}

impl fmt::Display for XdgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XdgError::NoHomeDirectory => write!(f, "could not determine home directory"),
            XdgError::CreateFailed(path) => write!(f, "failed to create directory: {path}"),
        }
    }
}

impl std::error::Error for XdgError {}

/// Return the user's home directory from environment variables.
///
/// Checks `$HOME` first, then platform-specific variables.
fn home_dir() -> Option<PathBuf> {
    if let Ok(home) = env::var("HOME") {
        if !home.is_empty() {
            return Some(PathBuf::from(home));
        }
    }
    // Fallback: USERPROFILE on platforms that set it.
    if let Ok(profile) = env::var("USERPROFILE") {
        if !profile.is_empty() {
            return Some(PathBuf::from(profile));
        }
    }
    None
}

impl XdgDirs {
    /// Resolve XDG base directories from environment variables and defaults.
    ///
    /// On Linux the well-known `$XDG_*` variables are honoured with
    /// fallbacks to `$HOME/.local/share`, `$HOME/.config`, etc.
    ///
    /// On other platforms the paths are chosen to mirror the same intent:
    /// - data_home   -> `<home>/AppData/Local/share`   (or `<home>/.local/share` if no APPDATA)
    /// - config_home -> `<home>/AppData/Local/config`  (or `<home>/.config`)
    /// - cache_home  -> `<home>/AppData/Local/cache`   (or `<home>/.cache`)
    /// - state_home  -> `<home>/AppData/Local/state`   (or `<home>/.local/state`)
    /// - runtime_dir -> `$XDG_RUNTIME_DIR` or temp dir fallback
    pub fn new() -> Result<Self, XdgError> {
        let home = home_dir().ok_or(XdgError::NoHomeDirectory)?;

        let data_home = env_or_default("XDG_DATA_HOME", || default_data_home(&home));
        let config_home = env_or_default("XDG_CONFIG_HOME", || default_config_home(&home));
        let cache_home = env_or_default("XDG_CACHE_HOME", || default_cache_home(&home));
        let state_home = env_or_default("XDG_STATE_HOME", || default_state_home(&home));
        let runtime_dir = resolve_runtime_dir();

        Ok(XdgDirs {
            data_home,
            config_home,
            cache_home,
            state_home,
            runtime_dir,
        })
    }

    /// Create a new `XdgDirs` using the given path as the home directory,
    /// ignoring all environment variables. Uses the canonical XDG paths
    /// relative to `home`. Useful for testing and sandboxed environments.
    pub fn with_home(home: &Path) -> Self {
        XdgDirs {
            data_home: home.join(".local").join("share"),
            config_home: home.join(".config"),
            cache_home: home.join(".cache"),
            state_home: home.join(".local").join("state"),
            runtime_dir: None,
        }
    }

    /// Create all base directories if they do not yet exist.
    pub fn ensure_dirs(&self) -> Result<(), XdgError> {
        let dirs = [
            &self.data_home,
            &self.config_home,
            &self.cache_home,
            &self.state_home,
        ];
        for dir in dirs {
            create_dir_if_missing(dir)?;
        }
        if let Some(ref rt) = self.runtime_dir {
            create_dir_if_missing(rt)?;
        }
        Ok(())
    }

    /// Return a path inside `data_home` for the given application.
    pub fn data_home_for(&self, app: &str) -> PathBuf {
        self.data_home.join(app)
    }

    /// Return a path inside `config_home` for the given application.
    pub fn config_home_for(&self, app: &str) -> PathBuf {
        self.config_home.join(app)
    }

    /// Return a path inside `cache_home` for the given application.
    pub fn cache_home_for(&self, app: &str) -> PathBuf {
        self.cache_home.join(app)
    }

    /// Return a path inside `state_home` for the given application.
    pub fn state_home_for(&self, app: &str) -> PathBuf {
        self.state_home.join(app)
    }
}

/// System-wide data directories (`$XDG_DATA_DIRS`).
///
/// Returns the colon-separated list from the environment, or the default
/// `/usr/local/share:/usr/share`.
pub fn data_dirs() -> Vec<PathBuf> {
    split_dirs_env("XDG_DATA_DIRS", &["/usr/local/share", "/usr/share"])
}

/// System-wide configuration directories (`$XDG_CONFIG_DIRS`).
///
/// Returns the colon-separated list from the environment, or `/etc/xdg`.
pub fn config_dirs() -> Vec<PathBuf> {
    split_dirs_env("XDG_CONFIG_DIRS", &["/etc/xdg"])
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn env_or_default<F: FnOnce() -> PathBuf>(var: &str, default: F) -> PathBuf {
    match env::var(var) {
        Ok(val) if !val.is_empty() => PathBuf::from(val),
        _ => default(),
    }
}

fn default_data_home(home: &Path) -> PathBuf {
    if let Ok(appdata) = env::var("LOCALAPPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("share");
        }
    }
    home.join(".local").join("share")
}

fn default_config_home(home: &Path) -> PathBuf {
    if let Ok(appdata) = env::var("LOCALAPPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("config");
        }
    }
    home.join(".config")
}

fn default_cache_home(home: &Path) -> PathBuf {
    if let Ok(appdata) = env::var("LOCALAPPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("cache");
        }
    }
    home.join(".cache")
}

fn default_state_home(home: &Path) -> PathBuf {
    if let Ok(appdata) = env::var("LOCALAPPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("state");
        }
    }
    home.join(".local").join("state")
}

fn resolve_runtime_dir() -> Option<PathBuf> {
    if let Ok(val) = env::var("XDG_RUNTIME_DIR") {
        if !val.is_empty() {
            return Some(PathBuf::from(val));
        }
    }
    // Fallback: use the system temporary directory.
    let tmp = env::temp_dir();
    if tmp.exists() {
        Some(tmp)
    } else {
        None
    }
}

fn split_dirs_env(var: &str, defaults: &[&str]) -> Vec<PathBuf> {
    match env::var(var) {
        Ok(val) if !val.is_empty() => {
            let sep = if val.contains(';') { ';' } else { ':' };
            val.split(sep)
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect()
        }
        _ => defaults.iter().map(|s| PathBuf::from(s)).collect(),
    }
}

fn create_dir_if_missing(path: &Path) -> Result<(), XdgError> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|_| XdgError::CreateFailed(path.display().to_string()))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn with_home_sets_data_home() {
        let dirs = XdgDirs::with_home(Path::new("/fakehome"));
        assert_eq!(dirs.data_home, PathBuf::from("/fakehome/.local/share"));
    }

    #[test]
    fn with_home_sets_config_home() {
        let dirs = XdgDirs::with_home(Path::new("/fakehome"));
        assert_eq!(dirs.config_home, PathBuf::from("/fakehome/.config"));
    }

    #[test]
    fn with_home_sets_cache_home() {
        let dirs = XdgDirs::with_home(Path::new("/fakehome"));
        assert_eq!(dirs.cache_home, PathBuf::from("/fakehome/.cache"));
    }

    #[test]
    fn with_home_sets_state_home() {
        let dirs = XdgDirs::with_home(Path::new("/fakehome"));
        assert_eq!(dirs.state_home, PathBuf::from("/fakehome/.local/state"));
    }

    #[test]
    fn with_home_runtime_is_none() {
        let dirs = XdgDirs::with_home(Path::new("/fakehome"));
        assert_eq!(dirs.runtime_dir, None);
    }

    #[test]
    fn data_home_for_app() {
        let dirs = XdgDirs::with_home(Path::new("/h"));
        assert_eq!(dirs.data_home_for("myapp"), PathBuf::from("/h/.local/share/myapp"));
    }

    #[test]
    fn config_home_for_app() {
        let dirs = XdgDirs::with_home(Path::new("/h"));
        assert_eq!(dirs.config_home_for("myapp"), PathBuf::from("/h/.config/myapp"));
    }

    #[test]
    fn cache_home_for_app() {
        let dirs = XdgDirs::with_home(Path::new("/h"));
        assert_eq!(dirs.cache_home_for("myapp"), PathBuf::from("/h/.cache/myapp"));
    }

    #[test]
    fn state_home_for_app() {
        let dirs = XdgDirs::with_home(Path::new("/h"));
        assert_eq!(dirs.state_home_for("myapp"), PathBuf::from("/h/.local/state/myapp"));
    }

    #[test]
    fn ensure_dirs_creates_directories() {
        let tmp = env::temp_dir().join("liquide_xdg_test_ensure");
        let _ = std::fs::remove_dir_all(&tmp);
        let dirs = XdgDirs::with_home(&tmp);
        dirs.ensure_dirs().unwrap();
        assert!(dirs.data_home.exists());
        assert!(dirs.config_home.exists());
        assert!(dirs.cache_home.exists());
        assert!(dirs.state_home.exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn data_dirs_returns_defaults() {
        // When XDG_DATA_DIRS is not set, we get the standard defaults.
        // (We cannot guarantee the env var is unset in CI, so just check non-empty.)
        let dirs = data_dirs();
        assert!(!dirs.is_empty());
    }

    #[test]
    fn config_dirs_returns_defaults() {
        let dirs = config_dirs();
        assert!(!dirs.is_empty());
    }

    #[test]
    fn xdg_error_display() {
        let e = XdgError::NoHomeDirectory;
        assert_eq!(e.to_string(), "could not determine home directory");
        let e2 = XdgError::CreateFailed("/x".into());
        assert_eq!(e2.to_string(), "failed to create directory: /x");
    }

    #[test]
    fn env_or_default_uses_fallback() {
        // Env var that almost certainly does not exist.
        let result = env_or_default("_LIQUIDE_XDG_TEST_NONEXISTENT_12345", || {
            PathBuf::from("/fallback")
        });
        assert_eq!(result, PathBuf::from("/fallback"));
    }

    #[test]
    fn split_dirs_env_semicolon() {
        // SAFETY: This test is single-threaded and uses a unique env var name.
        unsafe { env::set_var("_LIQUIDE_XDG_SPLIT_TEST", "/a;/b;/c") };
        let dirs = split_dirs_env("_LIQUIDE_XDG_SPLIT_TEST", &["/default"]);
        assert_eq!(dirs, vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ]);
        unsafe { env::remove_var("_LIQUIDE_XDG_SPLIT_TEST") };
    }
}
