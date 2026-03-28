use std::fmt;

/// Error parsing a cron expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Wrong number of fields (expected 5).
    WrongFieldCount { found: usize },
    /// A field value is out of the allowed range.
    OutOfRange {
        field: &'static str,
        value: u8,
        min: u8,
        max: u8,
    },
    /// A field token could not be parsed.
    InvalidToken {
        field: &'static str,
        token: String,
    },
    /// Step value is zero.
    ZeroStep { field: &'static str },
    /// Range start > end.
    InvalidRange {
        field: &'static str,
        start: u8,
        end: u8,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::WrongFieldCount { found } => {
                write!(f, "expected 5 cron fields, found {found}")
            }
            ParseError::OutOfRange {
                field,
                value,
                min,
                max,
            } => {
                write!(f, "{field}: value {value} out of range [{min}, {max}]")
            }
            ParseError::InvalidToken { field, token } => {
                write!(f, "{field}: invalid token '{token}'")
            }
            ParseError::ZeroStep { field } => {
                write!(f, "{field}: step value cannot be zero")
            }
            ParseError::InvalidRange { field, start, end } => {
                write!(f, "{field}: range start {start} > end {end}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// A single cron field specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CronField {
    /// `*` — matches any value.
    Any,
    /// A single value.
    Value(u8),
    /// An inclusive range `start-end`.
    Range(u8, u8),
    /// `*/step` — every N values from the field minimum.
    Step(u8),
    /// A list of specific values.
    List(Vec<u8>),
}

impl CronField {
    /// Whether this field matches a given value.
    pub fn matches(&self, value: u8, min: u8) -> bool {
        match self {
            CronField::Any => true,
            CronField::Value(v) => *v == value,
            CronField::Range(lo, hi) => value >= *lo && value <= *hi,
            CronField::Step(step) => {
                if *step == 0 {
                    return false;
                }
                (value - min) % step == 0
            }
            CronField::List(vals) => vals.contains(&value),
        }
    }

    /// Return all values this field can match within [min, max].
    fn expand(&self, min: u8, max: u8) -> Vec<u8> {
        match self {
            CronField::Any => (min..=max).collect(),
            CronField::Value(v) => {
                if *v >= min && *v <= max {
                    vec![*v]
                } else {
                    vec![]
                }
            }
            CronField::Range(lo, hi) => {
                let lo = (*lo).max(min);
                let hi = (*hi).min(max);
                (lo..=hi).collect()
            }
            CronField::Step(step) => {
                if *step == 0 {
                    return vec![];
                }
                (min..=max).filter(|v| (v - min) % step == 0).collect()
            }
            CronField::List(vals) => {
                let mut out: Vec<u8> = vals.iter().copied().filter(|v| *v >= min && *v <= max).collect();
                out.sort();
                out.dedup();
                out
            }
        }
    }
}

/// Parse a single cron field token.
fn parse_field(token: &str, field_name: &'static str, min: u8, max: u8) -> Result<CronField, ParseError> {
    // Check for list (comma-separated)
    if token.contains(',') {
        let mut vals = Vec::new();
        for part in token.split(',') {
            let v = part.trim().parse::<u8>().map_err(|_| ParseError::InvalidToken {
                field: field_name,
                token: part.to_string(),
            })?;
            if v < min || v > max {
                return Err(ParseError::OutOfRange {
                    field: field_name,
                    value: v,
                    min,
                    max,
                });
            }
            vals.push(v);
        }
        vals.sort();
        vals.dedup();
        return Ok(CronField::List(vals));
    }

    // Check for step: */N
    if let Some(rest) = token.strip_prefix("*/") {
        let step = rest.parse::<u8>().map_err(|_| ParseError::InvalidToken {
            field: field_name,
            token: token.to_string(),
        })?;
        if step == 0 {
            return Err(ParseError::ZeroStep { field: field_name });
        }
        return Ok(CronField::Step(step));
    }

    // Wildcard
    if token == "*" {
        return Ok(CronField::Any);
    }

    // Range: A-B
    if token.contains('-') {
        let parts: Vec<&str> = token.splitn(2, '-').collect();
        let lo = parts[0].parse::<u8>().map_err(|_| ParseError::InvalidToken {
            field: field_name,
            token: token.to_string(),
        })?;
        let hi = parts[1].parse::<u8>().map_err(|_| ParseError::InvalidToken {
            field: field_name,
            token: token.to_string(),
        })?;
        if lo > hi {
            return Err(ParseError::InvalidRange {
                field: field_name,
                start: lo,
                end: hi,
            });
        }
        if lo < min || hi > max {
            return Err(ParseError::OutOfRange {
                field: field_name,
                value: if lo < min { lo } else { hi },
                min,
                max,
            });
        }
        return Ok(CronField::Range(lo, hi));
    }

    // Single value
    let v = token.parse::<u8>().map_err(|_| ParseError::InvalidToken {
        field: field_name,
        token: token.to_string(),
    })?;
    if v < min || v > max {
        return Err(ParseError::OutOfRange {
            field: field_name,
            value: v,
            min,
            max,
        });
    }
    Ok(CronField::Value(v))
}

/// A simplified cron expression with five fields:
/// `minute hour day_of_month month day_of_week`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronExpr {
    pub minute: CronField,
    pub hour: CronField,
    pub day_of_month: CronField,
    pub month: CronField,
    pub day_of_week: CronField,
}

impl CronExpr {
    /// Parse a cron expression string like `"*/5 * * * *"`.
    ///
    /// Fields: `minute(0-59) hour(0-23) day_of_month(1-31) month(1-12) day_of_week(0-6, 0=Sun)`.
    pub fn parse(expr: &str) -> Result<CronExpr, ParseError> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(ParseError::WrongFieldCount {
                found: fields.len(),
            });
        }

        Ok(CronExpr {
            minute: parse_field(fields[0], "minute", 0, 59)?,
            hour: parse_field(fields[1], "hour", 0, 23)?,
            day_of_month: parse_field(fields[2], "day_of_month", 1, 31)?,
            month: parse_field(fields[3], "month", 1, 12)?,
            day_of_week: parse_field(fields[4], "day_of_week", 0, 6)?,
        })
    }

    /// Check whether a set of time components matches this cron expression.
    pub fn matches(&self, minute: u8, hour: u8, dom: u8, month: u8, dow: u8) -> bool {
        self.minute.matches(minute, 0)
            && self.hour.matches(hour, 0)
            && self.day_of_month.matches(dom, 1)
            && self.month.matches(month, 1)
            && self.day_of_week.matches(dow, 0)
    }

    /// Find the next timestamp (at or after `now`) that matches this expression.
    ///
    /// Searches up to ~2 years ahead, returning `None` if no match found.
    pub fn next_match(
        &self,
        start_year: u32,
        start_month: u8,
        _start_day: u8,
        _start_hour: u8,
        _start_minute: u8,
        now: u64,
    ) -> Option<u64> {
        let months = self.month.expand(1, 12);
        let hours = self.hour.expand(0, 23);
        let minutes = self.minute.expand(0, 59);

        if months.is_empty() || hours.is_empty() || minutes.is_empty() {
            return None;
        }

        // Iterate year, month, day, hour, minute — skip impossible branches early.
        // Search up to 2 years ahead to handle edge cases.
        for year in start_year..=(start_year + 2) {
            for &mo in &months {
                // Skip months entirely in the past
                if year == start_year && mo < start_month {
                    continue;
                }

                let max_day = days_in_month(year, mo);
                let days = self.day_of_month.expand(1, max_day);

                for &d in &days {
                    // Check day-of-week constraint
                    let ts_midnight = compose_ts(year, mo, d, 0, 0, 0);
                    let dow = weekday_from_ts(ts_midnight);
                    if !self.day_of_week.matches(dow, 0) {
                        continue;
                    }

                    for &h in &hours {
                        for &m in &minutes {
                            let candidate = compose_ts(year, mo, d, h, m, 0);
                            if candidate >= now {
                                return Some(candidate);
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

// ---- internal helpers (duplicated from task.rs to keep cron self-contained) ----

const SECS_PER_DAY: u64 = 86400;

fn days_in_month(year: u32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn compose_ts(year: u32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> u64 {
    let y = year as i64;
    let m = month as i64;
    let d = day as i64;
    let (y_adj, m_adj) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = if y_adj >= 0 {
        y_adj / 400
    } else {
        (y_adj - 399) / 400
    };
    let yoe = (y_adj - era * 400) as u64;
    let doy = (153 * m_adj as u64 + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let total_days = era * 146097 + doe as i64 - 719468;
    (total_days as u64) * SECS_PER_DAY
        + (hour as u64) * 3600
        + (minute as u64) * 60
        + (second as u64)
}

fn weekday_from_ts(ts: u64) -> u8 {
    let total_days = (ts / SECS_PER_DAY) as i64;
    ((total_days % 7 + 4) % 7) as u8
}
