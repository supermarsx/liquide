use crate::weekday::Weekday;

/// A simple date-time representation with no external dependencies.
///
/// Stores year, month, day, hour, minute, second in the Gregorian calendar.
/// All conversions assume UTC unless explicitly combined with a timezone offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DateTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl DateTime {
    /// Create a new DateTime. No validation is performed — callers should
    /// ensure values are in-range (month 1..=12, day 1..=31, etc.).
    pub fn new(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Self {
        Self { year, month, day, hour, minute, second }
    }

    /// Returns `true` if `self.year` is a leap year in the Gregorian calendar.
    pub fn is_leap_year(&self) -> bool {
        is_leap_year(self.year)
    }

    /// Number of days in `self.month` for `self.year`.
    pub fn days_in_month(&self) -> u32 {
        days_in_month(self.year, self.month)
    }

    /// 1-based day of the year (Jan 1 = 1, Dec 31 = 365 or 366).
    pub fn day_of_year(&self) -> u32 {
        let mut doy = 0u32;
        for m in 1..self.month {
            doy += days_in_month(self.year, m);
        }
        doy + self.day
    }

    /// Compute the day of the week using Zeller's congruence (Gregorian variant).
    pub fn weekday(&self) -> Weekday {
        // Zeller's congruence: adjust January and February to be months 13, 14
        // of the previous year.
        let (mut y, mut m) = (self.year as i64, self.month as i64);
        if m < 3 {
            m += 12;
            y -= 1;
        }
        let q = self.day as i64;
        let k = y % 100;
        let j = y / 100;
        let h = (q + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 + 5 * j) % 7;
        // h: 0 = Saturday, 1 = Sunday, 2 = Monday, ...
        // Convert to our numbering: Sunday = 0
        let dow = ((h + 6) % 7) as u32;
        Weekday::from_number(dow)
    }

    /// Convert to a Unix timestamp (seconds since 1970-01-01T00:00:00 UTC).
    ///
    /// Works for dates from years well before 1970 through far into the future.
    pub fn to_unix_timestamp(&self) -> i64 {
        // Days from civil date to Unix epoch using the algorithm from
        // Howard Hinnant's `days_from_civil`.
        let y = if self.month <= 2 { self.year as i64 - 1 } else { self.year as i64 };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = (y - era * 400) as u64; // year of era [0, 399]
        let m = self.month as u64;
        let doy = if m > 2 {
            (153 * (m - 3) + 2) / 5 + self.day as u64 - 1
        } else {
            (153 * (m + 9) + 2) / 5 + self.day as u64 - 1
        };
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // day of era
        let days = era * 146097 + doe as i64 - 719468; // days since epoch

        days * 86400 + self.hour as i64 * 3600 + self.minute as i64 * 60 + self.second as i64
    }

    /// Construct a `DateTime` from a Unix timestamp (seconds since epoch, UTC).
    pub fn from_unix_timestamp(ts: i64) -> Self {
        let mut secs = ts;
        let day_secs = secs.rem_euclid(86400);
        secs -= day_secs;
        let mut days = secs / 86400;

        // Shift to March-based calendar epoch for easier computation
        days += 719468;
        let era = if days >= 0 { days } else { days - 146096 } / 146097;
        let doe = (days - era * 146097) as u64; // day of era [0, 146096]
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = if m <= 2 { y + 1 } else { y };

        let hour = (day_secs / 3600) as u32;
        let minute = ((day_secs % 3600) / 60) as u32;
        let second = (day_secs % 60) as u32;

        DateTime {
            year: year as i32,
            month: m as u32,
            day: d as u32,
            hour,
            minute,
            second,
        }
    }

    /// Format as ISO 8601 string: "2024-01-15T13:30:00".
    pub fn format_iso8601(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.year, self.month, self.day,
            self.hour, self.minute, self.second,
        )
    }

    /// Add a signed offset in minutes, producing a new DateTime.
    /// Useful for converting UTC to local time given a timezone offset.
    pub fn with_offset_minutes(&self, offset: i32) -> DateTime {
        let ts = self.to_unix_timestamp() + offset as i64 * 60;
        DateTime::from_unix_timestamp(ts)
    }
}

impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.format_iso8601())
    }
}

impl PartialOrd for DateTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DateTime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_unix_timestamp().cmp(&other.to_unix_timestamp())
    }
}

/// Returns `true` if the given year is a leap year.
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Number of days in the given month (1-indexed) of the given year.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 => 31,
        2 => if is_leap_year(year) { 29 } else { 28 },
        3 => 31,
        4 => 30,
        5 => 31,
        6 => 30,
        7 => 31,
        8 => 31,
        9 => 30,
        10 => 31,
        11 => 30,
        12 => 31,
        _ => 30, // fallback
    }
}
