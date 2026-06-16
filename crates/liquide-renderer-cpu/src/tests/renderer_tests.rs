use crate::glyph::{GlyphKey, GlyphMetrics};
use crate::renderer::*;
use liquide_compositor::damage::DamageClass;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{Color, PixelFormat};
use liquide_compositor::{FrameMemoryKind, RendererBackendKind, RendererRejectReason};

use liquide_compositor::damage::{DamageSet, DamageTile};
use liquide_compositor::framebuffer::{FrameBuffer, FrameMemory};
use liquide_compositor::scene::{FlatNode, SceneNodeKind};
use liquide_font_rasterizer::database::FontDatabase;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn text_node(text: &str, font_family: &str) -> FlatNode {
    FlatNode {
        id: 1_000,
        kind: SceneNodeKind::Text {
            text: text.to_string(),
            color: Color::WHITE,
            scale: 1,
            font_family: font_family.to_string(),
            font_size: 16.0,
            font_weight: 400,
            font_style_italic: false,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            line_height: 0.0,
            text_align: 0,
            text_transform: 0,
            text_overflow: 0,
            white_space: 0,
            word_break: liquide_compositor::scene::WordBreak::Normal,
            text_indent: 0.0,
            text_decoration: None,
            text_shadows: Vec::new(),
            text_emphasis: None,
        }
        .into(),
        absolute_bounds: Rect::new(0.0, 0.0, 128.0, 32.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    }
}

fn render_text_once(renderer: &mut SoftwareRenderer, node: FlatNode) {
    let mut fb = FrameBuffer::new(128, 64, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::TextGlyph,
    });
    renderer.render(&[node], &mut fb, &damage).unwrap();
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "liquide-renderer-cpu-{label}-{}-{unique}",
        std::process::id()
    ))
}

fn fixture_font_bytes() -> Option<Vec<u8>> {
    let candidates = [
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "C:\\Windows\\Fonts\\calibri.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        "/Library/Fonts/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ];

    candidates.iter().find_map(|path| {
        let data = std::fs::read(path).ok()?;
        let mut db = FontDatabase::new();
        db.load_bytes(data.clone(), "Probe", 400, false).ok()?;
        Some(data)
    })
}

fn write_fixture_font(label: &str) -> Option<(PathBuf, PathBuf)> {
    let data = fixture_font_bytes()?;
    let dir = unique_temp_dir(label);
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("fixture.ttf");
    std::fs::write(&path, data).ok()?;
    Some((dir, path))
}

/// A solid opaque Background node covering `bounds`.
fn bg_node(id: u64, bounds: Rect, color: Color) -> FlatNode {
    FlatNode {
        id,
        kind: SceneNodeKind::Background { color }.into(),
        absolute_bounds: bounds,
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    }
}

/// t76: a sparse damage set must clip every node's fill to the damaged region —
/// a full-surface Background painted under single-tile damage only touches that
/// tile (plus the small effect-bleed padding); tiles well outside the damaged
/// region stay untouched (transparent black). The damage bbox carries a 32px
/// padding for blur/shadow bleed, so we assert on tiles ≥1 full tile beyond it.
#[test]
fn damage_clipped_raster_only_writes_damaged_tiles() {
    let mut renderer = SoftwareRenderer::new();
    let tile = 64u32;
    let (w, h) = (320u32, 320u32); // 5x5 tiles
    let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);

    // Damage exactly the center tile (2,2).
    let (dtx, dty) = (2u32, 2u32);
    let mut damage = DamageSet::new(tile);
    damage.mark_tile_with_class(dtx, dty, DamageClass::UiPrimitive);

    let node = bg_node(
        1,
        Rect::new(0.0, 0.0, w as f32, h as f32),
        Color::new(255, 0, 0, 255),
    );
    renderer
        .render_live(&[node], &mut fb, &damage, RenderMode::LiveCursor)
        .unwrap();

    // Inside the damaged tile -> red.
    let inside = fb.get_pixel(dtx * tile + 10, dty * tile + 10);
    assert_eq!(inside.r, 255, "damaged tile must be painted: {inside:?}");

    // Tiles ≥1 full tile beyond the padded damage bbox must stay untouched.
    // Damaged tile spans [128,192); padded clip is [96,224). Tiles (0,*),(4,*),
    // (*,0),(*,4) sample at offset 10 -> coords ≤74 or ≥266, all outside [96,224).
    for (tx, ty) in [
        (0u32, 0u32),
        (4, 0),
        (0, 4),
        (4, 4),
        (0, 2),
        (4, 2),
        (2, 0),
        (2, 4),
    ] {
        let p = fb.get_pixel(tx * tile + 10, ty * tile + 10);
        assert_eq!(
            p.r, 0,
            "tile ({tx},{ty}) outside padded damage must be untouched, got {p:?}"
        );
    }
}

/// t76: full damage must fall back to a whole-surface raster (clip = None).
#[test]
fn full_damage_rasters_whole_surface() {
    let mut renderer = SoftwareRenderer::new();
    let tile = 64u32;
    let (w, h) = (256u32, 256u32);
    let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
    let damage = DamageSet::full(tile, w / tile, h / tile, DamageClass::UiPrimitive);

    let node = bg_node(1, Rect::new(0.0, 0.0, w as f32, h as f32), Color::new(0, 255, 0, 255));
    renderer
        .render_live(&[node], &mut fb, &damage, RenderMode::LiveCursor)
        .unwrap();

    // Corners and center all painted.
    for (x, y) in [(2, 2), (w - 2, 2), (2, h - 2), (w - 2, h - 2), (w / 2, h / 2)] {
        assert_eq!(fb.get_pixel(x, y).g, 255, "full damage must paint ({x},{y})");
    }
}

/// t80 ANTI-FAKE-GREEN no-escape test for the damage write-scissor.
///
/// REGRESSION (t79): the per-frame `raster_clip` was honoured by only SOME node
/// kinds (Background / Surface / Glass-tint / Tint / Text). The full-bleed kinds
/// — Image, BackgroundFill (wallpaper), the backdrop-filter write, and Shadow —
/// IGNORED the clip and overpainted the whole screen on a partial-damage frame,
/// wiping preserved framebuffer content that was never repainted → a permanent
/// hole (hovering a context-menu item with `backdrop-filter: blur` triggered it).
///
/// The existing clip tests only cover the already-clipped kinds, so they pass
/// while the bug is live — a fake-green gap. This test paints a SMALL partial
/// `raster_clip` (one center tile) under a stack of FULL-BLEED nodes that each
/// previously escaped the clip — BackgroundFill, Image, a BackdropFilter, AND a
/// Shadow — over a pre-filled framebuffer, then asserts every pixel OUTSIDE the
/// padded clip rect is BYTE-FOR-BYTE unchanged. Against the pre-fix code at
/// least one of these nodes overwrites the sentinel outside the clip and the
/// assertion fails; after the write-scissor fix every write is confined to the
/// damage rect and the sentinel survives.
#[test]
fn partial_clip_full_bleed_nodes_never_write_outside_damage() {
    use liquide_compositor::scene::{
        BackdropFilterSpec, BackgroundRepeat, BackgroundSize, BackgroundSpec, ImageFit,
    };

    let tile = 64u32;
    let (w, h) = (320u32, 320u32); // 5x5 tiles
    let full = Rect::new(0.0, 0.0, w as f32, h as f32);

    // Sentinel the whole framebuffer with a distinctive opaque colour. Any
    // out-of-clip write by a full-bleed node will change one of these bytes.
    let sentinel = Color::new(17, 71, 137, 255);
    let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
    fb.clear(sentinel);
    let baseline = fb.pixels().to_vec();

    // Damage exactly the center tile (2,2): a small partial clip, NOT full.
    let (dtx, dty) = (2u32, 2u32);
    let mut damage = DamageSet::new(tile);
    damage.mark_tile_with_class(dtx, dty, DamageClass::UiPrimitive);

    let mk = |id: u64, kind: SceneNodeKind| FlatNode {
        id,
        kind: kind.into(),
        absolute_bounds: full,
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    // Every node spans the FULL framebuffer, so none is culled by the damage
    // bbox (they all intersect it) — exactly the t79 condition.
    let nodes = vec![
        // Full-screen wallpaper: solid BackgroundFill (the t79 overpainter).
        mk(
            1,
            SceneNodeKind::BackgroundFill {
                background: BackgroundSpec {
                    color: Some(Color::new(200, 30, 30, 255)),
                    image: None,
                    size: BackgroundSize::Cover,
                    position: (0.0, 0.0),
                    repeat: BackgroundRepeat::NoRepeat,
                },
            },
        ),
        // Full-screen Image (no texture loaded -> placeholder fill, still a
        // full-bleed write that previously ignored the clip).
        mk(
            2,
            SceneNodeKind::Image {
                image_id: 0xDEAD_BEEF,
                width: w,
                height: h,
                fit: ImageFit::Fill,
            },
        ),
        // Full-screen backdrop filter (the context-menu panel's class): an
        // in-place brightness write over the whole bounds.
        mk(
            3,
            SceneNodeKind::BackdropFilter {
                filters: vec![BackdropFilterSpec::Brightness(1.5)],
            },
        ),
        // Full-screen drop shadow: composited mask that previously ignored clip.
        mk(
            4,
            SceneNodeKind::Shadow {
                spread: 0.0,
                blur_radius: 8.0,
                color: Color::new(0, 0, 0, 200),
                corner_radius: 0.0,
            },
        ),
    ];

    // LiveCursor avoids any glyph-drain wait; the damage is partial so the
    // renderer installs the write-scissor (the path under test).
    let mut renderer = SoftwareRenderer::new();
    renderer
        .render_live(&nodes, &mut fb, &damage, RenderMode::LiveCursor)
        .unwrap();

    // The damage bbox carries 32px effect-bleed padding. The damaged tile spans
    // [128,192); the padded clip is [96,224). Every pixel strictly outside that
    // padded rect must be byte-identical to the sentinel baseline.
    let clip_x0 = 96u32;
    let clip_y0 = 96u32;
    let clip_x1 = 224u32;
    let clip_y1 = 224u32;
    let pixels = fb.pixels();
    let stride = fb.stride as usize;
    let mut escapes = 0usize;
    let mut first: Option<(u32, u32)> = None;
    for y in 0..h {
        for x in 0..w {
            let inside_clip =
                x >= clip_x0 && x < clip_x1 && y >= clip_y0 && y < clip_y1;
            if inside_clip {
                continue;
            }
            let off = y as usize * stride + x as usize * 4;
            if pixels[off..off + 4] != baseline[off..off + 4] {
                escapes += 1;
                first.get_or_insert((x, y));
            }
        }
    }
    assert_eq!(
        escapes, 0,
        "{escapes} pixel(s) outside the partial damage clip were overwritten by a \
         full-bleed node (first at {first:?}); the write-scissor must confine \
         Image/BackgroundFill/BackdropFilter/Shadow to the damage rect (t79/t80)"
    );

    // Sanity: the damaged tile itself WAS painted (the fix must not over-clip and
    // leave the damage region blank).
    let inside = fb.get_pixel(dtx * tile + 10, dty * tile + 10);
    assert_ne!(
        [inside.r, inside.g, inside.b, inside.a],
        [sentinel.r, sentinel.g, sentinel.b, sentinel.a],
        "the damaged tile must still be painted inside the clip"
    );
}

#[test]
fn renderer_creates() {
    let r = SoftwareRenderer::new();
    assert!(r.glyph_atlas().is_empty());
}

/// `compute_font_id` must map distinct (family, weight, italic) tuples to
/// distinct ids — otherwise two different fonts share a glyph-atlas key and the
/// cache returns the WRONG glyph (the text-garbling regression). This pins the
/// collision-free property the renderer relies on, including the weight bits
/// that the old `(weight & 0xFF) << 16` packing truncated and aliased.
#[test]
fn font_id_is_collision_free_across_family_weight_italic() {
    use std::collections::HashSet;

    let families = ["", "Inter", "Segoe UI", "Inter Display", "Arial"];
    let weights: [u16; 9] = [100, 200, 300, 400, 500, 600, 700, 800, 900];
    let mut ids = HashSet::new();
    for fam in families {
        for w in weights {
            for italic in [false, true] {
                let id = compute_font_id(fam, w, italic);
                assert!(
                    ids.insert(id),
                    "font_id collision: ({fam:?}, {w}, italic={italic}) aliased to an \
                     existing id {id:#010x} — distinct fonts would share atlas keys and \
                     return wrong glyphs"
                );
            }
        }
    }

    // The id is a pure function of its inputs (stable run-to-run).
    assert_eq!(
        compute_font_id("Inter", 700, true),
        compute_font_id("Inter", 700, true),
    );
    // Italic and weight each flip the id (no aliasing into the upright/other-weight key).
    assert_ne!(
        compute_font_id("Inter", 400, false),
        compute_font_id("Inter", 400, true),
    );
    assert_ne!(
        compute_font_id("Inter", 400, false),
        compute_font_id("Inter", 700, false),
    );
}

/// DETERMINISM REGRESSION (t65): rendering the SAME text scene through the full
/// async font-worker path must produce a BYTE-IDENTICAL framebuffer every time.
///
/// The regression this guards: glyphs rasterise on a background thread, so a
/// non-blocking poll committed only whatever happened to have arrived, and text
/// layout reads glyph advances out of the atlas — a partially-populated atlas
/// reflowed/garbled text differently each run. Driving each render to quiescence
/// (`has_pending_glyphs` clear) must now land on the same pixels regardless of
/// worker thread timing.
#[test]
fn identical_text_scene_renders_byte_identically_across_renderers() {
    // Render a fresh renderer to quiescence and return the final framebuffer.
    fn render_to_quiescence(text: &str, family: &str) -> Vec<u8> {
        let mut renderer = SoftwareRenderer::new();
        let mut fb = FrameBuffer::new(256, 64, PixelFormat::Bgra8);
        let mut damage = DamageSet::new(64);
        damage.add(DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::TextGlyph,
        });
        // Drive render passes until no glyphs remain pending (mirrors the
        // capture path's reflush). Bounded so a missing system font can't hang.
        for _ in 0..8 {
            renderer
                .render(&[text_node(text, family)], &mut fb, &damage)
                .unwrap();
            if !renderer.has_pending_glyphs() {
                break;
            }
        }
        fb.pixels().to_vec()
    }

    // Use a real system font when present (exercises the async rasteriser); the
    // bitmap fallback is fine too — both must be deterministic.
    let family = if fixture_font_bytes().is_some() {
        "Arial"
    } else {
        ""
    };
    let text = "Open Terminal File Manager Settings";

    let a = render_to_quiescence(text, family);
    let b = render_to_quiescence(text, family);
    let c = render_to_quiescence(text, family);

    assert_eq!(
        a, b,
        "identical text scene rendered to two byte-different framebuffers — \
         glyph render path is nondeterministic"
    );
    assert_eq!(a, c, "third render diverged — glyph render path is nondeterministic");
    // Sanity: the text actually painted something (not a vacuously-equal blank).
    assert!(a.iter().any(|&p| p != 0), "scene produced an all-zero framebuffer");
}

/// The live render entry must return PROMPTLY (no multi-second block-drain) and
/// must signal pending glyphs so the session schedules a follow-up frame, while
/// the deterministic capture entry stays byte-stable.
///
/// This is the t69-drain2 contract: `render_live` is the non-blocking live path
/// (t68 cause #1 / C2), and `render` (capture) keeps block-draining for goldens.
#[test]
fn live_render_returns_promptly_with_pending_glyphs_then_quiesces() {
    use std::time::{Duration, Instant};

    let family = if fixture_font_bytes().is_some() {
        "Arial"
    } else {
        ""
    };
    let text = "Open Terminal File Manager Settings";

    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(256, 64, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::TextGlyph,
    });

    // First LiveFull frame: text is freshly requested, so glyphs are still in
    // flight. The call must return well under the 500 ms render watchdog (it is
    // budgeted at a few ms) and must flag pending glyphs so the caller asks for
    // another frame.
    let t0 = Instant::now();
    renderer
        .render_live(&[text_node(text, family)], &mut fb, &damage, RenderMode::LiveFull)
        .unwrap();
    let elapsed = t0.elapsed();
    assert!(
        elapsed < Duration::from_millis(250),
        "live full render blocked for {elapsed:?} — it must not block-drain glyphs"
    );
    assert!(
        renderer.has_pending_glyphs(),
        "first live frame for fresh text must report pending glyphs to drive a follow-up frame"
    );

    // Follow-up LiveFull frames must also return promptly and eventually quiesce
    // once the worker has rasterised everything (mirrors the session loop
    // re-rendering while has_pending_glyphs() is true).
    let mut quiesced = false;
    for _ in 0..200 {
        let t = Instant::now();
        renderer
            .render_live(&[text_node(text, family)], &mut fb, &damage, RenderMode::LiveFull)
            .unwrap();
        assert!(
            t.elapsed() < Duration::from_millis(250),
            "live full render must never block for a perceptible duration"
        );
        if !renderer.has_pending_glyphs() {
            quiesced = true;
            break;
        }
        std::thread::yield_now();
    }
    assert!(
        quiesced,
        "live render never quiesced — pending glyphs were never resolved on follow-up frames"
    );

    // A LiveCursor frame is a pure non-blocking poll and must return immediately
    // and never wait on text.
    let t = Instant::now();
    renderer
        .render_live(&[text_node(text, family)], &mut fb, &damage, RenderMode::LiveCursor)
        .unwrap();
    assert!(
        t.elapsed() < Duration::from_millis(100),
        "cursor-only live render must never block"
    );
}

/// t77: the LIVE full-render glyph-drain budget must stay tiny so it is not a
/// per-frame present tax at the 200 fps target (~5 ms/frame). This pins the
/// budget AND proves the budget→deadline translation is bounded by ~1 ms, NOT
/// the old 4 ms.
///
/// TEETH (these fail if the budget is reverted to 4 ms, or if Capture's
/// determinism budget is collateral-damaged, or if LiveCursor stops being a
/// pure non-blocking poll):
///   * the constant assertion fails the instant `LIVE_GLYPH_DRAIN_BUDGET_MS`
///     goes above 1;
///   * the `drain_deadline(LiveFull)` window assertion fails if the live
///     deadline lands ~4 ms out (it caps the budget at 2 ms incl. slack);
///   * Capture must still block far out for goldens; LiveCursor must be `None`.
#[test]
fn live_full_glyph_drain_budget_is_one_ms_not_four() {
    use crate::renderer::{drain_deadline, GLYPH_DRAIN_BUDGET_MS, LIVE_GLYPH_DRAIN_BUDGET_MS};
    use std::time::{Duration, Instant};

    // Tooth 1: the conservative 1 ms ceiling. Reverting to 4 fails here.
    assert!(
        LIVE_GLYPH_DRAIN_BUDGET_MS <= 1,
        "LIVE_GLYPH_DRAIN_BUDGET_MS={LIVE_GLYPH_DRAIN_BUDGET_MS} — the live full-render \
         glyph-drain budget must stay <= 1 ms; at 200 fps a larger budget is a direct \
         per-frame present stall (t77)"
    );

    // Tooth 2: the budget actually flows into the LiveFull drain deadline as a
    // ~1 ms wait, not 4 ms. Allow 2 ms of headroom for the millisecond rounding;
    // a 4 ms budget produces a deadline ~4 ms out and trips this.
    let before = Instant::now();
    let live_deadline = drain_deadline(RenderMode::LiveFull)
        .expect("LiveFull must use a (tiny) bounded drain deadline, not a non-blocking poll");
    let live_window = live_deadline.saturating_duration_since(before);
    assert!(
        live_window <= Duration::from_millis(2),
        "LiveFull drain deadline is {live_window:?} out — must be a ~1 ms budget, not 4 ms"
    );

    // Capture (golden) determinism budget must be left far out, untouched.
    let cap_deadline = drain_deadline(RenderMode::Capture)
        .expect("Capture must block-drain for determinism");
    assert!(
        cap_deadline.saturating_duration_since(Instant::now())
            >= Duration::from_millis(GLYPH_DRAIN_BUDGET_MS / 2),
        "Capture drain budget must stay large for golden determinism"
    );

    // LiveCursor must remain a pure non-blocking poll (no deadline at all).
    assert!(
        drain_deadline(RenderMode::LiveCursor).is_none(),
        "LiveCursor must never wait on glyphs"
    );
}

/// t77: a LiveFull render that still has glyphs in flight must (a) return well
/// under a few ms — bounded by the tiny ~1 ms drain, NOT a 4 ms stall — and
/// (b) report `has_pending_glyphs()` afterward so the session knows to resubmit
/// a follow-up frame. This is the resubmit contract the live present path relies
/// on; it must hold independently of the (peer-owned) session loop.
///
/// TOOTH: with the budget at 4 ms a fresh-text LiveFull frame whose glyphs miss
/// the budget would routinely take ~4 ms; we assert it returns under 3 ms so a
/// revert to 4 ms surfaces here too. And if the renderer ever stopped flagging
/// pending glyphs, `has_pending_glyphs()` would be false and the assertion
/// fails — proving the caller would still be told to resubmit.
#[test]
fn live_full_render_with_pending_glyphs_returns_fast_and_flags_resubmit() {
    use std::time::{Duration, Instant};

    // Use a real font family if one is present so glyphs are genuinely requested
    // and tracked as pending; an empty family still requests bitmap-fallback
    // glyphs, so the pending/resubmit contract holds either way.
    let family = if fixture_font_bytes().is_some() {
        "Arial"
    } else {
        ""
    };
    let text = "Open Terminal File Manager Settings Software Center Task Manager";

    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(512, 64, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::TextGlyph,
    });

    // First LiveFull frame: glyphs are freshly requested and the async worker has
    // not produced them yet, so the drain hits its tiny budget. The call must
    // return in well under 3 ms (a 4 ms budget would routinely overrun this) AND
    // must flag pending glyphs so the caller resubmits.
    let t0 = Instant::now();
    renderer
        .render_live(
            &[text_node(text, family)],
            &mut fb,
            &damage,
            RenderMode::LiveFull,
        )
        .unwrap();
    let elapsed = t0.elapsed();
    assert!(
        renderer.has_pending_glyphs(),
        "a LiveFull frame for fresh text whose glyphs missed the drain budget MUST report \
         has_pending_glyphs() so the session resubmits — the live present path depends on it"
    );
    // The drain is bounded by ~1 ms; the rest of render_live (paint) is cheap for
    // this tiny surface. 3 ms gives generous slack for CI scheduler jitter while
    // still catching a revert to the 4 ms budget.
    assert!(
        elapsed < Duration::from_millis(3),
        "LiveFull render with pending glyphs took {elapsed:?} — the live glyph drain must be \
         bounded by ~1 ms, not 4 ms (t77)"
    );
}

/// The capture entry (`render`) and the live entry driven to quiescence
/// (`render_live`) must converge on the SAME pixels for a fully-rasterised
/// scene — the mode only controls glyph drain policy, not painting. This proves
/// the live path does not perturb the deterministic capture output.
#[test]
fn capture_render_is_byte_deterministic_and_matches_quiesced_live() {
    fn capture_to_quiescence(text: &str, family: &str) -> Vec<u8> {
        let mut renderer = SoftwareRenderer::new();
        let mut fb = FrameBuffer::new(256, 64, PixelFormat::Bgra8);
        let mut damage = DamageSet::new(64);
        damage.add(DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::TextGlyph,
        });
        for _ in 0..8 {
            renderer
                .render(&[text_node(text, family)], &mut fb, &damage)
                .unwrap();
            if !renderer.has_pending_glyphs() {
                break;
            }
        }
        fb.pixels().to_vec()
    }

    fn live_to_quiescence(text: &str, family: &str) -> Vec<u8> {
        let mut renderer = SoftwareRenderer::new();
        let mut fb = FrameBuffer::new(256, 64, PixelFormat::Bgra8);
        let mut damage = DamageSet::new(64);
        damage.add(DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::TextGlyph,
        });
        for _ in 0..400 {
            renderer
                .render_live(&[text_node(text, family)], &mut fb, &damage, RenderMode::LiveFull)
                .unwrap();
            if !renderer.has_pending_glyphs() {
                break;
            }
            std::thread::yield_now();
        }
        // Intermediate live frames paint with estimated advances for glyphs that
        // were not yet ready, leaving stale pixels in the reused buffer (the live
        // session loop clears the damaged tiles each frame). Render the final,
        // fully-quiesced frame into a CLEAN buffer so the comparison reflects the
        // converged pixels, not leftovers from the fill-in frames.
        let mut clean = FrameBuffer::new(256, 64, PixelFormat::Bgra8);
        renderer
            .render_live(&[text_node(text, family)], &mut clean, &damage, RenderMode::LiveFull)
            .unwrap();
        clean.pixels().to_vec()
    }

    let family = if fixture_font_bytes().is_some() {
        "Arial"
    } else {
        ""
    };
    let text = "Open Terminal File Manager Settings";

    let cap_a = capture_to_quiescence(text, family);
    let cap_b = capture_to_quiescence(text, family);
    assert_eq!(cap_a, cap_b, "capture render must be byte-deterministic");

    let live = live_to_quiescence(text, family);
    assert_eq!(
        cap_a, live,
        "quiesced live render must converge on the same pixels as the capture render"
    );
    assert!(cap_a.iter().any(|&p| p != 0), "scene produced an all-zero framebuffer");
}

#[test]
fn renderer_options_disable_common_glyph_prewarm() {
    let mut renderer = SoftwareRenderer::with_options(SoftwareRendererOptions {
        glyph_prewarm: GlyphPrewarmMode::Disabled,
    });

    render_text_once(&mut renderer, text_node("A", "Inter"));

    assert_eq!(renderer.prewarmed_font_count(), 0);
    assert_eq!(renderer.pending_glyph_request_count(), 1);
}

#[test]
fn renderer_default_options_prewarm_common_glyphs() {
    let mut renderer = SoftwareRenderer::new();

    render_text_once(&mut renderer, text_node("A", "Inter"));

    assert_eq!(renderer.prewarmed_font_count(), 1);
    assert!(
        renderer.pending_glyph_request_count() > 1,
        "default prewarm should enqueue common glyphs in addition to visible text"
    );
}

#[test]
fn stale_font_invalidation_returns_faces_and_clears_cpu_glyph_state() {
    let Some((dir, path)) = write_fixture_font("stale-font") else {
        return;
    };
    let mut db = FontDatabase::new();
    let face_id = db.load_file(&path, "Fixture", 400, false).unwrap();
    let mut renderer = SoftwareRenderer::with_font_db(db);

    render_text_once(&mut renderer, text_node("A", "Fixture"));
    renderer
        .glyph_atlas_mut()
        .insert(
            GlyphKey {
                font_id: 7,
                glyph_id: 'A' as u32,
                size_px: 16,
                subpixel: false,
            },
            &[255],
            &GlyphMetrics {
                width: 1,
                height: 1,
                bearing_x: 0,
                bearing_y: 1,
                advance: 1.0,
            },
        )
        .unwrap();
    assert!(!renderer.glyph_atlas().is_empty());
    assert_eq!(renderer.prewarmed_font_count(), 1);
    assert!(renderer.pending_glyph_request_count() > 0);
    assert!(renderer.invalidate_stale_fonts().is_empty());

    let mut data = std::fs::read(&path).unwrap();
    data.extend_from_slice(b"stale");
    std::fs::write(&path, data).unwrap();

    let stale_faces = renderer.invalidate_stale_fonts();

    assert_eq!(stale_faces, vec![face_id]);
    assert!(renderer.glyph_atlas().is_empty());
    assert_eq!(renderer.prewarmed_font_count(), 0);
    assert_eq!(renderer.pending_glyph_request_count(), 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cpu_renderer_reports_backend_capabilities() {
    let renderer = SoftwareRenderer::new();

    let info = renderer.backend_info();
    assert_eq!(info.kind, RendererBackendKind::Software);
    assert_eq!(info.name, "liquide-renderer-cpu");
    assert_eq!(info.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));

    let capabilities = renderer.capabilities();
    assert!(capabilities.supports_frame_memory(FrameMemoryKind::Cpu));
    assert!(capabilities.supports_pixel_format(PixelFormat::Bgra8));
    assert!(capabilities.supports_pixel_format(PixelFormat::Rgba8));
    assert!(capabilities.supports_pixel_format(PixelFormat::Rgb8));
    assert!(capabilities.supports_partial_damage);
    assert!(capabilities.supports_blur);
    assert!(capabilities.supports_skeleton_window);
    assert!(capabilities.supports_async_glyphs);
}

#[test]
fn cpu_negotiation_accepts_writable_cpu_framebuffers() {
    let renderer = SoftwareRenderer::new();
    let fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);
    let damage = DamageSet::new(64);

    let negotiation = renderer.negotiate_render(&[], &fb, &damage);

    assert!(negotiation.is_accepted());
}

#[test]
fn cpu_negotiation_rejects_gpu_and_unsupported_formats() {
    let renderer = SoftwareRenderer::new();
    let damage = DamageSet::new(64);
    let gpu_fb = FrameBuffer {
        memory: FrameMemory::Gpu {
            handle: 1,
            dmabuf_fd: -1,
            width: 16,
            height: 16,
        },
        width: 16,
        height: 16,
        stride: 64,
        format: PixelFormat::Bgra8,
    };

    let gpu_negotiation = renderer.negotiate_render(&[], &gpu_fb, &damage);
    assert!(matches!(
        gpu_negotiation.reject_reason(),
        Some(RendererRejectReason::UnsupportedFrameMemory {
            memory: FrameMemoryKind::Gpu
        })
    ));

    let unsupported_format = FrameBuffer::new(16, 16, PixelFormat::Rgb565);
    let format_negotiation = renderer.negotiate_render(&[], &unsupported_format, &damage);
    assert!(matches!(
        format_negotiation.reject_reason(),
        Some(RendererRejectReason::UnsupportedPixelFormat {
            format: PixelFormat::Rgb565
        })
    ));
}

#[test]
fn cpu_negotiation_rejects_unwritable_cpu_buffers() {
    let renderer = SoftwareRenderer::new();
    let damage = DamageSet::new(64);
    let fb = FrameBuffer {
        memory: FrameMemory::Cpu(vec![0; 8]),
        width: 4,
        height: 4,
        stride: 16,
        format: PixelFormat::Bgra8,
    };

    let negotiation = renderer.negotiate_render(&[], &fb, &damage);

    assert!(matches!(
        negotiation.reject_reason(),
        Some(RendererRejectReason::Other(reason)) if reason.contains("requires at least")
    ));
}

#[test]
fn render_background() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });

    let node = FlatNode {
        id: 1,
        kind: SceneNodeKind::Background {
            color: Color::new(0, 100, 200, 255),
        }
        .into(),
        absolute_bounds: Rect::new(0.0, 0.0, 128.0, 128.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
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
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });

    let node = FlatNode {
        id: 10,
        kind: SceneNodeKind::Surface {
            surface_id: 1,
            buffer: None,
        }
        .into(),
        absolute_bounds: Rect::new(0.0, 0.0, 64.0, 64.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    // Should not error even with no buffer
    let result = renderer.render(&[node], &mut fb, &damage);
    assert!(
        result.is_ok(),
        "render Surface with no buffer should succeed"
    );
}

#[test]
fn render_glass_node() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    fb.clear(Color::new(100, 100, 100, 255));
    let before = fb.pixels().to_vec();

    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });

    let node = FlatNode {
        id: 20,
        kind: SceneNodeKind::Glass(liquide_compositor::scene::GlassParams::default()).into(),
        absolute_bounds: Rect::new(0.0, 0.0, 64.0, 64.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    let result = renderer.render(&[node], &mut fb, &damage);
    assert!(result.is_ok(), "render Glass node should succeed");
    assert_ne!(fb.pixels(), &before[..], "Glass tint should modify pixels");
}

#[test]
fn render_decoration_node() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);

    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });

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
            button_colors: liquide_compositor::scene::DecorationColors::default(),
            button_layout: liquide_compositor::scene::DecorationLayout::default(),
        }
        .into(),
        absolute_bounds: Rect::new(0.0, 0.0, 64.0, 32.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    let result = renderer.render(&[node], &mut fb, &damage);
    assert!(result.is_ok(), "render Decoration node should succeed");
    // The background fill should have modified some pixels
    let center = fb.get_pixel(32, 16);
    assert!(
        center.r > 0 || center.g > 0 || center.b > 0,
        "decoration should fill pixels: got {:?}",
        center
    );
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
    let before = fb.pixels().to_vec();

    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });
    damage.add(DamageTile {
        x: 1,
        y: 0,
        class: DamageClass::UiPrimitive,
    });
    damage.add(DamageTile {
        x: 0,
        y: 1,
        class: DamageClass::UiPrimitive,
    });
    damage.add(DamageTile {
        x: 1,
        y: 1,
        class: DamageClass::UiPrimitive,
    });

    let node = FlatNode {
        id: 40,
        kind: SceneNodeKind::LockScreen.into(),
        absolute_bounds: Rect::new(0.0, 0.0, 128.0, 128.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    let result = renderer.render(&[node], &mut fb, &damage);
    assert!(result.is_ok(), "render LockScreen node should succeed");
    assert_ne!(
        fb.pixels(),
        &before[..],
        "LockScreen should modify pixels (backdrop blur + dark tint)"
    );
}

#[test]
fn render_classifies_cursor_and_surface_damage() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 64, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });
    damage.add(DamageTile {
        x: 1,
        y: 0,
        class: DamageClass::UiPrimitive,
    });

    let cursor = FlatNode {
        id: 100,
        kind: SceneNodeKind::Cursor {
            shape: liquide_compositor::scene::CursorShape::Arrow,
        }
        .into(),
        absolute_bounds: Rect::new(0.0, 0.0, 24.0, 24.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };
    let surface = FlatNode {
        id: 101,
        kind: SceneNodeKind::Surface {
            surface_id: 1,
            buffer: None,
        }
        .into(),
        absolute_bounds: Rect::new(64.0, 0.0, 64.0, 64.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 1,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    let classified = renderer
        .render(&[cursor, surface], &mut fb, &damage)
        .unwrap();
    let classes: std::collections::HashMap<(u32, u32), DamageClass> = classified
        .into_iter()
        .map(|tile| ((tile.x, tile.y), tile.class))
        .collect();

    assert_eq!(classes.get(&(0, 0)), Some(&DamageClass::CursorOnly));
    assert_eq!(classes.get(&(1, 0)), Some(&DamageClass::BitmapRegion));
}

#[test]
fn render_text_damage_overrides_bitmap_damage_on_same_tile() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });

    let surface = FlatNode {
        id: 200,
        kind: SceneNodeKind::Surface {
            surface_id: 2,
            buffer: None,
        }
        .into(),
        absolute_bounds: Rect::new(0.0, 0.0, 64.0, 64.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };
    let caret = FlatNode {
        id: 201,
        kind: SceneNodeKind::TextCaret {
            color: Color::WHITE,
            width: 2.0,
        }
        .into(),
        absolute_bounds: Rect::new(8.0, 8.0, 2.0, 24.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 1,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    let classified = renderer
        .render(&[surface, caret], &mut fb, &damage)
        .unwrap();
    assert_eq!(classified.len(), 1);
    assert_eq!(classified[0].class, DamageClass::TextGlyph);
}

// ── word-break / text-emphasis primitive tests ─────────────────────────
//
// These use a renderer with synthetically-inserted glyphs (no system font
// dependency) so wrapping/emphasis behaviour is deterministic. An empty
// font_family with weight 400, upright, yields the `compute_font_id` value used
// by the text path (the single source of truth for atlas keying) and
// glyph_height = 16, and suppresses the common-glyph prewarm path. We derive the
// id via the same function the renderer uses so the inserted glyphs are found.

const TEST_SIZE_PX: u16 = 16;

fn test_font_id() -> u32 {
    crate::renderer::compute_font_id("", 400, false)
}

/// Insert a solid square glyph of the given logical width/height with a known
/// advance into the atlas, so text layout is fully deterministic.
fn insert_block_glyph(renderer: &mut SoftwareRenderer, ch: char, advance: f32) {
    let w = 10u32;
    let h = 12u32;
    let bitmap = vec![255u8; (w * h) as usize];
    renderer
        .glyph_atlas_mut()
        .insert(
            GlyphKey {
                font_id: test_font_id(),
                glyph_id: ch as u32,
                size_px: TEST_SIZE_PX,
                subpixel: false,
            },
            &bitmap,
            &GlyphMetrics {
                width: w,
                height: h,
                bearing_x: 0,
                bearing_y: h as i32,
                advance,
            },
        )
        .unwrap();
}

/// Insert a small mark glyph at the emphasis size (round(16 * 0.5) = 8).
fn insert_mark_glyph(renderer: &mut SoftwareRenderer, ch: char) {
    let w = 4u32;
    let h = 4u32;
    let bitmap = vec![255u8; (w * h) as usize];
    renderer
        .glyph_atlas_mut()
        .insert(
            GlyphKey {
                font_id: test_font_id(),
                glyph_id: ch as u32,
                size_px: 8,
                subpixel: false,
            },
            &bitmap,
            &GlyphMetrics {
                width: w,
                height: h,
                bearing_x: 0,
                bearing_y: h as i32,
                advance: 4.0,
            },
        )
        .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn word_break_text_node(
    text: &str,
    bounds: Rect,
    word_break: liquide_compositor::scene::WordBreak,
    white_space: u8,
    text_emphasis: Option<liquide_compositor::scene::TextEmphasis>,
) -> FlatNode {
    FlatNode {
        id: 42,
        kind: SceneNodeKind::Text {
            text: text.to_string(),
            color: Color::WHITE,
            scale: 1,
            font_family: String::new(),
            font_size: 0.0,
            font_weight: 400,
            font_style_italic: false,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            line_height: 14.0,
            text_align: 0,
            text_transform: 0,
            text_overflow: 0,
            white_space,
            word_break,
            text_indent: 0.0,
            text_decoration: None,
            text_shadows: Vec::new(),
            text_emphasis,
        }
        .into(),
        absolute_bounds: bounds,
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    }
}

fn render_node_to_fb(renderer: &mut SoftwareRenderer, node: FlatNode, w: u32, h: u32) -> FrameBuffer {
    let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::TextGlyph,
    });
    renderer.render(&[node], &mut fb, &damage).unwrap();
    fb
}

/// Count rows (scanlines) that contain at least one non-transparent pixel.
fn nonempty_rows(fb: &FrameBuffer) -> u32 {
    let mut rows = 0;
    for y in 0..fb.height {
        let mut any = false;
        for x in 0..fb.width {
            if fb.get_pixel(x, y).a > 0 {
                any = true;
                break;
            }
        }
        if any {
            rows += 1;
        }
    }
    rows
}

fn nonempty_pixels(fb: &FrameBuffer) -> u32 {
    let mut n = 0;
    for y in 0..fb.height {
        for x in 0..fb.width {
            if fb.get_pixel(x, y).a > 0 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn word_break_break_all_wraps_long_word_normal_does_not() {
    use liquide_compositor::scene::WordBreak;
    // A single long unbreakable word, in a box only ~3 glyphs wide.
    // advance 10 → box width 35 fits ~3 chars; the 8-char word overflows.
    let word = "AAAAAAAA";
    let bounds = Rect::new(0.0, 0.0, 35.0, 120.0);

    let mut normal = SoftwareRenderer::new();
    insert_block_glyph(&mut normal, 'A', 10.0);
    let fb_normal = render_node_to_fb(
        &mut normal,
        word_break_text_node(word, bounds, WordBreak::Normal, 0, None),
        64,
        128,
    );

    let mut break_all = SoftwareRenderer::new();
    insert_block_glyph(&mut break_all, 'A', 10.0);
    let fb_break = render_node_to_fb(
        &mut break_all,
        word_break_text_node(word, bounds, WordBreak::BreakAll, 0, None),
        64,
        128,
    );

    let rows_normal = nonempty_rows(&fb_normal);
    let rows_break = nonempty_rows(&fb_break);

    // Normal: one overflowing line (single glyph row of height ~12).
    // break-all: the word is split across multiple lines → taller footprint.
    assert!(
        rows_break > rows_normal,
        "break-all should wrap the long word onto more lines \
         (break-all rows={rows_break}, normal rows={rows_normal})"
    );
}

#[test]
fn text_emphasis_renders_extra_marks() {
    use liquide_compositor::scene::{TextEmphasis, TextEmphasisPosition, WordBreak};
    let text = "AAA";
    let bounds = Rect::new(0.0, 20.0, 200.0, 40.0);

    let mut plain = SoftwareRenderer::new();
    insert_block_glyph(&mut plain, 'A', 12.0);
    let fb_plain = render_node_to_fb(
        &mut plain,
        word_break_text_node(text, bounds, WordBreak::Normal, 1, None),
        256,
        80,
    );

    let mut emphasized = SoftwareRenderer::new();
    insert_block_glyph(&mut emphasized, 'A', 12.0);
    insert_mark_glyph(&mut emphasized, '•');
    let emph = TextEmphasis {
        mark: "•".to_string(),
        color: None,
        position: TextEmphasisPosition::Over,
    };
    let fb_emph = render_node_to_fb(
        &mut emphasized,
        word_break_text_node(text, bounds, WordBreak::Normal, 1, Some(emph)),
        256,
        80,
    );

    let plain_px = nonempty_pixels(&fb_plain);
    let emph_px = nonempty_pixels(&fb_emph);
    assert!(
        emph_px > plain_px,
        "text-emphasis should draw additional mark pixels \
         (emphasized={emph_px}, plain={plain_px})"
    );
}
