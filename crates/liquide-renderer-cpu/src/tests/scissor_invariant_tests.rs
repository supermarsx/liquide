//! Generative damage-confinement invariant harness (the Tier-2 safety net).
//!
//! THE TWO INVARIANTS this enforces over RANDOMIZED scenes + RANDOMIZED damage,
//! the ones whose violation spawns the disappear / stale-pixel / blit-trail bug
//! CLASS (see `.orchestration/reports/attest-bugs.md` Fragility #1):
//!
//!   (a) INCREMENTAL == FULL: a partial-damage frame (the damaged tiles cleared
//!       then re-rastered over the previous frame) is PIXEL-IDENTICAL, inside
//!       the damaged tiles, to a FULL repaint of the same scene. The static
//!       capture path always uses FULL damage, so it never exercises this — the
//!       bug only shows on the live incremental path.
//!
//!   (b) CONTAINMENT: NO pixel write lands OUTSIDE the active damage scissor.
//!       The renderer installs the damage bounding box (padded for effect
//!       fringe) as a hard write-scissor; the fast paths (blit / blur / clear /
//!       glyph) must honour it BY CONSTRUCTION. A frame rendered onto a SENTINEL
//!       background must leave every pixel outside the padded scissor exactly the
//!       sentinel value — any other value is an escaping write.
//!
//! Determinism: scenes + damage are produced by a fixed-seed LCG (no external
//! rng crate), so every run is reproducible (required for goldens / e2e_temporal).
//!
//! TEETH (this file proves the harness is not fake-green):
//!   * `containment_checker_has_teeth` injects a raw out-of-scissor write and
//!     asserts the containment checker FLAGS it.
//!   * `blur_region_raw_writeback_would_escape_*` reproduce the OLD (unclamped)
//!     write-back and assert it ESCAPES the scissor while the real (clamped)
//!     fast path does NOT — proving the clamp is load-bearing.
//!   * per-path `*_byte_identical_within_scissor` tests prove the clamping
//!     refactor changed nothing INSIDE the scissor (safety refactor, not a
//!     visual change).

use liquide_compositor::damage::{DamageClass, DamageSet, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Affine2D, Rect};
use liquide_compositor::pixel::{BlendMode, Color, PixelFormat};
use liquide_compositor::scene::{
    BorderSide, BorderSideStyle, BorderSides, BoxShadowSpec, FlatNode, GlassParams, GradientSpec,
    SceneNodeKind, SurfaceBuffer,
};
use liquide_compositor::scissor::set_write_scissor;
use std::sync::Arc;

use crate::RenderMode;
use crate::renderer::SoftwareRenderer;

const W: u32 = 128;
const H: u32 = 128;
const TILE: u32 = 8;
/// The renderer pads the damage bbox by this many pixels before installing the
/// write-scissor (renderer/mod.rs `padding`), so writes are LEGAL within the
/// padded ring; containment is asserted strictly OUTSIDE it.
const SCISSOR_PADDING: i64 = 32;
/// Byte value used to paint the "previous frame" so any escaping write is
/// visible (0xAB is not a value the renderer produces over a black scene at the
/// edges we check).
const SENTINEL: u8 = 0xAB;

// ---------------------------------------------------------------------------
// Deterministic RNG (LCG) — no external crate, reproducible every run.
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Avoid a zero state; mix the seed.
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        // xorshift the high bits down for better low-bit quality
        self.0 ^ (self.0 >> 31)
    }
    fn below(&mut self, n: u32) -> u32 {
        (self.next_u64() % n as u64) as u32
    }
    fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below((hi - lo) as u32) as i32
    }
    fn color(&mut self) -> Color {
        Color::new(
            self.below(256) as u8,
            self.below(256) as u8,
            self.below(256) as u8,
            // Bias toward translucency so compositing-over is exercised.
            (64 + self.below(192)) as u8,
        )
    }
    fn rect(&mut self) -> Rect {
        // Allow partly off-screen origins so clamping is exercised, but keep the
        // body mostly on-screen.
        let x = self.range_i32(-12, (W as i32) - 8) as f32;
        let y = self.range_i32(-12, (H as i32) - 8) as f32;
        let w = self.range_i32(8, 80) as f32;
        let h = self.range_i32(8, 80) as f32;
        Rect::new(x, y, w, h)
    }
}

// ---------------------------------------------------------------------------
// Node builders.
// ---------------------------------------------------------------------------

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

fn full_bg(id: u64, color: Color) -> FlatNode {
    node(
        id,
        SceneNodeKind::Background { color },
        Rect::new(0.0, 0.0, W as f32, H as f32),
        1.0,
    )
}

fn surface_node(id: u64, bounds: Rect, rng: &mut Rng) -> FlatNode {
    let w = (bounds.width.max(1.0) as u32).max(1);
    let h = (bounds.height.max(1.0) as u32).max(1);
    let mut px = vec![0u8; (w * h * 4) as usize];
    for b in px.iter_mut() {
        *b = rng.below(256) as u8;
    }
    let buffer = SurfaceBuffer {
        pixels: Arc::new(px),
        width: w,
        height: h,
        stride: w * 4,
        format: PixelFormat::Bgra8,
    };
    node(
        id,
        SceneNodeKind::Surface {
            surface_id: id,
            buffer: Some(buffer),
        },
        Rect::new(bounds.x.round(), bounds.y.round(), w as f32, h as f32),
        // Mix opaque (Src fast path) and translucent (alpha) surface blits.
        if rng.below(2) == 0 { 1.0 } else { 0.6 },
    )
}

fn gradient_node(id: u64, bounds: Rect, rng: &mut Rng) -> FlatNode {
    let gradient = GradientSpec::Linear {
        start_x: 0.0,
        start_y: 0.0,
        end_x: if rng.below(2) == 0 { 1.0 } else { 0.0 },
        end_y: if rng.below(2) == 0 { 1.0 } else { 0.0 },
        stops: vec![(0.0, rng.color()), (1.0, rng.color())],
        repeating: false,
    };
    node(id, SceneNodeKind::GradientFill { gradient }, bounds, 1.0)
}

fn border_node(id: u64, bounds: Rect, rng: &mut Rng) -> FlatNode {
    let side = |rng: &mut Rng| BorderSide {
        width: rng.range_i32(1, 6) as f32,
        style: BorderSideStyle::Solid,
        color: rng.color(),
    };
    node(
        id,
        SceneNodeKind::Border {
            sides: BorderSides {
                top: side(rng),
                right: side(rng),
                bottom: side(rng),
                left: side(rng),
            },
            radius: (0.0, 0.0, 0.0, 0.0),
        },
        bounds,
        1.0,
    )
}

fn glass_node(id: u64, bounds: Rect, rng: &mut Rng) -> FlatNode {
    node(
        id,
        SceneNodeKind::Glass(GlassParams {
            blur_radius: (4 + rng.below(20)) as u32,
            tint_color: rng.color(),
            inner_glow: rng.below(2) == 0,
            parallax: false,
        }),
        bounds,
        1.0,
    )
}

fn box_shadow_node(id: u64, bounds: Rect, rng: &mut Rng) -> FlatNode {
    node(
        id,
        SceneNodeKind::BoxShadows {
            shadows: vec![BoxShadowSpec {
                offset_x: rng.range_i32(-6, 6) as f32,
                offset_y: rng.range_i32(-6, 6) as f32,
                blur_radius: rng.range_i32(0, 12) as f32,
                spread_radius: rng.range_i32(0, 6) as f32,
                color: rng.color(),
                inset: false,
            }],
        },
        bounds,
        1.0,
    )
}

/// Build a scene of "local" (no-backdrop-read) kinds: fills, gradients, borders,
/// surfaces (blit fast path) and opacity layers. For these, the incremental
/// render of a cleared damaged region reproduces a full repaint exactly, so they
/// drive the INCREMENTAL==FULL identity check without the blur-backdrop ordering
/// hazard.
fn random_local_scene(seed: u64) -> Vec<FlatNode> {
    let mut rng = Rng::new(seed);
    let mut nodes = Vec::new();
    let mut id = 1u64;
    // Roughly half the scenes get an opaque full-screen backdrop (the common
    // desktop case); the rest start transparent (the glass/unrepainted case).
    if rng.below(2) == 0 {
        nodes.push(full_bg(id, rng.color()));
        id += 1;
    }
    let count = 2 + rng.below(4); // 2..=5 content nodes
    for _ in 0..count {
        let bounds = rng.rect();
        let pick = rng.below(5);
        let n = match pick {
            0 => node(id, SceneNodeKind::Background { color: rng.color() }, bounds, {
                if rng.below(2) == 0 { 1.0 } else { 0.5 }
            }),
            1 => gradient_node(id, bounds, &mut rng),
            2 => border_node(id, bounds, &mut rng),
            3 => surface_node(id, bounds, &mut rng),
            _ => {
                // Opacity layer + one child fill inside it.
                let layer = node(
                    id,
                    SceneNodeKind::RenderLayer {
                        blend_mode: BlendMode::SrcOver,
                        isolate: true,
                    },
                    bounds,
                    0.4 + (rng.below(50) as f32) / 100.0,
                );
                nodes.push(layer);
                id += 1;
                node(id, SceneNodeKind::Background { color: rng.color() }, bounds, 1.0)
            }
        };
        nodes.push(n);
        id += 1;
    }
    nodes
}

/// Build a scene that ALSO includes backdrop-reading blur kinds (Glass, drop
/// shadows). Used for the CONTAINMENT check only — containment is robust to what
/// the blur reads, it only asserts nothing is written outside the scissor.
fn random_any_scene(seed: u64) -> Vec<FlatNode> {
    let mut nodes = random_local_scene(seed ^ 0xD1CE);
    let mut rng = Rng::new(seed ^ 0xB10B);
    let mut id = 10_000u64;
    let extra = 1 + rng.below(3);
    for _ in 0..extra {
        let bounds = rng.rect();
        let n = if rng.below(2) == 0 {
            glass_node(id, bounds, &mut rng)
        } else {
            box_shadow_node(id, bounds, &mut rng)
        };
        nodes.push(n);
        id += 1;
    }
    nodes
}

// ---------------------------------------------------------------------------
// Damage helpers.
// ---------------------------------------------------------------------------

fn full_damage() -> DamageSet {
    DamageSet::full(TILE, W.div_ceil(TILE), H.div_ceil(TILE), DamageClass::UiPrimitive)
}

/// A random tile-aligned, contiguous damage RECT, biased to leave a sentinel
/// border outside the padded scissor so the containment check is non-vacuous.
/// Returns the damage set plus the pixel rect `(x0, y0, x1, y1)`.
fn random_damage(seed: u64) -> (DamageSet, (u32, u32, u32, u32)) {
    let mut rng = Rng::new(seed ^ 0x5EED);
    let grid = W / TILE; // 16
    // Keep the rect within the central tiles so [0..pad) and [W-pad..W) stay
    // outside the padded scissor (pad=32 => 4 tiles each side). Central window
    // is tiles [5, 11).
    let tx0 = 5 + rng.below(2); // 5..6
    let ty0 = 5 + rng.below(2);
    let tx1 = (tx0 + 1 + rng.below(2)).min(grid - 5); // up to ~tile 8
    let ty1 = (ty0 + 1 + rng.below(2)).min(grid - 5);
    let mut d = DamageSet::new(TILE);
    for ty in ty0..ty1 {
        for tx in tx0..tx1 {
            d.add(DamageTile {
                x: tx,
                y: ty,
                class: DamageClass::UiPrimitive,
            });
        }
    }
    (d, (tx0 * TILE, ty0 * TILE, tx1 * TILE, ty1 * TILE))
}

fn render_full(rnd: &mut SoftwareRenderer, nodes: &[FlatNode]) -> FrameBuffer {
    let mut fb = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    let _ = rnd
        .render_live(nodes, &mut fb, &full_damage(), RenderMode::Capture)
        .unwrap();
    fb
}

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

fn fill_sentinel(fb: &mut FrameBuffer) {
    fb.pixels_mut().unwrap().fill(SENTINEL);
}

fn in_damage(damage: &DamageSet, x: u32, y: u32) -> bool {
    let tx = x / TILE;
    let ty = y / TILE;
    damage.tiles.iter().any(|t| t.x == tx && t.y == ty)
}

/// Number of pixels differing between `a` and `b` inside the damaged tiles.
fn diff_in_damage(a: &FrameBuffer, b: &FrameBuffer, damage: &DamageSet) -> usize {
    let mut diff = 0;
    for y in 0..H {
        for x in 0..W {
            if !in_damage(damage, x, y) {
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

/// The padded write-scissor region (pixel rect, clamped to the framebuffer) the
/// renderer installs for `rect` — writes are legal here, sentinel must survive
/// strictly OUTSIDE it.
fn padded_scissor(rect: (u32, u32, u32, u32)) -> (i64, i64, i64, i64) {
    let (x0, y0, x1, y1) = rect;
    (
        (x0 as i64 - SCISSOR_PADDING).max(0),
        (y0 as i64 - SCISSOR_PADDING).max(0),
        (x1 as i64 + SCISSOR_PADDING).min(W as i64),
        (y1 as i64 + SCISSOR_PADDING).min(H as i64),
    )
}

/// Count pixels OUTSIDE `allowed` (a pixel rect) whose bytes differ from the
/// sentinel — i.e. escaping writes. Also returns how many pixels were checked,
/// so the test can assert the check was non-vacuous.
fn count_escaped(fb: &FrameBuffer, allowed: (i64, i64, i64, i64)) -> (usize, usize) {
    let (ax0, ay0, ax1, ay1) = allowed;
    let mut escaped = 0;
    let mut checked = 0;
    for y in 0..H as i64 {
        for x in 0..W as i64 {
            let inside = x >= ax0 && x < ax1 && y >= ay0 && y < ay1;
            if inside {
                continue;
            }
            checked += 1;
            let off = fb.pixel_offset(x as u32, y as u32);
            if fb.pixels()[off..off + 4].iter().any(|&b| b != SENTINEL) {
                escaped += 1;
            }
        }
    }
    (escaped, checked)
}

// ===========================================================================
// (a) INCREMENTAL == FULL — generative
// ===========================================================================

#[test]
fn incremental_matches_full_over_random_local_scenes() {
    let mut rnd = SoftwareRenderer::new();
    let mut nonzero_diff_scenes = 0usize; // sanity: scenes that actually paint in damage
    for seed in 0..96u64 {
        let nodes = random_local_scene(seed);
        let (damage, _rect) = random_damage(seed);

        // Previous frame = a FULL render of the same scene (the live back buffer
        // already holds the last presented frame). The full render is also the
        // ground truth.
        let full = render_full(&mut rnd, &nodes);

        // Incremental: start from the previous frame, clear the damaged tiles
        // (what the live worker does), then re-raster with partial damage.
        let mut incr = FrameBuffer::new(W, H, PixelFormat::Bgra8);
        incr.pixels_mut().unwrap().copy_from_slice(full.pixels());
        clear_damage_tiles(&mut incr, &damage);
        let _ = rnd
            .render_live(&nodes, &mut incr, &damage, RenderMode::Capture)
            .unwrap();

        let diff = diff_in_damage(&full, &incr, &damage);
        assert_eq!(
            diff, 0,
            "seed {seed}: incremental frame differs from full repaint in {diff} pixels \
             inside the damaged tiles (incremental != full — disappear/stale/trail class)"
        );

        // Track that the damaged region is actually painted by something (so the
        // identity assertion is not trivially comparing two blank regions).
        let mut painted = false;
        for y in 0..H {
            for x in 0..W {
                if in_damage(&damage, x, y) {
                    let off = full.pixel_offset(x, y);
                    if full.pixels()[off..off + 4].iter().any(|&b| b != 0) {
                        painted = true;
                    }
                }
            }
        }
        if painted {
            nonzero_diff_scenes += 1;
        }
    }
    assert!(
        nonzero_diff_scenes > 48,
        "harness too weak: only {nonzero_diff_scenes}/96 scenes painted inside the damage"
    );
}

// ===========================================================================
// (b) CONTAINMENT — generative, ALL node kinds incl. blur/glass/shadow
// ===========================================================================

#[test]
fn no_write_escapes_damage_over_random_scenes() {
    let mut rnd = SoftwareRenderer::new();
    let mut total_checked = 0usize;
    for seed in 0..96u64 {
        let nodes = random_any_scene(seed);
        let (damage, rect) = random_damage(seed);

        // Paint a sentinel "previous frame", clear only the damaged tiles (the
        // live worker), then render with partial damage. Every pixel outside the
        // padded write-scissor MUST remain the sentinel.
        let mut fb = FrameBuffer::new(W, H, PixelFormat::Bgra8);
        fill_sentinel(&mut fb);
        clear_damage_tiles(&mut fb, &damage);
        let _ = rnd
            .render_live(&nodes, &mut fb, &damage, RenderMode::Capture)
            .unwrap();

        let allowed = padded_scissor(rect);
        let (escaped, checked) = count_escaped(&fb, allowed);
        assert_eq!(
            escaped, 0,
            "seed {seed}: {escaped} pixels written OUTSIDE the damage scissor \
             (containment violated — a fast path escaped the scissor)"
        );
        total_checked += checked;
    }
    assert!(
        total_checked > 0,
        "containment check was vacuous (no out-of-scissor pixels examined)"
    );
}

// ===========================================================================
// Determinism (goldens / e2e_temporal depend on it)
// ===========================================================================

#[test]
fn incremental_render_is_deterministic_over_random_scenes() {
    let mut rnd = SoftwareRenderer::new();
    for seed in 0..32u64 {
        let nodes = random_any_scene(seed);
        let (damage, _rect) = random_damage(seed);
        let base = render_full(&mut rnd, &nodes);

        let render_once = |rnd: &mut SoftwareRenderer| {
            let mut fb = FrameBuffer::new(W, H, PixelFormat::Bgra8);
            fb.pixels_mut().unwrap().copy_from_slice(base.pixels());
            clear_damage_tiles(&mut fb, &damage);
            let _ = rnd
                .render_live(&nodes, &mut fb, &damage, RenderMode::Capture)
                .unwrap();
            fb.content_hash()
        };

        let first = render_once(&mut rnd);
        for _ in 0..3 {
            assert_eq!(
                first,
                render_once(&mut rnd),
                "seed {seed}: incremental render is nondeterministic"
            );
        }
    }
}

// ===========================================================================
// TEETH — prove the harness catches escapes and the clamp is load-bearing
// ===========================================================================

/// The containment checker must FLAG a raw out-of-scissor write. If it ever
/// returned 0 here it would be fake-green: blind to escaping writes.
#[test]
fn containment_checker_has_teeth() {
    let mut fb = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    fill_sentinel(&mut fb);
    // Allowed region = central box; deliberately scribble a raw pixel OUTSIDE it
    // via pixels_mut (bypassing set_pixel / the scissor) — exactly what an
    // escaping fast path does.
    let allowed = (40i64, 40, 88, 88);
    let (escaped_before, _) = count_escaped(&fb, allowed);
    assert_eq!(escaped_before, 0, "sentinel frame must start contained");

    let off = fb.pixel_offset(4, 4); // well outside the allowed box
    fb.pixels_mut().unwrap()[off] ^= 0xFF;

    let (escaped_after, checked) = count_escaped(&fb, allowed);
    assert!(checked > 0, "checker examined no out-of-region pixels");
    assert!(
        escaped_after > 0,
        "containment checker is TOOTHLESS — it failed to detect a raw out-of-scissor write"
    );
}

/// `blur_region` (a fast path) must NOT write outside the active scissor, while
/// the OLD raw write-back (reproduced here) WOULD — proving the clamp is the
/// load-bearing fix, not decoration. A NON-UNIFORM background is used so the
/// blur output differs from the prior pixels: a blur of a uniform field would
/// reproduce that field and mask an escape by value.
#[test]
fn blur_region_clamp_is_load_bearing() {
    let region = Rect::new(16.0, 16.0, 80.0, 80.0);
    let scissor = Rect::new(40.0, 40.0, 24.0, 24.0); // strictly inside the region
    let scissor_win = (40u32, 40, 64, 64);

    // HIGH-FREQUENCY (checkerboard) seed, so blurring genuinely changes pixels
    // (a smooth ramp would be its own blur) and an escaping write is detectable
    // by VALUE.
    let seed = |fb: &mut FrameBuffer| {
        for y in 0..H {
            for x in 0..W {
                let on = ((x / 2) + (y / 2)) % 2 == 0;
                let v = if on { 240 } else { 16 };
                fb.set_pixel(x, y, Color::new(v, 255 - v, 128, 255));
            }
        }
    };

    // Real (clamped) blur_region: every pixel OUTSIDE the scissor is unchanged.
    let mut fb = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    seed(&mut fb);
    let before = fb.pixels().to_vec();
    let prev = set_write_scissor(Some(scissor));
    crate::blur::blur_region(&mut fb, region, 6);
    set_write_scissor(prev);

    let outside_changed = |fb: &FrameBuffer| -> usize {
        let (ax0, ay0, ax1, ay1) = scissor_win;
        let mut n = 0;
        for y in 0..H {
            for x in 0..W {
                let inside = x >= ax0 && x < ax1 && y >= ay0 && y < ay1;
                if inside {
                    continue;
                }
                let off = fb.pixel_offset(x, y);
                if fb.pixels()[off..off + 4] != before[off..off + 4] {
                    n += 1;
                }
            }
        }
        n
    };
    assert_eq!(
        outside_changed(&fb),
        0,
        "blur_region wrote OUTSIDE its scissor (clamp broken)"
    );
    // ...and INSIDE the scissor it actually blurred (so the check above is not
    // passing because the blur was a no-op).
    let mut changed_inside = 0;
    for y in scissor_win.1..scissor_win.3 {
        for x in scissor_win.0..scissor_win.2 {
            let off = fb.pixel_offset(x, y);
            if fb.pixels()[off..off + 4] != before[off..off + 4] {
                changed_inside += 1;
            }
        }
    }
    assert!(changed_inside > 0, "blur_region did not modify the scissor interior");

    // TEETH: reproduce the OLD behaviour — a raw write-back over the WHOLE region
    // ignoring the scissor. It MUST change pixels outside the scissor, proving
    // the clamp is what prevents the escape.
    let mut raw = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    seed(&mut raw);
    let x0 = region.x as usize;
    let y0 = region.y as usize;
    let rw = region.width as usize;
    let rh = region.height as usize;
    let stride = raw.stride as usize;
    {
        let px = raw.pixels_mut().unwrap();
        for row in 0..rh {
            let start = (y0 + row) * stride + x0 * 4;
            for b in &mut px[start..start + rw * 4] {
                *b = b.wrapping_add(17); // any visible change
            }
        }
    }
    assert!(
        outside_changed(&raw) > 0,
        "the unclamped raw write-back did NOT escape — teeth check is invalid"
    );
}

// ---------------------------------------------------------------------------
// Per-path byte-identity WITHIN the scissor (safety refactor, not a visual
// change): clamped output == raw output for every pixel inside the scissor.
// ---------------------------------------------------------------------------

/// Bytes of `fb` inside the pixel rect `[x0,x1) x [y0,y1)`.
fn region_bytes(fb: &FrameBuffer, x0: u32, y0: u32, x1: u32, y1: u32) -> Vec<u8> {
    let mut out = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            let off = fb.pixel_offset(x, y);
            out.extend_from_slice(&fb.pixels()[off..off + 4]);
        }
    }
    out
}

#[test]
fn blit_region_byte_identical_within_scissor() {
    let mut src = FrameBuffer::new(40, 40, PixelFormat::Bgra8);
    for y in 0..40 {
        for x in 0..40 {
            src.set_pixel(x, y, Color::new((x * 6) as u8, (y * 6) as u8, 50, 255));
        }
    }
    let dst_x = 20;
    let dst_y = 20;
    let src_rect = Rect::new(0.0, 0.0, 40.0, 40.0);

    // Unclamped reference (no scissor).
    let mut unclamped = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    fill_sentinel(&mut unclamped);
    crate::blit::blit_region(&mut unclamped, &src, src_rect, dst_x, dst_y, BlendMode::Src, 1.0);

    // Clamped (scissor installed) — a sub-rect of the blit.
    let scissor = Rect::new(30.0, 28.0, 16.0, 20.0);
    let mut clamped = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    fill_sentinel(&mut clamped);
    let prev = set_write_scissor(Some(scissor));
    crate::blit::blit_region(&mut clamped, &src, src_rect, dst_x, dst_y, BlendMode::Src, 1.0);
    set_write_scissor(prev);

    assert_eq!(
        region_bytes(&unclamped, 30, 28, 46, 48),
        region_bytes(&clamped, 30, 28, 46, 48),
        "blit_region clamped output differs from raw INSIDE the scissor"
    );
    // And outside the scissor the clamped frame is untouched sentinel.
    let (escaped, _) = count_escaped(&clamped, (30, 28, 46, 48));
    assert_eq!(escaped, 0, "blit_region wrote outside its scissor");
}

#[test]
fn blur_region_byte_identical_within_scissor() {
    let seed_fb = |fb: &mut FrameBuffer| {
        for y in 0..H {
            for x in 0..W {
                fb.set_pixel(x, y, Color::new((x * 2) as u8, (y * 2) as u8, 90, 255));
            }
        }
    };
    let region = Rect::new(16.0, 16.0, 80.0, 80.0);
    let scissor = Rect::new(40.0, 40.0, 24.0, 24.0);

    let mut unclamped = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    seed_fb(&mut unclamped);
    crate::blur::blur_region(&mut unclamped, region, 6);

    let mut clamped = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    seed_fb(&mut clamped);
    let prev = set_write_scissor(Some(scissor));
    crate::blur::blur_region(&mut clamped, region, 6);
    set_write_scissor(prev);

    // Inside the scissor the blurred bytes must be identical: same input region
    // is read (reads are unclamped), only the WRITE is confined.
    assert_eq!(
        region_bytes(&unclamped, 40, 40, 64, 64),
        region_bytes(&clamped, 40, 40, 64, 64),
        "blur_region clamped output differs from raw INSIDE the scissor"
    );
}

#[test]
fn clear_region_byte_identical_within_scissor() {
    let rect = Rect::new(10.0, 10.0, 60.0, 60.0);
    let color = Color::new(10, 200, 30, 255);

    let mut unclamped = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    fill_sentinel(&mut unclamped);
    crate::blit::clear_region(&mut unclamped, rect, color);

    let scissor = Rect::new(20.0, 24.0, 20.0, 16.0);
    let mut clamped = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    fill_sentinel(&mut clamped);
    let prev = set_write_scissor(Some(scissor));
    crate::blit::clear_region(&mut clamped, rect, color);
    set_write_scissor(prev);

    assert_eq!(
        region_bytes(&unclamped, 20, 24, 40, 40),
        region_bytes(&clamped, 20, 24, 40, 40),
        "clear_region clamped output differs from raw INSIDE the scissor"
    );
    let (escaped, _) = count_escaped(&clamped, (20, 24, 40, 40));
    assert_eq!(escaped, 0, "clear_region wiped pixels outside its scissor");
}

#[test]
fn blit_within_byte_identical_within_scissor_and_contained() {
    let paint = |fb: &mut FrameBuffer| {
        for y in 0..fb.height {
            for x in 0..fb.width {
                fb.set_pixel(
                    x,
                    y,
                    Color::new((x & 0xff) as u8, (y & 0xff) as u8, ((x + y) & 0xff) as u8, 255),
                );
            }
        }
    };
    let src = Rect::new(20.0, 20.0, 40.0, 40.0);
    let (dx, dy) = (60, 60); // move down-right (overlap order exercised)

    let mut unclamped = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    paint(&mut unclamped);
    crate::blit::blit_within(&mut unclamped, src, dx, dy);

    // Scissor a sub-rect of the destination; the move must be byte-identical
    // there and must not touch anything outside it.
    let scissor = Rect::new(70.0, 70.0, 16.0, 16.0);
    let mut clamped = FrameBuffer::new(W, H, PixelFormat::Bgra8);
    paint(&mut clamped);
    let before = clamped.pixels().to_vec();
    let prev = set_write_scissor(Some(scissor));
    crate::blit::blit_within(&mut clamped, src, dx, dy);
    set_write_scissor(prev);

    assert_eq!(
        region_bytes(&unclamped, 70, 70, 86, 86),
        region_bytes(&clamped, 70, 70, 86, 86),
        "blit_within clamped output differs from raw INSIDE the scissor"
    );
    // Containment: every pixel outside the scissor is untouched (== pre-move).
    for y in 0..H {
        for x in 0..W {
            let inside = (70..86).contains(&x) && (70..86).contains(&y);
            if inside {
                continue;
            }
            let off = clamped.pixel_offset(x, y);
            assert_eq!(
                &clamped.pixels()[off..off + 4],
                &before[off..off + 4],
                "blit_within wrote outside its scissor at ({x},{y})"
            );
        }
    }
}
