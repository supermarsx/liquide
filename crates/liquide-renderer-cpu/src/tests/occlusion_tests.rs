//! Front-to-back occlusion culling tests (t137, t90 lever #5).
//!
//! These assert the two halves of the contract independently and with TEETH:
//!  (a) a node fully covered by a LATER fully-opaque rect is SKIPPED (not
//!      rasterized) — and the output is byte-identical to a scene where that
//!      covered node is simply absent (proving the cull cannot change a pixel);
//!  (b) a node under a SEMI-TRANSPARENT / glass / rounded later node is NOT
//!      culled (still painted) — proving a non-opaque cover never wrongly culls;
//!  (c) a PARTIALLY-covered node is still painted.
//!
//! The "was this node rastered" assertions ride a thread-local cull probe
//! (`reset_cull_probe` / `was_culled`); the correctness assertions ride
//! byte-equality of the rendered framebuffer.

use liquide_compositor::Renderer;
use liquide_compositor::damage::{DamageClass, DamageSet};
use liquide_compositor::geometry::{Affine2D, Rect};
use liquide_compositor::pixel::{BlendMode, Color, PixelFormat};
use liquide_compositor::scene::{
    BackgroundImage, BackgroundRepeat, BackgroundSize, BackgroundSpec, FlatNode, GlassParams,
    NodeId, SceneNodeKind,
};

use crate::renderer::{SoftwareRenderer, reset_cull_probe, was_culled};
use liquide_compositor::framebuffer::FrameBuffer;

const W: u32 = 64;
const H: u32 = 64;

/// A full-frame damage set (clip = None) — the deterministic capture path these
/// tests model. Generous grid so it covers the whole framebuffer.
fn full_damage() -> DamageSet {
    DamageSet::full(8, 16, 16, DamageClass::UiPrimitive)
}

fn node(id: NodeId, kind: SceneNodeKind, bounds: Rect) -> FlatNode {
    FlatNode {
        id,
        kind: kind.into(),
        absolute_bounds: bounds,
        absolute_transform: Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    }
}

fn solid(id: NodeId, color: Color, bounds: Rect) -> FlatNode {
    node(id, SceneNodeKind::Background { color }, bounds)
}

fn render(nodes: &[FlatNode]) -> FrameBuffer {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    reset_cull_probe();
    renderer.render(nodes, &mut fb, &full_damage()).unwrap();
    fb
}

// ── (a) Opaque cover culls + byte-identical ──────────────────────────────

#[test]
fn node_fully_covered_by_later_opaque_rect_is_skipped() {
    // Bottom green fill (id 1) entirely under a later opaque red fill (id 2).
    let covered = solid(1, Color::new(0, 255, 0, 255), Rect::new(8.0, 8.0, 20.0, 20.0));
    let cover = solid(2, Color::new(255, 0, 0, 255), Rect::new(0.0, 0.0, 40.0, 40.0));

    let fb = render(&[covered, cover]);

    // The covered node must have been occlusion-culled (skipped, not painted).
    assert!(
        was_culled(1),
        "fully-covered opaque node should be occlusion-culled"
    );
    assert!(
        !was_culled(2),
        "the top cover must NOT be culled — it has nothing above it"
    );

    // The visible result at the covered region is the red cover (the green was
    // never visible anyway).
    let p = fb.get_pixel(12, 12);
    assert_eq!((p.r, p.g, p.b), (255, 0, 0), "cover color must win");
}

#[test]
fn culling_is_byte_identical_to_removing_the_covered_node() {
    // A scene where node 1 is fully covered by node 2; a fully-occluded node
    // contributes ZERO pixels, so rendering it (then culling) must be
    // byte-identical to a scene that simply omits it. This is the cull
    // correctness proof: a wrong cull (or a wrong NON-cull that leaked a pixel)
    // would diverge here.
    let covered = solid(1, Color::new(0, 255, 0, 255), Rect::new(8.0, 8.0, 20.0, 20.0));
    let cover = solid(2, Color::new(255, 0, 0, 255), Rect::new(0.0, 0.0, 40.0, 40.0));

    let with_covered = render(&[covered.clone(), cover.clone()]);
    // Capture the cull BEFORE the next render resets the (thread-local) probe.
    let culled = was_culled(1);
    let without_covered = render(&[cover]);

    assert!(culled, "the covered node must actually have been culled");
    assert_eq!(
        with_covered.pixels(),
        without_covered.pixels(),
        "culling a fully-occluded node must be byte-identical to omitting it"
    );
}

#[test]
fn cover_with_exact_matching_bounds_culls() {
    // Cover exactly equal to the covered rect (closed containment) still culls.
    let covered = solid(1, Color::new(0, 200, 0, 255), Rect::new(10.0, 10.0, 24.0, 24.0));
    let cover = solid(2, Color::new(10, 20, 30, 255), Rect::new(10.0, 10.0, 24.0, 24.0));
    let _ = render(&[covered, cover]);
    assert!(was_culled(1), "equal-bounds opaque cover should cull");
}

#[test]
fn union_of_two_opaque_rects_culls() {
    // Neither cover alone contains the covered node, but together they tile it
    // completely: left half + right half. The subtraction-based coverage test
    // must recognise the union.
    let covered = solid(1, Color::new(0, 255, 0, 255), Rect::new(0.0, 0.0, 40.0, 20.0));
    let left = solid(2, Color::new(255, 0, 0, 255), Rect::new(0.0, 0.0, 20.0, 20.0));
    let right = solid(3, Color::new(0, 0, 255, 255), Rect::new(20.0, 0.0, 20.0, 20.0));
    let _ = render(&[covered, left, right]);
    assert!(
        was_culled(1),
        "node tiled completely by a union of later opaque rects should be culled"
    );
}

// ── (b) Non-opaque covers must NOT cull ──────────────────────────────────

#[test]
fn semi_transparent_cover_does_not_cull() {
    // A half-alpha red cover SAMPLES what is beneath, so the green below must
    // still be painted — and must be visible through the cover.
    let covered = solid(1, Color::new(0, 255, 0, 255), Rect::new(8.0, 8.0, 20.0, 20.0));
    let cover = solid(2, Color::new(255, 0, 0, 128), Rect::new(0.0, 0.0, 40.0, 40.0));

    let fb = render(&[covered, cover]);

    assert!(
        !was_culled(1),
        "a semi-transparent cover must NOT cull the node beneath it"
    );
    // The covered green contributes to the blended pixel: green channel is
    // non-zero where it would be pure red if the green had been (wrongly) culled.
    let p = fb.get_pixel(12, 12);
    assert!(
        p.g > 0,
        "green beneath the semi-transparent cover must show through (got {:?})",
        p
    );
}

#[test]
fn cover_with_reduced_opacity_does_not_cull() {
    // Same opaque color but the COVER node carries opacity < 1.0 → it samples
    // the backdrop, so it is not an occluder.
    let covered = solid(1, Color::new(0, 255, 0, 255), Rect::new(8.0, 8.0, 20.0, 20.0));
    let mut cover = solid(2, Color::new(255, 0, 0, 255), Rect::new(0.0, 0.0, 40.0, 40.0));
    cover.opacity = 0.5;

    let fb = render(&[covered, cover]);
    assert!(!was_culled(1), "opacity<1 cover must not cull");
    let p = fb.get_pixel(12, 12);
    assert!(p.g > 0, "green must show through the faded cover");
}

#[test]
fn rounded_cover_does_not_cull() {
    // A rounded opaque cover leaves its corners uncovered → not an occluder.
    let covered = solid(1, Color::new(0, 255, 0, 255), Rect::new(8.0, 8.0, 20.0, 20.0));
    let mut cover = solid(2, Color::new(255, 0, 0, 255), Rect::new(0.0, 0.0, 40.0, 40.0));
    cover.corner_radius = (12.0, 12.0, 12.0, 12.0);

    let _ = render(&[covered, cover]);
    assert!(
        !was_culled(1),
        "a rounded opaque cover must not cull (corners are not covered)"
    );
}

#[test]
fn glass_cover_does_not_cull() {
    // A Glass node samples/blurs what is beneath it — never an occluder.
    let covered = solid(1, Color::new(0, 255, 0, 255), Rect::new(8.0, 8.0, 20.0, 20.0));
    let cover = node(
        2,
        SceneNodeKind::Glass(GlassParams::default()),
        Rect::new(0.0, 0.0, 40.0, 40.0),
    );
    let _ = render(&[covered, cover]);
    assert!(!was_culled(1), "a glass cover must not cull the backdrop");
}

#[test]
fn image_cover_does_not_cull() {
    // An Image may have transparent texels / not fill its box → not an occluder.
    let covered = solid(1, Color::new(0, 255, 0, 255), Rect::new(8.0, 8.0, 20.0, 20.0));
    let cover = node(
        2,
        SceneNodeKind::Image {
            image_id: 7,
            width: 4,
            height: 4,
            fit: liquide_compositor::scene::ImageFit::Fill,
        },
        Rect::new(0.0, 0.0, 40.0, 40.0),
    );
    let _ = render(&[covered, cover]);
    assert!(!was_culled(1), "an image cover must not cull");
}

#[test]
fn backgroundfill_with_image_layer_does_not_cull() {
    // BackgroundFill that carries an image layer is not a guaranteed opaque
    // solid even if it also has an opaque color.
    let covered = solid(1, Color::new(0, 255, 0, 255), Rect::new(8.0, 8.0, 20.0, 20.0));
    let cover = node(
        2,
        SceneNodeKind::BackgroundFill {
            background: BackgroundSpec {
                color: Some(Color::new(255, 0, 0, 255)),
                image: Some(BackgroundImage::ImageId(9)),
                size: BackgroundSize::Auto,
                position: (0.0, 0.0),
                repeat: BackgroundRepeat::NoRepeat,
            },
        },
        Rect::new(0.0, 0.0, 40.0, 40.0),
    );
    let _ = render(&[covered, cover]);
    assert!(
        !was_culled(1),
        "a BackgroundFill with an image layer must not be treated as an opaque occluder"
    );
}

#[test]
fn opaque_backgroundfill_solid_culls() {
    // The positive control for BackgroundFill: a pure opaque solid (no image)
    // IS an occluder.
    let covered = solid(1, Color::new(0, 255, 0, 255), Rect::new(8.0, 8.0, 20.0, 20.0));
    let cover = node(
        2,
        SceneNodeKind::BackgroundFill {
            background: BackgroundSpec {
                color: Some(Color::new(255, 0, 0, 255)),
                image: None,
                size: BackgroundSize::Auto,
                position: (0.0, 0.0),
                repeat: BackgroundRepeat::NoRepeat,
            },
        },
        Rect::new(0.0, 0.0, 40.0, 40.0),
    );
    let _ = render(&[covered, cover]);
    assert!(
        was_culled(1),
        "a pure opaque BackgroundFill solid should cull the node beneath it"
    );
}

// ── (c) Partial coverage still paints ────────────────────────────────────

#[test]
fn partially_covered_node_is_still_painted() {
    // Cover only the top-left quadrant of the covered node. The uncovered part
    // must remain visible → the node must NOT be culled.
    let covered = solid(1, Color::new(0, 255, 0, 255), Rect::new(0.0, 0.0, 40.0, 40.0));
    let cover = solid(2, Color::new(255, 0, 0, 255), Rect::new(0.0, 0.0, 20.0, 20.0));

    let fb = render(&[covered, cover]);

    assert!(
        !was_culled(1),
        "a partially-covered node must still be painted"
    );
    // Covered quadrant → red; uncovered quadrant → the green is still visible.
    assert_eq!(fb.get_pixel(5, 5).g, 0, "covered region shows the cover");
    assert_eq!(
        fb.get_pixel(30, 30).g,
        255,
        "uncovered region must still show the (painted) covered node"
    );
}

#[test]
fn earlier_opaque_rect_does_not_cull_a_later_node() {
    // Z-ORDER MATTERS: an opaque rect painted BEFORE (below) a node cannot
    // occlude it. The later node is on top and must always paint.
    let cover_below = solid(1, Color::new(255, 0, 0, 255), Rect::new(0.0, 0.0, 40.0, 40.0));
    let on_top = solid(2, Color::new(0, 255, 0, 255), Rect::new(8.0, 8.0, 20.0, 20.0));

    let fb = render(&[cover_below, on_top]);

    assert!(
        !was_culled(2),
        "a node on TOP of an opaque rect must never be culled by it"
    );
    assert_eq!(
        fb.get_pixel(12, 12).g,
        255,
        "the top node must be visible"
    );
}

// ── Blend-mode state correctness ─────────────────────────────────────────

#[test]
fn cover_under_non_srcover_blend_layer_does_not_cull() {
    // A RenderLayer sets a non-SrcOver blend mode for subsequent nodes, so the
    // following opaque-colored rect composites with the backdrop instead of
    // replacing it → it is NOT an occluder.
    let covered = solid(1, Color::new(0, 255, 0, 255), Rect::new(8.0, 8.0, 20.0, 20.0));
    let layer = node(
        2,
        SceneNodeKind::RenderLayer {
            blend_mode: BlendMode::Multiply,
            isolate: false,
        },
        Rect::new(0.0, 0.0, 0.0, 0.0),
    );
    let cover = solid(3, Color::new(255, 0, 0, 255), Rect::new(0.0, 0.0, 40.0, 40.0));

    let _ = render(&[covered, layer, cover]);
    assert!(
        !was_culled(1),
        "a cover painted under a Multiply layer must not be treated as an occluder"
    );
}
