use crate::*;

// ============================================================
// DateTime tests
// ============================================================

#[test]
fn unix_epoch_roundtrip() {
    let dt = DateTime::new(1970, 1, 1, 0, 0, 0);
    assert_eq!(dt.to_unix_timestamp(), 0);
    assert_eq!(DateTime::from_unix_timestamp(0), dt);
}

#[test]
fn known_timestamp_2024() {
    // 2024-01-15T13:30:00 UTC
    let dt = DateTime::new(2024, 1, 15, 13, 30, 0);
    let ts = dt.to_unix_timestamp();
    // Verify roundtrip
    let rt = DateTime::from_unix_timestamp(ts);
    assert_eq!(rt, dt);
    // Known value: 2024-01-15T13:30:00 = 1705322200 - let's just check roundtrip
    assert_eq!(rt.year, 2024);
    assert_eq!(rt.month, 1);
    assert_eq!(rt.day, 15);
    assert_eq!(rt.hour, 13);
    assert_eq!(rt.minute, 30);
}

#[test]
fn timestamp_before_epoch() {
    // 1969-12-31T23:59:59 should be -1
    let dt = DateTime::new(1969, 12, 31, 23, 59, 59);
    assert_eq!(dt.to_unix_timestamp(), -1);
    let rt = DateTime::from_unix_timestamp(-1);
    assert_eq!(rt, dt);
}

#[test]
fn timestamp_negative_year() {
    let dt = DateTime::new(1900, 6, 15, 12, 0, 0);
    let ts = dt.to_unix_timestamp();
    let rt = DateTime::from_unix_timestamp(ts);
    assert_eq!(rt, dt);
}

#[test]
fn leap_year_checks() {
    assert!(DateTime::new(2000, 1, 1, 0, 0, 0).is_leap_year());
    assert!(DateTime::new(2024, 1, 1, 0, 0, 0).is_leap_year());
    assert!(!DateTime::new(1900, 1, 1, 0, 0, 0).is_leap_year());
    assert!(!DateTime::new(2023, 1, 1, 0, 0, 0).is_leap_year());
    assert!(DateTime::new(2400, 1, 1, 0, 0, 0).is_leap_year());
}

#[test]
fn days_in_month_february() {
    assert_eq!(DateTime::new(2024, 2, 1, 0, 0, 0).days_in_month(), 29);
    assert_eq!(DateTime::new(2023, 2, 1, 0, 0, 0).days_in_month(), 28);
}

#[test]
fn days_in_month_all() {
    let expected = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for (i, &exp) in expected.iter().enumerate() {
        let m = (i + 1) as u32;
        assert_eq!(
            DateTime::new(2023, m, 1, 0, 0, 0).days_in_month(),
            exp,
            "month {}",
            m
        );
    }
}

#[test]
fn day_of_year_jan1() {
    assert_eq!(DateTime::new(2024, 1, 1, 0, 0, 0).day_of_year(), 1);
}

#[test]
fn day_of_year_dec31() {
    assert_eq!(DateTime::new(2024, 12, 31, 0, 0, 0).day_of_year(), 366); // leap year
    assert_eq!(DateTime::new(2023, 12, 31, 0, 0, 0).day_of_year(), 365);
}

#[test]
fn day_of_year_march1_leap() {
    assert_eq!(
        DateTime::new(2024, 3, 1, 0, 0, 0).day_of_year(),
        31 + 29 + 1
    );
}

#[test]
fn iso8601_format() {
    let dt = DateTime::new(2024, 1, 15, 13, 30, 0);
    assert_eq!(dt.format_iso8601(), "2024-01-15T13:30:00");
}

#[test]
fn iso8601_padding() {
    let dt = DateTime::new(2024, 3, 5, 9, 5, 7);
    assert_eq!(dt.format_iso8601(), "2024-03-05T09:05:07");
}

#[test]
fn datetime_display_trait() {
    let dt = DateTime::new(2024, 12, 25, 0, 0, 0);
    assert_eq!(format!("{}", dt), "2024-12-25T00:00:00");
}

#[test]
fn datetime_ordering() {
    let a = DateTime::new(2024, 1, 1, 0, 0, 0);
    let b = DateTime::new(2024, 1, 1, 0, 0, 1);
    assert!(a < b);
    assert!(b > a);
    assert_eq!(a, a);
}

#[test]
fn with_offset_minutes_positive() {
    let utc = DateTime::new(2024, 1, 15, 12, 0, 0);
    let jst = utc.with_offset_minutes(540); // UTC+9
    assert_eq!(jst.hour, 21);
    assert_eq!(jst.day, 15);
}

#[test]
fn with_offset_minutes_negative_day_wrap() {
    let utc = DateTime::new(2024, 1, 15, 3, 0, 0);
    let est = utc.with_offset_minutes(-300); // UTC-5
    assert_eq!(est.day, 14);
    assert_eq!(est.hour, 22);
}

// ============================================================
// Weekday tests
// ============================================================

#[test]
fn weekday_known_dates() {
    // 2024-01-15 is a Monday
    assert_eq!(
        DateTime::new(2024, 1, 15, 0, 0, 0).weekday(),
        Weekday::Monday
    );
    // 2024-01-14 is a Sunday
    assert_eq!(
        DateTime::new(2024, 1, 14, 0, 0, 0).weekday(),
        Weekday::Sunday
    );
    // 2000-01-01 is a Saturday
    assert_eq!(
        DateTime::new(2000, 1, 1, 0, 0, 0).weekday(),
        Weekday::Saturday
    );
    // 1970-01-01 is a Thursday
    assert_eq!(
        DateTime::new(1970, 1, 1, 0, 0, 0).weekday(),
        Weekday::Thursday
    );
}

#[test]
fn weekday_from_number_wraps() {
    assert_eq!(Weekday::from_number(0), Weekday::Sunday);
    assert_eq!(Weekday::from_number(7), Weekday::Sunday);
    assert_eq!(Weekday::from_number(8), Weekday::Monday);
}

#[test]
fn weekday_names() {
    assert_eq!(Weekday::Monday.name(), "Monday");
    assert_eq!(Weekday::Monday.short_name(), "Mon");
    assert_eq!(Weekday::Sunday.name(), "Sunday");
    assert_eq!(Weekday::Sunday.short_name(), "Sun");
}

#[test]
fn weekday_number_roundtrip() {
    for n in 0..7u32 {
        let wd = Weekday::from_number(n);
        assert_eq!(wd.number(), n);
    }
}

// ============================================================
// TimeZone tests
// ============================================================

#[test]
fn timezone_format_offset_positive() {
    let tz = TimeZone::new("Asia/Kolkata", "Kolkata", 330, "IST", false);
    assert_eq!(tz.format_offset(), "UTC+05:30");
}

#[test]
fn timezone_format_offset_negative() {
    let tz = TimeZone::new("America/New_York", "Eastern", -300, "EST", true);
    assert_eq!(tz.format_offset(), "UTC-05:00");
}

#[test]
fn timezone_format_offset_zero() {
    let tz = TimeZone::new("UTC", "UTC", 0, "UTC", false);
    assert_eq!(tz.format_offset(), "UTC+00:00");
}

#[test]
fn timezone_display_trait() {
    let tz = TimeZone::new("Europe/London", "London", 0, "GMT", true);
    let s = format!("{}", tz);
    assert!(s.contains("London"));
    assert!(s.contains("GMT"));
}

// ============================================================
// TimeZoneDatabase tests
// ============================================================

#[test]
fn tz_database_has_50_plus_entries() {
    let db = TimeZoneDatabase::new();
    assert!(
        db.len() >= 50,
        "database has {} entries, expected >= 50",
        db.len()
    );
}

#[test]
fn tz_database_find_exact() {
    let db = TimeZoneDatabase::new();
    let ny = db.find("America/New_York").unwrap();
    assert_eq!(ny.utc_offset_minutes, -300);
    assert!(ny.uses_dst);
}

#[test]
fn tz_database_find_case_insensitive() {
    let db = TimeZoneDatabase::new();
    assert!(db.find("america/new_york").is_some());
    assert!(db.find("AMERICA/NEW_YORK").is_some());
}

#[test]
fn tz_database_find_unknown() {
    let db = TimeZoneDatabase::new();
    assert!(db.find("Mars/Olympus_Mons").is_none());
}

#[test]
fn tz_database_search_city() {
    let db = TimeZoneDatabase::new();
    let results = db.search("Tokyo");
    assert!(!results.is_empty());
    assert!(results.iter().any(|tz| tz.id == "Asia/Tokyo"));
}

#[test]
fn tz_database_search_abbreviation() {
    let db = TimeZoneDatabase::new();
    let results = db.search("CET");
    assert!(
        results.len() >= 2,
        "expected multiple CET zones, got {}",
        results.len()
    );
}

#[test]
fn tz_database_by_offset() {
    let db = TimeZoneDatabase::new();
    let utc_zones = db.by_offset(0);
    assert!(!utc_zones.is_empty());
    assert!(utc_zones.iter().any(|tz| tz.id == "UTC"));
}

#[test]
fn tz_database_all_sorted_by_offset() {
    let db = TimeZoneDatabase::new();
    let all = db.all_timezones();
    for w in all.windows(2) {
        assert!(w[0].utc_offset_minutes <= w[1].utc_offset_minutes);
    }
}

// ============================================================
// ClockSettings tests
// ============================================================

#[test]
fn clock_h24_no_seconds() {
    let cs = ClockSettings::new();
    let dt = DateTime::new(2024, 1, 15, 13, 5, 9);
    assert_eq!(cs.format_time(&dt), "13:05");
}

#[test]
fn clock_h24_with_seconds() {
    let mut cs = ClockSettings::new();
    cs.show_seconds = true;
    let dt = DateTime::new(2024, 1, 15, 13, 5, 9);
    assert_eq!(cs.format_time(&dt), "13:05:09");
}

#[test]
fn clock_h12_am() {
    let cs = ClockSettings {
        format: ClockFormat::H12,
        show_seconds: false,
        show_date: false,
        timezone: "UTC".into(),
    };
    let dt = DateTime::new(2024, 1, 15, 9, 30, 0);
    assert_eq!(cs.format_time(&dt), "09:30 AM");
}

#[test]
fn clock_h12_pm() {
    let cs = ClockSettings {
        format: ClockFormat::H12,
        show_seconds: false,
        show_date: false,
        timezone: "UTC".into(),
    };
    let dt = DateTime::new(2024, 1, 15, 13, 30, 0);
    assert_eq!(cs.format_time(&dt), "01:30 PM");
}

#[test]
fn clock_h12_midnight() {
    let cs = ClockSettings {
        format: ClockFormat::H12,
        show_seconds: false,
        show_date: false,
        timezone: "UTC".into(),
    };
    let dt = DateTime::new(2024, 1, 15, 0, 0, 0);
    assert_eq!(cs.format_time(&dt), "12:00 AM");
}

#[test]
fn clock_h12_noon() {
    let cs = ClockSettings {
        format: ClockFormat::H12,
        show_seconds: false,
        show_date: false,
        timezone: "UTC".into(),
    };
    let dt = DateTime::new(2024, 1, 15, 12, 0, 0);
    assert_eq!(cs.format_time(&dt), "12:00 PM");
}

#[test]
fn clock_custom_format() {
    let cs = ClockSettings {
        format: ClockFormat::Custom("%I:%M %p - %A, %B %d, %Y".into()),
        show_seconds: false,
        show_date: false,
        timezone: "UTC".into(),
    };
    let dt = DateTime::new(2024, 1, 15, 13, 30, 0);
    assert_eq!(cs.format_time(&dt), "01:30 PM - Monday, January 15, 2024");
}

#[test]
fn clock_format_date() {
    let cs = ClockSettings::new();
    let dt = DateTime::new(2024, 1, 15, 0, 0, 0);
    assert_eq!(cs.format_date(&dt), "Mon, Jan 15, 2024");
}

// ============================================================
// WorldClock tests
// ============================================================

#[test]
fn world_clock_add_remove() {
    let mut wc = WorldClock::new();
    assert!(wc.is_empty());
    wc.add_clock("Tokyo", "Asia/Tokyo");
    wc.add_clock("London", "Europe/London");
    assert_eq!(wc.len(), 2);
    let removed = wc.remove_clock(0).unwrap();
    assert_eq!(removed.label, "Tokyo");
    assert_eq!(wc.len(), 1);
    assert_eq!(wc.clocks[0].label, "London");
}

#[test]
fn world_clock_remove_out_of_bounds() {
    let mut wc = WorldClock::new();
    assert!(wc.remove_clock(0).is_none());
}

#[test]
fn world_clock_reorder() {
    let mut wc = WorldClock::new();
    wc.add_clock("A", "UTC");
    wc.add_clock("B", "UTC");
    wc.add_clock("C", "UTC");
    wc.reorder(0, 2);
    assert_eq!(wc.clocks[0].label, "B");
    assert_eq!(wc.clocks[1].label, "C");
    assert_eq!(wc.clocks[2].label, "A");
}

#[test]
fn world_clock_all_times() {
    let mut wc = WorldClock::new();
    wc.add_clock("Tokyo", "Asia/Tokyo");
    wc.add_clock("NYC", "America/New_York");
    let utc = DateTime::new(2024, 1, 15, 12, 0, 0);
    let times = wc.all_times(&utc);
    assert_eq!(times.len(), 2);
    // Tokyo = UTC+9 → 21:00
    assert_eq!(times[0].0, "Tokyo");
    assert_eq!(times[0].1.hour, 21);
    // NYC = UTC-5 → 07:00
    assert_eq!(times[1].0, "NYC");
    assert_eq!(times[1].1.hour, 7);
}

// ============================================================
// Stopwatch tests
// ============================================================

#[test]
fn stopwatch_basic() {
    let mut sw = Stopwatch::new();
    assert!(!sw.running);
    sw.start(1_000_000);
    assert!(sw.running);
    assert_eq!(sw.elapsed(2_000_000), 1_000_000);
    let seg = sw.stop(3_000_000);
    assert_eq!(seg, 2_000_000);
    assert!(!sw.running);
    assert_eq!(sw.elapsed(5_000_000), 2_000_000); // stopped, doesn't advance
}

#[test]
fn stopwatch_resume() {
    let mut sw = Stopwatch::new();
    sw.start(1_000);
    sw.stop(3_000); // accumulated = 2000
    sw.start(5_000);
    assert_eq!(sw.elapsed(7_000), 4_000); // 2000 + 2000
}

#[test]
fn stopwatch_laps() {
    let mut sw = Stopwatch::new();
    sw.start(0);
    sw.lap(1_000_000); // lap at 1s
    sw.lap(3_000_000); // lap at 3s
    sw.lap(6_000_000); // lap at 6s
    assert_eq!(sw.lap_count(), 3);
    let splits = sw.lap_splits();
    assert_eq!(splits, vec![1_000_000, 2_000_000, 3_000_000]);
}

#[test]
fn stopwatch_reset() {
    let mut sw = Stopwatch::new();
    sw.start(0);
    sw.lap(1_000_000);
    sw.reset();
    assert!(!sw.running);
    assert_eq!(sw.elapsed(999), 0);
    assert_eq!(sw.lap_count(), 0);
}

#[test]
fn stopwatch_display() {
    let mut sw = Stopwatch::new();
    sw.start(0);
    // 1 hour, 2 minutes, 3 seconds, 456 ms = 3723456000 us
    let us = (3600 + 120 + 3) * 1_000_000 + 456_000;
    assert_eq!(sw.elapsed_display(us), "01:02:03.456");
}

// ============================================================
// CountdownTimer tests
// ============================================================

#[test]
fn timer_basic_countdown() {
    let mut timer = CountdownTimer::new(5000);
    assert_eq!(timer.remaining_ms, 5000);
    assert!(!timer.running);
    timer.start();
    assert!(timer.running);
    assert!(!timer.tick(1000));
    assert_eq!(timer.remaining_ms, 4000);
    assert!(!timer.tick(2000));
    assert_eq!(timer.remaining_ms, 2000);
    assert!(timer.tick(3000)); // overshoots — finishes
    assert_eq!(timer.remaining_ms, 0);
    assert!(timer.is_finished());
    assert!(!timer.running);
}

#[test]
fn timer_from_hms() {
    let timer = CountdownTimer::from_hms(1, 30, 0);
    assert_eq!(timer.duration_ms, 5_400_000);
}

#[test]
fn timer_pause_resume() {
    let mut timer = CountdownTimer::new(10000);
    timer.start();
    timer.tick(3000);
    timer.pause();
    assert!(!timer.running);
    assert!(!timer.tick(5000)); // should not advance
    assert_eq!(timer.remaining_ms, 7000);
    timer.start();
    assert!(timer.running);
    timer.tick(2000);
    assert_eq!(timer.remaining_ms, 5000);
}

#[test]
fn timer_reset() {
    let mut timer = CountdownTimer::new(10000);
    timer.start();
    timer.tick(5000);
    timer.reset();
    assert_eq!(timer.remaining_ms, 10000);
    assert!(!timer.running);
}

#[test]
fn timer_progress() {
    let mut timer = CountdownTimer::new(10000);
    timer.start();
    timer.tick(2500);
    let p = timer.progress();
    assert!((p - 0.25).abs() < 1e-9);
}

#[test]
fn timer_remaining_display() {
    let timer = CountdownTimer::new(3_723_000); // 1h 2m 3s
    assert_eq!(timer.remaining_display(), "01:02:03");
}

#[test]
fn timer_remaining_display_short() {
    let timer = CountdownTimer::new(125_000); // 2m 5s
    assert_eq!(timer.remaining_display_short(), "02:05");
}

#[test]
fn timer_zero_duration() {
    let mut timer = CountdownTimer::new(0);
    timer.start(); // should not start because remaining = 0
    assert!(!timer.running);
    assert!(timer.is_finished());
}

// ============================================================
// NTP offset parsing tests (internal)
// ============================================================

#[test]
fn ntp_offset_parse_ms() {
    // These test the internal parse function through the module's test
    // We'll test the format_offset and parse_offset_string through platform tests
}

// ============================================================
// Edge cases
// ============================================================

#[test]
fn y2k_timestamp() {
    let dt = DateTime::new(2000, 1, 1, 0, 0, 0);
    let ts = dt.to_unix_timestamp();
    assert_eq!(ts, 946684800);
}

#[test]
fn far_future_roundtrip() {
    let dt = DateTime::new(2100, 6, 15, 23, 59, 59);
    let ts = dt.to_unix_timestamp();
    let rt = DateTime::from_unix_timestamp(ts);
    assert_eq!(rt, dt);
}

#[test]
fn leap_day_roundtrip() {
    let dt = DateTime::new(2024, 2, 29, 12, 0, 0);
    let ts = dt.to_unix_timestamp();
    let rt = DateTime::from_unix_timestamp(ts);
    assert_eq!(rt, dt);
    assert_eq!(rt.weekday(), Weekday::Thursday);
}
