//! Font preview text generation and multilingual pangrams.

/// Configuration for generating a font preview.
#[derive(Debug, Clone)]
pub struct PreviewConfig {
    /// The text to display.
    pub text: String,
    /// Font size in points.
    pub size_pt: f32,
    /// Line height multiplier (e.g. 1.4).
    pub line_height: f32,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            text: FontPreview::default_preview_text(),
            size_pt: 16.0,
            line_height: 1.4,
        }
    }
}

/// Font preview utilities — sample text, pangrams, character sets.
pub struct FontPreview;

impl FontPreview {
    /// Default preview text combining a pangram, digits, and special characters.
    #[must_use]
    pub fn default_preview_text() -> String {
        "The quick brown fox jumps over the lazy dog.\n\
         ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
         abcdefghijklmnopqrstuvwxyz\n\
         0123456789\n\
         !@#$%^&*()_+-=[]{}|;':\",./<>?"
            .to_string()
    }

    /// Return a pangram for the given language code (ISO 639-1).
    ///
    /// Falls back to English if the language is not recognised.
    #[must_use]
    pub fn pangram_for_language(lang: &str) -> &'static str {
        match lang.to_lowercase().as_str() {
            "en" => "The quick brown fox jumps over the lazy dog.",
            "de" => "Victor jagt zw\u{00f6}lf Boxk\u{00e4}mpfer quer \u{00fc}ber den gro\u{00df}en Sylter Deich.",
            "fr" => "Portez ce vieux whisky au juge blond qui fume sur son \u{00ee}le intoxiqu\u{00e9}e.",
            "es" => "El veloz murci\u{00e9}lago hind\u{00fa} com\u{00ed}a feliz cardillo y kiwi.",
            "it" => "Quel fsjbqhdocamizz pigro kex vult.",
            "pt" => "Lusjbafdq \u{00e0} noite, vejo c\u{00e9}u repleto, com quarenta zigzagues.",
            "nl" => "Pa's wijze lynx bezag vroom het fikse aquaduct.",
            "pl" => "Pchn\u{0105}\u{0107} w t\u{0119} \u{0142}\u{00f3}d\u{017a} je\u{017c}a lub o\u{015b}m skrzy\u{0144} fig.",
            "cs" | "cz" => "P\u{0159}\u{00ed}li\u{0161} \u{017e}lu\u{0165}ou\u{010d}k\u{00fd} k\u{016f}\u{0148} \u{00fa}p\u{011b}l \u{010f}\u{00e1}belsk\u{00e9} \u{00f3}dy.",
            "ru" => "\u{0421}\u{044a}\u{0435}\u{0448}\u{044c} \u{0436}\u{0435} \u{0435}\u{0449}\u{0451} \u{044d}\u{0442}\u{0438}\u{0445} \u{043c}\u{044f}\u{0433}\u{043a}\u{0438}\u{0445} \u{0444}\u{0440}\u{0430}\u{043d}\u{0446}\u{0443}\u{0437}\u{0441}\u{043a}\u{0438}\u{0445} \u{0431}\u{0443}\u{043b}\u{043e}\u{043a} \u{0434}\u{0430} \u{0432}\u{044b}\u{043f}\u{0435}\u{0439} \u{0447}\u{0430}\u{044e}.",
            "ja" => "\u{3044}\u{308d}\u{306f}\u{306b}\u{307b}\u{3078}\u{3068}\u{3061}\u{308a}\u{306c}\u{308b}\u{3092}\u{308f}\u{304b}\u{3088}\u{305f}\u{308c}\u{305d}\u{3064}\u{306d}\u{306a}\u{3089}\u{3080}\u{3046}\u{3090}\u{306e}\u{304a}\u{304f}\u{3084}\u{307e}\u{3051}\u{3075}\u{3053}\u{3048}\u{3066}\u{3042}\u{3055}\u{304d}\u{3086}\u{3081}\u{307f}\u{3057}\u{3091}\u{3072}\u{3082}\u{305b}\u{3059}",
            "ko" => "\u{d0a4}\u{c2a4}\u{c758} \u{ace0}\u{c720}\u{c870}\u{ac74}\u{c740} \u{d0c0}\u{c778}\u{c5d0} \u{c758}\u{d574}\u{c11c} \u{ce68}\u{d574}\u{b420} \u{c218} \u{c5c6}\u{b2e4}.",
            "ar" => "\u{0635}\u{0650}\u{0641} \u{062e}\u{064e}\u{0644}\u{0642}\u{064e} \u{0630}\u{0650}\u{0643}\u{0631}\u{064b}\u{0627} \u{0643}\u{064e}\u{0645} \u{062b}\u{064e}\u{0645}\u{064e}\u{0646}\u{064e} \u{0634}\u{064e}\u{0647}\u{062f}\u{064e} \u{0639}\u{064e}\u{0632}\u{064e} \u{0646}\u{064e}\u{0641}\u{0652}\u{0633}\u{064e} \u{0623}\u{064e}\u{062d}\u{064e}\u{062f}\u{064e} \u{0628}\u{0650}\u{0644}\u{0627} \u{0637}\u{064f}\u{0639}\u{0645}\u{064d} \u{0641}\u{064e}\u{0642}\u{064e}\u{062f}\u{0652}.",
            "hi" => "\u{0928}\u{0939}\u{0940}\u{0902} \u{0928}\u{093e}\u{0928}\u{093e} \u{0915}\u{0939}\u{0924}\u{0947} \u{0939}\u{0948}\u{0902} \u{0917}\u{0927}\u{0947} \u{0915}\u{094b} \u{0939}\u{0940}\u{0930}\u{0947}.",
            "el" | "gr" => "\u{039e}\u{03b5}\u{03c3}\u{03ba}\u{03b5}\u{03c0}\u{03ac}\u{03b6}\u{03c9} \u{03c4}\u{03b7}\u{03bd} \u{03c8}\u{03c5}\u{03c7}\u{03bf}\u{03c6}\u{03b8}\u{03cc}\u{03c1}\u{03b1} \u{03b2}\u{03b4}\u{03b5}\u{03bb}\u{03c5}\u{03b3}\u{03bc}\u{03af}\u{03b1}.",
            "tr" => "Fahrettin\u{0027}in pi\u{015f}kin \u{00e7}orbac\u{0131}s\u{0131} m\u{00fc}jdeyi duyunca k\u{00fc}\u{00e7}\u{00fc}k bir ah \u{00e7}ekti.",
            _ => "The quick brown fox jumps over the lazy dog.",
        }
    }

    /// Return the list of supported language codes for pangrams.
    #[must_use]
    pub fn supported_languages() -> &'static [&'static str] {
        &[
            "en", "de", "fr", "es", "it", "pt", "nl", "pl", "cs", "ru",
            "ja", "ko", "ar", "hi", "el", "tr",
        ]
    }

    /// Generate a character-set sample for preview: uppercase, lowercase,
    /// digits, and common punctuation.
    #[must_use]
    pub fn charset_sample() -> &'static str {
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
         abcdefghijklmnopqrstuvwxyz\n\
         0123456789\n\
         !@#$%^&*()-_=+[]{}|;:'\",.<>/?\n\
         \u{00c0}\u{00c9}\u{00d1}\u{00d6}\u{00dc} \u{00e0}\u{00e9}\u{00f1}\u{00f6}\u{00fc} \u{00df}"
    }

    /// Create a [`PreviewConfig`] with the given font size and default text.
    #[must_use]
    pub fn config_for_size(size_pt: f32) -> PreviewConfig {
        PreviewConfig {
            text: Self::default_preview_text(),
            size_pt,
            line_height: 1.4,
        }
    }

    /// Create a [`PreviewConfig`] with custom text.
    #[must_use]
    pub fn config_with_text(text: &str, size_pt: f32) -> PreviewConfig {
        PreviewConfig {
            text: text.to_string(),
            size_pt,
            line_height: 1.4,
        }
    }
}
