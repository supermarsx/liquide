//! @font-face loading — async font fetching with `font-display` behavior.
//!
//! Implements the CSS `@font-face` loading lifecycle:
//!
//! 1. **Block period**: Use invisible fallback (font-display: block)
//! 2. **Swap period**: Use fallback font, then swap when loaded
//! 3. **Failure period**: Keep using fallback permanently
//!
//! Supports loading from:
//! - Local file paths (`local()`)
//! - Network URLs (`url()`) via async fetch
//! - Data URIs (`data:font/...;base64,...`)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::database::{FontDatabase, FontFaceId};

/// CSS `font-display` descriptor values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontDisplay {
    /// 3s block, infinite swap.
    #[default]
    Auto,
    /// 3s block, infinite swap.
    Block,
    /// 100ms block, 3s swap.
    Swap,
    /// 100ms block, 3s swap (may fail).
    Fallback,
    /// 100ms block, no swap (use fallback forever if not loaded in time).
    Optional,
}

impl FontDisplay {
    /// Block period duration.
    #[must_use]
    pub fn block_period(&self) -> Duration {
        match self {
            Self::Auto | Self::Block => Duration::from_secs(3),
            Self::Swap | Self::Fallback | Self::Optional => Duration::from_millis(100),
        }
    }

    /// Swap period duration (None = infinite).
    #[must_use]
    pub fn swap_period(&self) -> Option<Duration> {
        match self {
            Self::Auto | Self::Block | Self::Swap => None, // infinite
            Self::Fallback => Some(Duration::from_secs(3)),
            Self::Optional => Some(Duration::from_millis(0)),
        }
    }
}

/// Loading state for a @font-face rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontLoadState {
    /// Not yet started loading.
    Unloaded,
    /// Currently loading (block/swap period active).
    Loading,
    /// Successfully loaded and available.
    Loaded,
    /// Failed to load.
    Failed,
}

/// A single @font-face source.
#[derive(Debug, Clone)]
pub enum FontSource {
    /// `local("Font Name")` — from system/loaded fonts.
    Local(String),
    /// `url("path/to/font.woff2")` — file or network.
    Url(String),
    /// `url("data:font/ttf;base64,...")` — inline data.
    DataUri(Vec<u8>),
}

/// A parsed @font-face rule ready for loading.
#[derive(Debug, Clone)]
pub struct FontFaceRule {
    pub family: String,
    pub sources: Vec<FontSource>,
    pub weight_range: (u16, u16),
    pub style: FontFaceStyle,
    pub display: FontDisplay,
    pub unicode_range: Option<Vec<(u32, u32)>>,
}

/// @font-face font-style value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFaceStyle {
    Normal,
    Italic,
    Oblique(i16, i16), // angle range in degrees
}

impl Default for FontFaceStyle {
    fn default() -> Self {
        Self::Normal
    }
}

/// State for a font face being loaded.
struct LoadingFontFace {
    rule: FontFaceRule,
    state: FontLoadState,
    face_id: Option<FontFaceId>,
    started_at: Option<Instant>,
    error: Option<String>,
}

/// Font face loader — manages async loading of @font-face rules.
pub struct FontFaceLoader {
    loading: Mutex<HashMap<String, Vec<LoadingFontFace>>>,
}

impl FontFaceLoader {
    #[must_use]
    pub fn new() -> Self {
        Self {
            loading: Mutex::new(HashMap::new()),
        }
    }

    /// Register a @font-face rule for loading.
    pub fn register(&self, rule: FontFaceRule) {
        let family = rule.family.to_lowercase();
        let mut loading = liquide_common::sync::lock_or_recover(&self.loading);
        loading.entry(family).or_default().push(LoadingFontFace {
            rule,
            state: FontLoadState::Unloaded,
            face_id: None,
            started_at: None,
            error: None,
        });
    }

    /// Begin loading all registered but unloaded font faces.
    ///
    /// For `local()` sources, this resolves synchronously.
    /// For `url()` sources, this starts async loading.
    /// For `data:` URIs, this decodes and loads synchronously.
    pub fn begin_loading(&self, db: &mut FontDatabase) {
        let mut loading = liquide_common::sync::lock_or_recover(&self.loading);
        for faces in loading.values_mut() {
            for face in faces.iter_mut() {
                if face.state != FontLoadState::Unloaded {
                    continue;
                }
                face.state = FontLoadState::Loading;
                face.started_at = Some(Instant::now());

                // Try each source in order
                for source in &face.rule.sources {
                    match source {
                        FontSource::Local(name) => {
                            // Try to find in already-loaded fonts
                            let italic = matches!(face.rule.style, FontFaceStyle::Italic);
                            if let Some(id) = db.resolve(name, face.rule.weight_range.0, italic) {
                                face.face_id = Some(id);
                                face.state = FontLoadState::Loaded;
                                break;
                            }
                        }
                        FontSource::Url(url) => {
                            // Handle file:// URLs and local paths
                            let path = if url.starts_with("file://") {
                                PathBuf::from(&url[7..])
                            } else if !url.starts_with("http://") && !url.starts_with("https://") {
                                PathBuf::from(url)
                            } else {
                                // Network URL — would need async fetch
                                // For now, mark as needing async load
                                continue;
                            };

                            if path.exists() {
                                let italic = matches!(face.rule.style, FontFaceStyle::Italic);
                                match db.load_file(&path, &face.rule.family, face.rule.weight_range.0, italic) {
                                    Ok(id) => {
                                        face.face_id = Some(id);
                                        face.state = FontLoadState::Loaded;
                                        break;
                                    }
                                    Err(e) => {
                                        face.error = Some(e.to_string());
                                    }
                                }
                            }
                        }
                        FontSource::DataUri(data) => {
                            let italic = matches!(face.rule.style, FontFaceStyle::Italic);
                            match db.load_bytes(data.clone(), &face.rule.family, face.rule.weight_range.0, italic) {
                                Ok(id) => {
                                    face.face_id = Some(id);
                                    face.state = FontLoadState::Loaded;
                                    break;
                                }
                                Err(e) => {
                                    face.error = Some(e.to_string());
                                }
                            }
                        }
                    }
                }

                // If no source worked, mark as failed
                if face.state == FontLoadState::Loading && face.face_id.is_none() {
                    // Check if all sources were tried
                    let all_local_or_file = face.rule.sources.iter().all(|s| {
                        matches!(s, FontSource::Local(_) | FontSource::DataUri(_))
                            || matches!(s, FontSource::Url(u) if !u.starts_with("http"))
                    });
                    if all_local_or_file {
                        face.state = FontLoadState::Failed;
                    }
                }
            }
        }
    }

    /// Supply font data that was fetched asynchronously (e.g., from a network request).
    pub fn complete_load(&self, db: &mut FontDatabase, family: &str, weight: u16, italic: bool, data: Vec<u8>) -> Option<FontFaceId> {
        let key = family.to_lowercase();
        let mut loading = liquide_common::sync::lock_or_recover(&self.loading);
        if let Some(faces) = loading.get_mut(&key) {
            for face in faces.iter_mut() {
                if face.state == FontLoadState::Loading {
                    match db.load_bytes(data, family, weight, italic) {
                        Ok(id) => {
                            face.face_id = Some(id);
                            face.state = FontLoadState::Loaded;
                            return Some(id);
                        }
                        Err(e) => {
                            face.error = Some(e.to_string());
                            face.state = FontLoadState::Failed;
                        }
                    }
                    break;
                }
            }
        }
        None
    }

    /// Resolve a font family, respecting font-display timing.
    ///
    /// Returns `Some(face_id)` if the face is loaded, or `None` if still
    /// loading and in the block period (invisible text) or failed.
    #[must_use]
    pub fn resolve(&self, family: &str, weight: u16, italic: bool) -> FontFaceResolveResult {
        let key = family.to_lowercase();
        let loading = liquide_common::sync::lock_or_recover(&self.loading);

        if let Some(faces) = loading.get(&key) {
            for face in faces {
                // Check weight range
                if weight < face.rule.weight_range.0 || weight > face.rule.weight_range.1 {
                    continue;
                }
                let style_match = match face.rule.style {
                    FontFaceStyle::Normal => !italic,
                    FontFaceStyle::Italic => italic,
                    FontFaceStyle::Oblique(_, _) => italic,
                };
                if !style_match { continue; }

                match face.state {
                    FontLoadState::Loaded => {
                        if let Some(id) = face.face_id {
                            return FontFaceResolveResult::Loaded(id);
                        }
                    }
                    FontLoadState::Loading => {
                        let elapsed = face.started_at
                            .map(|t| t.elapsed())
                            .unwrap_or_default();

                        let block_period = face.rule.display.block_period();
                        if elapsed < block_period {
                            return FontFaceResolveResult::Blocking;
                        }

                        let swap_period = face.rule.display.swap_period();
                        if let Some(swap) = swap_period {
                            if elapsed < block_period + swap {
                                return FontFaceResolveResult::SwapFallback;
                            }
                            return FontFaceResolveResult::Failed;
                        }

                        // Infinite swap
                        return FontFaceResolveResult::SwapFallback;
                    }
                    FontLoadState::Failed => {
                        return FontFaceResolveResult::Failed;
                    }
                    FontLoadState::Unloaded => {}
                }
            }
        }

        FontFaceResolveResult::NotRegistered
    }

    /// Get the load state for a font family.
    #[must_use]
    pub fn state(&self, family: &str) -> FontLoadState {
        let key = family.to_lowercase();
        let loading = liquide_common::sync::lock_or_recover(&self.loading);
        if let Some(faces) = loading.get(&key) {
            if faces.iter().any(|f| f.state == FontLoadState::Loaded) {
                return FontLoadState::Loaded;
            }
            if faces.iter().any(|f| f.state == FontLoadState::Loading) {
                return FontLoadState::Loading;
            }
            if faces.iter().all(|f| f.state == FontLoadState::Failed) {
                return FontLoadState::Failed;
            }
        }
        FontLoadState::Unloaded
    }
}

impl Default for FontFaceLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of resolving a @font-face family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFaceResolveResult {
    /// Face is loaded and ready.
    Loaded(FontFaceId),
    /// Face is loading; we're in the block period — render invisible text.
    Blocking,
    /// Face is loading; we're in the swap period — render with fallback.
    SwapFallback,
    /// Loading failed — use fallback permanently.
    Failed,
    /// No @font-face rule registered for this family.
    NotRegistered,
}

/// Parse a CSS `@font-face` `src` descriptor into `FontSource` list.
///
/// Example input: `local("Inter"), url("fonts/Inter.woff2") format("woff2")`
#[must_use]
pub fn parse_font_face_src(src: &str) -> Vec<FontSource> {
    let mut sources = Vec::new();

    for part in src.split(',') {
        let part = part.trim();

        if part.starts_with("local(") {
            // local("FontName") or local(FontName)
            let name = part
                .trim_start_matches("local(")
                .trim_end_matches(')')
                .trim()
                .trim_matches(|c| c == '"' || c == '\'');
            if !name.is_empty() {
                sources.push(FontSource::Local(name.to_string()));
            }
        } else if part.starts_with("url(") {
            // Extract URL, ignoring format() hints
            let url_part = part.trim_start_matches("url(");
            let url_end = url_part.find(')').unwrap_or(url_part.len());
            let url = url_part[..url_end]
                .trim()
                .trim_matches(|c| c == '"' || c == '\'');

            if url.starts_with("data:") {
                // Base64-encoded data URI
                if let Some(base64_start) = url.find(";base64,") {
                    let encoded = &url[base64_start + 8..];
                    // Simple base64 decode (for production, use a proper base64 crate)
                    if let Some(decoded) = simple_base64_decode(encoded) {
                        sources.push(FontSource::DataUri(decoded));
                    }
                }
            } else if !url.is_empty() {
                sources.push(FontSource::Url(url.to_string()));
            }
        }
    }

    sources
}

/// Very basic base64 decoder (for font data URIs).
fn simple_base64_decode(input: &str) -> Option<Vec<u8>> {
    let input: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let mut output = Vec::with_capacity(input.len() * 3 / 4);

    let decode_char = |c: u8| -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            b'=' => Some(0),
            _ => None,
        }
    };

    let bytes = input.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        let a = decode_char(bytes[i])?;
        let b = decode_char(bytes[i + 1])?;
        let c = decode_char(bytes[i + 2])?;
        let d = decode_char(bytes[i + 3])?;

        output.push((a << 2) | (b >> 4));
        if bytes[i + 2] != b'=' {
            output.push((b << 4) | (c >> 2));
        }
        if bytes[i + 3] != b'=' {
            output.push((c << 6) | d);
        }
        i += 4;
    }

    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_display_periods() {
        assert_eq!(FontDisplay::Block.block_period(), Duration::from_secs(3));
        assert_eq!(FontDisplay::Swap.block_period(), Duration::from_millis(100));
        assert!(FontDisplay::Swap.swap_period().is_none());
        assert!(FontDisplay::Optional.swap_period().is_some());
    }

    #[test]
    fn test_parse_font_face_src() {
        let sources = parse_font_face_src(r#"local("Inter"), url("fonts/Inter.woff2") format("woff2")"#);
        assert_eq!(sources.len(), 2);
        assert!(matches!(&sources[0], FontSource::Local(n) if n == "Inter"));
        assert!(matches!(&sources[1], FontSource::Url(u) if u == "fonts/Inter.woff2"));
    }

    #[test]
    fn test_font_face_loader_lifecycle() {
        let loader = FontFaceLoader::new();

        let rule = FontFaceRule {
            family: "TestFont".to_string(),
            sources: vec![FontSource::Local("NonExistent".to_string())],
            weight_range: (400, 400),
            style: FontFaceStyle::Normal,
            display: FontDisplay::Swap,
            unicode_range: None,
        };

        loader.register(rule);
        let mut db = crate::database::FontDatabase::new();
        loader.begin_loading(&mut db);

        // Should be failed since "NonExistent" isn't loaded
        let result = loader.resolve("TestFont", 400, false);
        assert_eq!(result, FontFaceResolveResult::Failed);
    }
}
