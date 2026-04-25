mod clock_settings;
mod datetime;
mod error;
mod ntp;
mod platform;
mod stopwatch;
mod timer;
mod timezone;
mod timezone_db;
mod weekday;
mod world_clock;

pub use clock_settings::{ClockFormat, ClockSettings};
pub use datetime::DateTime;
pub use error::TimeError;
pub use ntp::NtpSync;
pub use platform::PlatformTimeBridge;
pub use stopwatch::Stopwatch;
pub use timer::CountdownTimer;
pub use timezone::TimeZone;
pub use timezone_db::TimeZoneDatabase;
pub use weekday::Weekday;
pub use world_clock::{WorldClock, WorldClockEntry};

/// Convenience: current wall-clock time in the local timezone.
///
/// Equivalent to [`DateTime::now_local`], re-exported at crate root for
/// quick access from callers like the desktop status bar.
pub fn local_now() -> DateTime {
    DateTime::now_local()
}

/// Convenience: current wall-clock time in UTC.
pub fn utc_now() -> DateTime {
    DateTime::now_utc()
}

#[cfg(test)]
mod tests;
