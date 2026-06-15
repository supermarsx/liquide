use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use liquide_compositor::damage::DamageTracker;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Affine2D, Rect};
use liquide_compositor::pixel::{Color, PixelFormat};
use liquide_compositor::scene::{
    BorderSide, BorderSideStyle, BorderSides, CursorShape, DecorationButtons, DecorationColors,
    DecorationLayout, FlatNode, NodeProperties, SceneNode, SceneNodeKind,
};

const SCREEN_WIDTH: f32 = 1920.0;
const SCREEN_HEIGHT: f32 = 1080.0;

fn solid_border(color: Color) -> BorderSides {
    let side = BorderSide {
        width: 1.0,
        style: BorderSideStyle::Solid,
        color,
    };
    BorderSides {
        top: side,
        right: side,
        bottom: side,
        left: side,
    }
}

fn build_desktop_scene(window_count: usize) -> SceneNode {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, SCREEN_WIDTH, SCREEN_HEIGHT)),
    );
    root.add_child(SceneNode::new(
        1,
        SceneNodeKind::Background {
            color: Color::new(18, 24, 36, 255),
        },
        NodeProperties::new(Rect::new(0.0, 0.0, SCREEN_WIDTH, SCREEN_HEIGHT)).with_z_order(0),
    ));

    for index in 0..window_count {
        let id_base = 10_000 + index as u64 * 10;
        let column = index % 4;
        let row = index / 4;
        let x = 72.0 + column as f32 * 430.0;
        let y = 84.0 + row as f32 * 180.0;
        let z_order = index as u32 + 1;

        root.add_child(SceneNode::new(
            id_base,
            SceneNodeKind::Shadow {
                spread: 10.0,
                blur_radius: 24.0,
                color: Color::new(0, 0, 0, 110),
                corner_radius: 16.0,
            },
            NodeProperties::new(Rect::new(x - 8.0, y - 8.0, 392.0, 292.0)).with_z_order(z_order),
        ));
        root.add_child(SceneNode::new(
            id_base + 1,
            SceneNodeKind::Background {
                color: Color::new(28, 34, 48, 255),
            },
            NodeProperties::new(Rect::new(x, y, 376.0, 276.0))
                .with_z_order(z_order + 1)
                .with_corner_radius((16.0, 16.0, 16.0, 16.0)),
        ));
        root.add_child(SceneNode::new(
            id_base + 2,
            SceneNodeKind::Decoration {
                title: Some(format!(
                    "Profiler Window {index}: render graph, tile stats, and commit telemetry"
                )),
                title_color: Color::new(245, 247, 252, 255),
                background: Color::new(38, 44, 58, 240),
                border_color: Color::new(255, 255, 255, 26),
                border_width: 1.0,
                corner_radius: 16.0,
                button_state: DecorationButtons::default(),
                button_colors: DecorationColors::default(),
                button_layout: DecorationLayout::default(),
            },
            NodeProperties::new(Rect::new(x, y, 376.0, 40.0)).with_z_order(z_order + 2),
        ));
        root.add_child(SceneNode::new(
            id_base + 3,
            SceneNodeKind::Text {
                text: format!(
                    "fn render_window_{index}() -> RenderStats {{ cached_damage.union(frame_delta); }}"
                ),
                color: Color::new(222, 228, 235, 255),
                scale: 1,
                font_family: "Manrope".to_string(),
                font_size: 14.0,
                font_weight: 600,
                font_style_italic: false,
                letter_spacing: 0.1,
                word_spacing: 0.0,
                line_height: 18.0,
                text_align: 0,
                text_transform: 0,
                text_overflow: 0,
                white_space: 0,
                word_break: liquide_compositor::scene::WordBreak::Normal,
                text_indent: 0.0,
                text_decoration: None,
                text_shadows: Vec::new(),
                text_emphasis: None,
            },
            NodeProperties::new(Rect::new(x + 18.0, y + 60.0, 332.0, 24.0)).with_z_order(z_order + 3),
        ));
        root.add_child(SceneNode::new(
            id_base + 4,
            SceneNodeKind::Border {
                sides: solid_border(Color::new(255, 255, 255, 30)),
                radius: (16.0, 16.0, 16.0, 16.0),
            },
            NodeProperties::new(Rect::new(x, y, 376.0, 276.0)).with_z_order(z_order + 4),
        ));
    }

    root
}

fn cursor_flat_node(x: f32, y: f32) -> FlatNode {
    FlatNode {
        id: 999_999,
        kind: SceneNodeKind::Cursor {
            shape: CursorShape::Pointer,
        }
        .into(),
        absolute_bounds: Rect::new(x, y, 24.0, 24.0),
        absolute_transform: Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 99_999,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    }
}

fn build_deep_tree(id: &mut u64, depth: u32, breadth: u32) -> SceneNode {
    let node_id = *id;
    *id += 1;
    let kind = if depth == 0 {
        SceneNodeKind::Background {
            color: Color::BLACK,
        }
    } else {
        SceneNodeKind::Workspace { index: 0 }
    };
    let mut node = SceneNode::new(
        node_id,
        kind,
        NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0)),
    );
    if depth > 0 {
        for _ in 0..breadth {
            node.add_child(build_deep_tree(id, depth - 1, breadth));
        }
    }
    node
}

fn bench_scene_flatten(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_flatten_hot_cache");
    for &window_count in &[8usize, 24usize] {
        let scene = build_desktop_scene(window_count);
        let _warm = scene.flatten();
        let mut flat = Vec::with_capacity(window_count * 5 + 2);
        group.bench_with_input(
            BenchmarkId::new("full_flatten", window_count),
            &window_count,
            |b, _| {
                b.iter(|| {
                    scene.flatten_into(&mut flat);
                    black_box(flat.len());
                });
            },
        );
    }
    group.finish();
}

fn bench_cursor_only_cached_flat_nodes(c: &mut Criterion) {
    let mut group = c.benchmark_group("cursor_only_cached_flat_nodes");
    for &window_count in &[8usize, 24usize] {
        let scene = build_desktop_scene(window_count);
        let cached_flat_nodes = scene.flatten();
        let mut flat_nodes_buf = Vec::with_capacity(cached_flat_nodes.len() + 1);
        group.bench_with_input(
            BenchmarkId::new("reuse_cached_nodes", window_count),
            &window_count,
            |b, _| {
                b.iter(|| {
                    flat_nodes_buf.clear();
                    flat_nodes_buf.extend(cached_flat_nodes.iter().cloned());
                    flat_nodes_buf.push(cursor_flat_node(black_box(960.0), black_box(540.0)));
                    black_box(flat_nodes_buf.len());
                });
            },
        );
    }
    group.finish();
}

fn bench_scene_find(c: &mut Criterion) {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );
    let mut id = 1u64;
    let tree = build_deep_tree(&mut id, 3, 5);
    root.add_child(tree);
    let last_id = root.descendants().last().copied().unwrap_or(0);

    c.bench_function("scene_find_deep_node", |b| {
        b.iter(|| {
            let found = black_box(&root).find(black_box(last_id));
            black_box(found);
        });
    });
}

fn bench_damage_tracker(c: &mut Criterion) {
    let mut tracker = DamageTracker::new(64, 1920, 1080);
    let fb = FrameBuffer::new(1920, 1080, PixelFormat::Bgra8);
    let _ = tracker.compute_damage(&fb);

    c.bench_function("damage_tracker_1080p", |b| {
        b.iter(|| {
            let damage = tracker.compute_damage(black_box(&fb));
            black_box(damage.len());
        });
    });
}

fn bench_affine_compose(c: &mut Criterion) {
    let transforms: Vec<Affine2D> = (0..100)
        .map(|i| {
            let t = Affine2D::translation(i as f32, i as f32 * 0.5);
            let s = Affine2D::scale(1.01, 0.99);
            t.then(&s)
        })
        .collect();

    c.bench_function("affine_compose_100", |b| {
        b.iter(|| {
            let mut acc = Affine2D::identity();
            for t in &transforms {
                acc = acc.then(black_box(t));
            }
            black_box(acc);
        });
    });
}

criterion_group!(
    benches,
    bench_scene_flatten,
    bench_cursor_only_cached_flat_nodes,
    bench_scene_find,
    bench_damage_tracker,
    bench_affine_compose,
);
criterion_main!(benches);
