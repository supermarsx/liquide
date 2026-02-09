//! Theme loading and management.

use crate::Theme;
use std::path::Path;

/// Load a theme from a directory containing CSS files.
///
/// Each `.css` file in the directory is parsed and added to the theme in
/// alphabetical order.
pub fn load_theme(name: &str, _dir: &Path) -> crate::Result<Theme> {
    // Stub— real implementation would read the directory.
    Ok(Theme::new(name))
}

/// The built-in default (light) theme name.
pub const DEFAULT_THEME: &str = "liquide-light";

/// The built-in dark theme name.
pub const DARK_THEME: &str = "liquide-dark";
