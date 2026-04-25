use crate::locale::Locale;

/// Calendar system used by a locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalendarSystem {
    /// Gregorian calendar (default for most locales).
    Gregorian,
    /// Japanese calendar with era names (Reiwa, Heisei, Showa, etc.).
    Japanese,
    /// Islamic (Hijri) calendar.
    Islamic,
    /// Buddhist calendar (Gregorian + 543 years).
    Buddhist,
}

impl CalendarSystem {
    /// Human-readable name of the calendar system.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Gregorian => "Gregorian",
            Self::Japanese => "Japanese",
            Self::Islamic => "Islamic (Hijri)",
            Self::Buddhist => "Buddhist",
        }
    }

    /// Convert a Gregorian year to this calendar's year representation.
    ///
    /// Returns `(era_name_or_empty, year_in_era)`.
    pub fn convert_year(&self, gregorian_year: i32) -> (&'static str, i32) {
        match self {
            Self::Gregorian => ("", gregorian_year),
            Self::Japanese => japanese_era(gregorian_year),
            Self::Islamic => ("AH", gregorian_to_hijri_year(gregorian_year)),
            Self::Buddhist => ("BE", gregorian_year + 543),
        }
    }
}

/// Determine the primary calendar system for a locale.
pub fn calendar_for_locale(locale: &Locale) -> CalendarSystem {
    match locale.language.as_str() {
        "ja" => CalendarSystem::Japanese,
        "th" => CalendarSystem::Buddhist,
        "ar" | "fa" | "ur" | "ps" => {
            // Arabic-script locales generally use Islamic calendar alongside Gregorian
            match locale.territory.as_deref() {
                Some("SA") | Some("AE") | Some("QA") | Some("BH") | Some("KW") | Some("OM")
                | Some("YE") | Some("IR") | Some("AF") => CalendarSystem::Islamic,
                _ => CalendarSystem::Gregorian,
            }
        }
        _ => CalendarSystem::Gregorian,
    }
}

/// Map a Gregorian year to a Japanese era name and year within that era.
fn japanese_era(year: i32) -> (&'static str, i32) {
    if year >= 2019 {
        ("Reiwa", year - 2018)
    } else if year >= 1989 {
        ("Heisei", year - 1988)
    } else if year >= 1926 {
        ("Showa", year - 1925)
    } else if year >= 1912 {
        ("Taisho", year - 1911)
    } else if year >= 1868 {
        ("Meiji", year - 1867)
    } else {
        ("", year)
    }
}

/// Approximate conversion of a Gregorian year to a Hijri year.
///
/// This is a simplified calculation (not accounting for month/day precision).
/// A Hijri year is approximately 354.37 days.
fn gregorian_to_hijri_year(gregorian_year: i32) -> i32 {
    // Hijri calendar starts in 622 CE
    // One Hijri year ~ 354.36667 days, one Gregorian year ~ 365.2425 days
    // Ratio: 365.2425 / 354.36667 ~ 1.030684
    let diff = gregorian_year as f64 - 621.5694;
    (diff * 1.030684).floor() as i32
}
