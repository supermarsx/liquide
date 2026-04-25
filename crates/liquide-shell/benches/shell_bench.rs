use criterion::{Criterion, black_box, criterion_group, criterion_main};

use liquide_compositor::geometry::Rect;
use liquide_shell::app_history::AppHistory;
use liquide_shell::focus::*;
use liquide_shell::history::{WindowEventKind, WindowHistory};
use liquide_shell::layout::TilingLayout;
use liquide_shell::screen_time::ScreenTimeTracker;
use liquide_shell::shell::Shell;
use liquide_shell::stats::StatsCollector;
use liquide_shell::window::WindowId;

fn bench_open_close_1000_windows(c: &mut Criterion) {
    c.bench_function("open_close_1000_windows", |b| {
        b.iter(|| {
            let mut shell = Shell::new(1920.0, 1080.0);
            let mut ids = Vec::with_capacity(1000);
            for i in 0..1000u64 {
                let id = shell.open_window(format!("Win{i}"), Rect::new(0.0, 0.0, 200.0, 150.0));
                ids.push(id);
            }
            for id in ids {
                let _ = black_box(shell.close_window(id));
            }
        })
    });
}

fn bench_visible_windows_sort_500(c: &mut Criterion) {
    c.bench_function("visible_windows_sort_500", |b| {
        let mut shell = Shell::new(1920.0, 1080.0);
        for i in 0..500u64 {
            let id = shell.open_window(format!("Win{i}"), Rect::new(0.0, 0.0, 100.0, 100.0));
            shell.window_mut(id).unwrap().z_order = (500 - i as i32) % 100;
        }
        b.iter(|| {
            let _ = black_box(shell.visible_windows());
        })
    });
}

fn bench_tiling_layout_100_windows(c: &mut Criterion) {
    c.bench_function("tiling_layout_100_windows", |b| {
        let mut shell = Shell::new(1920.0, 1080.0);
        shell.set_layout(Box::new(TilingLayout::new(5.0, 10)));
        for i in 0..100u64 {
            shell.open_window(format!("Win{i}"), Rect::new(0.0, 0.0, 100.0, 100.0));
        }
        b.iter(|| {
            shell.arrange_windows();
        })
    });
}

fn bench_focus_cycle_100_windows(c: &mut Criterion) {
    c.bench_function("focus_cycle_100_windows", |b| {
        let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
        for i in 0..100 {
            fm.set_focus(WindowId(i));
        }
        b.iter(|| {
            for _ in 0..100 {
                fm.focus_next();
            }
        })
    });
}

fn bench_record_10000_events(c: &mut Criterion) {
    c.bench_function("record_10000_events", |b| {
        b.iter(|| {
            let mut h = WindowHistory::new(10_000);
            for i in 0..10_000u64 {
                h.record(WindowId(i % 100), WindowEventKind::Opened);
            }
            black_box(&h);
        })
    });
}

fn bench_query_events_for_window_from_10000(c: &mut Criterion) {
    c.bench_function("query_events_for_window_from_10000", |b| {
        let mut h = WindowHistory::new(10_000);
        for i in 0..10_000u64 {
            h.record(WindowId(i % 100), WindowEventKind::Opened);
        }
        b.iter(|| {
            let events = h.events_for_window(WindowId(42));
            black_box(events);
        })
    });
}

fn bench_app_history_1000_apps(c: &mut Criterion) {
    c.bench_function("app_history_1000_apps", |b| {
        b.iter(|| {
            let mut h = AppHistory::new(2000);
            let bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
            for i in 0..1000u64 {
                let app_id = format!("com.example.app{i}");
                h.record_open(&app_id, WindowId(i), bounds, i);
                h.record_close(&app_id, WindowId(i), bounds, i + 1);
            }
            black_box(h.most_recent(10));
            black_box(h.most_frequent(10));
        })
    });
}

fn bench_window_stats_from_10000_events(c: &mut Criterion) {
    c.bench_function("window_stats_from_10000_events", |b| {
        let mut wh = WindowHistory::new(10_000);
        let ah = AppHistory::new(100);
        let r1 = Rect::new(0.0, 0.0, 100.0, 100.0);
        let r2 = Rect::new(10.0, 10.0, 200.0, 200.0);
        for i in 0..10_000u64 {
            let wid = WindowId(i % 50);
            match i % 5 {
                0 => wh.record_at(wid, WindowEventKind::Opened, i),
                1 => wh.record_at(wid, WindowEventKind::Focused, i),
                2 => wh.record_at(wid, WindowEventKind::Moved { from: r1, to: r2 }, i),
                3 => wh.record_at(wid, WindowEventKind::Unfocused, i),
                _ => wh.record_at(wid, WindowEventKind::Closed, i),
            }
        }
        b.iter(|| {
            let c = StatsCollector::new(&wh, &ah);
            black_box(c.window_stats(WindowId(7)));
        })
    });
}

fn bench_system_stats_from_10000_events(c: &mut Criterion) {
    c.bench_function("system_stats_from_10000_events", |b| {
        let mut wh = WindowHistory::new(10_000);
        let mut ah = AppHistory::new(500);
        let r1 = Rect::new(0.0, 0.0, 100.0, 100.0);
        let r2 = Rect::new(10.0, 10.0, 200.0, 200.0);
        for i in 0..10_000u64 {
            let wid = WindowId(i % 100);
            let app = format!("app{}", i % 20);
            match i % 6 {
                0 => {
                    wh.record_at(wid, WindowEventKind::Opened, i);
                    ah.record_open(&app, wid, r1, i);
                }
                1 => wh.record_at(wid, WindowEventKind::Focused, i),
                2 => wh.record_at(wid, WindowEventKind::Moved { from: r1, to: r2 }, i),
                3 => wh.record_at(wid, WindowEventKind::Resized { from: r1, to: r2 }, i),
                4 => wh.record_at(wid, WindowEventKind::Unfocused, i),
                _ => {
                    wh.record_at(wid, WindowEventKind::Closed, i);
                    ah.record_close(&app, wid, r2, i);
                }
            }
        }
        b.iter(|| {
            let c = StatsCollector::new(&wh, &ah);
            black_box(c.system_stats());
        })
    });
}

fn bench_screen_time_feed_10000_events(c: &mut Criterion) {
    c.bench_function("screen_time_feed_10000_events", |b| {
        b.iter(|| {
            let us_per_second: u64 = 1_000_000;
            let mut tracker = ScreenTimeTracker::with_tick_duration(0, 0, 1);
            for i in 0..10_000u64 {
                let app = format!("app{}", i % 20);
                let wid = WindowId(i % 100);
                match i % 4 {
                    0 => tracker.feed_open(&app, wid, i * us_per_second),
                    1 => tracker.feed_focus(&app, wid, i * us_per_second),
                    2 => tracker.feed_unfocus(i * us_per_second),
                    _ => tracker.feed_close(&app, wid, i * us_per_second),
                }
            }
            // Query after all events
            let dk = ScreenTimeTracker::day_key(5000 * us_per_second);
            black_box(tracker.daily_report(dk));
        })
    });
}

fn bench_screen_time_hourly_heatmap(c: &mut Criterion) {
    c.bench_function("screen_time_hourly_heatmap", |b| {
        let us_per_hour: u64 = 3_600_000_000;
        let mut tracker = ScreenTimeTracker::with_tick_duration(0, 0, 1);
        // Feed events spanning 24 hours in day 0
        for h in 0..24u64 {
            let app = format!("app{}", h % 5);
            let wid = WindowId(h);
            let start = h * us_per_hour + 1000;
            let end = start + us_per_hour / 2; // half hour per session
            tracker.feed_focus(&app, wid, start);
            tracker.feed_unfocus(end);
        }
        b.iter(|| {
            let day = tracker.daily_report(0).unwrap();
            black_box(&day.hourly);
            black_box(day.peak_hour());
            black_box(day.top_apps(5));
        })
    });
}

criterion_group!(
    benches,
    bench_open_close_1000_windows,
    bench_visible_windows_sort_500,
    bench_tiling_layout_100_windows,
    bench_focus_cycle_100_windows,
    bench_record_10000_events,
    bench_query_events_for_window_from_10000,
    bench_app_history_1000_apps,
    bench_window_stats_from_10000_events,
    bench_system_stats_from_10000_events,
    bench_screen_time_feed_10000_events,
    bench_screen_time_hourly_heatmap,
);
criterion_main!(benches);
