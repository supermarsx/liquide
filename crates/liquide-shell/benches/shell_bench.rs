use criterion::{black_box, criterion_group, criterion_main, Criterion};

use liquide_compositor::geometry::Rect;
use liquide_shell::shell::Shell;
use liquide_shell::layout::TilingLayout;
use liquide_shell::focus::*;
use liquide_shell::window::WindowId;

fn bench_open_close_1000_windows(c: &mut Criterion) {
    c.bench_function("open_close_1000_windows", |b| {
        b.iter(|| {
            let mut shell = Shell::new(1920.0, 1080.0);
            let mut ids = Vec::with_capacity(1000);
            for i in 0..1000u64 {
                let id = shell.open_window(
                    format!("Win{i}"),
                    Rect::new(0.0, 0.0, 200.0, 150.0),
                );
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
            let id = shell.open_window(
                format!("Win{i}"),
                Rect::new(0.0, 0.0, 100.0, 100.0),
            );
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

criterion_group!(
    benches,
    bench_open_close_1000_windows,
    bench_visible_windows_sort_500,
    bench_tiling_layout_100_windows,
    bench_focus_cycle_100_windows,
);
criterion_main!(benches);
