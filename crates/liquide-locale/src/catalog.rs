use std::collections::HashMap;

use crate::error::LocaleError;
use crate::locale::Locale;

/// A translation catalog: maps message keys to translated strings for a single locale.
///
/// Supports simple key-value translations and basic plural forms using the
/// convention `key.one` / `key.other`.
#[derive(Debug, Clone)]
pub struct Catalog {
    locale: Locale,
    messages: HashMap<String, String>,
}

impl Catalog {
    /// Create a new empty catalog for the given locale.
    pub fn new(locale: Locale) -> Self {
        Self {
            locale,
            messages: HashMap::new(),
        }
    }

    /// The locale this catalog provides translations for.
    pub fn locale(&self) -> &Locale {
        &self.locale
    }

    /// Add a single translation.
    pub fn add_translation(&mut self, key: &str, value: &str) {
        self.messages.insert(key.to_string(), value.to_string());
    }

    /// Look up a translation by key. Returns the key itself as fallback if no
    /// translation exists.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        match self.messages.get(key) {
            Some(val) => val.as_str(),
            None => key,
        }
    }

    /// Look up a plural form. Uses the convention:
    /// - `key.one` for count == 1
    /// - `key.other` for all other counts
    ///
    /// Falls back to `key` if neither plural form is found.
    pub fn get_plural<'a>(&'a self, key: &'a str, count: u64) -> &'a str {
        let plural_key = if count == 1 {
            format!("{}.one", key)
        } else {
            format!("{}.other", key)
        };

        if let Some(val) = self.messages.get(&plural_key) {
            return val.as_str();
        }

        // Fallback to base key
        self.get(key)
    }

    /// Load translations from a simple text format.
    ///
    /// Format: one `key = value` pair per line. Empty lines and lines starting
    /// with `#` are ignored. Leading/trailing whitespace on keys and values is trimmed.
    ///
    /// # Example
    /// ```text
    /// # Greetings
    /// greeting.hello = Hello!
    /// greeting.goodbye = Goodbye!
    /// items.one = 1 item
    /// items.other = {count} items
    /// ```
    pub fn load_from_string(data: &str, locale: Locale) -> Result<Self, LocaleError> {
        let mut catalog = Self::new(locale);

        for (line_num, line) in data.lines().enumerate() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let eq_pos = line.find('=').ok_or_else(|| {
                LocaleError::ParseError(format!("line {}: missing '=' separator", line_num + 1))
            })?;

            let key = line[..eq_pos].trim();
            let value = line[eq_pos + 1..].trim();

            if key.is_empty() {
                return Err(LocaleError::ParseError(format!(
                    "line {}: empty key",
                    line_num + 1
                )));
            }

            catalog.messages.insert(key.to_string(), value.to_string());
        }

        Ok(catalog)
    }

    /// Merge another catalog's translations into this one.
    ///
    /// Translations from `other` override existing ones with the same key.
    /// This is useful for overlaying theme or plugin translations.
    pub fn merge(&mut self, other: &Catalog) {
        for (key, value) in &other.messages {
            self.messages.insert(key.clone(), value.clone());
        }
    }

    /// Return the number of translations in this catalog.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether this catalog is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Iterate over all translation entries.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.messages.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Check whether a key exists in this catalog.
    pub fn contains_key(&self, key: &str) -> bool {
        self.messages.contains_key(key)
    }
}
