//! Font database — loads and caches TrueType/OpenType font files.
//!
//! Manages a collection of loaded font faces indexed by family name and
//! weight. Supports loading from file paths and from embedded byte slices.

use ab_glyph::FontArc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{debug, info};

use crate::{FontRasterizerError, Result};

/// Raw bytes of the embedded fallback UI font (Roboto Regular, Apache-2.0).
///
/// This font is compiled into the binary so the desktop environment **always**
/// has a real proportional UI font, even on a fresh checkout where
/// `assets/fonts/` is empty (i.e. `scripts/download-fonts.ps1` was never run).
/// Without it, zero faces load from disk and the renderer falls back to a blocky
/// 8x16 bitmap font (root cause H2, see `.orchestration/reports/t56-diagnosis.md`).
///
/// Disk-loaded fonts are always preferred; this is only registered when disk
/// loading yields zero faces. See [`FontDatabase::register_embedded_fallback`].
///
/// License: Apache-2.0. See `assets/Roboto-Regular.LICENSE.txt`.
pub const EMBEDDED_FALLBACK_FONT: &[u8] =
    include_bytes!("../assets/Roboto-Regular.ttf");

/// Family name the embedded fallback font is registered under.
///
/// It is intentionally registered under both this canonical name and the
/// common generic UI family names so that `resolve("sans-serif", ..)` and the
/// like succeed when only the embedded fallback is present.
pub const EMBEDDED_FALLBACK_FAMILY: &str = "Roboto";

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

/// Filesystem metadata captured when a font face is loaded from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontSourceStamp {
    /// Canonical source file path when available.
    pub path: PathBuf,
    /// File length in bytes at load time.
    pub len: u64,
    /// Last modification timestamp at load time, if the filesystem exposes it.
    pub modified: Option<SystemTime>,
}

impl FontSourceStamp {
    /// Capture a source stamp for an existing file.
    pub fn from_path(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let metadata = std::fs::metadata(path)?;
        let canonical_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        Ok(Self {
            path: canonical_path,
            len: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }

    /// Check whether the file backing this stamp is missing or has changed.
    #[must_use]
    pub fn is_stale(&self) -> bool {
        let Ok(metadata) = std::fs::metadata(&self.path) else {
            return true;
        };
        if metadata.len() != self.len {
            return true;
        }
        match (&self.modified, metadata.modified().ok()) {
            (Some(loaded_modified), Some(current_modified)) => loaded_modified != &current_modified,
            _ => false,
        }
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
    /// Source file stamp (if loaded from disk).
    pub source_stamp: Option<FontSourceStamp>,
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
            .field("source_stamp", &self.source_stamp)
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

        // Read the bytes first, then stamp the file. Stamping *after* the read
        // means the captured (len, mtime) describe the exact bytes we parsed —
        // if the file is rewritten between read and stamp, the stamp reflects
        // the newer state and the face is correctly flagged stale on the next
        // poll (fixes the stamp-before-read race, t49-e3-F41).
        let (font, raw_data, source_stamp) = Self::read_and_stamp(path)?;

        // Extract variation axes if this is a variable font
        let variation_axes = Self::extract_variation_axes(&raw_data);

        let id = FontFaceId(self.next_id);
        self.next_id += 1;

        let face = LoadedFace {
            font,
            family: family.clone(),
            weight,
            italic,
            path: Some(source_stamp.path.clone()),
            source_stamp: Some(source_stamp),
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

    /// Read a font file, parse it, and capture its source stamp.
    ///
    /// The stamp is taken *after* the bytes are read so it describes the parsed
    /// content (avoiding the load-time stamp/read race in t49-e3-F41). Used by
    /// both [`load_file`](Self::load_file) and [`reload_face`](Self::reload_face).
    fn read_and_stamp(path: &Path) -> Result<(FontArc, Vec<u8>, FontSourceStamp)> {
        let data = std::fs::read(path).map_err(|e| FontRasterizerError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;

        let raw_data = data.clone();
        let font = FontArc::try_from_vec(data).map_err(|_| FontRasterizerError::InvalidFont {
            path: path.display().to_string(),
            reason: "failed to parse TrueType/OpenType data".into(),
        })?;

        let source_stamp =
            FontSourceStamp::from_path(path).map_err(|e| FontRasterizerError::IoError {
                path: path.display().to_string(),
                source: e,
            })?;

        Ok((font, raw_data, source_stamp))
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
            source_stamp: None,
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
            if rec + 16 > raw_data.len() {
                break;
            }
            if &raw_data[rec..rec + 4] == b"fvar" {
                fvar_offset = u32::from_be_bytes([
                    raw_data[rec + 8],
                    raw_data[rec + 9],
                    raw_data[rec + 10],
                    raw_data[rec + 11],
                ]) as usize;
                fvar_length = u32::from_be_bytes([
                    raw_data[rec + 12],
                    raw_data[rec + 13],
                    raw_data[rec + 14],
                    raw_data[rec + 15],
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
        if fvar.len() < 16 {
            return Vec::new();
        }
        let axes_offset = u16::from_be_bytes([fvar[4], fvar[5]]) as usize;
        let axis_count = u16::from_be_bytes([fvar[8], fvar[9]]) as usize;
        let axis_size = u16::from_be_bytes([fvar[10], fvar[11]]) as usize;
        if axis_size < 20 {
            return Vec::new();
        }

        let mut axes = Vec::with_capacity(axis_count);
        for i in 0..axis_count {
            let off = axes_offset + i * axis_size;
            if off + 20 > fvar.len() {
                break;
            }

            let tag = [fvar[off], fvar[off + 1], fvar[off + 2], fvar[off + 3]];
            let min_val =
                i32::from_be_bytes([fvar[off + 4], fvar[off + 5], fvar[off + 6], fvar[off + 7]])
                    as f32
                    / 65536.0;
            let def_val =
                i32::from_be_bytes([fvar[off + 8], fvar[off + 9], fvar[off + 10], fvar[off + 11]])
                    as f32
                    / 65536.0;
            let max_val = i32::from_be_bytes([
                fvar[off + 12],
                fvar[off + 13],
                fvar[off + 14],
                fvar[off + 15],
            ]) as f32
                / 65536.0;
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
        self.faces
            .get(&face_id)
            .map(|f| f.variation_axes.as_slice())
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
            "sans-serif" | "system-ui" | "ui-sans-serif" => &["inter", "manrope", "noto sans"],
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

    /// Get the source stamp for a file-backed face.
    #[must_use]
    pub fn face_source_stamp(&self, face_id: FontFaceId) -> Option<&FontSourceStamp> {
        self.faces
            .get(&face_id)
            .and_then(|face| face.source_stamp.as_ref())
    }

    /// Check whether a face's file-backed source has become stale.
    ///
    /// Memory-loaded faces and unknown face IDs are not stale by definition.
    #[must_use]
    pub fn is_face_stale(&self, face_id: FontFaceId) -> bool {
        self.face_source_stamp(face_id)
            .is_some_and(FontSourceStamp::is_stale)
    }

    /// Return all loaded file-backed faces whose source metadata changed.
    #[must_use]
    pub fn stale_faces(&self) -> Vec<FontFaceId> {
        let mut faces: Vec<FontFaceId> = self
            .faces
            .iter()
            .filter_map(|(face_id, face)| {
                face.source_stamp
                    .as_ref()
                    .filter(|stamp| stamp.is_stale())
                    .map(|_| *face_id)
            })
            .collect();
        faces.sort_by_key(|face_id| face_id.0);
        faces
    }

    /// Reload a single file-backed face from its source path.
    ///
    /// Re-reads the font file from disk, re-parses it, re-extracts variation
    /// axes, and re-stamps the source metadata — replacing the cached bytes in
    /// place under the **same** [`FontFaceId`]. This is the missing piece that
    /// makes stale-face invalidation actually take effect: before this existed,
    /// invalidating caches only re-rasterized the *same stale bytes* forever
    /// (t49-e3-F15).
    ///
    /// Returns:
    /// - `Ok(true)`  — the face was file-backed and its bytes were replaced.
    /// - `Ok(false)` — the face is unknown or memory-loaded (nothing to reload).
    /// - `Err(_)`    — the file could not be read or no longer parses; the
    ///   previously loaded face is left untouched so the renderer keeps working.
    pub fn reload_face(&mut self, face_id: FontFaceId) -> Result<bool> {
        let Some(path) = self.faces.get(&face_id).and_then(|face| face.path.clone()) else {
            // Unknown face or memory-loaded face: nothing to reload.
            return Ok(false);
        };

        let (font, raw_data, source_stamp) = Self::read_and_stamp(&path)?;
        let variation_axes = Self::extract_variation_axes(&raw_data);

        // Only commit once the new bytes parsed successfully (the `?` above):
        // a failed reload must not corrupt the currently-serving face.
        let Some(face) = self.faces.get_mut(&face_id) else {
            return Ok(false);
        };
        face.font = font;
        face.raw_data = raw_data;
        face.variation_axes = variation_axes;
        face.path = Some(source_stamp.path.clone());
        face.source_stamp = Some(source_stamp);

        info!(
            face_id = face_id.0,
            family = %face.family,
            path = %path.display(),
            "reloaded font face from changed source"
        );

        Ok(true)
    }

    /// Reload every file-backed face whose source metadata changed.
    ///
    /// Returns the IDs of the faces that were actually reloaded (parsed
    /// successfully from fresh bytes). Faces whose source vanished or no longer
    /// parses are skipped and left serving their last-good bytes; callers can
    /// still discover them via [`stale_faces`](Self::stale_faces).
    pub fn reload_stale_faces(&mut self) -> Vec<FontFaceId> {
        let stale = self.stale_faces();
        let mut reloaded = Vec::with_capacity(stale.len());
        for face_id in stale {
            match self.reload_face(face_id) {
                Ok(true) => reloaded.push(face_id),
                Ok(false) => {}
                Err(err) => {
                    debug!(face_id = face_id.0, error = %err, "stale font face reload failed");
                }
            }
        }
        reloaded
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

    /// Register the embedded fallback font (Roboto Regular, Apache-2.0).
    ///
    /// Loads [`EMBEDDED_FALLBACK_FONT`] from memory and registers it under its
    /// canonical family ("Roboto") **and** under every concrete family the
    /// [`resolve`](Self::resolve) generic-family mapping looks for (Inter,
    /// Manrope, Noto Sans, JetBrains Mono). Registering under those names means
    /// `resolve("sans-serif"/"system-ui"/"monospace"/…)` succeeds even when the
    /// embedded font is the *only* loaded face — so the renderer never falls
    /// back to its 8x16 bitmap font (root cause H2).
    ///
    /// The same bytes are registered at all standard CSS weights (100..=900) so
    /// weight resolution always finds a face; the embedded font is a single
    /// static Regular cut, so every weight shares its outlines (no synthetic
    /// bolding here — that is the rasterizer's concern). Disk fonts, when
    /// present, are the preferred source and are loaded *before* this is called.
    ///
    /// Returns the number of faces registered (each (family, weight) pair counts
    /// as one face). Returns 0 only if the embedded bytes fail to parse, which
    /// would indicate a corrupt vendored asset.
    pub fn register_embedded_fallback(&mut self) -> usize {
        // Families to register the fallback under: its real name plus every
        // concrete family the generic-family resolver maps to. Keep this list in
        // sync with `resolve`'s `concrete_families` mapping.
        const FALLBACK_FAMILIES: &[&str] = &[
            EMBEDDED_FALLBACK_FAMILY,
            "Inter",
            "Manrope",
            "Noto Sans",
            "JetBrains Mono",
        ];
        const WEIGHTS: &[u16] = &[100, 200, 300, 400, 500, 600, 700, 800, 900];

        let mut registered = 0;
        for family in FALLBACK_FAMILIES {
            for &weight in WEIGHTS {
                if self
                    .load_bytes(EMBEDDED_FALLBACK_FONT.to_vec(), *family, weight, false)
                    .is_ok()
                {
                    registered += 1;
                }
            }
        }

        if registered == 0 {
            // The vendored asset failed to parse — should never happen.
            tracing::error!(
                "embedded fallback font failed to parse; text will fall back to the 8x16 bitmap font"
            );
        } else {
            info!(
                registered,
                family = EMBEDDED_FALLBACK_FAMILY,
                "registered embedded fallback font (no disk fonts found)"
            );
        }
        registered
    }

    /// Load the default font set from the assets directory.
    ///
    /// Loads the primary fonts for each role defined in `liquide-fonts`. If disk
    /// loading yields **zero** faces (e.g. a fresh checkout where
    /// `scripts/download-fonts.ps1` was never run), the embedded fallback font is
    /// registered via [`register_embedded_fallback`](Self::register_embedded_fallback)
    /// so the desktop environment always has a real proportional UI font. Disk
    /// fonts are always preferred when present.
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

        // H2 fallback: if NOTHING loaded from disk, the desktop would otherwise
        // drop to a blocky 8x16 bitmap font. Register the embedded Roboto
        // fallback so a real proportional UI font is always available. Disk
        // fonts, when present, are preferred and we leave them untouched.
        if loaded == 0 {
            tracing::warn!(
                assets_dir = %dir.display(),
                "no fonts found on disk; registering embedded fallback font \
                 (run scripts/download-fonts.ps1 to install the full font set)"
            );
            loaded += self.register_embedded_fallback();
        }

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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "liquide-font-rasterizer-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    fn fixture_font_bytes() -> Option<Vec<u8>> {
        let candidates = [
            "C:\\Windows\\Fonts\\segoeui.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
            "C:\\Windows\\Fonts\\calibri.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
            "/Library/Fonts/Arial.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
        ];

        candidates.iter().find_map(|path| {
            let data = std::fs::read(path).ok()?;
            FontArc::try_from_vec(data.clone()).ok()?;
            Some(data)
        })
    }

    fn write_fixture_font(label: &str) -> Option<(PathBuf, PathBuf)> {
        let data = fixture_font_bytes()?;
        let dir = unique_temp_dir(label);
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join("fixture.ttf");
        std::fs::write(&path, data).ok()?;
        Some((dir, path))
    }

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
    fn embedded_fallback_parses_and_registers_faces() {
        // The vendored Roboto-Regular.ttf must parse and register >= 1 face.
        let mut db = FontDatabase::new();
        let registered = db.register_embedded_fallback();
        assert!(
            registered >= 1,
            "embedded fallback font must register at least one face"
        );
        assert_eq!(db.face_count(), registered);
    }

    #[test]
    fn load_default_fonts_registers_embedded_fallback_when_dir_empty() {
        // Point load_default_fonts at an EMPTY assets dir (no fonts/ subtree):
        // disk loading yields 0 faces, so the embedded fallback must kick in and
        // the database must end up with at least one face (NOT zero — which would
        // drop the renderer to the 8x16 bitmap font, root cause H2).
        let dir = unique_temp_dir("empty-assets");
        std::fs::create_dir_all(&dir).unwrap();

        let mut db = FontDatabase::new();
        let loaded = db.load_default_fonts(&dir);
        assert!(
            loaded >= 1,
            "empty assets dir must fall back to the embedded font (got {loaded})"
        );
        assert!(db.face_count() >= 1);

        // The fallback must satisfy generic-family resolution so the UI text
        // (which requests e.g. sans-serif / system-ui) actually finds a face.
        assert!(
            db.resolve("sans-serif", 400, false).is_some(),
            "sans-serif must resolve via the embedded fallback"
        );
        assert!(
            db.resolve("system-ui", 700, false).is_some(),
            "system-ui must resolve via the embedded fallback"
        );
        assert!(
            db.resolve(EMBEDDED_FALLBACK_FAMILY, 400, false).is_some(),
            "the fallback family must resolve directly"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn font_source_stamp_detects_missing_and_length_changes() {
        let dir = unique_temp_dir("stamp-direct");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("source.bin");
        std::fs::write(&path, b"font-source").unwrap();

        let stamp = FontSourceStamp::from_path(&path).unwrap();
        assert_eq!(stamp.len, 11);
        assert!(!stamp.is_stale());

        std::fs::write(&path, b"font-source-changed").unwrap();
        assert!(stamp.is_stale());

        let changed_stamp = FontSourceStamp::from_path(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(changed_stamp.is_stale());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn file_loaded_faces_capture_source_stamp_and_start_fresh() {
        let Some((dir, path)) = write_fixture_font("stamp-load") else {
            return;
        };
        let mut db = FontDatabase::new();
        let id = db.load_file(&path, "Fixture", 400, false).unwrap();

        let stamp = db.face_source_stamp(id).unwrap();
        assert!(stamp.path.is_absolute());
        assert_eq!(stamp.len, std::fs::metadata(&path).unwrap().len());
        assert!(!db.is_face_stale(id));
        assert!(db.stale_faces().is_empty());
        assert_eq!(db.get(id).unwrap().path.as_ref(), Some(&stamp.path));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_loaded_faces_have_no_source_stamp_and_never_report_stale() {
        let Some(data) = fixture_font_bytes() else {
            return;
        };
        let mut db = FontDatabase::new();
        let id = db.load_bytes(data, "Memory", 400, false).unwrap();

        assert!(db.face_source_stamp(id).is_none());
        assert!(!db.is_face_stale(id));
        assert!(db.stale_faces().is_empty());
    }

    #[test]
    fn file_backed_face_reports_stale_when_source_length_changes() {
        let Some((dir, path)) = write_fixture_font("stamp-changed") else {
            return;
        };
        let mut db = FontDatabase::new();
        let id = db.load_file(&path, "Fixture", 400, false).unwrap();

        assert!(!db.is_face_stale(id));
        let mut data = std::fs::read(&path).unwrap();
        data.extend_from_slice(b"changed");
        std::fs::write(&path, data).unwrap();

        assert!(db.is_face_stale(id));
        assert_eq!(db.stale_faces(), vec![id]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn file_backed_face_reports_stale_when_source_disappears() {
        let Some((dir, path)) = write_fixture_font("stamp-missing") else {
            return;
        };
        let mut db = FontDatabase::new();
        let id = db.load_file(&path, "Fixture", 400, false).unwrap();

        std::fs::remove_file(&path).unwrap();

        assert!(db.is_face_stale(id));
        assert_eq!(db.stale_faces(), vec![id]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reload_face_replaces_stale_bytes_and_clears_staleness() {
        let Some((dir, path)) = write_fixture_font("reload-bytes") else {
            return;
        };
        let mut db = FontDatabase::new();
        let id = db.load_file(&path, "Fixture", 400, false).unwrap();

        let original_len = db.get(id).unwrap().raw_data.len();
        assert!(!db.is_face_stale(id));

        // Rewrite the source with extra bytes appended; the face is now stale
        // and still holds the OLD bytes until we reload.
        let mut data = std::fs::read(&path).unwrap();
        data.extend_from_slice(b"reloaded-trailer");
        let new_len = data.len();
        std::fs::write(&path, &data).unwrap();

        assert!(db.is_face_stale(id), "appended bytes must mark face stale");
        assert_eq!(
            db.get(id).unwrap().raw_data.len(),
            original_len,
            "pre-reload the face still serves the stale bytes"
        );

        // Reload: the SAME face id now carries the fresh bytes and is no longer
        // stale (closes t49-e3-F15 — invalidation that actually re-reads).
        assert!(db.reload_face(id).unwrap());
        assert_eq!(
            db.get(id).unwrap().raw_data.len(),
            new_len,
            "reload must replace cached bytes in place"
        );
        assert!(
            !db.is_face_stale(id),
            "reload re-stamps so face is fresh again"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reload_stale_faces_returns_only_reloaded_ids() {
        let Some((dir, path)) = write_fixture_font("reload-stale-batch") else {
            return;
        };
        let mut db = FontDatabase::new();
        let file_id = db.load_file(&path, "Fixture", 400, false).unwrap();
        let mem_id = db
            .load_bytes(std::fs::read(&path).unwrap(), "Memory", 400, false)
            .unwrap();

        // Nothing stale yet.
        assert!(db.reload_stale_faces().is_empty());

        // Mutate the file-backed source only.
        let mut data = std::fs::read(&path).unwrap();
        data.extend_from_slice(b"x");
        std::fs::write(&path, &data).unwrap();

        let reloaded = db.reload_stale_faces();
        assert_eq!(
            reloaded,
            vec![file_id],
            "only the changed file face reloads"
        );
        assert!(
            !db.is_face_stale(file_id),
            "reloaded face is no longer stale"
        );
        // Memory face is never stale and never reloaded.
        assert!(!db.is_face_stale(mem_id));
        assert!(db.stale_faces().is_empty());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reload_face_is_noop_for_memory_and_unknown_faces() {
        let Some(data) = fixture_font_bytes() else {
            return;
        };
        let mut db = FontDatabase::new();
        let mem_id = db.load_bytes(data, "Memory", 400, false).unwrap();

        // Memory-loaded face has no path → nothing to reload.
        assert!(!db.reload_face(mem_id).unwrap());
        // Unknown face id → nothing to reload, no panic.
        assert!(!db.reload_face(FontFaceId(9999)).unwrap());
    }

    #[test]
    fn reload_face_preserves_old_bytes_when_source_disappears() {
        let Some((dir, path)) = write_fixture_font("reload-missing") else {
            return;
        };
        let mut db = FontDatabase::new();
        let id = db.load_file(&path, "Fixture", 400, false).unwrap();
        let original_len = db.get(id).unwrap().raw_data.len();

        std::fs::remove_file(&path).unwrap();

        // Reload fails (file gone) but must NOT drop the currently-serving face.
        assert!(db.reload_face(id).is_err());
        assert_eq!(
            db.get(id).unwrap().raw_data.len(),
            original_len,
            "failed reload leaves the last-good bytes intact"
        );
        // reload_stale_faces tolerates the missing source and reports nothing.
        assert!(db.reload_stale_faces().is_empty());

        let _ = std::fs::remove_dir_all(dir);
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
            tag: *b"wght",
            name: "Weight".into(),
            min_value: 100.0,
            default_value: 400.0,
            max_value: 900.0,
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
