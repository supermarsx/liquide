use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use liquide_compositor::compositor::CompositorContract;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
use liquide_compositor::{Compositor, QualityProfile};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const TILE_SIZE: u32 = 64;

fn build_scene(window_count: usize) -> SceneNode {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32)),
    );
    root.add_child(SceneNode::new(
        1,
        SceneNodeKind::Background {
            color: Color::new(12, 18, 28, 255),
        },
        NodeProperties::new(Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32)),
    ));

    for index in 0..window_count {
        let column = index % 4;
        let row = index / 4;
        let x = 48.0 + column as f32 * 296.0;
        let y = 56.0 + row as f32 * 160.0;
        let base_id = 10_000 + index as u64 * 10;
        let z_order = 10 + index as u32 * 4;

        root.add_child(SceneNode::new(
            base_id,
            SceneNodeKind::Background {
                color: Color::new(30, 38, 54, 255),
            },
            NodeProperties::new(Rect::new(x, y, 248.0, 128.0))
                .with_z_order(z_order)
                .with_corner_radius((10.0, 10.0, 10.0, 10.0)),
        ));
        root.add_child(SceneNode::new(
            base_id + 1,
            SceneNodeKind::Tint {
                color: Color::new(96, 170, 255, 34),
            },
            NodeProperties::new(Rect::new(x + 12.0, y + 16.0, 224.0, 16.0))
                .with_z_order(z_order + 1),
        ));
        root.add_child(SceneNode::new(
            base_id + 2,
            SceneNodeKind::TextCaret {
                color: Color::new(240, 245, 255, 255),
                width: 2.0,
            },
            NodeProperties::new(Rect::new(x + 20.0, y + 48.0, 2.0, 26.0)).with_z_order(z_order + 2),
        ));
    }

    root
}

fn fill_render_target(compositor: &mut Compositor, seed: u8) {
    let color = Color::new(seed, seed.wrapping_mul(2), seed.wrapping_mul(3), 255);
    compositor.frame_buffer_mut().clear(color);
}

fn drive_frame(compositor: &mut Compositor, scene: SceneNode, seed: u8) -> usize {
    compositor
        .submit_scene(scene)
        .expect("benchmark scene should be valid");
    compositor.prepare_frame();
    let flat_count = compositor.flat_scene().len();
    fill_render_target(compositor, seed);
    compositor.end_frame();
    let damage = compositor
        .compute_damage()
        .expect("benchmark damage should compute");
    compositor.present_frame();
    black_box(flat_count + damage.len())
}

fn bench_compositor_render_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("compositor_render_lifecycle_no_run_gate");
    for window_count in [4usize, 16usize] {
        group.bench_with_input(
            BenchmarkId::new("submit_flatten_damage_present", window_count),
            &window_count,
            |b, &window_count| {
                b.iter_batched(
                    || {
                        let compositor =
                            Compositor::new(WIDTH, HEIGHT, TILE_SIZE, QualityProfile::Balanced);
                        let scene = build_scene(window_count);
                        (compositor, scene)
                    },
                    |(mut compositor, scene)| {
                        let first = drive_frame(&mut compositor, scene.clone(), 31);
                        let second = drive_frame(&mut compositor, scene, 47);
                        black_box(first + second);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn bench_compositor_render_steady_scene(c: &mut Criterion) {
    let mut compositor = Compositor::new(WIDTH, HEIGHT, TILE_SIZE, QualityProfile::Balanced);
    let scene = build_scene(12);
    let _ = drive_frame(&mut compositor, scene.clone(), 89);

    c.bench_function("compositor_render_steady_scene_damage", |b| {
        b.iter(|| {
            let count = drive_frame(&mut compositor, scene.clone(), black_box(89));
            black_box(count);
        });
    });
}

criterion_group!(
    benches,
    bench_compositor_render_lifecycle,
    bench_compositor_render_steady_scene,
);
criterion_main!(benches);
