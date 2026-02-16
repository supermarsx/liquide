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
}

impl std::fmt::Debug for LoadedFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedFace")
            .field("family", &self.family)
            .field("weight", &self.weight)
            .field("italic", &self.italic)
            .field("path", &self.path)
            .field("raw_data_len", &self.raw_data.len())
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

        let id = FontFaceId(self.next_id);
        self.next_id += 1;

        let face = LoadedFace {
            font,
            family: family.clone(),
            weight,
            italic,
            path: Some(path.to_owned()),
            raw_data,
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

        let id = FontFaceId(self.next_id);
        self.next_id += 1;

        let face = LoadedFace {
            font,
            family: family.clone(),
            weight,
            italic,
            path: None,
            raw_data,
        };

        self.faces.insert(id, face);
        self.family_index
            .entry(family.to_lowercase())
            .or_default()
            .push(id);

        debug!(face_id = id.0, family = %family, weight, "loaded font face from memory");
        Ok(id)
    }

    /// Resolve a font face by family name and weight (closest match).
    #[must_use]
    pub fn resolve(&self, family: &str, weight: u16, italic: bool) -> Option<FontFaceId> {
        let key = family.to_lowercase();
        let candidates = self.family_index.get(&key)?;

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
}
