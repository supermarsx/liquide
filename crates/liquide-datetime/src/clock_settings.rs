use crate::datetime::DateTime;

/// Clock display format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClockFormat {
    /// 12-hour clock with AM/PM.
    H12,
    /// 24-hour clock.
    H24,
    /// Custom format string. Supported tokens:
    /// - `%H` — hour (24h, zero-padded)
    /// - `%I` — hour (12h, zero-padded)
    /// - `%M` — minute (zero-padded)
    /// - `%S` — second (zero-padded)
    /// - `%p` — AM/PM
    /// - `%Y` — 4-digit year
    /// - `%m` — month (zero-padded)
    /// - `%d` — day (zero-padded)
    /// - `%A` — full weekday name
    /// - `%a` — abbreviated weekday name
    /// - `%B` — full month name
    /// - `%b` — abbreviated month name
    Custom(String),
}

/// Desktop clock configuration for the status bar / panel.
#[derive(Debug, Clone)]
pub struct ClockSettings {
    /// Time display format.
    pub format: ClockFormat,
    /// Whether to show seconds in the time display.
    pub show_seconds: bool,
    /// Whether to show the date alongside time.
    pub show_date: bool,
    /// IANA timezone ID, e.g. "America/New_York".
    pub timezone: String,
}

impl ClockSettings {
    /// Create default clock settings: 24-hour, no seconds, show date, UTC.
    pub fn new() -> Self {
        Self {
            format: ClockFormat::H24,
            show_seconds: false,
            show_date: true,
            timezone: "UTC".to_string(),
        }
    }

    /// Format the time portion of a DateTime.
    pub fn format_time(&self, dt: &DateTime) -> String {
        match &self.format {
            ClockFormat::H24 => {
                if self.show_seconds {
                    format!("{:02}:{:02}:{:02}", dt.hour, dt.minute, dt.second)
                } else {
                    format!("{:02}:{:02}", dt.hour, dt.minute)
                }
            }
            ClockFormat::H12 => {
                let (h12, ampm) = to_12hour(dt.hour);
                if self.show_seconds {
                    format!("{:02}:{:02}:{:02} {}", h12, dt.minute, dt.second, ampm)
                } else {
                    format!("{:02}:{:02} {}", h12, dt.minute, ampm)
                }
            }
            ClockFormat::Custom(fmt) => apply_format(fmt, dt),
        }
    }

    /// Format the date portion of a DateTime, e.g. "Mon, Jan 15, 2024".
    pub fn format_date(&self, dt: &DateTime) -> String {
        let wd = dt.weekday();
        let month_name = month_short_name(dt.month);
        format!("{}, {} {}, {}", wd.short_name(), month_name, dt.day, dt.year)
    }
}

impl Default for ClockSettings {
    fn default() -> Self {
        Self::new()
    }
}

fn to_12hour(hour: u32) -> (u32, &'static str) {
    match hour {
        0 => (12, "AM"),
        1..=11 => (hour, "AM"),
        12 => (12, "PM"),
        _ => (hour - 12, "PM"),
    }
}

fn month_full_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "???",
    }
}

fn month_short_name(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    }
}

/// Apply a custom format string with `%` tokens.
fn apply_format(fmt: &str, dt: &DateTime) -> String {
    let mut result = String::with_capacity(fmt.len() + 16);
    let bytes = fmt.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 1 < bytes.len() {
            i += 1;
            match bytes[i] {
                b'H' => result.push_str(&format!("{:02}", dt.hour)),
                b'I' => {
                    let (h12, _) = to_12hour(dt.hour);
                    result.push_str(&format!("{:02}", h12));
                }
                b'M' => result.push_str(&format!("{:02}", dt.minute)),
                b'S' => result.push_str(&format!("{:02}", dt.second)),
                b'p' => {
                    let (_, ampm) = to_12hour(dt.hour);
                    result.push_str(ampm);
                }
                b'Y' => result.push_str(&format!("{:04}", dt.year)),
                b'm' => result.push_str(&format!("{:02}", dt.month)),
                b'd' => result.push_str(&format!("{:02}", dt.day)),
                b'A' => result.push_str(dt.weekday().name()),
                b'a' => result.push_str(dt.weekday().short_name()),
                b'B' => result.push_str(month_full_name(dt.month)),
                b'b' => result.push_str(month_short_name(dt.month)),
                b'%' => result.push('%'),
                other => {
                    result.push('%');
                    result.push(other as char);
                }
            }
        } else {
            result.push(bytes[i] as char);
        }
        i += 1;
    }
    result
}
