//! Pixel-identity (incremental == full) teeth for the group-opacity LayerScope.
//!
//! THE INVARIANT (disappear / artifact class): an INCREMENTAL frame (partial
//! damage composited onto the previous frame's framebuffer) MUST be
//! PIXEL-IDENTICAL to a FULL repaint of the same state. The static capture path
//! always uses FULL damage, so it looks clean; the bug only shows on the LIVE
//! incremental path where damage is a small box.
//!
//! `fix-isolation` wired CSS `opacity < 1` -> `RenderLayer { isolate: true }`, so
//! the offscreen group-opacity layer (snapshot backdrop -> CLEAR window ->
//! composite children -> merge ONCE at group opacity) now fires on nearly every
//! translucent element in the glass theme. If that snapshot/clear/merge does not
//! reproduce, on an incremental frame, the SAME pixels a full repaint produces,
//! the user sees trails / stale pixels / wrong compositing EVERYWHERE.
//!
//! These tests render the SAME scene twice through the real CPU rasterizer — once
//! FULL, once INCREMENTAL (partial damage over the prior full frame) — and assert
//! the two framebuffers are byte-identical inside the damage. RED before the fix
//! (the diff is the artifact), GREEN after. Teeth: shrinking the fix re-opens the
//! diff.

use liquide_compositor::damage::{DamageClass, DamageSet, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Affine2D, Rect};
use liquide_compositor::pixel::{BlendMode, Color, PixelFormat};
use liquide_compositor::scene::{FlatNode, GradientSpec, SceneNodeKind};

use crate::RenderMode;
use crate::renderer::SoftwareRenderer;

const W: u32 = 128;
const H: u32 = 128;
const TILE: u32 = 8;

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

/// A partial-damage set covering the pixel rect `[x0,x1) x [y0,y1)` (tile-aligned
/// the way the live worker expands a dirty rect to whole tiles).
fn partial_damage(x0: u32, y0: u32, x1: u32, y1: u32) -> DamageSet {
    let mut d = DamageSet::new(TILE);
    let tx0 = x0 / TILE;
    let ty0 = y0 / TILE;
    let tx1 = (x1 + TILE - 1) / TILE;
    let ty1 = (y1 + TILE - 1) / TILE;
    for ty in ty0..ty1 {
        for tx in tx0..tx1 {
            d.add(DamageTile {
                x: tx,
                y: ty,
                class: DamageClass::UiPrimitive,
            });
        }
    }
    d
}

/// Render `nodes` FULL onto a fresh frame (the authoritative reference).
fn render_full(rnd: &mut SoftwareRenderer, nodes: &[FlatNode]) -> FrameBuffer {
    let mut fb = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    let _ = rnd
        .render_live(nodes, &mut fb, &full_damage(), RenderMode::Capture)
        .unwrap();
    fb
}

/// Mirror of the production `expand_damage_for_group_layers` (lives in
/// `liquide-session`, which depends on this crate, so it cannot be called from
/// here): grow `damage` to the FULL tile bounds of every isolated `opacity < 1`
/// layer it touches. This is the FIX under test — driving it from the test keeps
/// the renderer-level pixel-identity check faithful to the production pipeline
/// (expand → clear → render → trim). `apply_fix = false` exercises the BUGGY
/// (un-expanded) path so the same scene goes RED, proving the teeth.
fn expand_damage_for_group_layers(damage: &mut DamageSet, nodes: &[FlatNode]) {
    if damage.tiles.is_empty() {
        return;
    }
    loop {
        let mut grew = false;
        for node in nodes {
            let SceneNodeKind::RenderLayer { isolate, .. } = node.kind.as_ref() else {
                continue;
            };
            if !*isolate || node.opacity >= 0.999 {
                continue;
            }
            let b = node.absolute_bounds;
            let tx0 = (b.x.max(0.0) as u32) / TILE;
            let ty0 = (b.y.max(0.0) as u32) / TILE;
            let tx1 = ((b.right().ceil() as u32).saturating_sub(1)) / TILE;
            let ty1 = ((b.bottom().ceil() as u32).saturating_sub(1)) / TILE;
            // Touches current damage?
            let touches = damage
                .tiles
                .iter()
                .any(|t| t.x >= tx0 && t.x <= tx1 && t.y >= ty0 && t.y <= ty1);
            if !touches {
                continue;
            }
            for ty in ty0..=ty1 {
                for tx in tx0..=tx1 {
                    if !damage.tiles.iter().any(|t| t.x == tx && t.y == ty) {
                        damage.add(DamageTile {
                            x: tx,
                            y: ty,
                            class: DamageClass::UiPrimitive,
                        });
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
}

/// Clear (zero) every pixel inside the damaged tiles — what the live worker's
/// `clear_damage_tiles` does before re-rastering, so partial damage has a clean
/// slate exactly like a fresh full frame.
fn clear_damage_tiles(fb: &mut FrameBuffer, damage: &DamageSet) {
    for t in &damage.tiles {
        let x0 = t.x * TILE;
        let y0 = t.y * TILE;
        for y in y0..(y0 + TILE).min(H) {
            for x in x0..(x0 + TILE).min(W) {
                fb.set_pixel(x, y, Color::new(0, 0, 0, 0));
            }
        }
    }
}

/// Render `nodes` INCREMENTALLY onto `base` (the previous frame) with `damage`,
/// modelling the production worker: optionally EXPAND damage for group layers
/// (the fix), CLEAR the damaged tiles, then re-raster. `apply_fix = false` skips
/// the expansion (the buggy path) so the teeth are provable on the same scene.
fn render_incremental_with(
    rnd: &mut SoftwareRenderer,
    base: &FrameBuffer,
    nodes: &[FlatNode],
    damage: &DamageSet,
    apply_fix: bool,
) -> (FrameBuffer, DamageSet) {
    let mut eff = damage.clone();
    if apply_fix {
        expand_damage_for_group_layers(&mut eff, nodes);
    }
    // Start from a copy of the previous frame's pixels — exactly the live path,
    // where the back buffer already holds the last presented frame.
    let mut fb = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    fb.pixels_mut().unwrap().copy_from_slice(base.pixels());
    clear_damage_tiles(&mut fb, &eff);
    let _ = rnd
        .render_live(nodes, &mut fb, &eff, RenderMode::Capture)
        .unwrap();
    (fb, eff)
}

/// The fixed incremental render (expansion applied) — used by the invariant tests.
fn render_incremental(
    rnd: &mut SoftwareRenderer,
    base: &FrameBuffer,
    nodes: &[FlatNode],
    damage: &DamageSet,
) -> FrameBuffer {
    render_incremental_with(rnd, base, nodes, damage, true).0
}

/// Count pixels where `a != b` INSIDE the damaged tiles (the region the
/// incremental frame is responsible for). Outside the damage both keep the prior
/// frame verbatim, so they are trivially equal and excluded.
fn diff_in_damage(a: &FrameBuffer, b: &FrameBuffer, damage: &DamageSet) -> usize {
    let mut diff = 0;
    for y in 0..H {
        for x in 0..W {
            let tx = x / TILE;
            let ty = y / TILE;
            let in_damage = damage
                .tiles
                .iter()
                .any(|t| t.x == tx && t.y == ty);
            if !in_damage {
                continue;
            }
            let off = a.pixel_offset(x, y);
            if a.pixels()[off..off + 4] != b.pixels()[off..off + 4] {
                diff += 1;
            }
        }
    }
    diff
}

/// Quiesce the async glyph atlas / blur worker so two renders of the same scene
/// paint identically (only matters for text, but harmless here).
fn quiesce(rnd: &mut SoftwareRenderer, nodes: &[FlatNode]) {
    for _ in 0..3 {
        let _ = render_full(rnd, nodes);
    }
}

/// Core check: build a scene with an `opacity < 1` isolated GROUP over `bg_nodes`
/// (background that must show THROUGH the group), render it FULL and then
/// INCREMENTALLY over a prior frame whose group region differs, and assert the
/// incremental frame is pixel-identical to the full repaint inside the damage.
fn assert_layer_incremental_matches_full(
    bg_nodes: Vec<FlatNode>,
    group_bounds: Rect,
    group_opacity: f32,
    children_a: Vec<FlatNode>,
    children_b: Vec<FlatNode>,
    damage: DamageSet,
) {
    let mut rnd = SoftwareRenderer::new();

    // Frame N (prior): background + group(children_a).
    let mut nodes_n = bg_nodes.clone();
    nodes_n.push(layer(900, group_bounds, group_opacity));
    nodes_n.extend(children_a);

    // Frame N+1: background + group(children_b) — the group content changed.
    let mut nodes_n1 = bg_nodes.clone();
    nodes_n1.push(layer(900, group_bounds, group_opacity));
    nodes_n1.extend(children_b);

    quiesce(&mut rnd, &nodes_n);
    quiesce(&mut rnd, &nodes_n1);

    let prev = render_full(&mut rnd, &nodes_n);
    let full = render_full(&mut rnd, &nodes_n1);
    let (incr, eff) = render_incremental_with(&mut rnd, &prev, &nodes_n1, &damage, true);

    // Pixel-identity inside the region the incremental frame is responsible for
    // (the expanded damage). A full repaint of frame N+1 is the ground truth.
    let diff = diff_in_damage(&full, &incr, &eff);
    assert_eq!(
        diff, 0,
        "incremental group-opacity frame differs from a full repaint in {diff} pixels \
         — stale/trail/wrong-composite artifact (incremental != full)"
    );
}

/// (a) opacity<1 group over OVERLAPPING translucent rects, partial damage.
#[test]
fn layer_over_overlapping_translucent_incremental_matches_full() {
    let white = Color::new(255, 255, 255, 255);
    let red = Color::new(255, 0, 0, 200);
    let blue = Color::new(0, 0, 255, 200);
    let green = Color::new(0, 200, 0, 200);

    let bgs = vec![bg(1, white, Rect::new(0.0, 0.0, W as f32, H as f32), 1.0)];
    // The group spans almost the whole frame so it extends well BEYOND the small
    // damaged region — this is the live case (a small interactive repaint inside a
    // large translucent group).
    let group = Rect::new(8.0, 8.0, 112.0, 112.0);

    // Two overlapping translucent children that SPAN the damage boundary, so the
    // damaged slice's merged content depends on a child that extends outside the
    // damage. Frame N+1 changes blue -> green.
    let a = vec![
        bg(10, red, Rect::new(16.0, 16.0, 64.0, 64.0), 1.0),
        bg(11, blue, Rect::new(48.0, 48.0, 64.0, 64.0), 1.0),
    ];
    let b = vec![
        bg(10, red, Rect::new(16.0, 16.0, 64.0, 64.0), 1.0),
        bg(11, green, Rect::new(48.0, 48.0, 64.0, 64.0), 1.0),
    ];
    // Damage covers only a small window in the OVERLAP region, far inside the group.
    let damage = partial_damage(56, 56, 72, 72);

    assert_layer_incremental_matches_full(bgs, group, 0.5, a, b, damage);
}

/// (b) opacity<1 group over a GRADIENT background (the gradient must show through
/// the dimmed group unchanged on an incremental frame).
#[test]
fn layer_over_gradient_incremental_matches_full() {
    let gradient = GradientSpec::Linear {
        start_x: 0.0,
        start_y: 0.0,
        end_x: 0.0,
        end_y: 1.0,
        stops: vec![
            (0.0, Color::new(255, 0, 0, 255)),
            (1.0, Color::new(0, 0, 255, 255)),
        ],
        repeating: false,
    };
    let bgs = vec![node(
        1,
        SceneNodeKind::GradientFill { gradient },
        Rect::new(0.0, 0.0, W as f32, H as f32),
        1.0,
    )];
    let group = Rect::new(16.0, 16.0, 96.0, 96.0);

    let white = Color::new(255, 255, 255, 220);
    let a = vec![bg(10, white, Rect::new(24.0, 24.0, 40.0, 40.0), 1.0)];
    let b = vec![bg(10, white, Rect::new(24.0, 24.0, 64.0, 64.0), 1.0)];
    let damage = partial_damage(24, 24, 88, 88);

    assert_layer_incremental_matches_full(bgs, group, 0.6, a, b, damage);
}

/// (c) opacity<1 group over content, with a hover/drag-style damage update that
/// only touches PART of the group window (the classic interactive repaint).
///
/// The group MOVES (drag): on frame N the group is at one position, on frame N+1
/// it has shifted, so the damage covers the leading + trailing edge. The trailing
/// edge must reveal the clean background (the group must un-paint there), and the
/// leading edge must show the group composited over the clean background — neither
/// over the STALE prior-frame group pixels.
#[test]
fn layer_drag_partial_damage_incremental_matches_full() {
    let white = Color::new(255, 255, 255, 255);
    let teal = Color::new(0, 180, 180, 255);

    let bgs = vec![bg(1, white, Rect::new(0.0, 0.0, W as f32, H as f32), 1.0)];

    // Frame N: group at x=24; frame N+1: group shifted to x=40 (a drag step). The
    // group window itself moves, so its open marker bounds differ between frames.
    let group_a = Rect::new(24.0, 24.0, 56.0, 56.0);
    let group_b = Rect::new(40.0, 24.0, 56.0, 56.0);
    let child_a = vec![bg(10, teal, Rect::new(28.0, 28.0, 48.0, 48.0), 1.0)];
    let child_b = vec![bg(10, teal, Rect::new(44.0, 28.0, 48.0, 48.0), 1.0)];

    let mut rnd = SoftwareRenderer::new();
    let mut nodes_n = bgs.clone();
    nodes_n.push(layer(900, group_a, 0.5));
    nodes_n.extend(child_a);
    let mut nodes_n1 = bgs.clone();
    nodes_n1.push(layer(900, group_b, 0.5));
    nodes_n1.extend(child_b);

    quiesce(&mut rnd, &nodes_n);
    quiesce(&mut rnd, &nodes_n1);
    let prev = render_full(&mut rnd, &nodes_n);
    let full = render_full(&mut rnd, &nodes_n1);

    // Damage covers the union of old + new group windows (the drag-dirtied span).
    let damage = partial_damage(24, 24, 96, 80);
    let (incr, eff) = render_incremental_with(&mut rnd, &prev, &nodes_n1, &damage, true);

    let diff = diff_in_damage(&full, &incr, &eff);
    assert_eq!(
        diff, 0,
        "dragged group-opacity frame differs from a full repaint in {diff} pixels \
         (trail/stale-backdrop artifact)"
    );
}

/// (e) STALE-BACKDROP probe: the group sits over a region that is NOT repainted by
/// any earlier node on the incremental frame (no full-screen background covers it),
/// so the layer's backdrop snapshot would capture the PRIOR frame's already-
/// composited group pixels instead of the clean backdrop. A full repaint clears
/// the whole frame first, so its backdrop there is transparent black; an
/// incremental frame that snapshots stale group pixels diverges.
#[test]
fn layer_over_unrepainted_backdrop_incremental_matches_full() {
    let red = Color::new(255, 0, 0, 200);
    let blue = Color::new(0, 0, 255, 200);
    let green = Color::new(0, 200, 0, 200);

    // NO full-screen background — the only opaque-ish content is the group's own
    // translucent children, so the group region's backdrop is the (transparent)
    // base everywhere a child does not paint.
    let bgs: Vec<FlatNode> = vec![];
    let group = Rect::new(8.0, 8.0, 112.0, 112.0);

    let a = vec![
        bg(10, red, Rect::new(16.0, 16.0, 64.0, 64.0), 1.0),
        bg(11, blue, Rect::new(48.0, 48.0, 64.0, 64.0), 1.0),
    ];
    let b = vec![
        bg(10, red, Rect::new(16.0, 16.0, 64.0, 64.0), 1.0),
        bg(11, green, Rect::new(48.0, 48.0, 64.0, 64.0), 1.0),
    ];
    let damage = partial_damage(56, 56, 72, 72);

    assert_layer_incremental_matches_full(bgs, group, 0.5, a, b, damage);
}

/// (f) THE ROOT-CAUSE REPRODUCTION + TEETH — sparse-damage GAP-tile escape.
///
/// The renderer's write-scissor is the damage BOUNDING BOX, but the live worker's
/// `clear_damage_tiles` clears only the actual damaged TILES. When damage is
/// SPARSE (two separated tiles with a GAP between them) and an `opacity < 1` group
/// layer spans both, the layer's snapshot/clear/merge — clamped to the BBOX —
/// writes into the GAP tile, which `clear_damage_tiles` left holding valid PRIOR
/// content. The merge composites the group over that stale gap content and
/// CORRUPTS it (the visible artifact). A full repaint has no gap (it repaints
/// everything), so the gap tile diverges.
///
/// FIX: `expand_damage_for_group_layers` grows the damage to the layer's full
/// bounds → the gap tile becomes damaged → cleared + repainted → identical to a
/// full repaint. TEETH: without the expansion, the gap tile diverges (RED).
#[test]
fn layer_sparse_damage_gap_tile_escape_repro_and_teeth() {
    let red = Color::new(255, 0, 0, 180);
    let yellow = Color::new(255, 255, 0, 180);

    // NO opaque backdrop covering the gap — this is the glass case: the translucent
    // group merges over whatever is behind it, so a gap tile that is never
    // re-rastered (not cleared, not covered) shows its STALE prior content through
    // the merge. (A full-screen opaque backdrop would self-heal the gap by
    // repainting it; the real artifact needs the gap's backdrop to be unrepainted.)
    let bgs: Vec<FlatNode> = vec![];
    // A wide translucent group strip spanning tile columns 1..14 on rows 4..6.
    let group = Rect::new(8.0, 32.0, 112.0, 16.0);

    // Frame N: a translucent red child fills the WHOLE strip (so the middle gap
    // tile holds composited red in the prior frame). Frame N+1: children ONLY at
    // the two ENDS (yellow) — the MIDDLE has NO child, so a correct frame leaves
    // the middle EMPTY (the strip's middle un-paints). The bug: the un-cleared,
    // un-repainted gap tile keeps the prior red because the layer restores its
    // STALE snapshot there.
    let a = vec![bg(10, red, Rect::new(8.0, 32.0, 112.0, 16.0), 1.0)];
    let b = vec![
        bg(11, yellow, Rect::new(8.0, 32.0, 24.0, 16.0), 1.0),
        bg(12, yellow, Rect::new(96.0, 32.0, 24.0, 16.0), 1.0),
    ];

    // SPARSE damage: only the two END tiles (left ~x[8,32), right ~x[96,120)),
    // with a GAP across the middle. Both lie on tile rows 4..6.
    let mut damage = DamageSet::new(TILE);
    for ty in 4..6u32 {
        for tx in [1u32, 2, 12, 13, 14] {
            damage.add(DamageTile {
                x: tx,
                y: ty,
                class: DamageClass::UiPrimitive,
            });
        }
    }

    let mut rnd = SoftwareRenderer::new();
    let mut nodes_n = bgs.clone();
    nodes_n.push(layer(900, group, 0.5));
    nodes_n.extend(a);
    let mut nodes_n1 = bgs.clone();
    nodes_n1.push(layer(900, group, 0.5));
    nodes_n1.extend(b);

    quiesce(&mut rnd, &nodes_n);
    quiesce(&mut rnd, &nodes_n1);
    let prev = render_full(&mut rnd, &nodes_n);
    let full = render_full(&mut rnd, &nodes_n1);

    // FIX path: expand → clear → render. Must be pixel-identical to a full repaint
    // across the whole layer region.
    let (incr, eff) = render_incremental_with(&mut rnd, &prev, &nodes_n1, &damage, true);
    let fixed_diff = diff_in_damage(&full, &incr, &eff);
    assert_eq!(
        fixed_diff, 0,
        "FIXED: incremental must equal a full repaint across the expanded layer region, \
         got {fixed_diff} differing pixels"
    );

    // TEETH: the buggy (un-expanded) path corrupts the GAP tiles. Measure over the
    // full layer region `eff`; the gap tiles (in the bbox between the two damaged
    // ends, but never cleared) get the layer merged over their stale prior content
    // and diverge from the full repaint.
    let (buggy, _) = render_incremental_with(&mut rnd, &prev, &nodes_n1, &damage, false);
    let buggy_diff = diff_in_damage(&full, &buggy, &eff);
    assert!(
        buggy_diff > 0,
        "TEETH: the un-expanded layer must corrupt the bbox GAP tiles (incremental != full), \
         but the frame matched — the fix would be untestable"
    );
}

/// (d) Determinism: the incremental render of a fixed scene is stable across runs
/// (no per-run divergence from the layer snapshot/merge).
#[test]
fn layer_incremental_is_deterministic() {
    let mut rnd = SoftwareRenderer::new();
    let white = Color::new(255, 255, 255, 255);
    let red = Color::new(255, 0, 0, 200);

    let mut nodes = vec![bg(1, white, Rect::new(0.0, 0.0, W as f32, H as f32), 1.0)];
    nodes.push(layer(900, Rect::new(16.0, 16.0, 96.0, 96.0), 0.5));
    nodes.push(bg(10, red, Rect::new(24.0, 24.0, 64.0, 64.0), 1.0));

    quiesce(&mut rnd, &nodes);
    let prev = render_full(&mut rnd, &nodes);
    let damage = partial_damage(24, 24, 88, 88);

    let first = render_incremental(&mut rnd, &prev, &nodes, &damage);
    for _ in 0..5 {
        let again = render_incremental(&mut rnd, &prev, &nodes, &damage);
        assert_eq!(
            first.content_hash(),
            again.content_hash(),
            "incremental group-opacity render must be deterministic"
        );
    }
}
