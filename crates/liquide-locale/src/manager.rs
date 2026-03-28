use crate::locale::Locale;

/// Manages system locale detection and session locale state.
pub struct LocaleManager {
    active: Locale,
}

impl LocaleManager {
    /// Create a new manager, detecting the system locale.
    pub fn new() -> Self {
        Self {
            active: system_locale(),
        }
    }

    /// Create a manager with a specific locale.
    pub fn with_locale(locale: Locale) -> Self {
        Self { active: locale }
    }

    /// Detect the system locale from environment variables.
    ///
    /// Checks (in order): `LC_ALL`, `LC_MESSAGES`, `LANG`.
    /// Falls back to `en_US.UTF-8` if nothing is set.
    pub fn system_locale(&self) -> Locale {
        system_locale()
    }

    /// List common available locales (built-in set).
    ///
    /// In a full desktop environment this would enumerate installed system locales;
    /// here we return a representative built-in set.
    pub fn available_locales(&self) -> Vec<Locale> {
        available_locales()
    }

    /// Set the active locale for this session.
    pub fn set_locale(&mut self, locale: &Locale) {
        self.active = locale.clone();
    }

    /// Return the currently active locale.
    pub fn active_locale(&self) -> &Locale {
        &self.active
    }
}

impl Default for LocaleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect the system locale from environment variables.
pub fn system_locale() -> Locale {
    // Try LC_ALL first, then LC_MESSAGES, then LANG
    for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() {
                if let Some(loc) = Locale::parse(&val) {
                    return loc;
                }
            }
        }
    }
    // Default fallback
    Locale::with_encoding("en", "US", "UTF-8")
}

/// Return a built-in list of common locales.
pub fn available_locales() -> Vec<Locale> {
    vec![
        Locale::with_encoding("en", "US", "UTF-8"),
        Locale::with_encoding("en", "GB", "UTF-8"),
        Locale::with_encoding("de", "DE", "UTF-8"),
        Locale::with_encoding("fr", "FR", "UTF-8"),
        Locale::with_encoding("es", "ES", "UTF-8"),
        Locale::with_encoding("it", "IT", "UTF-8"),
        Locale::with_encoding("pt", "BR", "UTF-8"),
        Locale::with_encoding("pt", "PT", "UTF-8"),
        Locale::with_encoding("ja", "JP", "UTF-8"),
        Locale::with_encoding("zh", "CN", "UTF-8"),
        Locale::with_encoding("zh", "TW", "UTF-8"),
        Locale::with_encoding("ko", "KR", "UTF-8"),
        Locale::with_encoding("ar", "SA", "UTF-8"),
        Locale::with_encoding("he", "IL", "UTF-8"),
        Locale::with_encoding("ru", "RU", "UTF-8"),
        Locale::with_encoding("nl", "NL", "UTF-8"),
        Locale::with_encoding("pl", "PL", "UTF-8"),
        Locale::with_encoding("sv", "SE", "UTF-8"),
        Locale::with_encoding("da", "DK", "UTF-8"),
        Locale::with_encoding("fi", "FI", "UTF-8"),
        Locale::with_encoding("nb", "NO", "UTF-8"),
        Locale::with_encoding("tr", "TR", "UTF-8"),
        Locale::with_encoding("th", "TH", "UTF-8"),
        Locale::with_encoding("vi", "VN", "UTF-8"),
        Locale::with_encoding("hi", "IN", "UTF-8"),
        Locale::with_encoding("uk", "UA", "UTF-8"),
        Locale::with_encoding("el", "GR", "UTF-8"),
        Locale::with_encoding("cs", "CZ", "UTF-8"),
        Locale::with_encoding("hu", "HU", "UTF-8"),
        Locale::with_encoding("ro", "RO", "UTF-8"),
    ]
}
