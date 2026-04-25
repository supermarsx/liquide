//! Internationalization, localization, and locale management for LiquiDE.
//!
//! Provides locale detection, number/date/time formatting, translation catalogs,
//! text direction detection, calendar systems, and measurement systems — all with
//! compact built-in data tables (no external data files required).

mod calendar;
mod catalog;
mod date_format;
mod direction;
mod error;
mod locale;
mod manager;
mod measurement;
mod number_format;

#[cfg(test)]
mod tests;

pub use calendar::{CalendarSystem, calendar_for_locale};
pub use catalog::Catalog;
pub use date_format::{
    DateStyle, TimeStyle, day_name, day_name_short, format_date, format_time, month_name,
    month_name_short,
};
pub use direction::{TextDir, text_direction};
pub use error::LocaleError;
pub use locale::{Locale, language_name_lookup};
pub use manager::{LocaleManager, available_locales, system_locale};
pub use measurement::{MeasurementSystem, measurement_for_locale};
pub use number_format::{format_currency, format_number, format_percent};
