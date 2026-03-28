use crate::locale::Locale;

/// Text direction for a locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextDir {
    /// Left-to-right (most languages).
    LTR,
    /// Right-to-left (Arabic, Hebrew, Persian, Urdu, etc.).
    RTL,
}

/// Languages that use right-to-left text direction.
const RTL_LANGUAGES: &[&str] = &[
    "ar",  // Arabic
    "arc", // Aramaic
    "az",  // Azerbaijani (when written in Arabic script)
    "dv",  // Divehi (Maldivian)
    "fa",  // Persian (Farsi)
    "ha",  // Hausa (when written in Arabic script)
    "he",  // Hebrew
    "khw", // Khowar
    "ks",  // Kashmiri
    "ku",  // Kurdish (Sorani)
    "ps",  // Pashto
    "sd",  // Sindhi
    "syr", // Syriac
    "ur",  // Urdu
    "yi",  // Yiddish
];

/// Determine the text direction for a given locale.
///
/// Returns `TextDir::RTL` for Arabic, Hebrew, Persian, Urdu, and other RTL languages.
/// Returns `TextDir::LTR` for all others.
pub fn text_direction(locale: &Locale) -> TextDir {
    if RTL_LANGUAGES.contains(&locale.language.as_str()) {
        TextDir::RTL
    } else {
        TextDir::LTR
    }
}
