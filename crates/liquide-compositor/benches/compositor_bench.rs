use criterion::{Criterion, black_box, criterion_group, criterion_main};

use liquide_compositor::damage::DamageTracker;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Affine2D, Rect};
use liquide_compositor::pixel::{Color, PixelFormat};
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};

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
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );
    let mut id = 1u64;
    let tree = build_deep_tree(&mut id, 2, 10);
    root.add_child(tree);

    c.bench_function("scene_flatten_100_nodes", |b| {
        b.iter(|| {
            let flat = black_box(&root).flatten();
            black_box(flat.len());
        });
    });
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
    bench_scene_find,
    bench_damage_tracker,
    bench_affine_compose,
);
criterion_main!(benches);
