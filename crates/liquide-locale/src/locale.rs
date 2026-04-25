use std::fmt;

/// A locale identifier following the POSIX convention: `language[_territory][.encoding]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Locale {
    /// ISO 639-1 language code (e.g., "en", "de", "ja").
    pub language: String,
    /// ISO 3166-1 territory code (e.g., "US", "DE", "JP").
    pub territory: Option<String>,
    /// Character encoding (e.g., "UTF-8").
    pub encoding: Option<String>,
}

impl Locale {
    /// Create a new locale with just a language code.
    pub fn new(language: &str) -> Self {
        Self {
            language: language.to_lowercase(),
            territory: None,
            encoding: None,
        }
    }

    /// Create a new locale with language and territory.
    pub fn with_territory(language: &str, territory: &str) -> Self {
        Self {
            language: language.to_lowercase(),
            territory: Some(territory.to_uppercase()),
            encoding: None,
        }
    }

    /// Create a new locale with language, territory, and encoding.
    pub fn with_encoding(language: &str, territory: &str, encoding: &str) -> Self {
        Self {
            language: language.to_lowercase(),
            territory: Some(territory.to_uppercase()),
            encoding: Some(encoding.to_string()),
        }
    }

    /// Parse a locale string such as "en_US.UTF-8", "de_DE", "ja", "fr_FR.ISO-8859-1".
    ///
    /// Returns `None` if the string is empty or has an invalid format.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        if s == "C" || s == "POSIX" {
            return Some(Self::new("en"));
        }

        // Split off encoding: language_territory.encoding
        let (lang_terr, encoding) = if let Some(dot_pos) = s.find('.') {
            let enc = &s[dot_pos + 1..];
            let enc = if let Some(at_pos) = enc.find('@') {
                &enc[..at_pos]
            } else {
                enc
            };
            (
                &s[..dot_pos],
                if enc.is_empty() {
                    None
                } else {
                    Some(enc.to_string())
                },
            )
        } else {
            // Strip @modifier if present
            let base = if let Some(at_pos) = s.find('@') {
                &s[..at_pos]
            } else {
                s
            };
            (base, None)
        };

        // Split language and territory
        let (language, territory) = if let Some(sep_pos) = lang_terr.find(|c| c == '_' || c == '-')
        {
            let lang = &lang_terr[..sep_pos];
            let terr = &lang_terr[sep_pos + 1..];
            if lang.is_empty() {
                return None;
            }
            (
                lang.to_lowercase(),
                if terr.is_empty() {
                    None
                } else {
                    Some(terr.to_uppercase())
                },
            )
        } else {
            if lang_terr.is_empty() {
                return None;
            }
            (lang_terr.to_lowercase(), None)
        };

        // Validate language code (2 or 3 chars, all alpha)
        if language.len() < 2
            || language.len() > 3
            || !language.chars().all(|c| c.is_ascii_alphabetic())
        {
            return None;
        }

        // Validate territory if present (2 chars alpha or 3 digit)
        if let Some(ref terr) = territory {
            let valid = (terr.len() == 2 && terr.chars().all(|c| c.is_ascii_alphabetic()))
                || (terr.len() == 3 && terr.chars().all(|c| c.is_ascii_digit()));
            if !valid {
                return None;
            }
        }

        Some(Self {
            language,
            territory,
            encoding,
        })
    }

    /// Returns the English name of the language from the built-in table.
    pub fn language_name(&self) -> &str {
        language_name_lookup(&self.language)
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.language)?;
        if let Some(ref terr) = self.territory {
            write!(f, "_{}", terr)?;
        }
        if let Some(ref enc) = self.encoding {
            write!(f, ".{}", enc)?;
        }
        Ok(())
    }
}

/// Look up the English name of a language by its ISO 639-1 code.
pub fn language_name_lookup(code: &str) -> &'static str {
    match code {
        "aa" => "Afar",
        "ab" => "Abkhazian",
        "af" => "Afrikaans",
        "am" => "Amharic",
        "ar" => "Arabic",
        "as" => "Assamese",
        "ay" => "Aymara",
        "az" => "Azerbaijani",
        "ba" => "Bashkir",
        "be" => "Belarusian",
        "bg" => "Bulgarian",
        "bn" => "Bengali",
        "bo" => "Tibetan",
        "br" => "Breton",
        "bs" => "Bosnian",
        "ca" => "Catalan",
        "cs" => "Czech",
        "cy" => "Welsh",
        "da" => "Danish",
        "de" => "German",
        "dz" => "Dzongkha",
        "el" => "Greek",
        "en" => "English",
        "eo" => "Esperanto",
        "es" => "Spanish",
        "et" => "Estonian",
        "eu" => "Basque",
        "fa" => "Persian",
        "fi" => "Finnish",
        "fj" => "Fijian",
        "fo" => "Faroese",
        "fr" => "French",
        "ga" => "Irish",
        "gd" => "Scottish Gaelic",
        "gl" => "Galician",
        "gu" => "Gujarati",
        "ha" => "Hausa",
        "he" => "Hebrew",
        "hi" => "Hindi",
        "hr" => "Croatian",
        "hu" => "Hungarian",
        "hy" => "Armenian",
        "id" => "Indonesian",
        "is" => "Icelandic",
        "it" => "Italian",
        "ja" => "Japanese",
        "ka" => "Georgian",
        "kk" => "Kazakh",
        "km" => "Khmer",
        "kn" => "Kannada",
        "ko" => "Korean",
        "ku" => "Kurdish",
        "ky" => "Kyrgyz",
        "la" => "Latin",
        "lo" => "Lao",
        "lt" => "Lithuanian",
        "lv" => "Latvian",
        "mg" => "Malagasy",
        "mk" => "Macedonian",
        "ml" => "Malayalam",
        "mn" => "Mongolian",
        "mr" => "Marathi",
        "ms" => "Malay",
        "mt" => "Maltese",
        "my" => "Burmese",
        "nb" => "Norwegian Bokmal",
        "ne" => "Nepali",
        "nl" => "Dutch",
        "nn" => "Norwegian Nynorsk",
        "no" => "Norwegian",
        "pa" => "Punjabi",
        "pl" => "Polish",
        "ps" => "Pashto",
        "pt" => "Portuguese",
        "qu" => "Quechua",
        "rm" => "Romansh",
        "ro" => "Romanian",
        "ru" => "Russian",
        "rw" => "Kinyarwanda",
        "sa" => "Sanskrit",
        "sd" => "Sindhi",
        "si" => "Sinhala",
        "sk" => "Slovak",
        "sl" => "Slovenian",
        "so" => "Somali",
        "sq" => "Albanian",
        "sr" => "Serbian",
        "sv" => "Swedish",
        "sw" => "Swahili",
        "ta" => "Tamil",
        "te" => "Telugu",
        "tg" => "Tajik",
        "th" => "Thai",
        "ti" => "Tigrinya",
        "tk" => "Turkmen",
        "tl" => "Tagalog",
        "tr" => "Turkish",
        "tt" => "Tatar",
        "uk" => "Ukrainian",
        "ur" => "Urdu",
        "uz" => "Uzbek",
        "vi" => "Vietnamese",
        "yi" => "Yiddish",
        "zh" => "Chinese",
        "zu" => "Zulu",
        _ => "Unknown",
    }
}
