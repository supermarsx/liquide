//! Google Fonts integration — discover, download, and install fonts
//! from the Google Fonts catalog.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::catalog::{FontEntry, FontSource};
use crate::error::{FontError, Result};

/// Configuration for Google Fonts integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleFontsConfig {
    /// Whether Google Fonts integration is enabled.
    pub enabled: bool,
    /// API key for the Google Fonts API (optional — many endpoints work without).
    pub api_key: Option<String>,
    /// Base URL for the Google Fonts CSS API.
    pub css_api_url: String,
    /// Download directory for fetched font files.
    pub download_dir: PathBuf,
    /// Maximum number of fonts to cache locally.
    pub max_cached: usize,
    /// Whether to auto-download fonts when they're first referenced.
    pub auto_download: bool,
}

impl Default for GoogleFontsConfig {
    fn default() -> Self {
        let download_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("liquide")
            .join("google-fonts");
        Self {
            enabled: true,
            api_key: None,
            css_api_url: "https://fonts.googleapis.com/css2".into(),
            download_dir,
            max_cached: 500,
            auto_download: true,
        }
    }
}

/// A font family available on Google Fonts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleFontFamily {
    /// Family name.
    pub family: String,
    /// Category.
    pub category: String,
    /// Available variants (e.g. "regular", "700", "italic", "700italic").
    pub variants: Vec<String>,
    /// Available subsets (e.g. "latin", "latin-ext", "cyrillic").
    pub subsets: Vec<String>,
    /// Version.
    pub version: String,
    /// Last modified date.
    pub last_modified: String,
    /// Number of styles.
    pub style_count: usize,
}

/// Client for interacting with Google Fonts.
pub struct GoogleFontsClient {
    config: GoogleFontsConfig,
    /// Cached catalog of available Google Fonts families.
    cached_catalog: Vec<GoogleFontFamily>,
}

impl GoogleFontsClient {
    /// Create a new Google Fonts client.
    #[must_use]
    pub fn new(config: GoogleFontsConfig) -> Self {
        Self {
            config,
            cached_catalog: Vec::new(),
        }
    }

    /// Whether Google Fonts integration is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the cached catalog of available families.
    #[must_use]
    pub fn catalog(&self) -> &[GoogleFontFamily] {
        &self.cached_catalog
    }

    /// Build a CSS API URL for a given family and weight.
    #[must_use]
    pub fn css_url(&self, family: &str, weight: u16) -> String {
        let family_param = family.replace(' ', "+");
        format!(
            "{}?family={}:wght@{}&display=swap",
            self.config.css_api_url, family_param, weight
        )
    }

    /// Build a download URL for getting the actual font file.
    ///
    /// This uses the direct Google Fonts static CDN.
    #[must_use]
    pub fn direct_download_url(family: &str, style: &str) -> String {
        let family_slug = family.replace(' ', "").to_lowercase();
        format!(
            "https://fonts.gstatic.com/s/{}/v1/{}.ttf",
            family_slug, style
        )
    }

    /// Check if a family is already cached locally.
    #[must_use]
    pub fn is_cached(&self, family: &str) -> bool {
        let family_dir = self.config.download_dir.join(family);
        family_dir.exists() && family_dir.is_dir()
    }

    /// Get the local cache path for a family.
    #[must_use]
    pub fn cache_path(&self, family: &str) -> PathBuf {
        self.config.download_dir.join(family)
    }

    /// Search the cached catalog for families matching a query.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&GoogleFontFamily> {
        let query_lower = query.to_lowercase();
        self.cached_catalog
            .iter()
            .filter(|f| f.family.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Register a set of pre-known popular Google Fonts families.
    ///
    /// This allows the client to work offline for common fonts.
    pub fn register_popular_fonts(&mut self) {
        let popular = [
            ("Manrope", "sans-serif", &["100", "200", "300", "regular", "500", "600", "700", "800"][..]),
            ("Space Grotesk", "sans-serif", &["300", "regular", "500", "600", "700"]),
            ("JetBrains Mono", "monospace", &["100", "200", "300", "regular", "500", "600", "700", "800", "100italic", "200italic", "300italic", "italic", "500italic", "600italic", "700italic", "800italic"]),
            ("Inter", "sans-serif", &["100", "200", "300", "regular", "500", "600", "700", "800", "900"]),
            ("Noto Sans", "sans-serif", &["100", "200", "300", "regular", "500", "600", "700", "800", "900", "100italic", "200italic", "300italic", "italic", "500italic", "600italic", "700italic", "800italic", "900italic"]),
            ("Noto Color Emoji", "sans-serif", &["regular"]),
            ("Fira Code", "monospace", &["300", "regular", "500", "600", "700"]),
            ("Roboto", "sans-serif", &["100", "300", "regular", "500", "700", "900"]),
            ("Open Sans", "sans-serif", &["300", "regular", "500", "600", "700", "800"]),
            ("Lato", "sans-serif", &["100", "300", "regular", "700", "900"]),
            ("Poppins", "sans-serif", &["100", "200", "300", "regular", "500", "600", "700", "800", "900"]),
            ("Source Code Pro", "monospace", &["200", "300", "regular", "500", "600", "700", "800", "900"]),
            ("Cascadia Code", "monospace", &["200", "300", "regular", "600", "700"]),
            ("IBM Plex Mono", "monospace", &["100", "200", "300", "regular", "500", "600", "700"]),
        ];

        for (family, category, variants) in &popular {
            self.cached_catalog.push(GoogleFontFamily {
                family: family.to_string(),
                category: category.to_string(),
                variants: variants.iter().map(|v| v.to_string()).collect(),
                subsets: vec!["latin".into(), "latin-ext".into()],
                version: "latest".into(),
                last_modified: String::new(),
                style_count: variants.len(),
            });
        }
    }

    /// Create catalog entries for a locally-cached Google Font family.
    ///
    /// Scans the cache directory for .ttf/.otf files and builds entries.
    pub fn build_catalog_entries(&self, family: &str) -> Result<Vec<FontEntry>> {
        let family_dir = self.cache_path(family);
        if !family_dir.exists() {
            return Err(FontError::FamilyNotFound {
                family: family.to_string(),
            });
        }

        let mut entries = Vec::new();
        if let Ok(dir) = std::fs::read_dir(&family_dir) {
            for entry in dir.flatten() {
                let path = entry.path();
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "ttf" || ext == "otf" || ext == "woff2" {
                    let file_size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Regular");
                    let (style, weight, italic) = parse_style_from_filename(stem);
                    entries.push(FontEntry {
                        family: family.to_string(),
                        style,
                        weight,
                        italic,
                        path,
                        format: ext,
                        file_size,
                        source: FontSource::GoogleFonts,
                        tags: vec!["google-fonts".into()],
                        activated: true,
                        glyph_count: 0,
                        script_coverage: vec!["latin".into()],
                        version: String::new(),
                        license: "OFL".into(),
                        designer: String::new(),
                    });
                }
            }
        }
        Ok(entries)
    }
}

/// Parse style name, weight, and italic flag from a font filename stem.
fn parse_style_from_filename(stem: &str) -> (String, u16, bool) {
    let lower = stem.to_lowercase();
    let italic = lower.contains("italic") || lower.contains("oblique");
    let weight = if lower.contains("thin") || lower.contains("hairline") {
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
        400
    };
    let style_name = if italic {
        match weight {
            400 => "Italic",
            700 => "Bold Italic",
            _ => "Italic",
        }
    } else {
        match weight {
            100 => "Thin",
            200 => "ExtraLight",
            300 => "Light",
            400 => "Regular",
            500 => "Medium",
            600 => "SemiBold",
            700 => "Bold",
            800 => "ExtraBold",
            900 => "Black",
            _ => "Regular",
        }
    };
    (style_name.to_string(), weight, italic)
}
