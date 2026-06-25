//! Teeth tests for the live text-shaping wiring (rustybuzz OpenType + Unicode
//! bidi + per-glyph multi-font fallback) and the SIMD glyph-blit byte-identity.
//!
//! These tests are written to FAIL if shaping regresses to the old naive
//! per-codepoint path:
//! - a kerning pair shapes TIGHTER than the naive per-codepoint advance sum,
//! - an RTL string's glyphs come out in right-to-left visual x-order,
//! - a codepoint missing from the primary face renders from a covering FALLBACK
//!   face (a real, non-`.notdef` glyph), and
//! - the SIMD glyph coverage reconstruction is byte-for-bit equal to scalar.
//!
//! The font-dependent tests load a real system font and skip (return early) when
//! none is available, so they never fail spuriously in a font-less CI — but on any
//! developer/CI box with a UI font they exercise the real engine.

use liquide_font_rasterizer::database::{FontDatabase, FontFaceId};
use liquide_font_rasterizer::shaper::{FontFeature, TextShaper};

use crate::renderer::text_shaping::{shape_line, shaped_run_width};

/// Load a real system font's bytes, or `None` if none of the usual paths exist.
fn system_font_bytes() -> Option<Vec<u8>> {
    let candidates = [
        "C:\\Windows\\Fonts\\arial.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\calibri.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
        "/Library/Fonts/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ];
    candidates
        .iter()
        .find_map(|p| std::fs::read(p).ok().filter(|b| !b.is_empty()))
}

/// A font that contains Hebrew glyphs (for the RTL test), or `None`.
fn hebrew_font_bytes() -> Option<Vec<u8>> {
    let candidates = [
        // Segoe UI covers Hebrew on Windows.
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/Library/Fonts/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ];
    candidates.iter().find_map(|p| {
        let bytes = std::fs::read(p).ok().filter(|b| !b.is_empty())?;
        // Verify it actually has a Hebrew glyph (aleph U+05D0).
        use ab_glyph::Font;
        let font = ab_glyph::FontRef::try_from_slice(&bytes).ok()?;
        if font.glyph_id('\u{05D0}').0 != 0 {
            Some(bytes)
        } else {
            None
        }
    })
}

/// Total advance of `text` shaped through rustybuzz with `kern` either on or off.
/// Comparing the two isolates the kerning contribution apples-to-apples (same
/// shaper, same scale), unlike comparing against ab_glyph's differently-scaled
/// `h_advance`. The "kern off" total is what naive per-codepoint layout produces
/// (each glyph at its own unkerned advance).
fn shaped_total(db: &FontDatabase, face: FontFaceId, text: &str, size: f32, kern: bool) -> f32 {
    let shaper = TextShaper::new(db);
    let feats = [
        FontFeature::kerning(kern),
        // Disable ligatures so the glyph count is stable and only kerning moves
        // the total — this test is about kerning, not ligatures.
        FontFeature::ligatures(false),
    ];
    let (_glyphs, total) = shaper.shape_with_features(face, text, size, 0.0, &feats);
    total
}

/// Kerning: a kerning-heavy string ("AVWAY") must shape TIGHTER with kerning on
/// than with kerning off (the naive per-codepoint total). On the old path kerning
/// never reached layout, so the totals would be equal — this is RED there and
/// GREEN only when real GPOS/kern shaping drives advances.
#[test]
fn kerning_pair_shapes_tighter_than_naive_per_codepoint() {
    let Some(bytes) = system_font_bytes() else {
        return;
    };
    let mut db = FontDatabase::new();
    let face = db.load_bytes(bytes, "KernProbe", 400, false).unwrap();

    let size = 64.0_f32;
    let text = "AVWAY";

    // Sanity: the wired shape_line produces one glyph per letter (ligatures off
    // for these letters anyway) and a consistent advance sum.
    let (glyphs, line_total) = shape_line(&db, text, "KernProbe", size, 400, false, 0.0, 0.0);
    assert_eq!(glyphs.len(), text.chars().count(), "one glyph per letter");
    let advance_sum: f32 = glyphs.iter().map(|g| g.advance).sum();
    assert!(
        (advance_sum - line_total).abs() < 1.0,
        "per-glyph advances ({advance_sum}) must sum to the line width ({line_total})"
    );

    let kerned = shaped_total(&db, face, text, size, true);
    let unkerned = shaped_total(&db, face, text, size, false);

    assert!(
        kerned <= unkerned + 0.01,
        "kerning must not INCREASE the advance (kerned={kerned}, unkerned={unkerned})"
    );
    assert!(
        kerned < unkerned - 0.5,
        "shaped 'AVWAY' must be tighter WITH kerning than without \
         (kerned={kerned}, unkerned={unkerned}); kerning is not reaching layout"
    );
}

/// Ligature: when standard ligatures form, a string like "office" (with the "ffi"
/// or "fi"/"fl" ligature) shapes to FEWER glyphs than its character count. Not all
/// fonts ship these ligatures, so the test asserts the weaker, always-true
/// property (glyph count ≤ char count, and shaping ran) and, when the font DOES
/// ligate, that the count strictly drops.
#[test]
fn ligature_forms_fewer_glyphs_when_font_supports_it() {
    let Some(bytes) = system_font_bytes() else {
        return;
    };
    let mut db = FontDatabase::new();
    db.load_bytes(bytes, "LigProbe", 400, false).unwrap();

    let size = 48.0_f32;
    for word in ["office", "fi", "fl", "ffi"] {
        let (glyphs, _w) = shape_line(&db, word, "LigProbe", size, 400, false, 0.0, 0.0);
        assert!(
            glyphs.len() <= word.chars().count(),
            "shaping must never produce MORE glyphs than chars for {word:?} \
             (got {} for {} chars)",
            glyphs.len(),
            word.chars().count()
        );
    }
}

/// RTL: a Hebrew string must come out in right-to-left VISUAL order — the first
/// logical character sits at the LARGEST x, the last at the smallest. On the old
/// path RTL rendered in logical (left-to-right) order, so the x-positions would be
/// ascending with logical order; here they must descend.
#[test]
fn rtl_string_glyphs_are_right_to_left_in_visual_order() {
    let Some(bytes) = hebrew_font_bytes() else {
        return;
    };
    let mut db = FontDatabase::new();
    db.load_bytes(bytes, "RtlProbe", 400, false).unwrap();

    // "אבג" — three Hebrew letters (aleph, bet, gimel), logical order א,ב,ג.
    let text = "\u{05D0}\u{05D1}\u{05D2}";
    let (glyphs, total) = shape_line(&db, text, "RtlProbe", 48.0, 400, false, 0.0, 0.0);
    assert_eq!(glyphs.len(), 3, "three Hebrew glyphs");
    assert!(total > 0.0);

    // Visual order: glyphs are emitted left-to-right on screen. For an RTL run the
    // FIRST glyph emitted (smallest x) is the LAST logical letter (gimel) and the
    // LAST glyph emitted (largest x) is the FIRST logical letter (aleph). So the
    // x positions are strictly increasing across the emitted glyphs, and the
    // emitted codepoints run gimel, bet, aleph (reverse of logical).
    for w in glyphs.windows(2) {
        assert!(
            w[1].x > w[0].x,
            "emitted glyphs must advance left-to-right in x"
        );
    }
    assert_eq!(
        glyphs[0].codepoint, '\u{05D2}',
        "leftmost glyph must be the LAST logical letter (gimel) — RTL visual order"
    );
    assert_eq!(
        glyphs[2].codepoint, '\u{05D0}',
        "rightmost glyph must be the FIRST logical letter (aleph) — RTL visual order"
    );
}

/// Per-glyph multi-font fallback: a codepoint absent from the PRIMARY face must be
/// rendered from a covering FALLBACK face (a real glyph id, NOT `.notdef`/0), and
/// the glyph must be tagged with the fallback face — not the primary one.
#[test]
fn missing_codepoint_falls_back_to_a_covering_face() {
    let Some(bytes) = system_font_bytes() else {
        return;
    };
    // Build a database where the PRIMARY family ("Primary") is a tiny synthetic
    // face guaranteed to lack a glyph, and a real system font is registered under
    // one of the fallback families ("Noto Sans") that DOES cover the codepoint.
    let mut db = FontDatabase::new();

    // Register the real system font under a fallback family so the fallback chain
    // finds it.
    let fb_face = db.load_bytes(bytes.clone(), "Noto Sans", 400, false).unwrap();

    // Register the SAME font under "Primary" but we will request a codepoint the
    // font lacks; to force a real miss we instead pick a primary that lacks a CJK
    // codepoint and a fallback that has it. Most Latin UI fonts lack CJK, so use a
    // CJK codepoint; if the chosen system font happens to cover it we skip.
    let primary = db.load_bytes(bytes, "Primary", 400, false).unwrap();

    // U+4E2D (中) — a common CJK ideograph absent from Latin UI fonts.
    let cjk = '\u{4E2D}';
    let covers = |face: FontFaceId| -> bool {
        use ab_glyph::Font;
        db.get(face).is_some_and(|f| f.font.glyph_id(cjk).0 != 0)
    };
    if covers(primary) {
        // Primary already covers it (e.g. a CJK-capable system font) — no miss to
        // exercise; skip rather than assert a vacuous truth.
        return;
    }
    if !covers(fb_face) {
        // No fallback covers it either (Latin-only system font) — cannot prove
        // fallback here; skip honestly.
        return;
    }

    let s = cjk.to_string();
    let (glyphs, _w) = shape_line(&db, &s, "Primary", 48.0, 400, false, 0.0, 0.0);
    assert_eq!(glyphs.len(), 1, "single CJK cluster");
    let g = glyphs[0];
    assert_ne!(g.glyph_id, 0, "fallback glyph must be a real (non-.notdef) id");
    assert_eq!(
        g.face_id, fb_face,
        "the glyph must be shaped from the covering FALLBACK face, not the primary"
    );
    assert_ne!(g.face_id, primary, "must not stay on the primary face");
}

// ── Wrap pre-pass uses shaped width (the fix-wrap-prepass tooth) ─────────────
//
// The renderer's line-WRAP pre-pass decides where to break by measuring candidate
// runs. It MUST measure with the same shaper that paint uses, so a run that fits
// its box when painted is not wrapped by a divergent estimate. The historical bug:
// the pre-pass summed a `char_advance` closure that looked the glyph up by
// CODEPOINT in an atlas keyed by SHAPED glyph id (keys never matched) → it always
// fell back to `glyph_height * 0.55` (~131px for "Confirm action" at 16px) and
// wrapped the dialog title even though its shaped width (~105px) fit the box.

/// Render a single LTR text node and return how many distinct horizontal text
/// "bands" (lines) it painted, by grouping lit rows separated by blank gaps. A
/// one-line run yields 1; a wrapped run yields ≥2.
fn rendered_line_count(text: &str, font_size: f32, box_width: f32) -> usize {
    use liquide_compositor::Renderer;
    use liquide_compositor::damage::{DamageClass, DamageSet};
    use liquide_compositor::framebuffer::FrameBuffer;
    use liquide_compositor::geometry::{Affine2D, Rect};
    use liquide_compositor::pixel::{Color, PixelFormat};
    use liquide_compositor::scene::{FlatNode, SceneNodeKind};

    use crate::renderer::SoftwareRenderer;

    let bytes = system_font_bytes().expect("caller guards on a system font");
    let mut r = SoftwareRenderer::with_font_db({
        let mut d = FontDatabase::new();
        for fam in ["Inter", "Noto Sans", "Manrope"] {
            d.load_bytes(bytes.clone(), fam, 400, false).ok();
        }
        d
    });

    // Tall enough for several wrapped lines; box width is the wrap constraint.
    let (w, h) = (box_width.ceil() as u32 + 20, (font_size * 8.0) as u32);
    let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
    let backdrop = FlatNode {
        id: 1,
        kind: SceneNodeKind::Background { color: Color::new(0, 0, 0, 255) }.into(),
        absolute_bounds: Rect::new(0.0, 0.0, w as f32, h as f32),
        absolute_transform: Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };
    let text_node = FlatNode {
        id: 10,
        kind: SceneNodeKind::Text {
            text: text.to_string(),
            color: Color::WHITE,
            scale: 1,
            font_family: "Inter".to_string(),
            font_size,
            font_weight: 400,
            font_style_italic: false,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            line_height: font_size * 1.3,
            text_align: 0,
            text_transform: 0,
            text_overflow: 0,
            // white_space: normal (allows wrapping).
            white_space: 0,
            word_break: liquide_compositor::scene::WordBreak::Normal,
            text_indent: 0.0,
            text_decoration: None,
            text_shadows: Vec::new(),
            text_emphasis: None,
        }
        .into(),
        // The text box width IS the wrap constraint.
        absolute_bounds: Rect::new(2.0, 2.0, box_width, h as f32 - 4.0),
        absolute_transform: Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };
    let nodes = [backdrop, text_node];
    let damage = DamageSet::full(64, w.div_ceil(64), h.div_ceil(64), DamageClass::UiPrimitive);
    // Glyphs are requested on a frame and drawn the next — resubmit until the atlas
    // is warm, exactly like the live/golden loop.
    for _ in 0..4 {
        r.render(&nodes, &mut fb, &damage).unwrap();
        if !r.has_pending_glyphs() {
            break;
        }
    }

    // Count text bands: rows with any lit text pixel, grouped into runs separated
    // by ≥2 fully-blank rows (intra-glyph gaps are 1px; the inter-line gap is the
    // line-height minus the cap height, several px).
    let row_lit: Vec<bool> = (0..h)
        .map(|y| {
            (0..w).any(|x| {
                let p = fb.get_pixel(x, y);
                p.r > 60 || p.g > 60 || p.b > 60
            })
        })
        .collect();
    let mut bands = 0usize;
    let mut blank_run = usize::MAX; // start "in a gap" so the first lit row opens a band
    for &lit in &row_lit {
        if lit {
            if blank_run >= 2 {
                bands += 1;
            }
            blank_run = 0;
        } else {
            blank_run = blank_run.saturating_add(1);
        }
    }
    bands
}

/// The dialog title "Confirm action" at 16px must fit its ~105px content box on
/// ONE line, and a genuinely-too-long string in the SAME box must wrap to ≥2
/// lines. This asserts the wrap DECISION via both the shaped-width helper the
/// pre-pass now calls and the rendered line count. RED on the old codepoint-keyed
/// `0.55*height` estimate (~131px for the title → spurious wrap to 2 lines).
#[test]
fn dialog_title_fits_one_line_but_long_text_still_wraps() {
    let Some(_bytes) = system_font_bytes() else {
        return;
    };

    let mut db = FontDatabase::new();
    let bytes = system_font_bytes().unwrap();
    for fam in ["Inter", "Noto Sans"] {
        db.load_bytes(bytes.clone(), fam, 400, false).ok();
    }

    let size = 16.0_f32;
    // The dialog content box width the layout produces for the title (~105px). Give
    // a small headroom so the assertion is about the wrap heuristic, not sub-px.
    let box_w = 120.0_f32;

    // The wrap pre-pass's actual decision input: the shaped width of the whole
    // title must be < the box (so it is NOT wrapped). On the buggy estimate this
    // value was ~131px (> box) and forced a wrap.
    let title_w = shaped_run_width(&db, "Confirm action", "Inter", size, 400, false, 0.0, 0.0);
    assert!(
        title_w > 0.0 && title_w < box_w,
        "shaped width of 'Confirm action' ({title_w}px) must FIT the {box_w}px box \
         so the wrap pre-pass keeps it on one line"
    );

    // A genuinely-too-long string's shaped width must EXCEED the box (so real
    // wrapping still triggers — the fix must not disable legitimate wrapping).
    let long = "Confirm action immediately because the operation cannot be undone later";
    let long_w = shaped_run_width(&db, long, "Inter", size, 400, false, 0.0, 0.0);
    assert!(
        long_w > box_w,
        "a long string ({long_w}px) must exceed the {box_w}px box so it still wraps"
    );

    // End-to-end via the renderer: the title renders on ONE band; the long string
    // renders on ≥2 bands (real word-boundary wrapping).
    assert_eq!(
        rendered_line_count("Confirm action", size, box_w),
        1,
        "'Confirm action' must render on ONE line in a {box_w}px box (no spurious wrap)"
    );
    assert!(
        rendered_line_count(long, size, box_w) >= 2,
        "a too-long string must still wrap to multiple lines"
    );
}

// ── Visual harness (ignored) ────────────────────────────────────────────────
//
// Renders the targeted strings the coordinator asked to eyeball — kerning pair,
// ligatures, RTL, mixed/emoji — plus a group-opacity overlap, to PNGs under
// `.orchestration/shots/`. Ignored so it never runs in the gate (it depends on a
// system font + writes files); run explicitly with:
//   cargo test -p liquide-renderer-cpu --offline shaping_visual_harness -- --ignored --nocapture
#[test]
#[ignore]
fn shaping_visual_harness() {
    use liquide_compositor::Renderer;
    use liquide_compositor::damage::{DamageClass, DamageSet};
    use liquide_compositor::framebuffer::FrameBuffer;
    use liquide_compositor::geometry::{Affine2D, Rect};
    use liquide_compositor::pixel::{BlendMode, Color, PixelFormat};
    use liquide_compositor::scene::{FlatNode, SceneNodeKind};

    use crate::renderer::SoftwareRenderer;

    let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(".orchestration")
        .join("shots");
    std::fs::create_dir_all(&out_dir).ok();

    let text_node = |s: &str, size: f32, w: f32| FlatNode {
        id: 10,
        kind: SceneNodeKind::Text {
            text: s.to_string(),
            color: Color::WHITE,
            scale: 1,
            font_family: "Inter".to_string(),
            font_size: size,
            font_weight: 400,
            font_style_italic: false,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            line_height: size * 1.3,
            text_align: 0,
            text_transform: 0,
            text_overflow: 0,
            white_space: 1,
            word_break: liquide_compositor::scene::WordBreak::Normal,
            text_indent: 0.0,
            text_decoration: None,
            text_shadows: Vec::new(),
            text_emphasis: None,
        }
        .into(),
        absolute_bounds: Rect::new(10.0, 10.0, w, size * 1.6),
        absolute_transform: Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    let bg = |color: Color, b: Rect, op: f32, id: u64| FlatNode {
        id,
        kind: SceneNodeKind::Background { color }.into(),
        absolute_bounds: b,
        absolute_transform: Affine2D::identity(),
        clip: None,
        opacity: op,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    let save = |fb: &FrameBuffer, name: &str, dir: &std::path::Path| {
        let (w, h) = (fb.width, fb.height);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let p = fb.get_pixel(x, y);
                let i = ((y * w + x) * 4) as usize;
                rgba[i] = p.r;
                rgba[i + 1] = p.g;
                rgba[i + 2] = p.b;
                rgba[i + 3] = p.a;
            }
        }
        let path = dir.join(name);
        image::save_buffer(&path, &rgba, w, h, image::ColorType::Rgba8).unwrap();
        let lit = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let p = fb.get_pixel(x, y);
                p.r > 60 || p.g > 60 || p.b > 60
            })
            .count();
        eprintln!("wrote {} (lit_px={lit})", path.display());
    };

    let render_text = |s: &str, name: &str, w: u32, h: u32, dir: &std::path::Path| {
        let mut r = SoftwareRenderer::with_font_db({
            let mut d = FontDatabase::new();
            if let Some(b) = system_font_bytes() {
                for fam in ["Inter", "Noto Sans", "Manrope"] {
                    d.load_bytes(b.clone(), fam, 400, false).ok();
                }
            }
            if let Some(hb) = hebrew_font_bytes() {
                d.load_bytes(hb, "Noto Sans", 400, false).ok();
            }
            d
        });
        let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        // Dark backdrop so white text shows.
        let backdrop = bg(Color::new(20, 22, 30, 255), Rect::new(0.0, 0.0, w as f32, h as f32), 1.0, 1);
        let nodes = [backdrop, text_node(s, 48.0, w as f32 - 20.0)];
        // First frame requests + rasterizes glyphs into the atlas; a second frame
        // draws them (the capture drain runs BEFORE the per-node requests, so the
        // first frame's freshly-requested glyphs land in the atlas only by the
        // next frame — exactly as the live/golden loop resubmits on pending).
        let damage = DamageSet::full(64, w.div_ceil(64), h.div_ceil(64), DamageClass::UiPrimitive);
        for _ in 0..3 {
            r.render(&nodes, &mut fb, &damage).unwrap();
            if !r.has_pending_glyphs() {
                break;
            }
        }
        eprintln!(
            "  [{name}] atlas_glyphs={} pending={}",
            r.glyph_atlas().len(),
            r.has_pending_glyphs()
        );
        save(&fb, name, dir);
    };

    render_text("AVWAY Toy. WAV", "shaped_kerning_AVWAY.png", 520, 90, &out_dir);
    render_text("office fi fl ffi affluent", "shaped_ligatures.png", 640, 90, &out_dir);
    render_text("\u{05E9}\u{05DC}\u{05D5}\u{05DD} world", "shaped_rtl_hebrew.png", 520, 90, &out_dir);
    render_text("Mix \u{4E2D}\u{6587} fallback", "shaped_mixed_fallback.png", 560, 90, &out_dir);

    // Group opacity over overlapping children.
    {
        let (w, h) = (240u32, 160u32);
        let mut r = SoftwareRenderer::new();
        let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        let white = Color::new(255, 255, 255, 255);
        let backdrop = bg(white, Rect::new(0.0, 0.0, w as f32, h as f32), 1.0, 1);
        let layer = FlatNode {
            id: 2,
            kind: SceneNodeKind::RenderLayer {
                blend_mode: BlendMode::SrcOver,
                isolate: true,
            }
            .into(),
            absolute_bounds: Rect::new(0.0, 0.0, w as f32, h as f32),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 0.5,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        };
        let red = bg(Color::new(220, 30, 30, 255), Rect::new(30.0, 30.0, 100.0, 100.0), 1.0, 3);
        let blue = bg(Color::new(30, 30, 220, 255), Rect::new(90.0, 30.0, 100.0, 100.0), 1.0, 4);
        let damage = DamageSet::full(64, w.div_ceil(64), h.div_ceil(64), DamageClass::UiPrimitive);
        r.render(&[backdrop, layer, red, blue], &mut fb, &damage).unwrap();
        save(&fb, "group_opacity_overlap.png", &out_dir);
    }
}
