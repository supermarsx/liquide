//! Teeth tests for isolated group / layer opacity.
//!
//! CSS group opacity must composite a group as ONE unit: overlapping children
//! inside an `opacity < 1` group are merged first, then the merged result is
//! dimmed a single time. The pre-fix renderer discarded `isolate` and instead
//! premultiplied each child's own alpha, so two overlapping translucent children
//! double-composited and the overlap came out darker than a single composite.
//!
//! These tests build a flat scene with a `RenderLayer { isolate: true }` carrying
//! the group opacity, followed by two overlapping children, and assert the overlap
//! equals a SINGLE group composite (not the double-darkened value). RED on the old
//! stub, GREEN with the offscreen-layer fix.

use liquide_compositor::Renderer;
use liquide_compositor::damage::{DamageClass, DamageSet};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Affine2D, Rect};
use liquide_compositor::pixel::{BlendMode, Color, PixelFormat};
use liquide_compositor::scene::{FlatNode, SceneNodeKind};

use crate::renderer::SoftwareRenderer;

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

fn bg(id: u64, color: Color, bounds: Rect, opacity: f32) -> FlatNode {
    node(id, SceneNodeKind::Background { color }, bounds, opacity)
}

fn render(nodes: &[FlatNode], w: u32, h: u32) -> FrameBuffer {
    let mut r = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
    // Full damage over the whole frame so every node paints (tile size 8).
    let damage = DamageSet::full(8, w.div_ceil(8), h.div_ceil(8), DamageClass::UiPrimitive);
    r.render(nodes, &mut fb, &damage).unwrap();
    fb
}

/// Straight-alpha SrcOver of `src` over opaque `dst`, matching the renderer's
/// integer compositing (round-to-nearest). Used to compute the EXPECTED single
/// group composite.
fn src_over_opaque(src: Color, src_a: f32, dst: Color) -> Color {
    let inv = 1.0 - src_a;
    let mix = |s: u8, d: u8| ((s as f32 * src_a + d as f32 * inv) + 0.5).clamp(0.0, 255.0) as u8;
    Color::new(mix(src.r, dst.r), mix(src.g, dst.g), mix(src.b, dst.b), 255)
}

/// Allow ±2 per channel for rounding across the two compositing routes.
fn close(a: Color, b: Color, tol: i32) -> bool {
    (a.r as i32 - b.r as i32).abs() <= tol
        && (a.g as i32 - b.g as i32).abs() <= tol
        && (a.b as i32 - b.b as i32).abs() <= tol
}

#[test]
fn isolated_group_opacity_composites_overlap_once_not_twice() {
    let (w, h) = (64u32, 64u32);
    let white = Color::new(255, 255, 255, 255);
    let red = Color::new(255, 0, 0, 255);
    let blue = Color::new(0, 0, 255, 255);

    // Backdrop: opaque white.
    let backdrop = bg(1, white, Rect::new(0.0, 0.0, w as f32, h as f32), 1.0);

    // Isolated group at 50% opacity covering the whole frame.
    let layer = node(
        2,
        SceneNodeKind::RenderLayer {
            blend_mode: BlendMode::SrcOver,
            isolate: true,
        },
        Rect::new(0.0, 0.0, w as f32, h as f32),
        0.5,
    );

    // Two OPAQUE overlapping children (full per-node opacity — the group opacity
    // lives on the layer). Red spans x[8,40), blue spans x[24,56); they overlap in
    // x[24,40). Blue is drawn AFTER red, so in the merged layer blue wins the
    // overlap.
    let red_rect = bg(3, red, Rect::new(8.0, 8.0, 32.0, 48.0), 1.0);
    let blue_rect = bg(4, blue, Rect::new(24.0, 8.0, 32.0, 48.0), 1.0);

    let fb = render(&[backdrop, layer, red_rect, blue_rect], w, h);

    // Sample points (centres of each region, within y[8,56)).
    let red_only = fb.get_pixel(14, 30); // x in [8,24)
    let overlap = fb.get_pixel(32, 30); // x in [24,40)
    let blue_only = fb.get_pixel(48, 30); // x in [40,56)

    // EXPECTED single-composite values: each region's MERGED colour (opaque)
    // composited ONCE over white at 0.5.
    let exp_red = src_over_opaque(red, 0.5, white);
    let exp_blue = src_over_opaque(blue, 0.5, white);

    assert!(
        close(red_only, exp_red, 2),
        "red-only region must be a single 0.5 composite of red over white: \
         got {red_only:?}, expected ~{exp_red:?}"
    );
    assert!(
        close(blue_only, exp_blue, 2),
        "blue-only region must be a single 0.5 composite of blue over white: \
         got {blue_only:?}, expected ~{exp_blue:?}"
    );

    // The overlap: blue won the merge, so it must equal a SINGLE 0.5 composite of
    // blue over white — the SAME as the blue-only region. The buggy double-
    // composite would instead darken it (blue at 0.5 over (red at 0.5 over white))
    // = 0.5*blue + 0.5*(0.5*red + 0.5*white), which has a non-zero red channel and
    // a dimmer blue. We assert the overlap matches the single composite AND that
    // it is NOT the double-composite value.
    assert!(
        close(overlap, exp_blue, 2),
        "overlap must be a SINGLE group composite (blue over white at 0.5): \
         got {overlap:?}, expected ~{exp_blue:?}"
    );

    // Compute the double-composite value the OLD stub produced and prove we differ.
    let red_half_over_white = src_over_opaque(red, 0.5, white);
    let double = src_over_opaque(blue, 0.5, red_half_over_white);
    assert!(
        !close(overlap, double, 2),
        "overlap must NOT equal the double-composited (darker) value {double:?} — \
         got {overlap:?}; group opacity is double-compositing overlaps"
    );

    // Concretely, the overlap must not carry a red tint (the double-composite
    // leaks ~0.25*red into the overlap; the correct single composite has red≈0).
    assert!(
        overlap.r <= exp_blue.r + 2,
        "overlap leaked red from a double-composite: r={} (expected ~{})",
        overlap.r,
        exp_blue.r
    );
}

/// A fully-opaque isolated group must be a visual no-op for compositing: its
/// merged children equal painting them directly (no snapshot/dimming applied).
/// Guards that the layer fast-out (opacity ≈ 1) does not perturb pixels.
#[test]
fn opaque_isolated_group_is_a_noop() {
    let (w, h) = (32u32, 32u32);
    let white = Color::new(255, 255, 255, 255);
    let green = Color::new(0, 200, 0, 255);

    let backdrop = bg(1, white, Rect::new(0.0, 0.0, w as f32, h as f32), 1.0);
    let layer = node(
        2,
        SceneNodeKind::RenderLayer {
            blend_mode: BlendMode::SrcOver,
            isolate: true,
        },
        Rect::new(0.0, 0.0, w as f32, h as f32),
        1.0,
    );
    let rect = bg(3, green, Rect::new(4.0, 4.0, 16.0, 16.0), 1.0);

    let fb = render(&[backdrop, layer, rect], w, h);
    assert_eq!(
        fb.get_pixel(10, 10),
        green,
        "opaque isolated group must paint its opaque child verbatim"
    );
    assert_eq!(
        fb.get_pixel(28, 28),
        white,
        "outside the child the backdrop is untouched"
    );
}
