use crate::cron::CronExpr;

/// Day of the week for weekly schedules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weekday {
    Sunday = 0,
    Monday = 1,
    Tuesday = 2,
    Wednesday = 3,
    Thursday = 4,
    Friday = 5,
    Saturday = 6,
}

impl Weekday {
    /// Convert a numeric day (0=Sunday .. 6=Saturday) to a Weekday.
    pub fn from_u8(v: u8) -> Option<Weekday> {
        match v {
            0 => Some(Weekday::Sunday),
            1 => Some(Weekday::Monday),
            2 => Some(Weekday::Tuesday),
            3 => Some(Weekday::Wednesday),
            4 => Some(Weekday::Thursday),
            5 => Some(Weekday::Friday),
            6 => Some(Weekday::Saturday),
            _ => None,
        }
    }
}

/// Result of running a scheduled task.
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// Process exit code (0 = success).
    pub exit_code: i32,
    /// First portion of stdout (truncated to 4 KiB).
    pub stdout_preview: String,
    /// First portion of stderr (truncated to 4 KiB).
    pub stderr_preview: String,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Unix timestamp when the process started.
    pub started_at: u64,
    /// Unix timestamp when the process finished.
    pub finished_at: u64,
}

/// When a task should run.
#[derive(Debug, Clone)]
pub enum Schedule {
    /// Run once at a specific Unix timestamp.
    Once(u64),
    /// Repeat every N seconds.
    Interval { seconds: u64 },
    /// Every day at a specific time.
    Daily { hour: u8, minute: u8 },
    /// Every week on a specific day and time.
    Weekly { day: Weekday, hour: u8, minute: u8 },
    /// Every month on a specific day and time.
    Monthly { day: u8, hour: u8, minute: u8 },
    /// Cron expression.
    Cron(CronExpr),
}

/// Seconds per minute/hour/day for timestamp arithmetic.
const SECS_PER_MINUTE: u64 = 60;
const SECS_PER_HOUR: u64 = 3600;
const SECS_PER_DAY: u64 = 86400;

/// Break a Unix timestamp into (year, month 1-12, day 1-31, hour 0-23, minute 0-59, second 0-59, weekday 0=Sun).
fn decompose_timestamp(ts: u64) -> (u32, u8, u8, u8, u8, u8, u8) {
    // Days since epoch (1970-01-01 is Thursday = weekday 4)
    let total_secs = ts;
    let day_secs = (total_secs % SECS_PER_DAY) as u32;
    let hour = (day_secs / 3600) as u8;
    let minute = ((day_secs % 3600) / 60) as u8;
    let second = (day_secs % 60) as u8;

    let mut total_days = (total_secs / SECS_PER_DAY) as i64;
    // 1970-01-01 is Thursday (4)
    let weekday = ((total_days % 7 + 4) % 7) as u8; // 0=Sun

    // Civil date from day count (algorithm from Howard Hinnant)
    total_days += 719468; // shift epoch to 0000-03-01
    let era = if total_days >= 0 {
        total_days / 146097
    } else {
        (total_days - 146096) / 146097
    };
    let doe = (total_days - era * 146097) as u32; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era
    let y = (yoe as i64 + era * 400) as i32;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month index [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let year = if m <= 2 { y + 1 } else { y };

    (year as u32, m as u8, d as u8, hour, minute, second, weekday)
}

/// Compose a Unix timestamp from civil date components (UTC).
fn compose_timestamp(year: u32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> u64 {
    let y = year as i64;
    let m = month as i64;
    let d = day as i64;

    // Adjust for March-based year (Hinnant algorithm)
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
        + (hour as u64) * SECS_PER_HOUR
        + (minute as u64) * SECS_PER_MINUTE
        + (second as u64)
}

/// Number of days in a given month (1-indexed), accounting for leap years.
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

impl Schedule {
    /// Compute the next occurrence at or after `now` (Unix timestamp, UTC).
    /// Returns `None` if the schedule will never trigger again.
    pub fn next_occurrence(&self, now: u64) -> Option<u64> {
        match self {
            Schedule::Once(ts) => {
                if *ts >= now {
                    Some(*ts)
                } else {
                    None
                }
            }
            Schedule::Interval { seconds } => {
                if *seconds == 0 {
                    return Some(now);
                }
                // Next interval boundary at or after now
                // Align to epoch: next = ceil(now / seconds) * seconds
                let next = ((now + seconds - 1) / seconds) * seconds;
                Some(next)
            }
            Schedule::Daily { hour, minute } => {
                let (year, month, day, _cur_h, _cur_m, _sec, _dow) = decompose_timestamp(now);
                // Today at the target time
                let target_today = compose_timestamp(year, month, day, *hour, *minute, 0);
                if target_today >= now {
                    Some(target_today)
                } else {
                    // Tomorrow
                    Some(target_today + SECS_PER_DAY)
                }
            }
            Schedule::Weekly {
                day: target_day,
                hour,
                minute,
            } => {
                let (year, month, day, _h, _m, _s, cur_dow) = decompose_timestamp(now);
                let target_dow = *target_day as u8;
                let today_target = compose_timestamp(year, month, day, *hour, *minute, 0);

                if cur_dow == target_dow && today_target >= now {
                    Some(today_target)
                } else {
                    let days_ahead = if target_dow > cur_dow {
                        (target_dow - cur_dow) as u64
                    } else if target_dow < cur_dow {
                        (7 - cur_dow + target_dow) as u64
                    } else {
                        7 // same day but time has passed
                    };
                    let base = compose_timestamp(year, month, day, *hour, *minute, 0);
                    Some(base + days_ahead * SECS_PER_DAY)
                }
            }
            Schedule::Monthly { day, hour, minute } => {
                let (mut year, mut month, _d, _h, _m, _s, _dow) = decompose_timestamp(now);
                // Try up to 24 months ahead (handles months where `day` doesn't exist)
                for _ in 0..24 {
                    let max_day = days_in_month(year, month);
                    if *day <= max_day {
                        let candidate = compose_timestamp(year, month, *day, *hour, *minute, 0);
                        if candidate >= now {
                            return Some(candidate);
                        }
                    }
                    // Advance to next month
                    month += 1;
                    if month > 12 {
                        month = 1;
                        year += 1;
                    }
                }
                None
            }
            Schedule::Cron(expr) => {
                let (year, month, day, hour, minute, _sec, _dow) = decompose_timestamp(now);
                expr.next_match(year, month, day, hour, minute, now)
            }
        }
    }
}

/// A scheduled task to run at specified times.
#[derive(Debug, Clone)]
pub struct ScheduledTask {
    /// Unique task identifier.
    pub id: u32,
    /// Human-readable name.
    pub name: String,
    /// Whether this task is enabled.
    pub enabled: bool,
    /// The command to execute (program path or shell command).
    pub command: String,
    /// Optional working directory for the command.
    pub working_dir: Option<String>,
    /// When to run.
    pub schedule: Schedule,
    /// Whether to run with elevated privileges.
    pub run_as_admin: bool,
    /// Unix timestamp when the task was created.
    pub created_at: u64,
    /// Unix timestamp of the last successful run.
    pub last_run: Option<u64>,
    /// Computed next run time.
    pub next_run: Option<u64>,
    /// How many times this task has been run.
    pub run_count: u32,
    /// Result of the last execution.
    pub last_result: Option<TaskResult>,
}

impl ScheduledTask {
    /// Create a new task with an auto-computed `next_run`.
    pub fn new(
        id: u32,
        name: String,
        command: String,
        schedule: Schedule,
        created_at: u64,
    ) -> Self {
        let next_run = schedule.next_occurrence(created_at);
        ScheduledTask {
            id,
            name,
            enabled: true,
            command,
            working_dir: None,
            schedule,
            run_as_admin: false,
            created_at,
            last_run: None,
            next_run,
            run_count: 0,
            last_result: None,
        }
    }

    /// Recompute `next_run` based on the current time.
    pub fn recompute_next_run(&mut self, now: u64) {
        self.next_run = self.schedule.next_occurrence(now);
    }

    /// Whether this task is due at or before `now`.
    pub fn is_due(&self, now: u64) -> bool {
        self.enabled && self.next_run.is_some_and(|nr| nr <= now)
    }
}

#[cfg(test)]
mod test_decompose {
    use super::*;

    #[test]
    fn epoch_is_thursday() {
        let (y, m, d, h, min, s, dow) = decompose_timestamp(0);
        assert_eq!((y, m, d, h, min, s), (1970, 1, 1, 0, 0, 0));
        assert_eq!(dow, 4); // Thursday
    }

    #[test]
    fn known_date() {
        // 2024-01-15 12:30:00 UTC = 1705322200 (Monday)
        // Manually: 2024-01-15 is a Monday
        let ts = compose_timestamp(2024, 1, 15, 12, 30, 0);
        let (y, m, d, h, min, s, dow) = decompose_timestamp(ts);
        assert_eq!((y, m, d, h, min, s), (2024, 1, 15, 12, 30, 0));
        assert_eq!(dow, 1); // Monday
    }

    #[test]
    fn roundtrip() {
        for ts in [0u64, 86400, 1_000_000_000, 1_700_000_000, 2_000_000_000] {
            let (y, m, d, h, min, s, _dow) = decompose_timestamp(ts);
            let recomposed = compose_timestamp(y, m, d, h, min, s);
            assert_eq!(ts, recomposed, "roundtrip failed for ts={ts}");
        }
    }

    #[test]
    fn leap_year() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
    }
}
