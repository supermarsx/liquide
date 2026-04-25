use crate::locale::Locale;

/// Date formatting style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateStyle {
    /// Short numeric: 1/15/24 (US) or 15/01/24 (EU) or 24/01/15 (JP).
    Short,
    /// Medium: Jan 15, 2024.
    Medium,
    /// Long: January 15, 2024.
    Long,
    /// ISO 8601: 2024-01-15.
    ISO,
}

/// Time formatting style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeStyle {
    /// Short 12-hour: 3:30 PM.
    Short,
    /// Medium 12-hour with seconds: 3:30:00 PM.
    Medium,
    /// Long 24-hour with seconds: 15:30:00.
    Long,
    /// Short 24-hour: 15:30.
    H24,
}

/// Month names for a locale (index 0 = January).
struct MonthNames {
    full: [&'static str; 12],
    abbreviated: [&'static str; 12],
}

/// Day names for a locale (index 0 = Sunday).
struct DayNames {
    full: [&'static str; 7],
    abbreviated: [&'static str; 7],
}

fn month_names_for_language(lang: &str) -> MonthNames {
    match lang {
        "de" => MonthNames {
            full: [
                "Januar",
                "Februar",
                "M\u{00e4}rz",
                "April",
                "Mai",
                "Juni",
                "Juli",
                "August",
                "September",
                "Oktober",
                "November",
                "Dezember",
            ],
            abbreviated: [
                "Jan",
                "Feb",
                "M\u{00e4}r",
                "Apr",
                "Mai",
                "Jun",
                "Jul",
                "Aug",
                "Sep",
                "Okt",
                "Nov",
                "Dez",
            ],
        },
        "fr" => MonthNames {
            full: [
                "janvier",
                "f\u{00e9}vrier",
                "mars",
                "avril",
                "mai",
                "juin",
                "juillet",
                "ao\u{00fb}t",
                "septembre",
                "octobre",
                "novembre",
                "d\u{00e9}cembre",
            ],
            abbreviated: [
                "janv.",
                "f\u{00e9}vr.",
                "mars",
                "avr.",
                "mai",
                "juin",
                "juil.",
                "ao\u{00fb}t",
                "sept.",
                "oct.",
                "nov.",
                "d\u{00e9}c.",
            ],
        },
        "es" => MonthNames {
            full: [
                "enero",
                "febrero",
                "marzo",
                "abril",
                "mayo",
                "junio",
                "julio",
                "agosto",
                "septiembre",
                "octubre",
                "noviembre",
                "diciembre",
            ],
            abbreviated: [
                "ene.", "feb.", "mar.", "abr.", "may.", "jun.", "jul.", "ago.", "sept.", "oct.",
                "nov.", "dic.",
            ],
        },
        "ja" => MonthNames {
            full: [
                "1\u{6708}",
                "2\u{6708}",
                "3\u{6708}",
                "4\u{6708}",
                "5\u{6708}",
                "6\u{6708}",
                "7\u{6708}",
                "8\u{6708}",
                "9\u{6708}",
                "10\u{6708}",
                "11\u{6708}",
                "12\u{6708}",
            ],
            abbreviated: [
                "1\u{6708}",
                "2\u{6708}",
                "3\u{6708}",
                "4\u{6708}",
                "5\u{6708}",
                "6\u{6708}",
                "7\u{6708}",
                "8\u{6708}",
                "9\u{6708}",
                "10\u{6708}",
                "11\u{6708}",
                "12\u{6708}",
            ],
        },
        "zh" => MonthNames {
            full: [
                "\u{4e00}\u{6708}",
                "\u{4e8c}\u{6708}",
                "\u{4e09}\u{6708}",
                "\u{56db}\u{6708}",
                "\u{4e94}\u{6708}",
                "\u{516d}\u{6708}",
                "\u{4e03}\u{6708}",
                "\u{516b}\u{6708}",
                "\u{4e5d}\u{6708}",
                "\u{5341}\u{6708}",
                "\u{5341}\u{4e00}\u{6708}",
                "\u{5341}\u{4e8c}\u{6708}",
            ],
            abbreviated: [
                "1\u{6708}",
                "2\u{6708}",
                "3\u{6708}",
                "4\u{6708}",
                "5\u{6708}",
                "6\u{6708}",
                "7\u{6708}",
                "8\u{6708}",
                "9\u{6708}",
                "10\u{6708}",
                "11\u{6708}",
                "12\u{6708}",
            ],
        },
        // English and fallback
        _ => MonthNames {
            full: [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ],
            abbreviated: [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ],
        },
    }
}

fn day_names_for_language(lang: &str) -> DayNames {
    match lang {
        "de" => DayNames {
            full: [
                "Sonntag",
                "Montag",
                "Dienstag",
                "Mittwoch",
                "Donnerstag",
                "Freitag",
                "Samstag",
            ],
            abbreviated: ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"],
        },
        "fr" => DayNames {
            full: [
                "dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi",
            ],
            abbreviated: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
        },
        "es" => DayNames {
            full: [
                "domingo",
                "lunes",
                "martes",
                "mi\u{00e9}rcoles",
                "jueves",
                "viernes",
                "s\u{00e1}bado",
            ],
            abbreviated: [
                "dom.",
                "lun.",
                "mar.",
                "mi\u{00e9}.",
                "jue.",
                "vie.",
                "s\u{00e1}b.",
            ],
        },
        "ja" => DayNames {
            full: [
                "\u{65e5}\u{66dc}\u{65e5}",
                "\u{6708}\u{66dc}\u{65e5}",
                "\u{706b}\u{66dc}\u{65e5}",
                "\u{6c34}\u{66dc}\u{65e5}",
                "\u{6728}\u{66dc}\u{65e5}",
                "\u{91d1}\u{66dc}\u{65e5}",
                "\u{571f}\u{66dc}\u{65e5}",
            ],
            abbreviated: [
                "\u{65e5}", "\u{6708}", "\u{706b}", "\u{6c34}", "\u{6728}", "\u{91d1}", "\u{571f}",
            ],
        },
        "zh" => DayNames {
            full: [
                "\u{661f}\u{671f}\u{65e5}",
                "\u{661f}\u{671f}\u{4e00}",
                "\u{661f}\u{671f}\u{4e8c}",
                "\u{661f}\u{671f}\u{4e09}",
                "\u{661f}\u{671f}\u{56db}",
                "\u{661f}\u{671f}\u{4e94}",
                "\u{661f}\u{671f}\u{516d}",
            ],
            abbreviated: [
                "\u{5468}\u{65e5}",
                "\u{5468}\u{4e00}",
                "\u{5468}\u{4e8c}",
                "\u{5468}\u{4e09}",
                "\u{5468}\u{56db}",
                "\u{5468}\u{4e94}",
                "\u{5468}\u{516d}",
            ],
        },
        _ => DayNames {
            full: [
                "Sunday",
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
            ],
            abbreviated: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        },
    }
}

/// Return the full month name for the given locale.
///
/// `month` is 1-based (1 = January, 12 = December).
pub fn month_name(month: u32, locale: &Locale) -> &'static str {
    let names = month_names_for_language(&locale.language);
    let idx = (month.saturating_sub(1) as usize).min(11);
    names.full[idx]
}

/// Return the abbreviated month name for the given locale.
pub fn month_name_short(month: u32, locale: &Locale) -> &'static str {
    let names = month_names_for_language(&locale.language);
    let idx = (month.saturating_sub(1) as usize).min(11);
    names.abbreviated[idx]
}

/// Return the full day name for the given locale.
///
/// `day` is 0-based (0 = Sunday, 6 = Saturday).
pub fn day_name(day: u32, locale: &Locale) -> &'static str {
    let names = day_names_for_language(&locale.language);
    let idx = (day as usize).min(6);
    names.full[idx]
}

/// Return the abbreviated day name for the given locale.
pub fn day_name_short(day: u32, locale: &Locale) -> &'static str {
    let names = day_names_for_language(&locale.language);
    let idx = (day as usize).min(6);
    names.abbreviated[idx]
}

/// Determine the date component order for a locale.
///
/// Returns a tuple of format characters: `('M', 'D', 'Y')` means month/day/year,
/// `('D', 'M', 'Y')` means day/month/year, `('Y', 'M', 'D')` means year/month/day.
fn date_order(locale: &Locale) -> (char, char, char) {
    match locale.language.as_str() {
        // Year-month-day: East Asian, some European
        "ja" | "zh" | "ko" | "hu" | "lt" => ('Y', 'M', 'D'),
        // Month-day-year: US English
        "en" => {
            match locale.territory.as_deref() {
                Some("US") | None => ('M', 'D', 'Y'),
                _ => ('D', 'M', 'Y'), // en_GB etc.
            }
        }
        // Day-month-year: most of the world
        _ => ('D', 'M', 'Y'),
    }
}

/// Format a date according to locale conventions.
///
/// `month` is 1-based (1 = January). `year` can be negative for BCE.
pub fn format_date(year: i32, month: u32, day: u32, locale: &Locale, style: DateStyle) -> String {
    match style {
        DateStyle::ISO => {
            format!("{:04}-{:02}-{:02}", year, month, day)
        }
        DateStyle::Short => {
            let y = (year % 100).unsigned_abs();
            let order = date_order(locale);
            let sep = match locale.language.as_str() {
                "ja" | "zh" | "ko" => "/",
                "de" | "fr" | "es" | "it" | "pt" | "ru" | "pl" | "nl" => ".",
                "en" => match locale.territory.as_deref() {
                    Some("US") | None => "/",
                    _ => "/",
                },
                _ => "/",
            };
            match order {
                ('M', 'D', 'Y') => format!("{}{}{}{}{}", month, sep, day, sep, y),
                ('D', 'M', 'Y') => format!("{}{}{}{}{}", day, sep, month, sep, y),
                ('Y', 'M', 'D') => format!("{}{}{}{}{}", y, sep, month, sep, day),
                _ => format!("{}{}{}{}{}", month, sep, day, sep, y),
            }
        }
        DateStyle::Medium => {
            let names = month_names_for_language(&locale.language);
            let month_idx = (month.saturating_sub(1) as usize).min(11);
            let abbr = names.abbreviated[month_idx];
            let order = date_order(locale);

            match locale.language.as_str() {
                "ja" | "zh" => {
                    format!(
                        "{}{}{}{}",
                        year,
                        "\u{5e74}",
                        abbr,
                        format!("{}\u{65e5}", day)
                    )
                }
                "de" => format!("{}. {} {}", day, abbr, year),
                "fr" => format!("{} {} {}", day, abbr, year),
                "es" => format!("{} {} {}", day, abbr, year),
                _ => match order {
                    ('M', 'D', 'Y') => format!("{} {}, {}", abbr, day, year),
                    ('D', 'M', 'Y') => format!("{} {} {}", day, abbr, year),
                    ('Y', 'M', 'D') => format!("{} {} {}", year, abbr, day),
                    _ => format!("{} {}, {}", abbr, day, year),
                },
            }
        }
        DateStyle::Long => {
            let names = month_names_for_language(&locale.language);
            let month_idx = (month.saturating_sub(1) as usize).min(11);
            let full = names.full[month_idx];
            let order = date_order(locale);

            match locale.language.as_str() {
                "ja" | "zh" => {
                    format!(
                        "{}{}{}{}{}{}",
                        year, "\u{5e74}", month, "\u{6708}", day, "\u{65e5}"
                    )
                }
                "de" => format!("{}. {} {}", day, full, year),
                "fr" => format!("{} {} {}", day, full, year),
                "es" => format!("{} de {} de {}", day, full, year),
                _ => match order {
                    ('M', 'D', 'Y') => format!("{} {}, {}", full, day, year),
                    ('D', 'M', 'Y') => format!("{} {} {}", day, full, year),
                    ('Y', 'M', 'D') => format!("{} {} {}", year, full, day),
                    _ => format!("{} {}, {}", full, day, year),
                },
            }
        }
    }
}

/// Format a time according to locale conventions.
pub fn format_time(
    hour: u32,
    minute: u32,
    second: u32,
    locale: &Locale,
    style: TimeStyle,
) -> String {
    match style {
        TimeStyle::H24 => {
            format!("{:02}:{:02}", hour, minute)
        }
        TimeStyle::Long => {
            format!("{:02}:{:02}:{:02}", hour, minute, second)
        }
        TimeStyle::Short => {
            let use_12h = uses_12h_clock(locale);
            if use_12h {
                let (h12, ampm) = to_12h(hour);
                format!("{}:{:02} {}", h12, minute, ampm)
            } else {
                format!("{:02}:{:02}", hour, minute)
            }
        }
        TimeStyle::Medium => {
            let use_12h = uses_12h_clock(locale);
            if use_12h {
                let (h12, ampm) = to_12h(hour);
                format!("{}:{:02}:{:02} {}", h12, minute, second, ampm)
            } else {
                format!("{:02}:{:02}:{:02}", hour, minute, second)
            }
        }
    }
}

/// Whether a locale typically uses 12-hour clock format.
fn uses_12h_clock(locale: &Locale) -> bool {
    match locale.language.as_str() {
        "en" | "ko" | "ar" | "hi" => true,
        _ => false,
    }
}

/// Convert 24h to 12h format. Returns (hour_12, "AM"/"PM").
fn to_12h(hour: u32) -> (u32, &'static str) {
    let ampm = if hour < 12 { "AM" } else { "PM" };
    let h12 = match hour % 12 {
        0 => 12,
        h => h,
    };
    (h12, ampm)
}
