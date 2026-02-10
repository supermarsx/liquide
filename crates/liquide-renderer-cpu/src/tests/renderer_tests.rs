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

#[test]
fn render_surface_node() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive });

    let node = FlatNode {
        id: 10,
        kind: SceneNodeKind::Surface {
            surface_id: 1,
            buffer: None,
        },
        absolute_bounds: Rect::new(0.0, 0.0, 64.0, 64.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
    };

    // Should not error even with no buffer
    let result = renderer.render(&[node], &mut fb, &damage);
    assert!(result.is_ok(), "render Surface with no buffer should succeed");
}

#[test]
fn render_glass_node() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    fb.clear(Color::new(100, 100, 100, 255));
    let before = fb.pixels.clone();

    let mut damage = DamageSet::new(64);
    damage.add(DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive });

    let node = FlatNode {
        id: 20,
        kind: SceneNodeKind::Glass(liquide_compositor::scene::GlassParams::default()),
        absolute_bounds: Rect::new(0.0, 0.0, 64.0, 64.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
    };

    let result = renderer.render(&[node], &mut fb, &damage);
    assert!(result.is_ok(), "render Glass node should succeed");
    assert_ne!(fb.pixels, before, "Glass tint should modify pixels");
}

#[test]
fn render_decoration_node() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);

    let mut damage = DamageSet::new(64);
    damage.add(DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive });

    let node = FlatNode {
        id: 30,
        kind: SceneNodeKind::Decoration {
            title: Some("Test Window".to_string()),
            title_color: Color::WHITE,
            background: Color::new(50, 50, 60, 255),
            border_color: Color::new(100, 100, 120, 255),
            border_width: 1.0,
            corner_radius: 8.0,
            button_state: liquide_compositor::scene::DecorationButtons::default(),
        },
        absolute_bounds: Rect::new(0.0, 0.0, 64.0, 32.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
    };

    let result = renderer.render(&[node], &mut fb, &damage);
    assert!(result.is_ok(), "render Decoration node should succeed");
    // The background fill should have modified some pixels
    let center = fb.get_pixel(32, 16);
    assert!(center.r > 0 || center.g > 0 || center.b > 0,
        "decoration should fill pixels: got {:?}", center);
}

#[test]
fn render_lock_screen_node() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    // Fill with content first
    for y in 0..128 {
        for x in 0..128 {
            fb.set_pixel(x, y, Color::new((x * 2) as u8, (y * 2) as u8, 128, 255));
        }
    }
    let before = fb.pixels.clone();

    let mut damage = DamageSet::new(64);
    damage.add(DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive });
    damage.add(DamageTile { x: 1, y: 0, class: DamageClass::UiPrimitive });
    damage.add(DamageTile { x: 0, y: 1, class: DamageClass::UiPrimitive });
    damage.add(DamageTile { x: 1, y: 1, class: DamageClass::UiPrimitive });

    let node = FlatNode {
        id: 40,
        kind: SceneNodeKind::LockScreen,
        absolute_bounds: Rect::new(0.0, 0.0, 128.0, 128.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
    };

    let result = renderer.render(&[node], &mut fb, &damage);
    assert!(result.is_ok(), "render LockScreen node should succeed");
    assert_ne!(fb.pixels, before, "LockScreen should modify pixels (backdrop blur + dark tint)");
}
