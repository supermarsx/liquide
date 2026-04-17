//! Font database — loads and caches TrueType/OpenType font files.
//!
//! Manages a collection of loaded font faces indexed by family name and
//! weight. Supports loading from file paths and from embedded byte slices.

use ab_glyph::FontArc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

use crate::{FontRasterizerError, Result};

/// An opaque handle to a loaded font face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontFaceId(pub u32);

impl FontFaceId {
    /// The fallback/built-in font face ID.
    pub const FALLBACK: Self = Self(0);

    /// Create from a raw ID.
    #[must_use]
    pub fn from_raw(id: u32) -> Self {
        Self(id)
    }
}

/// A loaded font face with metadata.
#[derive(Clone)]
pub struct LoadedFace {
    /// The parsed font.
    pub font: FontArc,
    /// Original family name.
    pub family: String,
    /// Font weight (100-900).
    pub weight: u16,
    /// Whether this is italic.
    pub italic: bool,
    /// Source file path (if loaded from disk).
    pub path: Option<PathBuf>,
    /// Raw font file bytes (needed by rustybuzz for OpenType shaping).
    pub raw_data: Vec<u8>,
    /// Variable font axes available in this face (if it's a variable font).
    pub variation_axes: Vec<VariationAxis>,
}

/// A font variation axis (for variable fonts).
#[derive(Debug, Clone)]
pub struct VariationAxis {
    /// 4-byte axis tag (e.g., b"wght", b"wdth", b"opsz").
    pub tag: [u8; 4],
    /// Human-readable name for this axis.
    pub name: String,
    /// Minimum value for this axis.
    pub min_value: f32,
    /// Default value for this axis.
    pub default_value: f32,
    /// Maximum value for this axis.
    pub max_value: f32,
}

impl VariationAxis {
    /// Weight axis (wght).
    pub const WEIGHT: [u8; 4] = *b"wght";
    /// Width axis (wdth).
    pub const WIDTH: [u8; 4] = *b"wdth";
    /// Optical size axis (opsz).
    pub const OPTICAL_SIZE: [u8; 4] = *b"opsz";
    /// Slant axis (slnt).
    pub const SLANT: [u8; 4] = *b"slnt";
    /// Italic axis (ital).
    pub const ITALIC: [u8; 4] = *b"ital";

    /// Check if this is the weight axis.
    #[must_use]
    pub fn is_weight(&self) -> bool {
        self.tag == Self::WEIGHT
    }

    /// Check if this is the width axis.
    #[must_use]
    pub fn is_width(&self) -> bool {
        self.tag == Self::WIDTH
    }

    /// Check if this is the optical size axis.
    #[must_use]
    pub fn is_optical_size(&self) -> bool {
        self.tag == Self::OPTICAL_SIZE
    }

    /// Clamp a value to the valid range for this axis.
    #[must_use]
    pub fn clamp(&self, value: f32) -> f32 {
        value.clamp(self.min_value, self.max_value)
    }
}

/// A set of variation axis values to apply to a variable font.
#[derive(Debug, Clone, Default)]
pub struct VariationSettings {
    /// Axis tag → value mappings.
    pub values: Vec<(VariationAxis, f32)>,
}

impl VariationSettings {
    /// Create empty variation settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the weight axis value.
    pub fn weight(&mut self, value: f32) -> &mut Self {
        self.set(VariationAxis::WEIGHT, value);
        self
    }

    /// Set the width axis value.
    pub fn width(&mut self, value: f32) -> &mut Self {
        self.set(VariationAxis::WIDTH, value);
        self
    }

    /// Set the optical size axis value.
    pub fn optical_size(&mut self, value: f32) -> &mut Self {
        self.set(VariationAxis::OPTICAL_SIZE, value);
        self
    }

    /// Set an axis value by tag.
    pub fn set(&mut self, tag: [u8; 4], value: f32) {
        // Remove existing value for this tag
        self.values.retain(|(axis, _)| axis.tag != tag);
        // Add new value
        self.values.push((
            VariationAxis {
                tag,
                name: String::from_utf8_lossy(&tag).to_string(),
                min_value: f32::MIN,
                default_value: value,
                max_value: f32::MAX,
            },
            value,
        ));
    }

    /// Parse CSS font-variation-settings string (e.g., "'wght' 700, 'wdth' 100").
    #[must_use]
    pub fn from_css(css: &str) -> Self {
        let mut settings = Self::new();

        for part in css.split(',') {
            let part = part.trim();
            // Parse "'tag' value" or "tag value"
            let tokens: Vec<&str> = part.split_whitespace().collect();
            if tokens.len() == 2 {
                let tag_str = tokens[0].trim_matches(|c| c == '\'' || c == '"');
                if let Ok(value) = tokens[1].parse::<f32>() {
                    if tag_str.len() == 4 {
                        let mut tag = [0u8; 4];
                        tag.copy_from_slice(tag_str.as_bytes());
                        settings.set(tag, value);
                    }
                }
            }
        }

        settings
    }

    /// Convert to rustybuzz Variation array for shaping.
    #[must_use]
    pub fn to_rustybuzz_variations(&self) -> Vec<rustybuzz::Variation> {
        self.values
            .iter()
            .map(|(axis, value)| {
                let tag = rustybuzz::ttf_parser::Tag::from_bytes_lossy(&axis.tag);
                rustybuzz::Variation { tag, value: *value }
            })
            .collect()
    }
}

impl std::fmt::Debug for LoadedFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedFace")
            .field("family", &self.family)
            .field("weight", &self.weight)
            .field("italic", &self.italic)
            .field("path", &self.path)
            .field("raw_data_len", &self.raw_data.len())
            .field("variation_axes", &self.variation_axes.len())
            .finish()
    }
}

/// Font database — loads, caches, and resolves fonts by family/weight.
pub struct FontDatabase {
    /// All loaded faces, indexed by ID.
    faces: HashMap<FontFaceId, LoadedFace>,
    /// Family name → list of face IDs (sorted by weight).
    family_index: HashMap<String, Vec<FontFaceId>>,
    /// Next face ID to assign.
    next_id: u32,
    /// Font search directories.
    search_dirs: Vec<PathBuf>,
}

impl std::fmt::Debug for FontDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontDatabase")
            .field("face_count", &self.faces.len())
            .field("family_count", &self.family_index.len())
            .field("search_dirs", &self.search_dirs)
            .finish()
    }
}

impl FontDatabase {
    /// Create a new empty font database.
    #[must_use]
    pub fn new() -> Self {
        Self {
            faces: HashMap::new(),
            family_index: HashMap::new(),
            next_id: 1, // 0 is reserved for FALLBACK
            search_dirs: Vec::new(),
        }
    }

    /// Add a directory to search for font files.
    pub fn add_search_dir(&mut self, dir: impl Into<PathBuf>) {
        self.search_dirs.push(dir.into());
    }

    /// Load a font from a file path, assigning it the given family & weight.
    pub fn load_file(
        &mut self,
        path: impl AsRef<Path>,
        family: impl Into<String>,
        weight: u16,
        italic: bool,
    ) -> Result<FontFaceId> {
        let path = path.as_ref();
        let family = family.into();

        let data = std::fs::read(path).map_err(|e| FontRasterizerError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;

        let raw_data = data.clone();
        let font = FontArc::try_from_vec(data).map_err(|_| FontRasterizerError::InvalidFont {
            path: path.display().to_string(),
            reason: "failed to parse TrueType/OpenType data".into(),
        })?;

        // Extract variation axes if this is a variable font
        let variation_axes = Self::extract_variation_axes(&raw_data);

        let id = FontFaceId(self.next_id);
        self.next_id += 1;

        let face = LoadedFace {
            font,
            family: family.clone(),
            weight,
            italic,
            path: Some(path.to_owned()),
            raw_data,
            variation_axes,
        };

        self.faces.insert(id, face);
        self.family_index
            .entry(family.to_lowercase())
            .or_default()
            .push(id);

        info!(
            face_id = id.0,
            family = %family,
            weight,
            italic,
            path = %path.display(),
            "loaded font face"
        );

        Ok(id)
    }

    /// Load a font from in-memory bytes.
    pub fn load_bytes(
        &mut self,
        data: Vec<u8>,
        family: impl Into<String>,
        weight: u16,
        italic: bool,
    ) -> Result<FontFaceId> {
        let family = family.into();

        let raw_data = data.clone();
        let font = FontArc::try_from_vec(data).map_err(|_| FontRasterizerError::InvalidFont {
            path: "<memory>".into(),
            reason: "failed to parse TrueType/OpenType data".into(),
        })?;

        // Extract variation axes if this is a variable font
        let variation_axes = Self::extract_variation_axes(&raw_data);

        let id = FontFaceId(self.next_id);
        self.next_id += 1;

        let face = LoadedFace {
            font,
            family: family.clone(),
            weight,
            italic,
            path: None,
            raw_data,
            variation_axes,
        };

        self.faces.insert(id, face);
        self.family_index
            .entry(family.to_lowercase())
            .or_default()
            .push(id);

        debug!(face_id = id.0, family = %family, weight, "loaded font face from memory");
        Ok(id)
    }

    /// Extract variation axes from raw font data (for variable fonts).
    ///
    /// Parses the fvar table if present to find available axes.
    fn extract_variation_axes(raw_data: &[u8]) -> Vec<VariationAxis> {
        // Locate the fvar table by scanning the font's table directory.
        // An OpenType font starts with an offset table:
        //   - sfVersion (4 bytes), numTables (u16), ...
        // Each table record: tag(4) + checkSum(4) + offset(4) + length(4) = 16 bytes
        if raw_data.len() < 12 {
            return Vec::new();
        }
        let num_tables = u16::from_be_bytes([raw_data[4], raw_data[5]]) as usize;
        let table_dir_start = 12;
        let mut fvar_offset = 0usize;
        let mut fvar_length = 0usize;

        for i in 0..num_tables {
            let rec = table_dir_start + i * 16;
            if rec + 16 > raw_data.len() { break; }
            if &raw_data[rec..rec + 4] == b"fvar" {
                fvar_offset = u32::from_be_bytes([
                    raw_data[rec + 8], raw_data[rec + 9],
                    raw_data[rec + 10], raw_data[rec + 11],
                ]) as usize;
                fvar_length = u32::from_be_bytes([
                    raw_data[rec + 12], raw_data[rec + 13],
                    raw_data[rec + 14], raw_data[rec + 15],
                ]) as usize;
                break;
            }
        }

        if fvar_offset == 0 || fvar_length < 16 {
            return Vec::new(); // No fvar table → not a variable font
        }

        let fvar = match raw_data.get(fvar_offset..fvar_offset + fvar_length) {
            Some(d) => d,
            None => return Vec::new(),
        };

        // fvar header:
        //   majorVersion (u16), minorVersion (u16),
        //   axesArrayOffset (u16), reserved (u16),
        //   axisCount (u16), axisSize (u16),
        //   instanceCount (u16), instanceSize (u16)
        if fvar.len() < 16 { return Vec::new(); }
        let axes_offset = u16::from_be_bytes([fvar[4], fvar[5]]) as usize;
        let axis_count = u16::from_be_bytes([fvar[8], fvar[9]]) as usize;
        let axis_size = u16::from_be_bytes([fvar[10], fvar[11]]) as usize;
        if axis_size < 20 { return Vec::new(); }

        let mut axes = Vec::with_capacity(axis_count);
        for i in 0..axis_count {
            let off = axes_offset + i * axis_size;
            if off + 20 > fvar.len() { break; }

            let tag = [fvar[off], fvar[off + 1], fvar[off + 2], fvar[off + 3]];
            let min_val = i32::from_be_bytes([fvar[off + 4], fvar[off + 5], fvar[off + 6], fvar[off + 7]]) as f32 / 65536.0;
            let def_val = i32::from_be_bytes([fvar[off + 8], fvar[off + 9], fvar[off + 10], fvar[off + 11]]) as f32 / 65536.0;
            let max_val = i32::from_be_bytes([fvar[off + 12], fvar[off + 13], fvar[off + 14], fvar[off + 15]]) as f32 / 65536.0;
            // fvar[off+16..off+18] = flags, fvar[off+18..off+20] = axisNameID

            let name = match &tag {
                b"wght" => "Weight".to_string(),
                b"wdth" => "Width".to_string(),
                b"ital" => "Italic".to_string(),
                b"slnt" => "Slant".to_string(),
                b"opsz" => "Optical Size".to_string(),
                _ => String::from_utf8_lossy(&tag).to_string(),
            };

            axes.push(VariationAxis {
                tag,
                name,
                min_value: min_val,
                default_value: def_val,
                max_value: max_val,
            });
        }

        axes
    }

    /// Check if a font face is a variable font.
    #[must_use]
    pub fn is_variable_font(&self, face_id: FontFaceId) -> bool {
        self.faces
            .get(&face_id)
            .map(|f| !f.variation_axes.is_empty())
            .unwrap_or(false)
    }

    /// Get the variation axes for a font face.
    #[must_use]
    pub fn get_variation_axes(&self, face_id: FontFaceId) -> Option<&[VariationAxis]> {
        self.faces.get(&face_id).map(|f| f.variation_axes.as_slice())
    }

    /// Resolve a font face by family name and weight (closest match).
    ///
    /// Maps CSS generic family names to concrete fonts:
    /// - `sans-serif` → Inter, Manrope, Noto Sans
    /// - `monospace`  → JetBrains Mono
    /// - `serif`      → Noto Sans (fallback; no true serif loaded)
    /// - `system-ui`  → Inter
    #[must_use]
    pub fn resolve(&self, family: &str, weight: u16, italic: bool) -> Option<FontFaceId> {
        let key = family.to_lowercase();

        // Map CSS generic family names to concrete loaded fonts.
        let concrete_families: &[&str] = match key.as_str() {
            "sans-serif" | "system-ui" | "ui-sans-serif" => {
                &["inter", "manrope", "noto sans"]
            }
            "monospace" | "ui-monospace" => &["jetbrains mono"],
            "serif" | "ui-serif" => &["noto sans"],
            "cursive" | "fantasy" => &["manrope", "inter"],
            _ => &[],
        };

        if !concrete_families.is_empty() {
            for concrete in concrete_families {
                if let Some(id) = self.resolve_exact(concrete, weight, italic) {
                    return Some(id);
                }
            }
            return None;
        }

        self.resolve_exact(&key, weight, italic)
    }

    /// Resolve by exact (lowercase) family key — no generic mapping.
    fn resolve_exact(&self, key: &str, weight: u16, italic: bool) -> Option<FontFaceId> {
        let candidates = self.family_index.get(key)?;

        // First: exact match.
        for &id in candidates {
            if let Some(face) = self.faces.get(&id) {
                if face.weight == weight && face.italic == italic {
                    return Some(id);
                }
            }
        }

        // Second: closest weight, matching italic.
        let mut best_id = None;
        let mut best_distance = u16::MAX;
        for &id in candidates {
            if let Some(face) = self.faces.get(&id) {
                if face.italic == italic {
                    let dist = (face.weight as i32 - weight as i32).unsigned_abs() as u16;
                    if dist < best_distance {
                        best_distance = dist;
                        best_id = Some(id);
                    }
                }
            }
        }

        // Third: closest weight, any italic.
        if best_id.is_none() {
            for &id in candidates {
                if let Some(face) = self.faces.get(&id) {
                    let dist = (face.weight as i32 - weight as i32).unsigned_abs() as u16;
                    if dist < best_distance {
                        best_distance = dist;
                        best_id = Some(id);
                    }
                }
            }
        }

        best_id
    }

    /// Resolve from a list of families (fallback chain).
    #[must_use]
    pub fn resolve_chain(
        &self,
        families: &[String],
        weight: u16,
        italic: bool,
    ) -> Option<FontFaceId> {
        for family in families {
            if let Some(id) = self.resolve(family, weight, italic) {
                return Some(id);
            }
        }
        None
    }

    /// Get a loaded face by ID.
    #[must_use]
    pub fn get(&self, id: FontFaceId) -> Option<&LoadedFace> {
        self.faces.get(&id)
    }

    /// Number of loaded faces.
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// List all loaded family names.
    #[must_use]
    pub fn families(&self) -> Vec<String> {
        self.family_index.keys().cloned().collect()
    }

    /// Load the default font set from the assets directory.
    ///
    /// Loads the primary fonts for each role defined in `liquide-fonts`.
    pub fn load_default_fonts(&mut self, assets_dir: impl AsRef<Path>) -> usize {
        let dir = assets_dir.as_ref().join("fonts");
        let mut loaded = 0;

        // Inter — Primary UI, Data Dense
        let inter_regular = dir.join("Inter").join("InterVariable.ttf");
        if inter_regular.exists() {
            // Variable font: load as weight 400 (regular)
            if self.load_file(&inter_regular, "Inter", 400, false).is_ok() {
                loaded += 1;
            }
            // Also register as other weights for the variable font
            for w in [100, 200, 300, 500, 600, 700, 800, 900] {
                if self.load_file(&inter_regular, "Inter", w, false).is_ok() {
                    loaded += 1;
                }
            }
        }
        let inter_italic = dir.join("Inter").join("InterVariable-Italic.ttf");
        if inter_italic.exists() {
            if self.load_file(&inter_italic, "Inter", 400, true).is_ok() {
                loaded += 1;
            }
        }

        // Manrope — Primary UI, Status Bar, Dock, Notification, Launcher
        let manrope_dir = dir.join("Manrope");
        if manrope_dir.exists() {
            // Look for variable or static weight files
            for entry in std::fs::read_dir(&manrope_dir).into_iter().flatten() {
                if let Ok(entry) = entry {
                    let p = entry.path();
                    if p.extension().is_some_and(|e| e == "ttf") {
                        let name = p.file_stem().unwrap_or_default().to_string_lossy();
                        let weight = Self::infer_weight_from_filename(&name);
                        if self.load_file(&p, "Manrope", weight, false).is_ok() {
                            loaded += 1;
                        }
                    }
                }
            }
        }

        // Space Grotesk — Display, Window Title
        let sg_dir = dir.join("SpaceGrotesk");
        if sg_dir.exists() {
            let weight_map = [
                ("Light", 300),
                ("Regular", 400),
                ("Medium", 500),
                ("Bold", 700),
            ];
            for (suffix, w) in weight_map {
                let p = sg_dir.join(format!("SpaceGrotesk-{suffix}.ttf"));
                if p.exists() {
                    if self.load_file(&p, "Space Grotesk", w, false).is_ok() {
                        loaded += 1;
                    }
                }
            }
        }

        // JetBrains Mono — Terminal, Code
        let jb_dir = dir.join("JetBrainsMono");
        if jb_dir.exists() {
            let weight_map = [
                ("ExtraLight", 200),
                ("Light", 300),
                ("Regular", 400),
                ("Medium", 500),
                ("SemiBold", 600),
                ("Bold", 700),
                ("ExtraBold", 800),
            ];
            for (suffix, w) in &weight_map {
                let p = jb_dir.join(format!("JetBrainsMono-{suffix}.ttf"));
                if p.exists() {
                    if self.load_file(&p, "JetBrains Mono", *w, false).is_ok() {
                        loaded += 1;
                    }
                }
                let p_italic = jb_dir.join(format!("JetBrainsMono-{suffix}Italic.ttf"));
                if p_italic.exists() {
                    if self
                        .load_file(&p_italic, "JetBrains Mono", *w, true)
                        .is_ok()
                    {
                        loaded += 1;
                    }
                }
            }
        }

        // Noto Sans — Accessibility, Fallback
        let noto_dir = dir.join("NotoSans");
        if noto_dir.exists() {
            let weight_map = [
                ("Thin", 100),
                ("ExtraLight", 200),
                ("Light", 300),
                ("Regular", 400),
                ("Medium", 500),
                ("SemiBold", 600),
                ("Bold", 700),
                ("ExtraBold", 800),
                ("Black", 900),
            ];
            for (suffix, w) in &weight_map {
                let p = noto_dir.join(format!("NotoSans-{suffix}.ttf"));
                if p.exists() {
                    if self.load_file(&p, "Noto Sans", *w, false).is_ok() {
                        loaded += 1;
                    }
                }
                let p_italic = noto_dir.join(format!("NotoSans-{suffix}Italic.ttf"));
                if p_italic.exists() {
                    if self.load_file(&p_italic, "Noto Sans", *w, true).is_ok() {
                        loaded += 1;
                    }
                }
            }
        }

        info!(loaded, "loaded default font set from {}", dir.display());
        loaded
    }

    /// Infer font weight from filename conventions.
    fn infer_weight_from_filename(name: &str) -> u16 {
        let lower = name.to_lowercase();
        if lower.contains("thin") || lower.contains("hairline") {
            100
        } else if lower.contains("extralight") || lower.contains("ultralight") {
            200
        } else if lower.contains("light") {
            300
        } else if lower.contains("medium") {
            500
        } else if lower.contains("semibold") || lower.contains("demibold") {
            600
        } else if lower.contains("extrabold") || lower.contains("ultrabold") {
            800
        } else if lower.contains("bold") {
            700
        } else if lower.contains("black") || lower.contains("heavy") {
            900
        } else {
            400 // Regular
        }
    }
}

impl Default for FontDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_database() {
        let db = FontDatabase::new();
        assert_eq!(db.face_count(), 0);
        assert!(db.families().is_empty());
    }

    #[test]
    fn test_weight_inference() {
        assert_eq!(
            FontDatabase::infer_weight_from_filename("Manrope-Bold"),
            700
        );
        assert_eq!(FontDatabase::infer_weight_from_filename("Inter-Light"), 300);
        assert_eq!(
            FontDatabase::infer_weight_from_filename("Noto-Regular"),
            400
        );
        assert_eq!(
            FontDatabase::infer_weight_from_filename("JetBrainsMono-SemiBold"),
            600
        );
        assert_eq!(
            FontDatabase::infer_weight_from_filename("font-unknown"),
            400
        );
    }

    #[test]
    fn test_load_default_fonts() {
        let mut db = FontDatabase::new();
        // This test only checks it doesn't panic — fonts may not be at this path
        // in CI, so we accept 0 loaded.
        let _count = db.load_default_fonts("../../assets");
    }

    #[test]
    fn test_load_bytes_invalid() {
        let mut db = FontDatabase::new();
        let result = db.load_bytes(vec![0, 1, 2, 3], "BadFont", 400, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_nonexistent_family() {
        let db = FontDatabase::new();
        assert!(db.resolve("NonExistent", 400, false).is_none());
    }

    #[test]
    fn test_resolve_chain_empty() {
        let db = FontDatabase::new();
        assert!(db.resolve_chain(&[], 400, false).is_none());
    }

    #[test]
    fn test_font_face_id_fallback() {
        assert_eq!(FontFaceId::FALLBACK, FontFaceId(0));
        assert_eq!(FontFaceId::from_raw(42), FontFaceId(42));
    }

    #[test]
    fn test_variation_settings_from_css() {
        let settings = VariationSettings::from_css("'wght' 700, 'wdth' 100");
        assert_eq!(settings.values.len(), 2);
    }

    #[test]
    fn test_variation_settings_weight() {
        let mut settings = VariationSettings::new();
        settings.weight(700.0);
        assert_eq!(settings.values.len(), 1);
        assert_eq!(settings.values[0].0.tag, *b"wght");
    }

    #[test]
    fn test_variation_axis_clamp() {
        let axis = VariationAxis {
            tag: *b"wght",
            name: "Weight".into(),
            min_value: 100.0,
            default_value: 400.0,
            max_value: 900.0,
        };
        assert_eq!(axis.clamp(50.0), 100.0);
        assert_eq!(axis.clamp(1000.0), 900.0);
        assert_eq!(axis.clamp(500.0), 500.0);
    }

    #[test]
    fn test_variation_axis_predicates() {
        let weight = VariationAxis {
            tag: *b"wght", name: "Weight".into(),
            min_value: 100.0, default_value: 400.0, max_value: 900.0,
        };
        assert!(weight.is_weight());
        assert!(!weight.is_width());
        assert!(!weight.is_optical_size());
    }

    #[test]
    fn test_add_search_dir() {
        let mut db = FontDatabase::new();
        db.add_search_dir("/some/path");
        // Just ensure it doesn't panic
        assert_eq!(db.face_count(), 0);
    }
}
