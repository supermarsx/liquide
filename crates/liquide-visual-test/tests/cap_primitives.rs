//! Capability tests — RENDERER PRIMITIVE fixes (test-harden, Part A.3–A.6).
//!
//! One test per audit-fix capability, each with a pixel golden AND a structural
//! tooth that fails if the capability regresses:
//!
//!   A.3 border-style gallery   — all 8 styles straight + rounded, distinct;
//!                                 dotted = round DOTS (gaps), not a solid band.
//!   A.4 group/isolate opacity  — overlapping translucent children in an isolated
//!                                 group composite ONCE (overlap NOT double-darkened).
//!   A.5 gradient smoothness     — a wide gradient spreads ink across many luma
//!                                 levels / per-row transitions (no banding).
//!   A.6 transform translate(%)  — a `%` translate lands at the size-relative
//!                                 offset (not collapsed to 0).
//!
//! Golden bless:
//!   `LIQUIDE_UPDATE_GOLDEN=1 cargo test -p liquide-visual-test --test cap_primitives`
//! Every golden here was RENDERED and visually INSPECTED before blessing.

use liquide_components::TemplateNode;
use liquide_compositor::Renderer;
use liquide_compositor::damage::{DamageClass, DamageSet};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Affine2D, Rect};
use liquide_compositor::pixel::{BlendMode, Color, PixelFormat};
use liquide_compositor::scene::{FlatNode, SceneNodeKind};
use liquide_renderer_cpu::SoftwareRenderer;

use liquide_visual_test::capture::Frame;
use liquide_visual_test::golden::assert_golden;
use liquide_visual_test::primitive_render::{distinct_luma_levels, render_fragment};

// ===========================================================================
// A.3 — Border-style gallery.
// ===========================================================================

const BORDER_STYLES: [&str; 8] = [
    "solid", "dashed", "dotted", "double", "groove", "ridge", "inset", "outset",
];

fn border_cell(x: i32, y: i32, style: &str, radius: &str) -> TemplateNode {
    TemplateNode::el("div")
        .style("position", "absolute")
        .style("left", &format!("{x}px"))
        .style("top", &format!("{y}px"))
        .style("width", "100px")
        .style("height", "60px")
        .style("border", &format!("8px {style} #ff5050"))
        .style("border-radius", radius)
        .style("background", "#283040")
}

/// Render the full 8-style x {straight, rounded} gallery.
fn render_border_gallery() -> Frame {
    let mut root = TemplateNode::el("div")
        .style("position", "absolute")
        .style("left", "0")
        .style("top", "0")
        .style("width", "100%")
        .style("height", "100%");
    for (i, s) in BORDER_STYLES.iter().enumerate() {
        let col = (i % 4) as i32;
        let row = (i / 4) as i32;
        let x = 20 + col * 130;
        let y = 20 + row * 160;
        root = root.child(border_cell(x, y, s, "0px")); // straight
        root = root.child(border_cell(x, y + 80, s, "16px")); // rounded
    }
    render_fragment(560, 360, "#101418", root)
}

/// Ink coverage along the TOP border band of a single straight cell at
/// `(x, y)` — counts border-colored (reddish) pixels in the 8px-tall top edge.
/// A solid border fills the whole band; a dotted border leaves GAPS (lower
/// coverage). Used to prove dotted ≠ solid.
fn top_border_ink(frame: &Frame, x: u32, y: u32) -> usize {
    let band = frame.crop(x, y, 100, 8);
    let mut n = 0;
    for px in band.rgba.chunks_exact(4) {
        // Reddish border color (#ff5050-ish), distinct from the #283040 fill.
        if px[0] > 150 && px[1] < 130 && px[2] < 130 {
            n += 1;
        }
    }
    n
}

/// A.3 — border styles render distinctly; dotted is round dots (gaps), and a
/// rounded dotted border is NOT downgraded to solid (au3 bugs #7/#8).
#[test]
fn border_style_gallery() {
    let frame = render_border_gallery();

    // Cell anchors (mirror render_border_gallery): solid is index 0, dotted is
    // index 2 — both on the top row (row 0), straight variant at y=20.
    // x = 20 + col*130, col = i%4.
    let solid_x = 20u32; // i=0, col 0
    let dotted_x = 20u32 + 2 * 130; // i=2, col 2
    let straight_y = 20u32;
    let rounded_y = 20u32 + 80; // rounded variant directly below

    let solid_ink = top_border_ink(&frame, solid_x, straight_y);
    let dotted_ink_straight = top_border_ink(&frame, dotted_x, straight_y);
    let dotted_ink_rounded = top_border_ink(&frame, dotted_x, rounded_y);

    // TOOTH 1: the solid border fills most of its top band.
    assert!(
        solid_ink > 500,
        "solid border top band has only {solid_ink} border pixels — the solid \
         style is not painting."
    );

    // TOOTH 2: the dotted border has GAPS — substantially LESS ink than solid.
    // A regression that draws dotted as a solid band (or squares filling the
    // band) would push this near solid_ink and fail.
    assert!(
        dotted_ink_straight > 60 && dotted_ink_straight < solid_ink * 3 / 4,
        "dotted border top band has {dotted_ink_straight} border pixels (solid \
         has {solid_ink}). Dotted must have GAPS (round dots) — if it equals \
         solid it is being drawn as a solid band (au3 bug #8 regressed)."
    );

    // TOOTH 3: the ROUNDED dotted border still draws dots (not silently
    // downgraded to a solid coverage fill — au3 bug #7). It must also have gaps.
    assert!(
        dotted_ink_rounded > 40 && dotted_ink_rounded < solid_ink * 3 / 4,
        "ROUNDED dotted border top band has {dotted_ink_rounded} border pixels \
         (solid straight has {solid_ink}). A rounded border must honour the \
         dotted style (dots with gaps), not downgrade to solid (au3 bug #7 \
         regressed)."
    );

    assert_golden("cap_border_styles", &frame);
}

// ===========================================================================
// A.4 — Group / isolate opacity: overlap composites once.
// ===========================================================================

fn scene_node(id: u64, kind: SceneNodeKind, bounds: Rect, opacity: f32) -> FlatNode {
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

/// Render a flat scene to a packed-RGBA [`Frame`] through the CPU renderer.
fn render_scene(nodes: &[FlatNode], w: u32, h: u32) -> Frame {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
    let damage = DamageSet::full(8, w.div_ceil(8), h.div_ceil(8), DamageClass::UiPrimitive);
    renderer.render(nodes, &mut fb, &damage).unwrap();
    // BGRA -> RGBA pack.
    let (wu, hu) = (w as usize, h as usize);
    let src = fb.pixels();
    let stride = wu * 4;
    let mut rgba = vec![0u8; wu * hu * 4];
    for y in 0..hu {
        for x in 0..wu {
            let s = &src[y * stride + x * 4..];
            let d = &mut rgba[(y * wu + x) * 4..];
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = s[3];
        }
    }
    Frame {
        width: w,
        height: h,
        rgba,
    }
}

/// A.4 — two overlapping translucent rects in an ISOLATED group composite as one
/// unit: the overlap equals a single group composite, NOT a double-darkened one.
///
/// Built at the SCENE level with a `RenderLayer { isolate: true }` carrying the
/// group opacity and OPAQUE children — the exact contract of the landed fix
/// (au3 bug #5). A CSS-driven group would also be ideal, but the scene-bridge's
/// `isolation:isolate` → RenderLayer translation is a separate (unfixed) wiring
/// gap; this golden pins the RENDERER capability the fix delivered, end-to-end
/// through `SoftwareRenderer` (a real golden the coordinator can eye-check).
#[test]
fn group_opacity_overlap_single_composite() {
    let (w, h) = (240u32, 160u32);
    let black = Color::new(0, 0, 0, 255);
    let white = Color::new(255, 255, 255, 255);

    let backdrop = scene_node(
        1,
        SceneNodeKind::Background { color: black },
        Rect::new(0.0, 0.0, w as f32, h as f32),
        1.0,
    );
    let layer = scene_node(
        2,
        SceneNodeKind::RenderLayer {
            blend_mode: BlendMode::SrcOver,
            isolate: true,
        },
        Rect::new(0.0, 0.0, w as f32, h as f32),
        0.5,
    );
    let rect_a = scene_node(
        3,
        SceneNodeKind::Background { color: white },
        Rect::new(20.0, 20.0, 120.0, 120.0),
        1.0,
    );
    let rect_b = scene_node(
        4,
        SceneNodeKind::Background { color: white },
        Rect::new(80.0, 20.0, 120.0, 120.0),
        1.0,
    );

    let frame = render_scene(&[backdrop, layer, rect_a, rect_b], w, h);

    // Sample a single-rect-only pixel and an overlap pixel.
    let single = frame.pixel(40, 80).expect("single px");
    let overlap = frame.pixel(110, 80).expect("overlap px");

    // White at 0.5 over black -> ~128. The overlap (two white rects merged in
    // the isolated layer, dimmed once) must ALSO be ~128.
    assert!(
        (single[0] as i32 - 128).abs() <= 4,
        "single-rect region must be a single 0.5 composite (~128): got {single:?}"
    );
    // THE TOOTH: the overlap must match the single composite (NOT the double-
    // composited ~192 the pre-fix per-node-alpha path produced).
    assert!(
        (overlap[0] as i32 - 128).abs() <= 4,
        "overlap is {overlap:?} but a single isolated-group composite must be \
         ~128. ~192 means the group double-composited the overlap (au3 bug #5 \
         regressed — the isolated RenderLayer offscreen merge is not applied)."
    );
    // Explicit anti-double-composite guard: overlap must be clearly darker than
    // the double-composite value (~192).
    assert!(
        overlap[0] < 160,
        "overlap channel {} is too bright — approaching the double-composite \
         (~192) value; group opacity is not isolating.",
        overlap[0]
    );

    assert_golden("cap_group_opacity", &frame);
}

/// A.4-CSS — the SAME single-composite contract, but driven END-TO-END through
/// the CSS path: a `<div style="opacity:0.5">` wrapping two overlapping OPAQUE
/// white children. This exercises the scene-bridge's CSS-`opacity` →
/// `RenderLayer{isolate:true}` wiring (the gap the scene-level golden above could
/// not cover): if the bridge does NOT emit an isolated layer the two children
/// double-composite and the overlap darkens to ~192 instead of the single-
/// composite ~128. RED before the wiring fix; GREEN after.
#[test]
fn group_opacity_overlap_single_composite_css() {
    let (w, h) = (240u32, 160u32);

    // opacity:0.5 group with two opaque white children overlapping in x[80,140).
    // Children are absolutely positioned so their geometry is exact.
    let group = TemplateNode::el("div")
        .style("position", "absolute")
        .style("left", "0")
        .style("top", "0")
        .style("width", "240px")
        .style("height", "160px")
        .style("opacity", "0.5")
        .child(
            TemplateNode::el("div")
                .style("position", "absolute")
                .style("left", "40px")
                .style("top", "40px")
                .style("width", "100px")
                .style("height", "80px")
                .style("background", "#ffffff"),
        )
        .child(
            TemplateNode::el("div")
                .style("position", "absolute")
                .style("left", "100px")
                .style("top", "40px")
                .style("width", "100px")
                .style("height", "80px")
                .style("background", "#ffffff"),
        );

    // Flat BLACK canvas so a single 0.5 white composite reads ~128.
    let frame = render_fragment(w, h, "#000000", group);

    // Single-child region (x[40,100)) and overlap region (x[100,140)).
    let single = frame.pixel(70, 80).expect("single-rect px");
    let overlap = frame.pixel(120, 80).expect("overlap px");

    assert!(
        (single[0] as i32 - 128).abs() <= 6,
        "single white-over-black at 0.5 must be ~128: got {single:?}"
    );
    assert!(
        (overlap[0] as i32 - 128).abs() <= 6,
        "overlap is {overlap:?} but a single isolated-group composite must be \
         ~128. ~192 means the CSS group double-composited the overlap — the \
         scene-bridge did NOT emit a RenderLayer{{isolate}} for `opacity:0.5`."
    );
    assert!(
        (overlap[0] as i32) < 170,
        "overlap r={} must be clearly below the ~192 double-composite value",
        overlap[0]
    );

    assert_golden("cap_group_opacity_css", &frame);
}

// ===========================================================================
// A.5 — Gradient smoothness (no banding).
// ===========================================================================

/// Count horizontal color transitions along the middle row — a smooth (dithered)
/// gradient changes color almost every column; a banded one holds long plateaus.
fn middle_row_transitions(frame: &Frame) -> usize {
    let mid = frame.height / 2;
    let mut transitions = 0;
    let mut prev = frame.pixel(0, mid).unwrap_or([0; 4]);
    for x in 1..frame.width {
        let p = frame.pixel(x, mid).unwrap_or([0; 4]);
        if p != prev {
            transitions += 1;
            prev = p;
        }
    }
    transitions
}

/// A.5 — a wide gradient is SMOOTH: ink spreads across many distinct luma levels
/// and the middle row transitions color frequently (au3 bug #3 dither fix).
#[test]
fn gradient_is_smooth_no_banding() {
    let grad = TemplateNode::el("div")
        .style("position", "absolute")
        .style("left", "0")
        .style("top", "0")
        .style("width", "100%")
        .style("height", "100%")
        .style("background", "linear-gradient(to right, #102040, #4080c0)");
    let frame = render_fragment(512, 120, "#000000", grad);

    let levels = distinct_luma_levels(&frame);
    let transitions = middle_row_transitions(&frame);

    // TOOTH 1: a smooth gradient spreads across many luma levels. A heavily
    // banded gradient (few flat plateaus) collapses to a handful.
    assert!(
        levels > 40,
        "gradient spans only {levels} distinct luma levels — that is BANDING. A \
         dithered/linear gradient spreads across many levels (au3 bug #3 \
         regressed)."
    );
    // TOOTH 2: over a 512px-wide gradient the middle row changes color in most
    // columns. Banding (wide plateaus) drops this far below the width.
    assert!(
        transitions > 150,
        "gradient middle row has only {transitions} color transitions over 512 \
         px — wide flat plateaus = banding (au3 bug #3 regressed)."
    );

    assert_golden("cap_gradient_smooth", &frame);
}

// ===========================================================================
// A.6 — transform: translate(%) lands at the size-relative offset.
// ===========================================================================

/// Compute the centroid (x, y) of green-ish ink in `frame`, or `None`.
fn green_centroid(frame: &Frame) -> Option<(u32, u32)> {
    let (mut sx, mut sy, mut n) = (0u64, 0u64, 0u64);
    for y in 0..frame.height {
        for x in 0..frame.width {
            let p = frame.pixel(x, y).unwrap_or([0; 4]);
            if p[1] > 150 && p[0] < 100 && p[2] < 100 {
                sx += x as u64;
                sy += y as u64;
                n += 1;
            }
        }
    }
    (n > 0).then(|| ((sx / n) as u32, (sy / n) as u32))
}

/// A.6 — a `transform: translate(100%, 100%)` on a fixed-size box lands at the
/// SIZE-RELATIVE offset (its own width/height), not collapsed to 0 (au3 bug #2 /
/// the style-engine percent-translate parse fix).
#[test]
fn transform_translate_percent_lands_size_relative() {
    // 50x50 green box at (20, 20), translated by translate(100%, 100%) = (50, 50).
    // Expected centre: (20 + 25 + 50, 20 + 25 + 50) = (95, 95). Without the fix
    // the percent is dropped and the box stays at centre (45, 45).
    let tx = TemplateNode::el("div")
        .style("position", "absolute")
        .style("left", "20px")
        .style("top", "20px")
        .style("width", "50px")
        .style("height", "50px")
        .style("background", "#00ff00")
        .style("transform", "translate(100%, 100%)");
    let frame = render_fragment(200, 200, "#000000", tx);

    let (cx, cy) = green_centroid(&frame).expect(
        "the green box must paint — if it is missing the element failed to render",
    );

    // TOOTH: the box centre must be near (95, 95) (size-relative translate), not
    // (45, 45) (percent dropped). Tolerate AA / rounding within a few px.
    assert!(
        (cx as i32 - 95).abs() <= 6 && (cy as i32 - 95).abs() <= 6,
        "translate(100%,100%) box centre is at ({cx},{cy}); expected ~(95,95) \
         (size-relative). ~(45,45) means the percent translate was dropped to 0 \
         (au3 bug #2 regressed — style-engine percent-translate parse)."
    );

    assert_golden("cap_transform_translate_percent", &frame);
}
