//! Capability test — TEXT SHAPING reaches pixels (test-harden, Part A.1).
//!
//! The audit (au3 bug #4) found the live render path did naive per-codepoint
//! layout (`glyph_id = ch as u32`) so the rustybuzz shaping engine — kerning,
//! ligatures, GSUB/GPOS — never reached pixels. The shaping-wire fix landed; this
//! file pins that fix with TEETH on two independent axes:
//!
//! 1. **Shaper unit teeth** (no golden): a kerning-pair string shaped with
//!    kerning+ligatures ENABLED is strictly TIGHTER than the same string shaped
//!    with those features DISABLED (the naive per-codepoint advance sum). And a
//!    ligature string ("office", "fi", "fl") shapes to FEWER glyphs than chars —
//!    proof a real `ffi`/`fi`/`fl` ligature substitution fired. Reverting the
//!    shaper to naive advances collapses BOTH (equal widths / glyph-count == char
//!    count) and the asserts fail.
//!
//! 2. **Pixel golden**: a shaped string is rendered through the REAL Shell →
//!    SoftwareRenderer pipeline and pinned. The golden was rendered + visually
//!    inspected before blessing (the "ffi"/"fi"/"fl" ligatures are visible as
//!    single glyphs; AVWAY kerns tight).
//!
//! Bless: `LIQUIDE_UPDATE_GOLDEN=1 cargo test -p liquide-visual-test --test cap_text_shaping`

use liquide_components::TemplateNode;
use liquide_font_rasterizer::{FontDatabase, FontFeature, TextShaper};
use liquide_visual_test::golden::assert_golden;
use liquide_visual_test::primitive_render::render_fragment;
use liquide_visual_test::scenarios::crate_test_assets_dir;

/// Load the pinned deterministic test font (same one the chrome scenarios use).
fn font_db() -> FontDatabase {
    let mut db = FontDatabase::new();
    let n = db.load_default_fonts(crate_test_assets_dir());
    assert!(
        n > 0,
        "no font faces loaded from {:?} — the pinned InterVariable test font \
         must be present for the shaping teeth",
        crate_test_assets_dir().join("fonts")
    );
    db
}

/// THE KERNING TOOTH: a kerning-pair-heavy string shaped with kerning enabled is
/// strictly NARROWER than the naive per-codepoint advance sum (kerning disabled).
///
/// "AVWAY" packs A/V, V/W, W/A, A/Y pairs — every one a negative-kern pair in a
/// proportional font. With shaping, the painted run is tighter; with naive
/// per-codepoint advances (the pre-fix renderer behaviour) the two widths are
/// IDENTICAL and this fails.
#[test]
fn kerning_tightens_advance_vs_naive() {
    let db = font_db();
    let face = db
        .resolve("sans-serif", 400, false)
        .or_else(|| db.resolve("Inter", 400, false))
        .expect("a sans-serif face must resolve");
    let shaper = TextShaper::new(&db);

    let text = "AVWAY";
    let size = 64.0;

    // Shaped (default features = kerning + ligatures ON).
    let (_g_shaped, w_shaped) = shaper.shape(face, text, size, 0.0);

    // Naive: kerning + ligatures + contextual alternates explicitly OFF — this is
    // the sum-of-bare-advances the pre-fix per-codepoint path produced.
    let naive_features = [
        FontFeature::kerning(false),
        FontFeature::ligatures(false),
        FontFeature::contextual_alternates(false),
    ];
    let (_g_naive, w_naive) = shaper.shape_with_features(face, text, size, 0.0, &naive_features);

    assert!(
        w_shaped > 0.0 && w_naive > 0.0,
        "both shaped ({w_shaped}) and naive ({w_naive}) widths must be positive"
    );
    // TEETH: a kerning-pair string MUST shape tighter than the naive sum. A
    // strict inequality (not >=) so a no-op kern fails. Allow a tiny epsilon so
    // an exactly-zero-kern font does not false-fail — but InterVariable kerns.
    assert!(
        w_shaped < w_naive - 0.5,
        "kerning did not tighten 'AVWAY': shaped width {w_shaped:.2} is not \
         narrower than the naive per-codepoint advance sum {w_naive:.2}. The \
         renderer is using naive advances (shaping not wired) — au3 bug #4 has \
         regressed."
    );
}

/// THE LIGATURE TOOTH: a string with ligature pairs shapes to FEWER glyphs than
/// it has characters — proof a GSUB ligature substitution (ffi / fi / fl) fired.
///
/// "office" has the "ffi" cluster; "fi"/"fl" are classic ligatures. With shaping,
/// `glyph_count < char_count`. Naive per-codepoint layout emits one glyph per
/// char (`glyph_count == char_count`) and this fails.
#[test]
fn ligatures_collapse_glyph_count() {
    let db = font_db();
    let face = db
        .resolve("sans-serif", 400, false)
        .or_else(|| db.resolve("Inter", 400, false))
        .expect("a sans-serif face must resolve");
    let shaper = TextShaper::new(&db);

    // Enable discretionary + standard ligatures explicitly so the assertion does
    // not depend on the default feature set spelling.
    let liga = [FontFeature::ligatures(true)];
    let text = "office fi fl affix";
    let char_count = text.chars().count();
    let (glyphs, _w) = shaper.shape_with_features(face, text, 48.0, 0.0, &liga);

    assert!(
        glyphs.len() < char_count,
        "no ligatures fired for {text:?}: shaped to {} glyphs for {char_count} \
         chars (expected fewer — ffi/fi/fl should each collapse). The shaper is \
         not running GSUB (shaping not wired) — au3 bug #4 has regressed.",
        glyphs.len()
    );
}

/// PIXEL GOLDEN: a shaped string rendered through the real pipeline.
///
/// Rendered with `white-space: nowrap` so the run stays on one line (the harness
/// uses the default text measurer, matching the live desktop capture path; nowrap
/// keeps the golden a single readable line). Inspected before blessing: the
/// "ffi" in "office" and the "fi"/"fl" pairs render as ligature glyphs.
#[test]
fn shaped_string_paints_golden() {
    let label = TemplateNode::el("div")
        .style("position", "absolute")
        .style("left", "16px")
        .style("top", "30px")
        .style("font-size", "40px")
        .style("color", "#ffffff")
        .style("white-space", "nowrap")
        .child(TemplateNode::text("AVWAY office fi fl"));

    let frame = render_fragment(640, 110, "#101820", label);

    // Content sanity: the run paints a substantial body of glyph ink.
    let non_bg = frame.non_background_pixels([0x10, 0x18, 0x20, 0xff], 24);
    assert!(
        non_bg > 600,
        "shaped string painted only {non_bg} non-background pixels — text not \
         rendering."
    );

    assert_golden("cap_text_shaping_string", &frame);
}
