use crate::cursor::{CursorShape, CursorImage};
use std::collections::HashMap;
use std::path::PathBuf;

/// A cursor theme — maps shapes to images
#[derive(Debug, Clone)]
pub struct CursorTheme {
    pub name: String,
    pub display_name: String,
    pub comment: String,
    pub default_size: u32,
    pub inherits: Option<String>,
    cursors: HashMap<CursorShape, Vec<CursorImage>>,  // multiple sizes
}

impl CursorTheme {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            display_name: name.to_string(),
            comment: String::new(),
            default_size: 24,
            inherits: None,
            cursors: HashMap::new(),
        }
    }

    pub fn add_cursor(&mut self, shape: CursorShape, image: CursorImage) {
        self.cursors.entry(shape).or_default().push(image);
    }

    /// Get cursor image for shape at closest available size
    pub fn get_cursor(&self, shape: CursorShape, size: u32) -> Option<&CursorImage> {
        let images = self.cursors.get(&shape)?;
        if images.is_empty() {
            return None;
        }

        // Find closest size
        images.iter()
            .min_by_key(|img| (img.nominal_size as i32 - size as i32).unsigned_abs())
    }

    /// Get all available sizes for a shape
    pub fn available_sizes(&self, shape: CursorShape) -> Vec<u32> {
        self.cursors.get(&shape)
            .map(|images| images.iter().map(|i| i.nominal_size).collect())
            .unwrap_or_default()
    }

    /// Check if this theme has a cursor for the given shape
    pub fn has_cursor(&self, shape: CursorShape) -> bool {
        self.cursors.get(&shape).map(|v| !v.is_empty()).unwrap_or(false)
    }

    /// Number of shapes defined
    pub fn shape_count(&self) -> usize {
        self.cursors.len()
    }
}

/// Cursor theme manager — handles loading and switching themes
pub struct CursorThemeManager {
    pub(crate) themes: HashMap<String, CursorTheme>,
    active_theme: String,
    search_paths: Vec<PathBuf>,
    default_size: u32,
}

impl CursorThemeManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            themes: HashMap::new(),
            active_theme: "default".to_string(),
            search_paths: Vec::new(),
            default_size: 24,
        };

        // Add standard search paths
        #[cfg(target_os = "linux")]
        {
            mgr.search_paths.push(PathBuf::from("/usr/share/icons"));
            mgr.search_paths.push(PathBuf::from("/usr/local/share/icons"));
            if let Ok(home) = std::env::var("HOME") {
                mgr.search_paths.push(PathBuf::from(home).join(".local/share/icons"));
                mgr.search_paths.push(PathBuf::from(home).join(".icons"));
            }
        }
        #[cfg(target_os = "windows")]
        {
            if let Ok(windir) = std::env::var("SystemRoot") {
                mgr.search_paths.push(PathBuf::from(windir).join("Cursors"));
            }
        }

        // Register builtin theme
        mgr.themes.insert("default".to_string(), crate::builtin::create_builtin_theme());

        mgr
    }

    /// Discover available cursor themes from search paths
    pub fn discover_themes(&mut self) -> Vec<String> {
        let mut found = Vec::new();

        for path in &self.search_paths.clone() {
            if !path.is_dir() { continue; }
            let entries = match std::fs::read_dir(path) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let dir = entry.path();
                if !dir.is_dir() { continue; }

                // Check for cursor theme indicator
                let cursor_dir = dir.join("cursors");
                let index_file = dir.join("cursor.theme");

                if cursor_dir.is_dir() || index_file.exists() {
                    let name = dir.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    if !name.is_empty() && !self.themes.contains_key(&name) {
                        found.push(name.clone());
                        // Create placeholder theme entry
                        let mut theme = CursorTheme::new(&name);
                        // Try to parse cursor.theme/index.theme for metadata
                        if let Ok(content) = std::fs::read_to_string(dir.join("cursor.theme")) {
                            parse_theme_file(&content, &mut theme);
                        } else if let Ok(content) = std::fs::read_to_string(dir.join("index.theme")) {
                            parse_theme_file(&content, &mut theme);
                        }
                        self.themes.insert(name, theme);
                    }
                }
            }
        }

        found
    }

    /// Set the active theme
    pub fn set_active(&mut self, name: &str) -> bool {
        if self.themes.contains_key(name) {
            self.active_theme = name.to_string();
            true
        } else {
            false
        }
    }

    /// Get cursor for shape from active theme
    pub fn get_cursor(&self, shape: CursorShape) -> Option<&CursorImage> {
        let theme = self.themes.get(&self.active_theme)?;
        theme.get_cursor(shape, self.default_size)
            .or_else(|| {
                // Fall back to "default" theme
                self.themes.get("default")?.get_cursor(shape, self.default_size)
            })
    }

    /// List available themes
    pub fn list_themes(&self) -> Vec<(&str, &str)> {
        self.themes.iter()
            .map(|(name, theme)| (name.as_str(), theme.display_name.as_str()))
            .collect()
    }

    /// Get active theme name
    pub fn active_theme(&self) -> &str {
        &self.active_theme
    }

    pub fn set_default_size(&mut self, size: u32) {
        self.default_size = size;
    }
}

impl Default for CursorThemeManager {
    fn default() -> Self { Self::new() }
}

pub(crate) fn parse_theme_file(content: &str, theme: &mut CursorTheme) {
    for line in content.lines() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "Name" => theme.display_name = value.trim().to_string(),
                "Comment" => theme.comment = value.trim().to_string(),
                "Inherits" => theme.inherits = Some(value.trim().to_string()),
                "Size" => {
                    if let Ok(s) = value.trim().parse() {
                        theme.default_size = s;
                    }
                }
                _ => {}
            }
        }
    }
}
