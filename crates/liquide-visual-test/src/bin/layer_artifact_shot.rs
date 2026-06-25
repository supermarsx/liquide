//! Diagnostic PNG dump for fix-incremental-artifacts.
//!
//! Renders a translucent (`opacity < 1`) isolated group strip over a SPARSE
//! partial-damage frame (two end tiles damaged, a middle GAP tile NOT damaged)
//! three ways and writes PNGs to `.orchestration/shots/`:
//!   * `layer_full.png`        — authoritative FULL repaint of frame N+1.
//!   * `layer_incr_fixed.png`  — incremental frame WITH the group-layer damage
//!                               expansion (the fix): pixel-identical to full.
//!   * `layer_incr_buggy.png`  — incremental frame WITHOUT the expansion: the
//!                               middle gap keeps STALE prior content (the trail).
//!
//! Run: `cargo run -p liquide-visual-test --bin layer_artifact_shot --offline`.

use liquide_compositor::damage::{DamageClass, DamageSet, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Affine2D, Rect};
use liquide_compositor::pixel::{BlendMode, Color, PixelFormat};
use liquide_compositor::scene::{FlatNode, SceneNodeKind};
use liquide_renderer_cpu::{RenderMode, SoftwareRenderer};

const W: u32 = 256;
const H: u32 = 128;
const TILE: u32 = 32;

fn node(id: u64, kind: SceneNodeKind, bounds: Rect, opacity: f32) -> FlatNode {
    FlatNode {
        id,
        kind: kind.into(),
        absolute_bounds: bounds,
        absolute_transform: Affine2D::identity(),
        clip: None,
        opacity,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    }
}

fn bg(id: u64, color: Color, bounds: Rect) -> FlatNode {
    node(id, SceneNodeKind::Background { color }, bounds, 1.0)
}

fn layer(id: u64, bounds: Rect, opacity: f32) -> FlatNode {
    node(
        id,
        SceneNodeKind::RenderLayer {
            blend_mode: BlendMode::SrcOver,
            isolate: true,
        },
        bounds,
        opacity,
    )
}

fn full_damage() -> DamageSet {
    DamageSet::full(TILE, W.div_ceil(TILE), H.div_ceil(TILE), DamageClass::UiPrimitive)
}

fn render(rnd: &mut SoftwareRenderer, base: Option<&FrameBuffer>, nodes: &[FlatNode], damage: &DamageSet) -> FrameBuffer {
    let mut fb = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    if let Some(b) = base {
        fb.pixels_mut().unwrap().copy_from_slice(b.pixels());
        // Mirror the worker: clear the damaged tiles before re-rastering.
        for t in &damage.tiles {
            for y in (t.y * TILE)..((t.y + 1) * TILE).min(H) {
                for x in (t.x * TILE)..((t.x + 1) * TILE).min(W) {
                    fb.set_pixel(x, y, Color::new(0, 0, 0, 0));
                }
            }
        }
    }
    let _ = rnd.render_live(nodes, &mut fb, damage, RenderMode::Capture).unwrap();
    fb
}

fn save(fb: &FrameBuffer, name: &str) {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let c = fb.get_pixel(x, y);
            let i = ((y * W + x) * 4) as usize;
            rgba[i] = c.r;
            rgba[i + 1] = c.g;
            rgba[i + 2] = c.b;
            rgba[i + 3] = c.a;
        }
    }
    let path = format!(".orchestration/shots/{name}");
    image::save_buffer(&path, &rgba, W, H, image::ColorType::Rgba8).expect("save png");
    println!("wrote {path}");
}

fn main() {
    let red = Color::new(255, 0, 0, 180);
    let yellow = Color::new(255, 255, 0, 180);
    // Group occupies exactly tile row 1 (y in [32,64)) so a "gap" is purely
    // horizontal (a column gap), keeping the diagnostic unambiguous.
    let group = Rect::new(8.0, 32.0, 240.0, 32.0);

    // Frame N: translucent red across the whole strip.
    let nodes_n = vec![layer(900, group, 0.5), bg(10, red, Rect::new(8.0, 32.0, 240.0, 32.0))];
    // Frame N+1: yellow only at the two ENDS; middle has no child (should empty).
    let nodes_n1 = vec![
        layer(900, group, 0.5),
        bg(11, yellow, Rect::new(8.0, 32.0, 48.0, 32.0)),
        bg(12, yellow, Rect::new(200.0, 32.0, 48.0, 32.0)),
    ];

    let mut rnd = SoftwareRenderer::new();
    for _ in 0..3 {
        let _ = render(&mut rnd, None, &nodes_n, &full_damage());
        let _ = render(&mut rnd, None, &nodes_n1, &full_damage());
    }
    let prev = render(&mut rnd, None, &nodes_n, &full_damage());
    let full = render(&mut rnd, None, &nodes_n1, &full_damage());

    // Sparse damage: end tiles only, GAP across the middle (tile row 48/32 = 1).
    let mut sparse = DamageSet::new(TILE);
    for tx in [0u32, 1, 6, 7] {
        sparse.add(DamageTile { x: tx, y: 1, class: DamageClass::UiPrimitive });
    }
    // FIXED: expand the sparse damage to the group's full tile bounds (cols 0..8).
    let mut expanded = sparse.clone();
    for tx in 0..8u32 {
        if !expanded.tiles.iter().any(|t| t.x == tx && t.y == 1) {
            expanded.add(DamageTile { x: tx, y: 1, class: DamageClass::UiPrimitive });
        }
    }

    let buggy = render(&mut rnd, Some(&prev), &nodes_n1, &sparse);
    let fixed = render(&mut rnd, Some(&prev), &nodes_n1, &expanded);

    save(&full, "layer_full.png");
    save(&fixed, "layer_incr_fixed.png");
    save(&buggy, "layer_incr_buggy.png");

    // Quick numeric confirmation on the GAP tile centre (tile row 1, y in [32,64)).
    let gp = (128u32, 48u32);
    println!(
        "gap-tile centre ({},{}): full={:?} fixed={:?} buggy={:?}",
        gp.0, gp.1,
        full.get_pixel(gp.0, gp.1),
        fixed.get_pixel(gp.0, gp.1),
        buggy.get_pixel(gp.0, gp.1),
    );
}
