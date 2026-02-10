use criterion::{black_box, criterion_group, criterion_main, Criterion};

use liquide_input::keyboard::*;
use liquide_input::mouse::*;
use liquide_input::event::InputEvent;
use liquide_input::state::InputState;
use liquide_input::router::*;

use liquide_compositor::geometry::Rect;

struct BenchSurface {
    id: u64,
    bounds: Rect,
}

impl InputTarget for BenchSurface {
    fn id(&self) -> u64 { self.id }
    fn bounds(&self) -> Rect { self.bounds }
}

fn bench_state_handle_1000_key_events(c: &mut Criterion) {
    c.bench_function("state_handle_1000_key_events", |b| {
        b.iter(|| {
            let mut state = InputState::new();
            for i in 0..1000u64 {
                let key = if i % 2 == 0 { KeyCode::A } else { KeyCode::B };
                let ks = if i % 2 == 0 { KeyState::Pressed } else { KeyState::Released };
                let evt = InputEvent::Keyboard(KeyEvent::new(key, ks, Modifiers::new(), 30, i));
                state.handle_event(black_box(&evt));
            }
            state
        })
    });
}

fn bench_router_hit_test_100_surfaces(c: &mut Criterion) {
    c.bench_function("router_hit_test_100_surfaces", |b| {
        let surfaces: Vec<BenchSurface> = (0..100)
            .map(|i| BenchSurface {
                id: i as u64,
                bounds: Rect::new((i % 10) as f32 * 100.0, (i / 10) as f32 * 100.0, 100.0, 100.0),
            })
            .collect();
        let surface_refs: Vec<&dyn InputTarget> = surfaces.iter().map(|s| s as &dyn InputTarget).collect();
        let router = InputRouter::new();

        b.iter(|| {
            let evt = InputEvent::Mouse(MouseEvent::Move { x: 550.0, y: 550.0 });
            router.route(black_box(&evt), black_box(&surface_refs))
        })
    });
}

fn bench_modifiers_bitops(c: &mut Criterion) {
    c.bench_function("modifiers_bitops", |b| {
        b.iter(|| {
            let a = Modifiers::from_bits(black_box(Modifiers::SHIFT | Modifiers::CTRL));
            let b_mod = Modifiers::from_bits(black_box(Modifiers::ALT | Modifiers::SUPER));
            let c_mod = a | b_mod;
            let _ = c_mod.shift();
            let _ = c_mod.ctrl();
            let _ = c_mod.alt();
            let _ = c_mod.super_key();
            let _ = c_mod.contains(Modifiers::CAPS_LOCK);
            c_mod
        })
    });
}

criterion_group!(
    benches,
    bench_state_handle_1000_key_events,
    bench_router_hit_test_100_surfaces,
    bench_modifiers_bitops,
);
criterion_main!(benches);
