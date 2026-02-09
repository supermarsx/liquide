use crate::renderer::*;
use liquide_compositor::damage::DamageClass;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{Color, PixelFormat};

use liquide_compositor::damage::{DamageSet, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::scene::{FlatNode, SceneNodeKind};

#[test]
fn renderer_creates() {
    let r = SoftwareRenderer::new();
    assert!(r.glyph_atlas().is_empty());
}

#[test]
fn render_background() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive });

    let node = FlatNode {
        id: 1,
        kind: SceneNodeKind::Background {
            color: Color::new(0, 100, 200, 255),
        },
        absolute_bounds: Rect::new(0.0, 0.0, 128.0, 128.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
    };

    renderer.render(&[node], &mut fb, &damage).unwrap();
    let c = fb.get_pixel(32, 32);
    assert_eq!(c.r, 0);
    assert_eq!(c.g, 100);
    assert_eq!(c.b, 200);
}
