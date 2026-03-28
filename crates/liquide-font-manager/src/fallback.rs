//! Font fallback chain resolution.
//!
//! A [`FallbackChain`] is an ordered list of font family names. The
//! resolver walks the chain and returns the first available font for
//! each entry.

use crate::font_info::FontInfo;

/// An ordered list of font family names to try in sequence.
#[derive(Debug, Clone)]
pub struct FallbackChain {
    /// Family names in priority order (most preferred first).
    pub families: Vec<String>,
}

impl FallbackChain {
    /// Create a new fallback chain from a list of family names.
    #[must_use]
    pub fn new(families: Vec<String>) -> Self {
        Self { families }
    }

    /// Create a chain from a comma-separated CSS-like font-family string.
    ///
    /// Each name is trimmed; quotes are stripped.
    #[must_use]
    pub fn from_css(css: &str) -> Self {
        let families = css
            .split(',')
            .map(|s| s.trim().trim_matches(|c| c == '"' || c == '\'').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self { families }
    }

    /// Number of families in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.families.len()
    }

    /// Whether the chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.families.is_empty()
    }
}

/// Fallback chain factory and resolver.
pub struct FontFallback;

impl FontFallback {
    /// Default sans-serif fallback chain.
    #[must_use]
    pub fn default_sans() -> FallbackChain {
        FallbackChain::new(vec![
            "Inter".into(),
            "Helvetica Neue".into(),
            "Helvetica".into(),
            "Arial".into(),
            "Noto Sans".into(),
            "Liberation Sans".into(),
            "DejaVu Sans".into(),
            "Segoe UI".into(),
            "Roboto".into(),
            "sans-serif".into(),
        ])
    }

    /// Default serif fallback chain.
    #[must_use]
    pub fn default_serif() -> FallbackChain {
        FallbackChain::new(vec![
            "Georgia".into(),
            "Times New Roman".into(),
            "Noto Serif".into(),
            "Liberation Serif".into(),
            "DejaVu Serif".into(),
            "Cambria".into(),
            "serif".into(),
        ])
    }

    /// Default monospace fallback chain.
    #[must_use]
    pub fn default_mono() -> FallbackChain {
        FallbackChain::new(vec![
            "JetBrains Mono".into(),
            "Fira Code".into(),
            "Cascadia Code".into(),
            "Source Code Pro".into(),
            "Consolas".into(),
            "Menlo".into(),
            "Monaco".into(),
            "Noto Sans Mono".into(),
            "Liberation Mono".into(),
            "DejaVu Sans Mono".into(),
            "Courier New".into(),
            "monospace".into(),
        ])
    }

    /// Resolve a fallback chain against a set of available fonts.
    ///
    /// Returns one `FontInfo` reference per chain entry that matched,
    /// in chain priority order. Families that are not available are
    /// skipped.
    #[must_use]
    pub fn resolve<'a>(
        chain: &FallbackChain,
        available: &'a [FontInfo],
    ) -> Vec<&'a FontInfo> {
        let mut result = Vec::new();
        for family_name in &chain.families {
            let lower = family_name.to_lowercase();
            // Find the first font in `available` whose family matches
            // (case-insensitive). Prefer Regular weight.
            let mut best: Option<&'a FontInfo> = None;
            for font in available {
                if font.family.to_lowercase() == lower {
                    match best {
                        None => best = Some(font),
                        Some(prev) => {
                            // Prefer Regular weight.
                            let prev_dist = prev.weight.distance(crate::weight::FontWeight::Regular);
                            let this_dist = font.weight.distance(crate::weight::FontWeight::Regular);
                            if this_dist < prev_dist {
                                best = Some(font);
                            }
                        }
                    }
                }
            }
            if let Some(font) = best {
                result.push(font);
            }
        }
        result
    }

    /// Resolve the entire chain and return the single best font (the
    /// first match).
    #[must_use]
    pub fn resolve_first<'a>(
        chain: &FallbackChain,
        available: &'a [FontInfo],
    ) -> Option<&'a FontInfo> {
        Self::resolve(chain, available).into_iter().next()
    }
}
