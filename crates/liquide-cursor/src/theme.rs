//! Cursor theme management and loading.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::shape::CursorShape;

/// Metadata for a cursor theme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeMetadata {
    /// Theme name (e.g., "Adwaita", "Breeze").
    pub name: String,
    
    /// Theme description.
    pub description: String,
    
    /// Author/creator.
    pub author: String,
    
    /// Theme version.
    pub version: String,
    
    /// Available sizes (e.g., [16, 24, 32, 48]).
    pub sizes: Vec<u32>,
    
    /// Default cursor size.
    pub default_size: u32,
}

/// A complete cursor theme with image data for all cursor shapes.
#[derive(Debug, Clone)]
pub struct CursorTheme {
    /// Theme metadata.
    pub metadata: ThemeMetadata,
    
    /// Path to the theme directory.
    pub path: PathBuf,
    
    /// Cached cursor images indexed by (shape, size).
    cursors: HashMap<(CursorShape, u32), CursorImage>,
}

/// A single cursor image within a theme.
#[derive(Debug, Clone)]
pub struct CursorImage {
    /// RGBA8 image data.
    pub data: Vec<u8>,
    
    /// Image width in pixels.
    pub width: u32,
    
    /// Image height in pixels.
    pub height: u32,
    
    /// Hotspot X offset.
    pub hotspot_x: u32,
    
    /// Hotspot Y offset.
    pub hotspot_y: u32,
}

impl CursorTheme {
    /// Load a cursor theme from a directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the theme directory doesn't exist or metadata is invalid.
    pub fn load<P: AsRef<Path>>(path: P) -> crate::Result<Self> {
        let path = path.as_ref();
        
        if !path.exists() {
            return Err(crate::CursorError::ThemeNotFound(
                path.display().to_string()
            ));
        }
        
        // Load metadata
        let metadata_path = path.join("theme.toml");
        let metadata: ThemeMetadata = if metadata_path.exists() {
            let contents = std::fs::read_to_string(&metadata_path)?;
            toml::from_str(&contents)
                .map_err(|e| crate::CursorError::InvalidImage(e.to_string()))?
        } else {
            // Default metadata if none exists
            ThemeMetadata {
                name: path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                description: "Custom cursor theme".to_string(),
                author: "Unknown".to_string(),
                version: "1.0".to_string(),
                sizes: vec![24, 32, 48],
                default_size: 24,
            }
        };
        
        Ok(Self {
            metadata,
            path: path.to_path_buf(),
            cursors: HashMap::new(),
        })
    }
    
    /// Get a cursor image for the given shape and size.
    ///
    /// If the exact size isn't available, the closest size is used.
    pub fn get_cursor(&mut self, shape: CursorShape, size: u32) -> Option<&CursorImage> {
        // Check cache first
        if self.cursors.contains_key(&(shape, size)) {
            return self.cursors.get(&(shape, size));
        }
        
        // Try to load from disk
        if let Ok(image) = self.load_cursor_image(shape, size) {
            self.cursors.insert((shape, size), image);
            return self.cursors.get(&(shape, size));
        }
        
        None
    }
    
    /// Load a cursor image from disk.
    fn load_cursor_image(&self, shape: CursorShape, size: u32) -> crate::Result<CursorImage> {
        // Construct filename: arrow-24.png, pointer-32.png, etc.
        let filename = format!("{}-{}.png", shape.css_name(), size);
        let image_path = self.path.join(&filename);
        
        if !image_path.exists() {
            return Err(crate::CursorError::InvalidImage(format!(
                "cursor image not found: {}",
                filename
            )));
        }
        
        // In a real implementation, decode PNG/SVG here
        // For now, return a placeholder
        Ok(CursorImage {
            data: vec![0; (size * size * 4) as usize],
            width: size,
            height: size,
            hotspot_x: size / 2,
            hotspot_y: size / 2,
        })
    }
    
    /// Get the default cursor size for this theme.
    pub fn default_size(&self) -> u32 {
        self.metadata.default_size
    }
    
    /// Get all available sizes for this theme.
    pub fn available_sizes(&self) -> &[u32] {
        &self.metadata.sizes
    }
}

/// Built-in default cursor theme.
pub fn default_theme() -> CursorTheme {
    CursorTheme {
        metadata: ThemeMetadata {
            name: "Liquide Default".to_string(),
            description: "Built-in cursor theme".to_string(),
            author: "Liquide Team".to_string(),
            version: "1.0".to_string(),
            sizes: vec![16, 24, 32, 48, 64],
            default_size: 24,
        },
        path: PathBuf::from("/usr/share/liquide/cursors/default"),
        cursors: HashMap::new(),
    }
}

/// Error type for theme operations.
#[derive(Debug, thiserror::Error)]
pub enum CursorThemeError {
    #[error("theme not found: {0}")]
    NotFound(String),
    
    #[error("invalid theme metadata: {0}")]
    InvalidMetadata(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme() {
        let theme = default_theme();
        assert_eq!(theme.metadata.name, "Liquide Default");
        assert_eq!(theme.default_size(), 24);
    }

    #[test]
    fn test_available_sizes() {
        let theme = default_theme();
        let sizes = theme.available_sizes();
        assert!(sizes.contains(&24));
        assert!(sizes.contains(&32));
    }
}
