//! Live text shaping for the CPU renderer.
//!
//! This module is the wiring that finally routes the project's real shaping
//! engine — `rustybuzz` OpenType shaping (GSUB/GPOS: kerning, ligatures,
//! contextual forms) via [`liquide_font_rasterizer::shaper::TextShaper`], plus
//! the Unicode bidirectional algorithm from [`liquide_text_engine::bidi`] — into
//! the renderer's live text-draw path. Before this, `renderer/text.rs` laid text
//! out one `char` at a time with `glyph_id = ch as u32` and naive estimated
//! advances, so kerning/ligatures never formed, right-to-left text rendered in
//! logical (wrong) order, and a codepoint missing from the primary face fell
//! straight to the 8x16 bitmap instead of a covering fallback font.
//!
//! [`shape_line`] takes a single already-wrapped visual line and returns its
//! glyphs in **visual left-to-right order**, each tagged with the concrete
//! [`FontFaceId`] it was shaped/rasterized from (so per-glyph multi-font fallback
//! reaches the atlas) and the real font glyph id (so ligature/substituted glyphs
//! reach the rasterizer). The output is a pure function of the text + font
//! database, so an identical scene shapes to identical glyphs every run
//! (determinism).

use liquide_font_rasterizer::database::{FontDatabase, FontFaceId};
use liquide_font_rasterizer::shaper::{FontFeature, TextShaper};
use liquide_text_engine::bidi::{BidiResolver, Direction};

/// One positioned, shaped glyph ready for atlas keying + blit.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ShapedRunGlyph {
    /// Concrete font face this glyph was shaped/rasterized from (primary or a
    /// fallback face). Folded into the atlas `font_id` so a fallback glyph keys
    /// distinctly from a primary-face glyph of the same id.
    pub face_id: FontFaceId,
    /// Real font glyph id (NOT the codepoint) — a ligature or substituted glyph
    /// has an id that corresponds to no single input codepoint.
    pub glyph_id: u32,
    /// First codepoint of the cluster this glyph represents (diagnostics / space
    /// handling / fallback lookup).
    pub codepoint: char,
    /// X position of the glyph pen, relative to the start of the line (visual).
    pub x: f32,
    /// Horizontal advance of this glyph. The renderer positions glyphs by their
    /// precomputed `x` (which already folds in advances + spacing), so the live
    /// blit path does not read this; it is retained as layout metadata and is
    /// asserted against the line width by the shaping tests.
    #[allow(dead_code)]
    pub advance: f32,
}

/// Fallback families consulted, in order, when the primary face lacks a glyph for
/// a codepoint. These are the concrete UI/text families the font database loads
/// (and the embedded fallback registers under); `Noto Sans` is the broad-coverage
/// catch-all. Kept deliberately small and deterministic.
const FALLBACK_FAMILIES: &[&str] = &["Noto Sans", "Inter", "Manrope", "JetBrains Mono", "Roboto"];

/// Standard live-text shaping features: kerning + standard ligatures + contextual
/// alternates on (the CSS defaults for `normal` text rendering).
fn default_features() -> [FontFeature; 3] {
    [
        FontFeature::kerning(true),
        FontFeature::ligatures(true),
        FontFeature::contextual_alternates(true),
    ]
}

/// Whether `text` contains any codepoint that could require bidirectional
/// reordering (RTL scripts, Arabic-Indic, bidi control marks). Pure-LTR text
/// (the common UI case) returns `false` so the caller can skip the full Unicode
/// bidi algorithm. Conservative: any char ≥ U+0590 (start of Hebrew) triggers the
/// full path, which is correct (it never *misses* RTL) and cheap to test.
fn needs_bidi(text: &str) -> bool {
    text.chars().any(|c| {
        let u = c as u32;
        // Hebrew, Arabic, Syriac, Thaana, NKo, … and the bidi control range. Also
        // catch the explicit bidi formatting controls below U+0590.
        u >= 0x0590 || matches!(u, 0x200E | 0x200F | 0x202A..=0x202E | 0x2066..=0x2069)
    })
}

/// Does `face` have a real (non-`.notdef`) glyph for `ch`?
fn face_covers(db: &FontDatabase, face_id: FontFaceId, ch: char) -> bool {
    use ab_glyph::Font;
    match db.get(face_id) {
        Some(face) => face.font.glyph_id(ch).0 != 0,
        None => false,
    }
}

/// Pick a fallback face that covers `ch`, or `None` if none of the known
/// fallback families do. Skips `primary` (already known not to cover `ch`).
fn fallback_face_for(
    db: &FontDatabase,
    primary: FontFaceId,
    ch: char,
    weight: u16,
    italic: bool,
) -> Option<FontFaceId> {
    for fam in FALLBACK_FAMILIES {
        if let Some(fid) = db.resolve(fam, weight, italic) {
            if fid != primary && face_covers(db, fid, ch) {
                return Some(fid);
            }
        }
    }
    None
}

/// Shape one already-wrapped visual line of text into ordered glyphs.
///
/// * Bidi: the line is split into directional runs and the runs are laid out in
///   visual order, so an Arabic/Hebrew run renders right-to-left.
/// * Shaping: each run is shaped with rustybuzz (kerning/ligatures/contextual).
/// * Fallback: any `.notdef` glyph whose codepoint a fallback face covers is
///   reshaped from that fallback face, so the covering glyph reaches the atlas.
///
/// `letter_spacing` and `word_spacing` are applied per glyph/space exactly as the
/// legacy path did, so spacing-driven layouts are preserved. Returns visual-order
/// glyphs with line-relative x positions plus the line's total advance width.
pub(crate) fn shape_line(
    db: &FontDatabase,
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    letter_spacing: f32,
    word_spacing: f32,
) -> (Vec<ShapedRunGlyph>, f32) {
    if text.is_empty() {
        return (Vec::new(), 0.0);
    }

    let primary = db
        .resolve(font_family, font_weight, italic)
        .unwrap_or(FontFaceId::FALLBACK);

    let shaper = TextShaper::new(db);
    let features = default_features();

    let mut out: Vec<ShapedRunGlyph> = Vec::with_capacity(text.len());
    let mut pen_x = 0.0_f32;

    // Fast path: text with no right-to-left / bidi-relevant codepoints is a single
    // left-to-right run, so skip the full Unicode bidi algorithm (a measurable
    // per-line cost on the live render path) and shape the whole line at once. The
    // vast majority of UI strings are LTR, so this keeps the common case cheap
    // while still routing complex/RTL text through the real bidi reordering below.
    let runs: Vec<(usize, usize, Direction)> = if needs_bidi(text) {
        // Bidi: resolve directional runs and reorder them for visual display. The
        // resolver returns runs in logical order; `visual_reorder` applies the L2
        // rule (reverse contiguous runs at each level) so iterating the result
        // places runs left-to-right on screen.
        let para = BidiResolver::resolve(text, None);
        let visual_runs = BidiResolver::visual_reorder(&para.runs);
        if visual_runs.is_empty() {
            vec![(0, text.len(), Direction::Ltr)]
        } else {
            visual_runs
                .iter()
                .map(|r| (r.start, r.end, r.direction()))
                .collect()
        }
    } else {
        vec![(0, text.len(), Direction::Ltr)]
    };

    for (start, end, _dir) in runs {
        let run_text = match text.get(start..end) {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };

        // Shape the run with the primary face. rustybuzz auto-detects the run's
        // script/direction from its content, so an RTL run's glyphs come back in
        // visual (left-to-right in memory) order — we lay them out at increasing
        // x and the run reads right-to-left on screen.
        let (glyphs, _w) =
            shaper.shape_with_features(primary, run_text, font_size, letter_spacing, &features);

        for g in &glyphs {
            let mut face_id = primary;
            let mut glyph_id = g.glyph_id;
            let mut advance = g.x_advance;

            // Per-glyph multi-font fallback: a `.notdef` (glyph id 0) for a
            // non-space codepoint that a fallback face covers is reshaped from
            // that fallback face, so a real covering glyph reaches the atlas
            // instead of the .notdef box / 8x16 bitmap.
            if glyph_id == 0 && !g.codepoint.is_whitespace() && g.codepoint != '\0' {
                if let Some(fb_face) =
                    fallback_face_for(db, primary, g.codepoint, font_weight, italic)
                {
                    // Reshape just this cluster's codepoint with the fallback face.
                    let mut buf = [0u8; 4];
                    let cluster_str = g.codepoint.encode_utf8(&mut buf);
                    let (fb_glyphs, _) = shaper.shape_with_features(
                        fb_face,
                        cluster_str,
                        font_size,
                        letter_spacing,
                        &features,
                    );
                    if let Some(fg) = fb_glyphs.first() {
                        face_id = fb_face;
                        glyph_id = fg.glyph_id;
                        advance = fg.x_advance;
                    }
                }
            }

            // Apply word-spacing to space glyphs (letter-spacing is already folded
            // into the shaped advance by the shaper).
            let extra = if g.codepoint == ' ' { word_spacing } else { 0.0 };

            out.push(ShapedRunGlyph {
                face_id,
                glyph_id,
                codepoint: g.codepoint,
                x: pen_x,
                advance,
            });
            pen_x += advance + extra;
        }
    }

    (out, pen_x)
}
