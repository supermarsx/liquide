use liquide_compositor::geometry::Rect;

use crate::screen_time::*;
use crate::shell::Shell;
use crate::window::WindowId;

// Constants matching screen_time.rs
const US_PER_SECOND: u64 = 1_000_000;
const US_PER_HOUR: u64 = 3_600 * US_PER_SECOND;
const US_PER_DAY: u64 = 24 * US_PER_HOUR;

/// Helper: create a tracker where mono=wall (tick=1us, anchor=0).
fn simple_tracker() -> ScreenTimeTracker {
    ScreenTimeTracker::new(0, 0)
}

// ========== Wall-clock & basics (6) ==========

#[test]
fn to_wall_clock_identity() {
    // wall_anchor=0, mono_anchor=0, tick=1 => wall = mono
    let t = simple_tracker();
    assert_eq!(t.to_wall_clock(0), 0);
    assert_eq!(t.to_wall_clock(42), 42);
    assert_eq!(t.to_wall_clock(1_000_000), 1_000_000);
}

#[test]
fn to_wall_clock_offset() {
    // wall_anchor=100, mono_anchor=10, tick=1 => wall = 100 + (mono - 10)
    let t = ScreenTimeTracker::new(100, 10);
    assert_eq!(t.to_wall_clock(10), 100);
    assert_eq!(t.to_wall_clock(20), 110);
    assert_eq!(t.to_wall_clock(110), 200);
}

#[test]
fn to_wall_clock_custom_tick_duration() {
    // tick_duration_us = 1000 => each mono tick = 1000us
    let t = ScreenTimeTracker::with_tick_duration(0, 0, 1000);
    assert_eq!(t.to_wall_clock(0), 0);
    assert_eq!(t.to_wall_clock(1), 1000);
    assert_eq!(t.to_wall_clock(100), 100_000);
}

#[test]
fn day_key_epoch_zero() {
    assert_eq!(ScreenTimeTracker::day_key(0), 0);
    assert_eq!(ScreenTimeTracker::day_key(US_PER_DAY - 1), 0);
    assert_eq!(ScreenTimeTracker::day_key(US_PER_DAY), 1);
}

#[test]
fn day_key_known_date() {
    // Day 19000 starts at 19000 * US_PER_DAY
    let wall = 19000u64 * US_PER_DAY + 42;
    assert_eq!(ScreenTimeTracker::day_key(wall), 19000);
}

#[test]
fn hour_of_day_boundaries() {
    assert_eq!(ScreenTimeTracker::hour_of_day(0), 0);
    assert_eq!(ScreenTimeTracker::hour_of_day(US_PER_HOUR - 1), 0);
    assert_eq!(ScreenTimeTracker::hour_of_day(US_PER_HOUR), 1);
    assert_eq!(ScreenTimeTracker::hour_of_day(12 * US_PER_HOUR), 12);
    assert_eq!(ScreenTimeTracker::hour_of_day(23 * US_PER_HOUR), 23);
    assert_eq!(ScreenTimeTracker::hour_of_day(US_PER_DAY - 1), 23);
}

// ========== Feed & aggregation (12) ==========

#[test]
fn feed_open_creates_daily_report() {
    let mut t = simple_tracker();
    t.feed_open("app", WindowId(1), 0);
    let dk = ScreenTimeTracker::day_key(0);
    assert!(t.daily_report(dk).is_some());
    assert_eq!(t.tracked_days(), 1);
}

#[test]
fn feed_open_increments_launch_count() {
    let mut t = simple_tracker();
    t.feed_open("app", WindowId(1), 100);
    t.feed_open("app", WindowId(2), 200);
    t.feed_open("other", WindowId(3), 300);
    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    assert_eq!(day.total_app_launches, 3);
    assert_eq!(day.apps.get("app").unwrap().launch_count, 2);
    assert_eq!(day.apps.get("other").unwrap().launch_count, 1);
}

#[test]
fn feed_close_increments_session_count() {
    let mut t = simple_tracker();
    t.feed_open("app", WindowId(1), 100);
    t.feed_close("app", WindowId(1), 200);
    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    assert_eq!(day.apps.get("app").unwrap().session_count, 1);
}

#[test]
fn feed_focus_unfocus_screen_time() {
    let mut t = simple_tracker();
    t.feed_open("app", WindowId(1), 0);
    t.feed_focus("app", WindowId(1), 100);
    t.feed_unfocus(200);
    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    assert_eq!(day.total_screen_time_us, 100);
    assert_eq!(day.apps.get("app").unwrap().screen_time_us, 100);
}

#[test]
fn feed_multiple_focus_sessions() {
    let mut t = simple_tracker();
    t.feed_open("app", WindowId(1), 0);
    // Session 1: 100us
    t.feed_focus("app", WindowId(1), 100);
    t.feed_unfocus(200);
    // Session 2: 50us
    t.feed_focus("app", WindowId(1), 300);
    t.feed_unfocus(350);
    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    assert_eq!(day.total_screen_time_us, 150);
    assert_eq!(day.apps.get("app").unwrap().screen_time_us, 150);
}

#[test]
fn feed_focus_updates_hourly_slot() {
    // Place focus in hour 2, day 0
    let start = 2 * US_PER_HOUR + 1000;
    let end = 2 * US_PER_HOUR + 5000;
    let mut t = simple_tracker();
    t.feed_focus("app", WindowId(1), start);
    t.feed_unfocus(end);
    let dk = ScreenTimeTracker::day_key(start);
    let day = t.daily_report(dk).unwrap();
    assert_eq!(day.hourly[2].screen_time_us, 4000);
}

#[test]
fn feed_focus_switches_count() {
    let mut t = simple_tracker();
    t.feed_focus("app1", WindowId(1), 100);
    t.feed_focus("app2", WindowId(2), 200);
    t.feed_focus("app1", WindowId(1), 300);
    t.feed_unfocus(400);
    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    // Each feed_focus increments focus_switches in the hourly slot
    let total_switches: u32 = day.hourly.iter().map(|h| h.focus_switches).sum();
    assert_eq!(total_switches, 3);
}

#[test]
fn feed_longest_session_tracked() {
    let mut t = simple_tracker();
    t.feed_open("app", WindowId(1), 0);
    // Session 1: 100us
    t.feed_focus("app", WindowId(1), 100);
    t.feed_unfocus(200);
    // Session 2: 500us (longer)
    t.feed_focus("app", WindowId(1), 300);
    t.feed_unfocus(800);
    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    assert_eq!(day.apps.get("app").unwrap().longest_session_us, 500);
}

#[test]
fn feed_first_last_used_tracked() {
    let mut t = simple_tracker();
    t.feed_open("app", WindowId(1), 1000);
    t.feed_focus("app", WindowId(1), 2000);
    t.feed_unfocus(3000);
    t.feed_close("app", WindowId(1), 5000);
    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    let app = day.apps.get("app").unwrap();
    assert_eq!(app.first_used_us, 1000);
    assert_eq!(app.last_used_us, 5000);
}

#[test]
fn feed_midnight_crossing_splits_days() {
    // Focus session that spans midnight: 1 hour before midnight to 1 hour after
    let day0_end = US_PER_DAY;
    let start = day0_end - US_PER_HOUR; // hour 23 of day 0
    let end = day0_end + US_PER_HOUR; // hour 1 of day 1
    let mut t = simple_tracker();
    t.feed_focus("app", WindowId(1), start);
    t.feed_unfocus(end);

    // Day 0 should have 1 hour of screen time
    let day0 = t.daily_report(0).unwrap();
    assert_eq!(day0.total_screen_time_us, US_PER_HOUR);

    // Day 1 should have 1 hour of screen time
    let day1 = t.daily_report(1).unwrap();
    assert_eq!(day1.total_screen_time_us, US_PER_HOUR);
}

#[test]
fn feed_midnight_crossing_splits_hours() {
    // Focus from hour 23:30 to hour 0:30 (next day)
    let day0_end = US_PER_DAY;
    let half_hour = US_PER_HOUR / 2;
    let start = day0_end - half_hour; // 23:30 day 0
    let end = day0_end + half_hour; // 00:30 day 1

    let mut t = simple_tracker();
    t.feed_focus("app", WindowId(1), start);
    t.feed_unfocus(end);

    let day0 = t.daily_report(0).unwrap();
    assert_eq!(day0.hourly[23].screen_time_us, half_hour);

    let day1 = t.daily_report(1).unwrap();
    assert_eq!(day1.hourly[0].screen_time_us, half_hour);
}

#[test]
fn feed_focus_auto_flushes_previous() {
    // feed_focus should flush the prior focus session automatically
    let mut t = simple_tracker();
    t.feed_focus("app1", WindowId(1), 100);
    // No explicit unfocus — feed_focus on different app should flush
    t.feed_focus("app2", WindowId(2), 300);
    t.feed_unfocus(500);

    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    // app1 had 200us of focus (100->300)
    assert_eq!(day.apps.get("app1").unwrap().screen_time_us, 200);
    // app2 had 200us of focus (300->500)
    assert_eq!(day.apps.get("app2").unwrap().screen_time_us, 200);
    assert_eq!(day.total_screen_time_us, 400);
}

// ========== Reports (8) ==========

#[test]
fn daily_report_total_screen_time() {
    let mut t = simple_tracker();
    // Two different apps focused
    t.feed_focus("app1", WindowId(1), 100);
    t.feed_unfocus(200);
    t.feed_focus("app2", WindowId(2), 300);
    t.feed_unfocus(500);

    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    assert_eq!(day.total_screen_time_us, 300); // 100 + 200
}

#[test]
fn daily_report_top_apps() {
    let mut t = simple_tracker();
    // app1: 100us focus
    t.feed_focus("app1", WindowId(1), 100);
    t.feed_unfocus(200);
    // app2: 300us focus
    t.feed_focus("app2", WindowId(2), 300);
    t.feed_unfocus(600);
    // app3: 50us focus
    t.feed_focus("app3", WindowId(3), 700);
    t.feed_unfocus(750);

    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    let top = day.top_apps(2);
    assert_eq!(top.len(), 2);
    assert_eq!(top[0].app_id, "app2");
    assert_eq!(top[0].screen_time_us, 300);
    assert_eq!(top[1].app_id, "app1");
    assert_eq!(top[1].screen_time_us, 100);
}

#[test]
fn daily_report_peak_hour() {
    let mut t = simple_tracker();
    // 100us in hour 0
    t.feed_focus("app", WindowId(1), 100);
    t.feed_unfocus(200);
    // 500us in hour 3
    let h3_start = 3 * US_PER_HOUR + 100;
    t.feed_focus("app", WindowId(1), h3_start);
    t.feed_unfocus(h3_start + 500);

    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    let (hour, time) = day.peak_hour().unwrap();
    assert_eq!(hour, 3);
    assert_eq!(time, 500);
}

#[test]
fn daily_report_empty_day() {
    let t = simple_tracker();
    assert!(t.daily_report(0).is_none());
    assert!(t.daily_report(99999).is_none());
}

#[test]
fn today_returns_current_day() {
    // Anchor at day 100 start
    let anchor = 100u64 * US_PER_DAY;
    let mut t = ScreenTimeTracker::new(anchor, 0);
    t.feed_open("app", WindowId(1), 1000);

    // now_mono=1000 => wall = anchor + 1000, which is still day 100
    let report = t.today(1000).unwrap();
    assert_eq!(report.day_key, 100);
}

#[test]
fn hourly_heatmap_24_slots() {
    let mut t = simple_tracker();
    // Feed an event — daily report should always have exactly 24 hourly slots
    t.feed_open("app", WindowId(1), 0);
    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    assert_eq!(day.hourly.len(), 24);
}

#[test]
fn app_screen_time_avg_session() {
    let mut t = simple_tracker();
    t.feed_open("app", WindowId(1), 0);
    // Focus 100us, then close
    t.feed_focus("app", WindowId(1), 10);
    t.feed_unfocus(110);
    t.feed_close("app", WindowId(1), 120);

    t.feed_open("app", WindowId(2), 200);
    // Focus 200us, then close
    t.feed_focus("app", WindowId(2), 210);
    t.feed_unfocus(410);
    t.feed_close("app", WindowId(2), 420);

    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    let app = day.apps.get("app").unwrap();
    assert_eq!(app.session_count, 2);
    // avg = total screen_time / session_count = 300 / 2 = 150
    assert_eq!(app.avg_session_us, 150);
}

#[test]
fn app_screen_time_multiple_windows() {
    let mut t = simple_tracker();
    // Window 1: 100us focus
    t.feed_open("app", WindowId(1), 0);
    t.feed_focus("app", WindowId(1), 100);
    t.feed_unfocus(200);
    // Window 2: 200us focus (same app)
    t.feed_open("app", WindowId(2), 250);
    t.feed_focus("app", WindowId(2), 300);
    t.feed_unfocus(500);

    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    let app = day.apps.get("app").unwrap();
    // Total from both windows
    assert_eq!(app.screen_time_us, 300);
    assert_eq!(app.launch_count, 2);
}

// ========== Categories (4) ==========

#[test]
fn set_category_and_breakdown() {
    let mut t = simple_tracker();
    t.set_category("browser", "Productivity");
    t.feed_open("browser", WindowId(1), 0);
    t.feed_focus("browser", WindowId(1), 100);
    t.feed_unfocus(500);

    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    let cats = day.category_breakdown(t.categories());
    let prod = cats.get("Productivity").unwrap();
    assert_eq!(prod.screen_time_us, 400);
    assert_eq!(prod.app_count, 1);
    assert_eq!(prod.launch_count, 1);
}

#[test]
fn category_multiple_apps() {
    let mut t = simple_tracker();
    t.set_category("browser", "Productivity");
    t.set_category("editor", "Productivity");
    t.set_category("game", "Entertainment");

    t.feed_open("browser", WindowId(1), 0);
    t.feed_focus("browser", WindowId(1), 100);
    t.feed_unfocus(200);
    t.feed_open("editor", WindowId(2), 200);
    t.feed_focus("editor", WindowId(2), 300);
    t.feed_unfocus(500);
    t.feed_open("game", WindowId(3), 500);
    t.feed_focus("game", WindowId(3), 600);
    t.feed_unfocus(900);

    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    let cats = day.category_breakdown(t.categories());

    let prod = cats.get("Productivity").unwrap();
    assert_eq!(prod.screen_time_us, 300); // 100 + 200
    assert_eq!(prod.app_count, 2);

    let ent = cats.get("Entertainment").unwrap();
    assert_eq!(ent.screen_time_us, 300);
    assert_eq!(ent.app_count, 1);
}

#[test]
fn category_uncategorized_apps() {
    let mut t = simple_tracker();
    t.set_category("browser", "Productivity");
    // "unknown_app" has no category
    t.feed_open("unknown_app", WindowId(1), 0);
    t.feed_focus("unknown_app", WindowId(1), 100);
    t.feed_unfocus(200);

    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    let cats = day.category_breakdown(t.categories());
    assert!(cats.contains_key("Uncategorized"));
    assert_eq!(cats.get("Uncategorized").unwrap().screen_time_us, 100);
}

#[test]
fn remove_category() {
    let mut t = simple_tracker();
    t.set_category("browser", "Productivity");
    assert!(t.categories().contains_key("browser"));
    t.remove_category("browser");
    assert!(!t.categories().contains_key("browser"));
}

// ========== Limits & alerts (5) ==========

#[test]
fn add_limit_check_not_exceeded() {
    let mut t = simple_tracker();
    t.add_limit(UsageLimit {
        target: LimitTarget::App("app".to_string()),
        daily_limit_us: 1000,
    });
    t.feed_open("app", WindowId(1), 0);
    t.feed_focus("app", WindowId(1), 100);
    t.feed_unfocus(200); // only 100us

    let dk = ScreenTimeTracker::day_key(0);
    let alerts = t.check_limits(dk);
    assert_eq!(alerts.len(), 1);
    assert!(!alerts[0].exceeded);
    assert_eq!(alerts[0].used_us, 100);
    assert_eq!(alerts[0].limit_us, 1000);
}

#[test]
fn add_limit_check_exceeded() {
    let mut t = simple_tracker();
    t.add_limit(UsageLimit {
        target: LimitTarget::App("app".to_string()),
        daily_limit_us: 100,
    });
    t.feed_open("app", WindowId(1), 0);
    t.feed_focus("app", WindowId(1), 100);
    t.feed_unfocus(300); // 200us > 100us limit

    let dk = ScreenTimeTracker::day_key(0);
    let alerts = t.check_limits(dk);
    assert_eq!(alerts.len(), 1);
    assert!(alerts[0].exceeded);
    assert_eq!(alerts[0].used_us, 200);
}

#[test]
fn limit_by_category() {
    let mut t = simple_tracker();
    t.set_category("browser", "Social");
    t.set_category("chat", "Social");
    t.add_limit(UsageLimit {
        target: LimitTarget::Category("Social".to_string()),
        daily_limit_us: 500,
    });

    t.feed_focus("browser", WindowId(1), 100);
    t.feed_unfocus(300); // 200us
    t.feed_focus("chat", WindowId(2), 400);
    t.feed_unfocus(800); // 400us

    let dk = ScreenTimeTracker::day_key(0);
    let alerts = t.check_limits(dk);
    assert_eq!(alerts.len(), 1);
    // 200 + 400 = 600 > 500
    assert!(alerts[0].exceeded);
    assert_eq!(alerts[0].used_us, 600);
}

#[test]
fn limit_all_apps() {
    let mut t = simple_tracker();
    t.add_limit(UsageLimit {
        target: LimitTarget::AllApps,
        daily_limit_us: 1000,
    });

    t.feed_focus("app1", WindowId(1), 100);
    t.feed_unfocus(400); // 300
    t.feed_focus("app2", WindowId(2), 500);
    t.feed_unfocus(600); // 100

    let dk = ScreenTimeTracker::day_key(0);
    let alerts = t.check_limits(dk);
    assert_eq!(alerts.len(), 1);
    assert!(!alerts[0].exceeded); // 400 < 1000
    assert_eq!(alerts[0].used_us, 400);
}

#[test]
fn limit_percent_used() {
    let mut t = simple_tracker();
    t.add_limit(UsageLimit {
        target: LimitTarget::App("app".to_string()),
        daily_limit_us: 200,
    });
    t.feed_focus("app", WindowId(1), 100);
    t.feed_unfocus(200); // 100us = 50% of 200

    let dk = ScreenTimeTracker::day_key(0);
    let alerts = t.check_limits(dk);
    assert!((alerts[0].percent_used - 50.0).abs() < 0.01);
}

// ========== Pickup detection (3) ==========

#[test]
fn pickup_after_idle_gap() {
    let mut t = simple_tracker();
    // Default idle threshold is 30s = 30_000_000 us
    let gap = 31 * US_PER_SECOND;
    t.feed_open("app", WindowId(1), 0);
    // After a long gap, feed_focus triggers pickup
    t.feed_focus("app", WindowId(1), gap);
    t.feed_unfocus(gap + 100);

    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    assert!(day.pickup_count >= 1);
}

#[test]
fn no_pickup_within_threshold() {
    let mut t = simple_tracker();
    // Activity at 0
    t.feed_open("app", WindowId(1), 0);
    // Activity within threshold (< 30s)
    t.feed_focus("app", WindowId(1), 10 * US_PER_SECOND);
    t.feed_unfocus(10 * US_PER_SECOND + 100);

    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    // Only the initial open might cause a pickup (since last_activity starts at anchor)
    // But feed_open at 0 has gap 0 from anchor 0, so no pickup there
    assert_eq!(day.pickup_count, 0);
}

#[test]
fn pickup_custom_threshold() {
    let mut t = simple_tracker();
    t.set_idle_threshold(5 * US_PER_SECOND); // 5s threshold

    t.feed_open("app", WindowId(1), 0);
    // Gap of 6s should trigger pickup
    t.feed_focus("app", WindowId(1), 6 * US_PER_SECOND);
    t.feed_unfocus(6 * US_PER_SECOND + 100);

    let dk = ScreenTimeTracker::day_key(0);
    let day = t.daily_report(dk).unwrap();
    assert!(day.pickup_count >= 1);
}

// ========== Comparison & weekly (4) ==========

#[test]
fn compare_days_basic() {
    let mut t = simple_tracker();
    // Day 0: 100us
    t.feed_focus("app", WindowId(1), 100);
    t.feed_unfocus(200);
    // Day 1: 300us
    let day1_start = US_PER_DAY + 100;
    t.feed_focus("app", WindowId(1), day1_start);
    t.feed_unfocus(day1_start + 300);

    let cmp = t.compare_days(0, 1).unwrap();
    assert_eq!(cmp.screen_time_a_us, 100);
    assert_eq!(cmp.screen_time_b_us, 300);
    assert_eq!(cmp.delta_us, 200);
}

#[test]
fn compare_days_percent_change() {
    let mut t = simple_tracker();
    // Day 0: 200us
    t.feed_focus("app", WindowId(1), 100);
    t.feed_unfocus(300);
    // Day 1: 400us (100% increase)
    let day1_start = US_PER_DAY + 100;
    t.feed_focus("app", WindowId(1), day1_start);
    t.feed_unfocus(day1_start + 400);

    let cmp = t.compare_days(0, 1).unwrap();
    assert!((cmp.percent_change - 100.0).abs() < 0.01);
}

#[test]
fn weekly_average_full_week() {
    let mut t = simple_tracker();
    // 7 days, each with 100us of screen time
    for d in 0..7u64 {
        let start = d * US_PER_DAY + 100;
        t.feed_focus("app", WindowId(1), start);
        t.feed_unfocus(start + 100);
    }

    let summary = t.weekly_average(6);
    assert_eq!(summary.days_tracked, 7);
    assert_eq!(summary.total_screen_time_us, 700);
    assert_eq!(summary.daily_average_us, 100);
}

#[test]
fn weekly_average_partial_data() {
    let mut t = simple_tracker();
    // Only days 4, 5, 6 have data
    for d in 4..7u64 {
        let start = d * US_PER_DAY + 100;
        t.feed_focus("app", WindowId(1), start);
        t.feed_unfocus(start + 300);
    }

    let summary = t.weekly_average(6);
    assert_eq!(summary.days_tracked, 3);
    assert_eq!(summary.total_screen_time_us, 900);
    assert_eq!(summary.daily_average_us, 300);
}

// ========== Edge cases (5) ==========

#[test]
fn no_events_empty_reports() {
    let t = simple_tracker();
    assert_eq!(t.tracked_days(), 0);
    assert!(t.daily_report(0).is_none());
    assert!(t.today(0).is_none());
    let summary = t.weekly_average(0);
    assert_eq!(summary.days_tracked, 0);
    assert_eq!(summary.total_screen_time_us, 0);
    assert!(t.compare_days(0, 1).is_none());
}

#[test]
fn feed_unfocus_without_focus() {
    let mut t = simple_tracker();
    // Should not panic
    t.feed_unfocus(100);
    assert_eq!(t.tracked_days(), 0);
}

#[test]
fn max_days_retained_eviction() {
    let mut t = simple_tracker();
    // Feed events across 95 days (max is 90)
    for d in 0..95u64 {
        let wall = d * US_PER_DAY + 100;
        t.feed_open("app", WindowId(d as u16 as u64), wall);
    }
    assert!(t.tracked_days() <= 91);
}

#[test]
fn empty_app_id_ignored() {
    let mut t = simple_tracker();
    t.feed_open("", WindowId(1), 0);
    t.feed_close("", WindowId(1), 100);
    t.feed_focus("", WindowId(1), 200);
    // None of these should create any data
    assert_eq!(t.tracked_days(), 0);
}

#[test]
fn zero_tick_duration_handled() {
    // with_tick_duration clamps to 1
    let t = ScreenTimeTracker::with_tick_duration(100, 0, 0);
    // Should not panic; tick clamped to 1
    assert_eq!(t.to_wall_clock(10), 110);
}

// ========== Serde & display (3) ==========

#[test]
fn serde_roundtrip_daily_report() {
    let mut report = DailyReport::new(42);
    report.total_screen_time_us = 12345;
    report.total_app_launches = 5;
    report.pickup_count = 2;

    let json = serde_json::to_string(&report).unwrap();
    let back: DailyReport = serde_json::from_str(&json).unwrap();
    assert_eq!(report.day_key, back.day_key);
    assert_eq!(report.total_screen_time_us, back.total_screen_time_us);
    assert_eq!(report.total_app_launches, back.total_app_launches);
    assert_eq!(report.pickup_count, back.pickup_count);
}

#[test]
fn serde_roundtrip_screen_time_alert() {
    let alert = ScreenTimeAlert {
        target: LimitTarget::App("browser".to_string()),
        limit_us: 3600_000_000,
        used_us: 4000_000_000,
        exceeded: true,
        percent_used: 111.1,
    };
    let json = serde_json::to_string(&alert).unwrap();
    let back: ScreenTimeAlert = serde_json::from_str(&json).unwrap();
    assert_eq!(alert, back);
}

#[test]
fn display_impls_all_types() {
    // HourlySlot
    let hs = HourlySlot::default();
    let s = format!("{hs}");
    assert!(s.contains("HourlySlot"));

    // AppScreenTime
    let ast = AppScreenTime {
        app_id: "test".to_string(),
        screen_time_us: 100,
        background_time_us: 0,
        launch_count: 1,
        session_count: 1,
        avg_session_us: 100,
        longest_session_us: 100,
        first_used_us: 0,
        last_used_us: 100,
    };
    assert!(format!("{ast}").contains("AppScreenTime"));

    // CategoryScreenTime
    let cst = CategoryScreenTime {
        category: "Work".to_string(),
        screen_time_us: 500,
        app_count: 2,
        launch_count: 3,
    };
    assert!(format!("{cst}").contains("CategoryScreenTime"));

    // LimitTarget
    assert!(format!("{}", LimitTarget::App("x".to_string())).contains("App"));
    assert!(format!("{}", LimitTarget::Category("y".to_string())).contains("Category"));
    assert!(format!("{}", LimitTarget::AllApps).contains("AllApps"));

    // UsageLimit
    let ul = UsageLimit {
        target: LimitTarget::AllApps,
        daily_limit_us: 1000,
    };
    assert!(format!("{ul}").contains("UsageLimit"));

    // ScreenTimeAlert
    let alert = ScreenTimeAlert {
        target: LimitTarget::AllApps,
        limit_us: 1000,
        used_us: 500,
        exceeded: false,
        percent_used: 50.0,
    };
    assert!(format!("{alert}").contains("Alert"));

    // DailyComparison
    let dc = DailyComparison {
        day_a: 1,
        day_b: 2,
        screen_time_a_us: 100,
        screen_time_b_us: 200,
        delta_us: 100,
        percent_change: 100.0,
        launches_a: 5,
        launches_b: 10,
        pickups_a: 1,
        pickups_b: 2,
    };
    assert!(format!("{dc}").contains("DailyComparison"));

    // WeeklySummary
    let ws = WeeklySummary {
        days_tracked: 7,
        total_screen_time_us: 7000,
        daily_average_us: 1000,
        most_used_app: Some(("app".to_string(), 5000)),
        total_pickups: 14,
        total_launches: 35,
    };
    assert!(format!("{ws}").contains("WeeklySummary"));

    // DailyReport
    let dr = DailyReport::new(42);
    assert!(format!("{dr}").contains("DailyReport"));

    // ScreenTimeTracker
    let t = simple_tracker();
    assert!(format!("{t}").contains("ScreenTimeTracker"));
}

// ========== Shell integration (3) ==========

#[test]
fn shell_screen_time_accessor() {
    let shell = Shell::new(1920.0, 1080.0);
    let st = shell.screen_time();
    assert_eq!(st.tracked_days(), 0);
}

#[test]
fn shell_focus_feeds_screen_time() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window_with_app("App", Rect::new(0.0, 0.0, 800.0, 600.0), "com.app");
    shell.set_focus(id).unwrap();
    // The screen time tracker should have been fed the focus event
    // It should have created at least one daily report
    assert!(shell.screen_time().tracked_days() >= 1);
}

#[test]
fn shell_open_close_feeds_screen_time() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window_with_app("App", Rect::new(0.0, 0.0, 800.0, 600.0), "com.app");
    // Open should have fed a launch event
    let days_after_open = shell.screen_time().tracked_days();
    assert!(days_after_open >= 1);

    shell.close_window(id).unwrap();
    // Close should have fed a close event (same day)
    assert!(shell.screen_time().tracked_days() >= 1);
}

// ========== Limit management (2) ==========

#[test]
fn remove_limit_by_index() {
    let mut t = simple_tracker();
    t.add_limit(UsageLimit {
        target: LimitTarget::AllApps,
        daily_limit_us: 1000,
    });
    t.add_limit(UsageLimit {
        target: LimitTarget::App("app".to_string()),
        daily_limit_us: 500,
    });
    assert_eq!(t.limits().len(), 2);
    t.remove_limit(0);
    assert_eq!(t.limits().len(), 1);
    assert_eq!(t.limits()[0].daily_limit_us, 500);
}

#[test]
fn remove_limit_out_of_bounds() {
    let mut t = simple_tracker();
    t.add_limit(UsageLimit {
        target: LimitTarget::AllApps,
        daily_limit_us: 1000,
    });
    t.remove_limit(99); // should not panic
    assert_eq!(t.limits().len(), 1);
}

// ========== Weekly summary most_used_app (1) ==========

#[test]
fn weekly_summary_most_used_app() {
    let mut t = simple_tracker();
    // Day 0: app1 200us, app2 100us
    t.feed_focus("app1", WindowId(1), 100);
    t.feed_unfocus(300);
    t.feed_focus("app2", WindowId(2), 400);
    t.feed_unfocus(500);
    // Day 1: app1 100us
    let d1 = US_PER_DAY + 100;
    t.feed_focus("app1", WindowId(1), d1);
    t.feed_unfocus(d1 + 100);

    let summary = t.weekly_average(1);
    let (app, time) = summary.most_used_app.unwrap();
    assert_eq!(app, "app1");
    assert_eq!(time, 300); // 200 + 100
}

// ========== Check limits no data (1) ==========

#[test]
fn check_limits_no_data_returns_empty() {
    let mut t = simple_tracker();
    t.add_limit(UsageLimit {
        target: LimitTarget::AllApps,
        daily_limit_us: 1000,
    });
    // No events => no day => empty alerts
    let alerts = t.check_limits(0);
    assert!(alerts.is_empty());
}
