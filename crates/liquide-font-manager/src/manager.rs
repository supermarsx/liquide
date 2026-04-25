//! Central font management: scanning, querying, installing, and
//! uninstalling fonts.

use std::collections::HashMap;
use std::path::Path;

use crate::error::FontError;
use crate::font_info::FontInfo;
use crate::format::FontFormat;
use crate::platform;
use crate::style::FontStyle;
use crate::weight::FontWeight;

/// Central font manager.
///
/// Holds the full list of discovered fonts and provides query, install,
/// and uninstall operations.
pub struct FontManager {
    /// All known fonts, populated by `scan_system_fonts()`.
    fonts: Vec<FontInfo>,
    /// Family name (lowercased) → indices into `fonts`.
    family_index: HashMap<String, Vec<usize>>,
}

impl FontManager {
    /// Create an empty font manager.
    ///
    /// Call [`scan_system_fonts()`](Self::scan_system_fonts) to populate it.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fonts: Vec::new(),
            family_index: HashMap::new(),
        }
    }

    /// Scan all platform font directories and populate the manager.
    ///
    /// Returns the list of discovered fonts (also stored internally).
    pub fn scan_system_fonts(&mut self) -> Vec<FontInfo> {
        self.fonts.clear();
        self.family_index.clear();

        // System directories.
        for dir in platform::system_font_dirs() {
            let paths = platform::scan_font_dir(&dir);
            for path in paths {
                if let Some(path_str) = path.to_str() {
                    if let Some(info) = FontInfo::from_path(path_str, true) {
                        self.add_font(info);
                    }
                }
            }
        }

        // User directory.
        if let Some(user_dir) = platform::user_font_dir() {
            let paths = platform::scan_font_dir(&user_dir);
            for path in paths {
                if let Some(path_str) = path.to_str() {
                    if let Some(info) = FontInfo::from_path(path_str, false) {
                        self.add_font(info);
                    }
                }
            }
        }

        tracing::info!(count = self.fonts.len(), "system font scan complete");
        self.fonts.clone()
    }

    /// Add a font to the internal database.
    fn add_font(&mut self, info: FontInfo) {
        let key = info.family.to_lowercase();
        let idx = self.fonts.len();
        self.family_index.entry(key).or_default().push(idx);
        self.fonts.push(info);
    }

    /// Rebuild the family index from scratch.
    fn rebuild_index(&mut self) {
        self.family_index.clear();
        for (i, font) in self.fonts.iter().enumerate() {
            let key = font.family.to_lowercase();
            self.family_index.entry(key).or_default().push(i);
        }
    }

    // ── Queries ──────────────────────────────────────────────────────

    /// Sorted list of unique family names.
    #[must_use]
    pub fn families(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .family_index
            .values()
            .filter_map(|indices| indices.first().map(|&i| self.fonts[i].family.clone()))
            .collect();
        names.sort_unstable_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        names.dedup_by(|a, b| a.to_lowercase() == b.to_lowercase());
        names
    }

    /// All fonts belonging to the given family (case-insensitive).
    #[must_use]
    pub fn fonts_in_family(&self, family: &str) -> Vec<&FontInfo> {
        let key = family.to_lowercase();
        self.family_index
            .get(&key)
            .map(|indices| indices.iter().filter_map(|&i| self.fonts.get(i)).collect())
            .unwrap_or_default()
    }

    /// Find the best matching font for a family + weight + style query.
    ///
    /// Matching strategy (CSS font-matching algorithm, simplified):
    /// 1. Filter to the requested family.
    /// 2. Prefer exact style match; fall back to Regular.
    /// 3. Among style matches, pick the closest weight.
    #[must_use]
    pub fn find_font(
        &self,
        family: &str,
        weight: FontWeight,
        style: FontStyle,
    ) -> Option<&FontInfo> {
        let candidates = self.fonts_in_family(family);
        if candidates.is_empty() {
            return None;
        }

        // First: try exact style match, closest weight.
        let style_matched: Vec<&&FontInfo> =
            candidates.iter().filter(|f| f.style == style).collect();
        if !style_matched.is_empty() {
            return style_matched
                .into_iter()
                .min_by_key(|f| f.weight.distance(weight))
                .copied();
        }

        // Fallback: any style, closest weight.
        candidates
            .into_iter()
            .min_by_key(|f| f.weight.distance(weight))
    }

    /// Install a font from a file path.
    ///
    /// Copies the file into the user font directory and adds it to the
    /// internal database.
    pub fn install_font(&mut self, path: &str) -> Result<FontInfo, FontError> {
        let source = Path::new(path);
        if !source.exists() {
            return Err(FontError::NotFound {
                path: path.to_string(),
            });
        }

        // Validate extension.
        let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("");
        if FontFormat::from_extension(ext).is_none() {
            return Err(FontError::UnsupportedFormat {
                path: path.to_string(),
            });
        }

        let user_dir = platform::user_font_dir().ok_or(FontError::NoUserFontDir)?;
        std::fs::create_dir_all(&user_dir)?;

        let file_name = source
            .file_name()
            .ok_or_else(|| FontError::UnsupportedFormat {
                path: path.to_string(),
            })?;
        let dest = user_dir.join(file_name);
        std::fs::copy(source, &dest)?;

        let dest_str = dest.to_string_lossy().to_string();
        let info =
            FontInfo::from_path(&dest_str, false).ok_or_else(|| FontError::InstallFailed {
                reason: format!("could not parse font metadata from {dest_str}"),
            })?;

        tracing::info!(
            family = %info.family,
            path = %info.file_path,
            "font installed"
        );

        self.add_font(info.clone());
        Ok(info)
    }

    /// Uninstall a user-installed font by its file path.
    ///
    /// System fonts cannot be uninstalled.
    pub fn uninstall_font(&mut self, path: &str) -> Result<(), FontError> {
        // Find the font in our database.
        let idx = self
            .fonts
            .iter()
            .position(|f| f.file_path == path)
            .ok_or_else(|| FontError::NotFound {
                path: path.to_string(),
            })?;

        if self.fonts[idx].is_system {
            return Err(FontError::SystemFont {
                path: path.to_string(),
            });
        }

        let file = Path::new(path);
        if file.exists() {
            std::fs::remove_file(file)?;
        }

        tracing::info!(
            family = %self.fonts[idx].family,
            path = path,
            "font uninstalled"
        );

        self.fonts.remove(idx);
        self.rebuild_index();
        Ok(())
    }

    /// Return all fonts that can render the given text (based on Unicode
    /// block coverage).
    #[must_use]
    pub fn font_for_text(&self, text: &str) -> Vec<&FontInfo> {
        self.fonts.iter().filter(|f| f.covers_text(text)).collect()
    }

    /// Return all monospace fonts.
    #[must_use]
    pub fn monospace_fonts(&self) -> Vec<&FontInfo> {
        self.fonts.iter().filter(|f| f.is_monospace).collect()
    }

    /// Total number of font faces loaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    /// Whether the manager has any fonts loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    /// Get an immutable slice of all loaded fonts.
    #[must_use]
    pub fn all_fonts(&self) -> &[FontInfo] {
        &self.fonts
    }

    /// Manually add a `FontInfo` (e.g. from a custom source).
    pub fn add(&mut self, info: FontInfo) {
        self.add_font(info);
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}
