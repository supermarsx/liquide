//! Internationalization, localization, and locale management for LiquiDE.
//!
//! Provides locale detection, number/date/time formatting, translation catalogs,
//! text direction detection, calendar systems, and measurement systems — all with
//! compact built-in data tables (no external data files required).

mod locale;
mod manager;
mod number_format;
mod date_format;
mod catalog;
mod direction;
mod calendar;
mod measurement;
mod error;

#[cfg(test)]
mod tests;

pub use locale::{Locale, language_name_lookup};
pub use manager::{LocaleManager, system_locale, available_locales};
pub use number_format::{format_number, format_currency, format_percent};
pub use date_format::{
    DateStyle, TimeStyle,
    format_date, format_time,
    month_name, month_name_short,
    day_name, day_name_short,
};
pub use catalog::Catalog;
pub use direction::{TextDir, text_direction};
pub use calendar::{CalendarSystem, calendar_for_locale};
pub use measurement::{MeasurementSystem, measurement_for_locale};
pub use error::LocaleError;
