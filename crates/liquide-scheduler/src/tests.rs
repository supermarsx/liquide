use crate::cron::{CronExpr, CronField, ParseError};
use crate::platform::PlatformBridge;
use crate::scheduler::Scheduler;
use crate::task::{Schedule, ScheduledTask, Weekday};

// -----------------------------------------------------------------------
// CronField tests
// -----------------------------------------------------------------------

#[test]
fn cron_field_any_matches_everything() {
    let f = CronField::Any;
    for v in 0..60 {
        assert!(f.matches(v, 0));
    }
}

#[test]
fn cron_field_value_matches_exact() {
    let f = CronField::Value(30);
    assert!(f.matches(30, 0));
    assert!(!f.matches(29, 0));
    assert!(!f.matches(31, 0));
}

#[test]
fn cron_field_range_matches_inclusive() {
    let f = CronField::Range(10, 20);
    assert!(!f.matches(9, 0));
    assert!(f.matches(10, 0));
    assert!(f.matches(15, 0));
    assert!(f.matches(20, 0));
    assert!(!f.matches(21, 0));
}

#[test]
fn cron_field_step_from_min() {
    let f = CronField::Step(15);
    // min=0: matches 0, 15, 30, 45
    assert!(f.matches(0, 0));
    assert!(f.matches(15, 0));
    assert!(f.matches(30, 0));
    assert!(f.matches(45, 0));
    assert!(!f.matches(1, 0));
    assert!(!f.matches(14, 0));
}

#[test]
fn cron_field_list_matches_members() {
    let f = CronField::List(vec![1, 5, 10, 25]);
    assert!(f.matches(1, 0));
    assert!(f.matches(5, 0));
    assert!(f.matches(10, 0));
    assert!(f.matches(25, 0));
    assert!(!f.matches(2, 0));
    assert!(!f.matches(0, 0));
}

// -----------------------------------------------------------------------
// CronExpr::parse tests
// -----------------------------------------------------------------------

#[test]
fn parse_every_five_minutes() {
    let expr = CronExpr::parse("*/5 * * * *").unwrap();
    assert_eq!(expr.minute, CronField::Step(5));
    assert_eq!(expr.hour, CronField::Any);
    assert_eq!(expr.day_of_month, CronField::Any);
    assert_eq!(expr.month, CronField::Any);
    assert_eq!(expr.day_of_week, CronField::Any);
}

#[test]
fn parse_specific_time() {
    let expr = CronExpr::parse("30 14 * * *").unwrap();
    assert_eq!(expr.minute, CronField::Value(30));
    assert_eq!(expr.hour, CronField::Value(14));
}

#[test]
fn parse_weekday_range() {
    let expr = CronExpr::parse("0 9 * * 1-5").unwrap();
    assert_eq!(expr.day_of_week, CronField::Range(1, 5));
}

#[test]
fn parse_list_field() {
    let expr = CronExpr::parse("0,15,30,45 * * * *").unwrap();
    assert_eq!(expr.minute, CronField::List(vec![0, 15, 30, 45]));
}

#[test]
fn parse_complex_expr() {
    let expr = CronExpr::parse("*/10 9-17 1 1,6 *").unwrap();
    assert_eq!(expr.minute, CronField::Step(10));
    assert_eq!(expr.hour, CronField::Range(9, 17));
    assert_eq!(expr.day_of_month, CronField::Value(1));
    assert_eq!(expr.month, CronField::List(vec![1, 6]));
    assert_eq!(expr.day_of_week, CronField::Any);
}

#[test]
fn parse_wrong_field_count() {
    let err = CronExpr::parse("* * *").unwrap_err();
    assert_eq!(err, ParseError::WrongFieldCount { found: 3 });
}

#[test]
fn parse_out_of_range() {
    let err = CronExpr::parse("60 * * * *").unwrap_err();
    assert!(matches!(err, ParseError::OutOfRange { field: "minute", value: 60, .. }));
}

#[test]
fn parse_zero_step() {
    let err = CronExpr::parse("*/0 * * * *").unwrap_err();
    assert!(matches!(err, ParseError::ZeroStep { field: "minute" }));
}

#[test]
fn parse_invalid_range() {
    let err = CronExpr::parse("* 20-10 * * *").unwrap_err();
    assert!(matches!(err, ParseError::InvalidRange { field: "hour", start: 20, end: 10 }));
}

#[test]
fn parse_invalid_token() {
    let err = CronExpr::parse("abc * * * *").unwrap_err();
    assert!(matches!(err, ParseError::InvalidToken { field: "minute", .. }));
}

// -----------------------------------------------------------------------
// CronExpr::matches tests
// -----------------------------------------------------------------------

#[test]
fn cron_matches_every_minute() {
    let expr = CronExpr::parse("* * * * *").unwrap();
    assert!(expr.matches(0, 0, 1, 1, 0));
    assert!(expr.matches(59, 23, 31, 12, 6));
}

#[test]
fn cron_matches_specific() {
    let expr = CronExpr::parse("30 14 15 6 3").unwrap();
    assert!(expr.matches(30, 14, 15, 6, 3));
    assert!(!expr.matches(31, 14, 15, 6, 3));
    assert!(!expr.matches(30, 15, 15, 6, 3));
}

// -----------------------------------------------------------------------
// Schedule::next_occurrence tests
// -----------------------------------------------------------------------

#[test]
fn once_in_future() {
    let s = Schedule::Once(1000);
    assert_eq!(s.next_occurrence(500), Some(1000));
}

#[test]
fn once_in_past() {
    let s = Schedule::Once(500);
    assert_eq!(s.next_occurrence(1000), None);
}

#[test]
fn once_exact() {
    let s = Schedule::Once(1000);
    assert_eq!(s.next_occurrence(1000), Some(1000));
}

#[test]
fn interval_alignment() {
    let s = Schedule::Interval { seconds: 300 };
    assert_eq!(s.next_occurrence(0), Some(0));
    assert_eq!(s.next_occurrence(1), Some(300));
    assert_eq!(s.next_occurrence(300), Some(300));
    assert_eq!(s.next_occurrence(301), Some(600));
}

#[test]
fn interval_zero() {
    let s = Schedule::Interval { seconds: 0 };
    assert_eq!(s.next_occurrence(12345), Some(12345));
}

#[test]
fn daily_today_not_passed() {
    // 2024-01-15 at midnight UTC
    let base = compose_test_ts(2024, 1, 15, 0, 0, 0);
    let s = Schedule::Daily {
        hour: 14,
        minute: 30,
    };
    let expected = compose_test_ts(2024, 1, 15, 14, 30, 0);
    assert_eq!(s.next_occurrence(base), Some(expected));
}

#[test]
fn daily_today_already_passed() {
    let base = compose_test_ts(2024, 1, 15, 15, 0, 0);
    let s = Schedule::Daily {
        hour: 14,
        minute: 30,
    };
    let expected = compose_test_ts(2024, 1, 16, 14, 30, 0);
    assert_eq!(s.next_occurrence(base), Some(expected));
}

#[test]
fn weekly_same_day_not_passed() {
    // 2024-01-15 is Monday (dow=1)
    let base = compose_test_ts(2024, 1, 15, 8, 0, 0);
    let s = Schedule::Weekly {
        day: Weekday::Monday,
        hour: 10,
        minute: 0,
    };
    let expected = compose_test_ts(2024, 1, 15, 10, 0, 0);
    assert_eq!(s.next_occurrence(base), Some(expected));
}

#[test]
fn weekly_next_week() {
    // 2024-01-15 is Monday, time past → next Monday
    let base = compose_test_ts(2024, 1, 15, 11, 0, 0);
    let s = Schedule::Weekly {
        day: Weekday::Monday,
        hour: 10,
        minute: 0,
    };
    let expected = compose_test_ts(2024, 1, 22, 10, 0, 0);
    assert_eq!(s.next_occurrence(base), Some(expected));
}

#[test]
fn weekly_different_day() {
    // 2024-01-15 is Monday, schedule for Wednesday
    let base = compose_test_ts(2024, 1, 15, 0, 0, 0);
    let s = Schedule::Weekly {
        day: Weekday::Wednesday,
        hour: 9,
        minute: 0,
    };
    let expected = compose_test_ts(2024, 1, 17, 9, 0, 0);
    assert_eq!(s.next_occurrence(base), Some(expected));
}

#[test]
fn monthly_same_month() {
    let base = compose_test_ts(2024, 1, 10, 0, 0, 0);
    let s = Schedule::Monthly {
        day: 20,
        hour: 12,
        minute: 0,
    };
    let expected = compose_test_ts(2024, 1, 20, 12, 0, 0);
    assert_eq!(s.next_occurrence(base), Some(expected));
}

#[test]
fn monthly_day_31_february_skips() {
    let base = compose_test_ts(2024, 2, 1, 0, 0, 0);
    let s = Schedule::Monthly {
        day: 31,
        hour: 0,
        minute: 0,
    };
    // February has 29 days (2024 is leap), March has 31
    let expected = compose_test_ts(2024, 3, 31, 0, 0, 0);
    assert_eq!(s.next_occurrence(base), Some(expected));
}

#[test]
fn cron_next_occurrence() {
    // Every 5 minutes starting from 2024-01-15 00:03:00
    let expr = CronExpr::parse("*/5 * * * *").unwrap();
    let s = Schedule::Cron(expr);
    let base = compose_test_ts(2024, 1, 15, 0, 3, 0);
    let expected = compose_test_ts(2024, 1, 15, 0, 5, 0);
    assert_eq!(s.next_occurrence(base), Some(expected));
}

// -----------------------------------------------------------------------
// Scheduler tests
// -----------------------------------------------------------------------

#[test]
fn scheduler_add_remove() {
    let mut sched = Scheduler::new();
    let task = ScheduledTask::new(0, "test".into(), "echo hi".into(), Schedule::Once(1000), 0);
    let id = sched.add_task(task);
    assert_eq!(id, 1);
    assert_eq!(sched.task_count(), 1);
    assert!(sched.remove_task(id));
    assert_eq!(sched.task_count(), 0);
    assert!(!sched.remove_task(id)); // already removed
}

#[test]
fn scheduler_enable_disable() {
    let mut sched = Scheduler::new();
    let task = ScheduledTask::new(0, "test".into(), "echo hi".into(), Schedule::Once(1000), 0);
    let id = sched.add_task(task);

    assert!(sched.get_task(id).unwrap().enabled);
    sched.disable_task(id);
    assert!(!sched.get_task(id).unwrap().enabled);
    sched.enable_task(id);
    assert!(sched.get_task(id).unwrap().enabled);
}

#[test]
fn scheduler_tick_returns_due_tasks() {
    let mut sched = Scheduler::new();
    let t1 = ScheduledTask::new(0, "a".into(), "echo a".into(), Schedule::Once(100), 0);
    let t2 = ScheduledTask::new(0, "b".into(), "echo b".into(), Schedule::Once(200), 0);
    let t3 = ScheduledTask::new(0, "c".into(), "echo c".into(), Schedule::Once(300), 0);
    let id1 = sched.add_task(t1);
    let id2 = sched.add_task(t2);
    let _id3 = sched.add_task(t3);

    let due = sched.tick(200);
    assert!(due.contains(&id1));
    assert!(due.contains(&id2));
    assert_eq!(due.len(), 2);
}

#[test]
fn scheduler_tick_disabled_not_due() {
    let mut sched = Scheduler::new();
    let t = ScheduledTask::new(0, "a".into(), "echo a".into(), Schedule::Once(100), 0);
    let id = sched.add_task(t);
    sched.disable_task(id);
    let due = sched.tick(200);
    assert!(due.is_empty());
}

#[test]
fn scheduler_tick_advances_next_run() {
    let mut sched = Scheduler::new();
    let t = ScheduledTask::new(
        0,
        "interval".into(),
        "echo tick".into(),
        Schedule::Interval { seconds: 60 },
        0,
    );
    let id = sched.add_task(t);
    // First tick at t=0: task is due (next_run=0)
    let due = sched.tick(0);
    assert!(due.contains(&id));
    // After tick, next_run should be advanced
    let task = sched.get_task(id).unwrap();
    assert!(task.next_run.unwrap() > 0);
}

#[test]
fn scheduler_pending_tasks() {
    let mut sched = Scheduler::new();
    let t1 = ScheduledTask::new(0, "a".into(), "echo a".into(), Schedule::Once(100), 0);
    let t2 = ScheduledTask::new(0, "b".into(), "echo b".into(), Schedule::Once(500), 0);
    sched.add_task(t1);
    sched.add_task(t2);

    let pending = sched.pending_tasks(200);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].name, "a");
}

#[test]
fn scheduler_run_task_echo() {
    let mut sched = Scheduler::new();
    let t = ScheduledTask::new(0, "echo".into(), "echo hello_world".into(), Schedule::Once(0), 0);
    let id = sched.add_task(t);

    let result = sched.run_task(id).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout_preview.contains("hello_world"));

    let task = sched.get_task(id).unwrap();
    assert_eq!(task.run_count, 1);
    assert!(task.last_run.is_some());
    assert!(task.last_result.is_some());
}

#[test]
fn scheduler_run_task_failing_command() {
    let mut sched = Scheduler::new();
    let cmd = if cfg!(target_os = "windows") {
        "cmd /C exit 42"
    } else {
        "exit 42"
    };
    let t = ScheduledTask::new(0, "fail".into(), cmd.into(), Schedule::Once(0), 0);
    let id = sched.add_task(t);
    let result = sched.run_task(id).unwrap();
    assert_eq!(result.exit_code, 42);
}

#[test]
fn scheduler_history() {
    let mut sched = Scheduler::new();
    let t = ScheduledTask::new(0, "echo".into(), "echo run".into(), Schedule::Once(0), 0);
    let id = sched.add_task(t);

    assert!(sched.history(id).is_empty());
    sched.run_task(id);
    assert_eq!(sched.history(id).len(), 1);
    sched.run_task(id);
    assert_eq!(sched.history(id).len(), 2);
}

#[test]
fn scheduler_run_nonexistent_task() {
    let mut sched = Scheduler::new();
    assert!(sched.run_task(999).is_none());
}

#[test]
fn scheduler_multiple_ids_sequential() {
    let mut sched = Scheduler::new();
    let id1 = sched.add_task(ScheduledTask::new(
        0, "a".into(), "echo".into(), Schedule::Once(0), 0,
    ));
    let id2 = sched.add_task(ScheduledTask::new(
        0, "b".into(), "echo".into(), Schedule::Once(0), 0,
    ));
    let id3 = sched.add_task(ScheduledTask::new(
        0, "c".into(), "echo".into(), Schedule::Once(0), 0,
    ));
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn scheduler_task_ids_sorted() {
    let mut sched = Scheduler::new();
    sched.add_task(ScheduledTask::new(
        0, "c".into(), "echo".into(), Schedule::Once(0), 0,
    ));
    sched.add_task(ScheduledTask::new(
        0, "a".into(), "echo".into(), Schedule::Once(0), 0,
    ));
    sched.add_task(ScheduledTask::new(
        0, "b".into(), "echo".into(), Schedule::Once(0), 0,
    ));
    assert_eq!(sched.task_ids(), vec![1, 2, 3]);
}

// -----------------------------------------------------------------------
// Weekday tests
// -----------------------------------------------------------------------

#[test]
fn weekday_from_u8_valid() {
    assert_eq!(Weekday::from_u8(0), Some(Weekday::Sunday));
    assert_eq!(Weekday::from_u8(1), Some(Weekday::Monday));
    assert_eq!(Weekday::from_u8(6), Some(Weekday::Saturday));
}

#[test]
fn weekday_from_u8_invalid() {
    assert_eq!(Weekday::from_u8(7), None);
    assert_eq!(Weekday::from_u8(255), None);
}

// -----------------------------------------------------------------------
// ScheduledTask tests
// -----------------------------------------------------------------------

#[test]
fn task_new_computes_next_run() {
    let t = ScheduledTask::new(1, "test".into(), "echo".into(), Schedule::Once(500), 100);
    assert_eq!(t.next_run, Some(500));
    assert_eq!(t.run_count, 0);
    assert!(t.enabled);
}

#[test]
fn task_is_due() {
    let t = ScheduledTask::new(1, "test".into(), "echo".into(), Schedule::Once(500), 100);
    assert!(!t.is_due(400));
    assert!(t.is_due(500));
    assert!(t.is_due(600));
}

#[test]
fn task_disabled_not_due() {
    let mut t = ScheduledTask::new(1, "test".into(), "echo".into(), Schedule::Once(500), 100);
    t.enabled = false;
    assert!(!t.is_due(600));
}

#[test]
fn task_recompute_next_run() {
    let mut t = ScheduledTask::new(
        1,
        "test".into(),
        "echo".into(),
        Schedule::Interval { seconds: 60 },
        0,
    );
    t.recompute_next_run(100);
    assert!(t.next_run.is_some());
    assert!(t.next_run.unwrap() >= 100);
}

// -----------------------------------------------------------------------
// Platform: describe_schedule
// -----------------------------------------------------------------------

#[test]
fn describe_schedule_variants() {
    assert!(PlatformBridge::describe_schedule(&Schedule::Once(12345)).contains("once"));
    assert!(PlatformBridge::describe_schedule(&Schedule::Interval { seconds: 30 }).contains("30 seconds"));
    assert!(PlatformBridge::describe_schedule(&Schedule::Interval { seconds: 120 }).contains("2 minutes"));
    assert!(PlatformBridge::describe_schedule(&Schedule::Interval { seconds: 7200 }).contains("2 hours"));
    assert!(PlatformBridge::describe_schedule(&Schedule::Daily { hour: 9, minute: 30 }).contains("09:30"));
    let weekly_desc = PlatformBridge::describe_schedule(&Schedule::Weekly {
        day: Weekday::Friday,
        hour: 17,
        minute: 0,
    });
    assert!(weekly_desc.contains("Friday"));
    assert!(PlatformBridge::describe_schedule(&Schedule::Monthly { day: 1, hour: 0, minute: 0 }).contains("day 1"));
}

// -----------------------------------------------------------------------
// CronExpr::next_match integration
// -----------------------------------------------------------------------

#[test]
fn cron_next_match_midnight_daily() {
    let expr = CronExpr::parse("0 0 * * *").unwrap();
    let s = Schedule::Cron(expr);
    // From 2024-01-15 00:00:01 → should be 2024-01-16 00:00:00
    let base = compose_test_ts(2024, 1, 15, 0, 0, 1);
    let expected = compose_test_ts(2024, 1, 16, 0, 0, 0);
    assert_eq!(s.next_occurrence(base), Some(expected));
}

#[test]
fn cron_next_match_exact_now() {
    let expr = CronExpr::parse("30 14 * * *").unwrap();
    let s = Schedule::Cron(expr);
    // Exactly at 14:30 → should return that same time
    let now = compose_test_ts(2024, 6, 1, 14, 30, 0);
    assert_eq!(s.next_occurrence(now), Some(now));
}

// -----------------------------------------------------------------------
// Helper: compose a test timestamp
// -----------------------------------------------------------------------

fn compose_test_ts(year: u32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> u64 {
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
    (total_days as u64) * 86400 + (hour as u64) * 3600 + (minute as u64) * 60 + (second as u64)
}
