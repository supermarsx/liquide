//! Text rendering for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::BlendMode;
use liquide_compositor::scene::FlatNode;

use crate::glyph::GlyphKey;
use crate::rasterizer;

use super::{SoftwareRenderer, WordSplitter};
use super::text_shaping::{self, ShapedRunGlyph};

/// Derive a collision-resistant 32-bit `font_id` from a (family, weight,
/// italic) tuple for use as the `GlyphKey::font_id` discriminator.
///
/// This is the single source of truth for the renderer-local font identity used
/// to key the glyph atlas. It folds the full family name, the full 16-bit
/// weight, and the italic flag through FNV-1a so that distinct font selections
/// never alias to the same atlas key (which would return the wrong glyph for a
/// distinct font and garble text). It is a pure function of its inputs, so the
/// id is stable across runs (no nondeterminism). `size_px` and the glyph
/// codepoint are tracked separately in `GlyphKey`, so they are intentionally
/// not folded in here.
pub(crate) fn compute_font_id(font_family: &str, font_weight: u16, italic: bool) -> u32 {
    const FNV_OFFSET: u32 = 0x811c_9dc5;
    const FNV_PRIME: u32 = 0x0100_0193;
    let mut h = FNV_OFFSET;
    for b in font_family.bytes() {
        h = (h ^ b as u32).wrapping_mul(FNV_PRIME);
    }
    // Separator so "family"+weight cannot collide with a differently-split tuple.
    h = (h ^ 0xFF).wrapping_mul(FNV_PRIME);
    h = (h ^ (font_weight as u32 & 0xFF)).wrapping_mul(FNV_PRIME);
    h = (h ^ ((font_weight as u32 >> 8) & 0xFF)).wrapping_mul(FNV_PRIME);
    h = (h ^ u32::from(italic)).wrapping_mul(FNV_PRIME);
    h
}

/// Derive an atlas `font_id` for a SHAPED glyph keyed on the concrete font face
/// it was rasterized from.
///
/// Shaped glyphs are keyed by their REAL font glyph id (not codepoint), and
/// per-glyph fallback means two glyphs in one run may come from different faces.
/// The atlas `font_id` must therefore distinguish (a) the exact face a glyph came
/// from, and (b) shaped entries from the legacy codepoint-keyed entries (whose
/// `font_id` comes from [`compute_font_id`] over a family string). We fold the
/// raw `FontFaceId` through FNV-1a with a distinct domain tag so a shaped glyph id
/// `N` from face `F` never aliases a legacy codepoint entry, and two faces never
/// share an id. Pure function of its inputs → stable across runs (determinism).
pub(crate) fn compute_shaped_font_id(face_raw: u32, italic: bool) -> u32 {
    const FNV_OFFSET: u32 = 0x811c_9dc5;
    const FNV_PRIME: u32 = 0x0100_0193;
    let mut h = FNV_OFFSET;
    // Domain tag distinguishing shaped entries from family-hashed legacy entries.
    h = (h ^ 0x5A).wrapping_mul(FNV_PRIME);
    for b in face_raw.to_le_bytes() {
        h = (h ^ b as u32).wrapping_mul(FNV_PRIME);
    }
    h = (h ^ u32::from(italic)).wrapping_mul(FNV_PRIME);
    h
}

impl SoftwareRenderer {
    /// Render a Text scene node.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_text_node(&mut self, node: &FlatNode, fb: &mut FrameBuffer) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;
        // Confine glyph blits to the active damage region (None = full frame).
        let clip = self.raster_clip;

        if let liquide_compositor::scene::SceneNodeKind::Text {
            text,
            color,
            scale,
            font_family,
            font_size,
            font_weight,
            font_style_italic,
            letter_spacing,
            word_spacing,
            line_height,
            text_align,
            text_transform,
            text_overflow,
            white_space: _,
            word_break,
            text_indent,
            text_decoration,
            text_shadows,
            text_emphasis,
        } = node.kind_ref()
        {
            let mut c = *color;
            if opacity < 1.0 {
                c.a = (c.a as f32 * opacity + 0.5) as u8;
            }

            // Apply text-transform before rendering
            let transformed: std::borrow::Cow<'_, str> = match text_transform {
                2 => std::borrow::Cow::Owned(text.to_uppercase()),
                3 => std::borrow::Cow::Owned(text.to_lowercase()),
                1 => {
                    let mut result = String::with_capacity(text.len());
                    let mut cap_next = true;
                    for ch in text.chars() {
                        if ch.is_whitespace() {
                            cap_next = true;
                            result.push(ch);
                        } else if cap_next {
                            result.extend(ch.to_uppercase());
                            cap_next = false;
                        } else {
                            result.push(ch);
                        }
                    }
                    std::borrow::Cow::Owned(result)
                }
                _ => std::borrow::Cow::Borrowed(text.as_str()),
            };
            let render_text = &*transformed;

            // Determine effective glyph height
            let glyph_height = if *font_size > 0.0 {
                (*font_size).ceil() as u32
            } else {
                16 * scale.max(&1)
            };

            // Derive a collision-resistant font_id from the full (family,
            // weight, italic) tuple.
            //
            // The old packing — `italic<<24 | (weight & 0xFF)<<16 | (hash &
            // 0xFFFF)` — truncated the weight to 8 bits (so weights ≥256, and
            // any two weights congruent mod 256, aliased to the SAME id) and
            // squeezed the family hash into 16 bits (raising the family-vs-family
            // collision rate). A collision returns the WRONG glyph from the atlas
            // for a distinct (family,weight,italic) combo, garbling text. We hash
            // the entire tuple into the full 32-bit space (FNV-1a) so distinct
            // combos map to distinct ids; `size_px` and `glyph_id` are separate
            // fields of `GlyphKey`, so they need not be folded in here. The hash
            // is a pure function of the inputs, so the id is identical run-to-run
            // (no nondeterminism introduced).
            let font_id = compute_font_id(font_family, *font_weight, *font_style_italic);

            let size_px = glyph_height as u16;
            let mut pen_y = bounds.y;
            let line_h = if *line_height > 0.0 {
                *line_height
            } else {
                glyph_height as f32 * 1.2
            };

            // Pre-warm common glyphs when enabled for this renderer.
            // NOTE: common-glyph prewarming is deferred until AFTER this run's
            // lines are shaped (see below). Prewarming enqueues async glyph
            // rasterizations that make the font worker grab the shared font-database
            // lock; shaping also needs that lock, so prewarming FIRST would make the
            // shape step block behind the worker on the first text frame (a
            // multi-millisecond stall on the live present path). Shaping first —
            // while the worker is still idle — keeps the lock uncontended.

            // Glyph requesting for the main text is handled per-line by the shaped
            // render path below (it shapes each visual line and requests each
            // SHAPED glyph id from its concrete face). The legacy per-codepoint
            // request loop is gone — it could not request ligature/substituted or
            // fallback-face glyph ids.

            // text-emphasis: marks are rendered as glyphs at ~50% of the text
            // size. Request the mark glyph up-front so it is in the atlas by the
            // time we draw it. The mark height is rounded to at least 1px.
            let emphasis_height: u32 = ((glyph_height as f32 * 0.5).round() as u32).max(1);
            let emphasis_size_px = emphasis_height as u16;
            if let Some(emph) = text_emphasis {
                if let Some(mark_ch) = emph.mark.chars().next() {
                    let key = GlyphKey {
                        font_id,
                        glyph_id: mark_ch as u32,
                        size_px: emphasis_size_px,
                        subpixel: false,
                    };
                    if self.glyph_atlas.get(&key).is_none() {
                        self.has_pending_glyphs = true;
                        self.font_worker.request_glyph_with_font(
                            key,
                            mark_ch,
                            emphasis_height,
                            font_family.clone(),
                            *font_weight,
                            false,
                        );
                    }
                }
            }

            // Track how many wrapped lines for per-line decoration
            let num_wrapped_lines: usize;

            // Render using the atlas
            {
                // Word-wrap aware line splitting
                let white_space_val = node.kind_ref().text_white_space().unwrap_or(0);
                let allows_wrap = matches!(white_space_val, 0 | 3 | 4 | 5);
                let max_line_width = bounds.width;

                // word-break: break-all / break-word allow breaking *inside* a
                // word (between any two characters) when a word would otherwise
                // overflow the line. keep-all / normal break only at word
                // boundaries (the default behaviour).
                use liquide_compositor::scene::WordBreak;
                let allow_intra_word_break =
                    matches!(word_break, WordBreak::BreakAll | WordBreak::BreakWord);
                // break-all may break even a word that *would* fit a fresh line;
                // break-word only breaks words too long to ever fit.
                let break_eagerly = matches!(word_break, WordBreak::BreakAll);

                // Hold the font-database lock across BOTH the wrap pre-pass and the
                // per-line shaping below. The wrap decision now measures candidate
                // runs with the SAME shaper (`shaped_run_width`) that `shape_line`
                // uses to position painted glyphs, so the width the wrap pre-pass
                // tests against equals the painted width — a run that fits its box
                // when shaped is no longer wrapped by a divergent estimate. (The
                // previous pre-pass summed a `char_advance` closure that looked the
                // glyph up by CODEPOINT in an atlas keyed by SHAPED glyph id, so the
                // key never matched and it always fell back to `glyph_height * 0.55`,
                // over-counting and wrapping text that actually fits.)
                let db = self
                    .font_worker
                    .font_db()
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());

                // Shaped width of a candidate run (word or single char), measured
                // with the same shaper as paint. Folds in word-spacing per space
                // and letter-spacing exactly as `shape_line` does.
                let run_width = |run: &str| -> f32 {
                    text_shaping::shaped_run_width(
                        &db,
                        run,
                        font_family,
                        glyph_height as f32,
                        *font_weight,
                        *font_style_italic,
                        *letter_spacing,
                        *word_spacing,
                    )
                };

                // Split into visual lines (pre-allocate for typical text)
                let mut wrapped_lines: Vec<String> = Vec::with_capacity(8);
                for hard_line in render_text.split('\n') {
                    if !allows_wrap || max_line_width <= 0.0 {
                        wrapped_lines.push(hard_line.to_string());
                    } else {
                        let indent = if wrapped_lines.is_empty() {
                            *text_indent
                        } else {
                            0.0
                        };
                        let mut current_line = String::with_capacity(hard_line.len());
                        let mut current_width = indent;

                        for word in WordSplitter::new(hard_line) {
                            let word_width: f32 = run_width(word);

                            // Decide whether this word must be broken character
                            // by character. With break-all every word is a
                            // candidate; with break-word only words too wide to
                            // fit a line on their own are split. A pure space run
                            // is never intra-broken.
                            let is_space_run = word.starts_with(' ');
                            let must_split = allow_intra_word_break
                                && !is_space_run
                                && word_width > max_line_width
                                && (break_eagerly || word_width > max_line_width);

                            if must_split {
                                // Flush whatever is on the current line first.
                                if !current_line.is_empty() {
                                    let trimmed = current_line.trim_end();
                                    wrapped_lines.push(trimmed.to_string());
                                    current_line.clear();
                                    current_line.reserve(hard_line.len());
                                    current_width = 0.0;
                                }
                                // Emit characters, wrapping whenever the next
                                // character would overflow the line.
                                let mut buf = [0u8; 4];
                                for ch in word.chars() {
                                    let cw = run_width(ch.encode_utf8(&mut buf));
                                    if !current_line.is_empty()
                                        && current_width + cw > max_line_width
                                    {
                                        wrapped_lines.push(current_line.clone());
                                        current_line.clear();
                                        current_line.reserve(hard_line.len());
                                        current_width = 0.0;
                                    }
                                    current_line.push(ch);
                                    current_width += cw;
                                }
                                continue;
                            }

                            if !current_line.is_empty()
                                && current_width + word_width > max_line_width
                            {
                                let trimmed = current_line.trim_end();
                                wrapped_lines.push(trimmed.to_string());
                                current_line.clear();
                                current_line.reserve(hard_line.len());
                                current_width = 0.0;
                            }

                            current_line.push_str(word);
                            current_width += word_width;
                        }
                        if !current_line.is_empty() {
                            let trimmed = current_line.trim_end();
                            wrapped_lines.push(trimmed.to_string());
                        } else if hard_line.is_empty() {
                            wrapped_lines.push(String::new());
                        }
                    }
                }

                num_wrapped_lines = wrapped_lines.len().max(1);

                // ── Shape each visual line ONCE via the rustybuzz/bidi engine ──
                //
                // This is the live shaping seam: each wrapped line is shaped with
                // OpenType (kerning/ligatures/contextual) + the Unicode bidi
                // algorithm + per-glyph multi-font fallback, producing glyphs in
                // VISUAL left-to-right order (so RTL renders right-to-left). The
                // shaped advances drive alignment and the shaped glyph ids/faces
                // drive atlas keying + rasterization. Shaping is a pure function of
                // (text, font database), so the result is identical run-to-run.
                // Reuses the font-db lock already held for the wrap pre-pass, so the
                // wrap measurement and the paint shaping see the identical database.
                let mut shaped_lines: Vec<(Vec<ShapedRunGlyph>, f32)> =
                    Vec::with_capacity(wrapped_lines.len());
                for line_text in &wrapped_lines {
                    // Drop a stray CR; shaping operates on the visual line text.
                    let clean: String = line_text.chars().filter(|&c| c != '\r').collect();
                    let shaped = text_shaping::shape_line(
                        &db,
                        &clean,
                        font_family,
                        glyph_height as f32,
                        *font_weight,
                        *font_style_italic,
                        *letter_spacing,
                        *word_spacing,
                    );
                    shaped_lines.push(shaped);
                }
                // Release the font-database lock before glyph requests / blits so
                // the font worker can rasterize the requested glyphs.
                drop(db);

                // Common-glyph prewarm: warm the SHAPED atlas keys for the run's
                // primary face so a freshly-seen (family, size) seeds the common
                // ASCII/Latin glyphs the next frames will use, without re-requesting
                // per frame. This runs AFTER shaping so the font-database lock is
                // uncontended during shape (prewarm enqueues async rasterizations
                // that make the worker grab the same lock). Prewarm only upright
                // runs (the prewarm path requests italic=false glyphs).
                let prewarm_key = (font_id, size_px);
                if self.common_glyph_prewarm_enabled()
                    && !font_family.is_empty()
                    && !*font_style_italic
                    && !self.prewarmed_fonts.contains(&prewarm_key)
                {
                    self.prewarmed_fonts.insert(prewarm_key);
                    self.prewarm_shaped_glyphs(
                        size_px,
                        glyph_height,
                        font_family,
                        *font_weight,
                        *font_style_italic,
                    );
                }

                // Request every shaped glyph by its REAL id from its concrete face,
                // so ligature/substituted/fallback glyphs reach the atlas. Whitespace
                // glyphs with no outline still get requested (the rasterizer returns
                // an empty bitmap with the correct advance — harmless, and keyed so
                // we don't re-request). The `font_size` (not the ceil'd glyph_height)
                // is the shaping size; the atlas key size is the integer cell height.
                for (glyphs, _w) in &shaped_lines {
                    for g in glyphs {
                        let key = GlyphKey {
                            font_id: compute_shaped_font_id(g.face_id.0, *font_style_italic),
                            glyph_id: g.glyph_id,
                            size_px,
                            subpixel: false,
                        };
                        if self.glyph_atlas.get(&key).is_none() {
                            self.has_pending_glyphs = true;
                            self.font_worker.request_shaped_glyph(
                                key,
                                g.face_id,
                                g.codepoint,
                                glyph_height,
                            );
                        }
                    }
                }

                // Helper: align offset for a shaped line of width `lw`.
                let align_offset = |lw: f32, is_first: bool| -> f32 {
                    let indent = if is_first { *text_indent } else { 0.0 };
                    let base = match text_align {
                        1 => ((bounds.width - lw) / 2.0).max(0.0),
                        2 => (bounds.width - lw).max(0.0),
                        _ => 0.0,
                    };
                    base + indent
                };

                // ── Text shadows BEFORE the main text (CSS: shadows behind text) ──
                if !text_shadows.is_empty() {
                    for shadow in text_shadows {
                        let mut shadow_c = shadow.color;
                        if opacity < 1.0 {
                            shadow_c.a = (shadow_c.a as f32 * opacity + 0.5) as u8;
                        }
                        if shadow_c.a == 0 {
                            continue;
                        }
                        let sx = shadow.offset_x;
                        let sy = shadow.offset_y;
                        let mut s_pen_y = bounds.y + sy;
                        for (i, (glyphs, lw)) in shaped_lines.iter().enumerate() {
                            let ax = align_offset(*lw, i == 0);
                            let line_x = bounds.x + ax + sx;
                            for g in glyphs {
                                let key = GlyphKey {
                                    font_id: compute_shaped_font_id(
                                        g.face_id.0,
                                        *font_style_italic,
                                    ),
                                    glyph_id: g.glyph_id,
                                    size_px,
                                    subpixel: false,
                                };
                                if let Some(cached) = self.glyph_atlas.get(&key) {
                                    let pos = liquide_compositor::geometry::Point::new(
                                        line_x + g.x,
                                        s_pen_y + glyph_height as f32,
                                    );
                                    self.glyph_atlas.blit_glyph(
                                        fb,
                                        cached,
                                        pos,
                                        shadow_c,
                                        clip,
                                        &self.srgb_lut,
                                    );
                                }
                            }
                            s_pen_y += line_h;
                        }
                    }
                }

                // ── Main shaped text pass ──
                let max_x = bounds.x + bounds.width;
                for (i, (glyphs, lw)) in shaped_lines.iter().enumerate() {
                    let is_first_line = i == 0;
                    let ax = align_offset(*lw, is_first_line);
                    let line_x = bounds.x + ax;
                    // Text overflow: ellipsis (1) when the shaped line exceeds the box.
                    let use_ellipsis = *text_overflow == 1 && *lw > bounds.width;

                    for g in glyphs {
                        let gx = line_x + g.x;
                        // Ellipsis: stop and draw "…" once we near the right edge.
                        if use_ellipsis && gx + glyph_height as f32 * 0.6 > max_x {
                            let ellipsis_key = GlyphKey {
                                font_id,
                                glyph_id: '\u{2026}' as u32,
                                size_px,
                                subpixel: false,
                            };
                            // Request the ellipsis from the legacy codepoint path so
                            // it is available regardless of shaping.
                            if self.glyph_atlas.get(&ellipsis_key).is_none() {
                                self.has_pending_glyphs = true;
                                self.font_worker.request_glyph_with_font(
                                    ellipsis_key,
                                    '\u{2026}',
                                    glyph_height,
                                    font_family.clone(),
                                    *font_weight,
                                    *font_style_italic,
                                );
                            }
                            if let Some(cached) = self.glyph_atlas.get(&ellipsis_key) {
                                let pos = liquide_compositor::geometry::Point::new(
                                    gx,
                                    pen_y + glyph_height as f32,
                                );
                                self.glyph_atlas.blit_glyph(
                                    fb,
                                    cached,
                                    pos,
                                    c,
                                    clip,
                                    &self.srgb_lut,
                                );
                            }
                            break;
                        }

                        let key = GlyphKey {
                            font_id: compute_shaped_font_id(g.face_id.0, *font_style_italic),
                            glyph_id: g.glyph_id,
                            size_px,
                            subpixel: false,
                        };
                        if let Some(cached) = self.glyph_atlas.get(&key) {
                            let pos = liquide_compositor::geometry::Point::new(
                                gx,
                                pen_y + glyph_height as f32,
                            );
                            let advance = cached.advance;
                            self.glyph_atlas
                                .blit_glyph(fb, cached, pos, c, clip, &self.srgb_lut);

                            // text-emphasis: draw the mark centered over (or under)
                            // each non-space glyph. Skip whitespace — emphasis marks
                            // are not drawn on separators (CSS Text Deco 3).
                            if let Some(emph) = text_emphasis {
                                if !g.codepoint.is_whitespace() {
                                    if let Some(mark_ch) = emph.mark.chars().next() {
                                        let mark_key = GlyphKey {
                                            font_id,
                                            glyph_id: mark_ch as u32,
                                            size_px: emphasis_size_px,
                                            subpixel: false,
                                        };
                                        if let Some(mark_glyph) = self.glyph_atlas.get(&mark_key) {
                                            let mut mc = emph.color.unwrap_or(c);
                                            if opacity < 1.0 {
                                                mc.a = (mc.a as f32 * opacity + 0.5) as u8;
                                            }
                                            let mark_x =
                                                gx + (advance - mark_glyph.advance) * 0.5;
                                            use liquide_compositor::scene::TextEmphasisPosition;
                                            let mark_y = match emph.position {
                                                TextEmphasisPosition::Over => pen_y,
                                                TextEmphasisPosition::Under => {
                                                    pen_y
                                                        + glyph_height as f32
                                                        + emphasis_height as f32
                                                }
                                            };
                                            let mark_pos = liquide_compositor::geometry::Point::new(
                                                mark_x, mark_y,
                                            );
                                            self.glyph_atlas.blit_glyph(
                                                fb,
                                                mark_glyph,
                                                mark_pos,
                                                mc,
                                                clip,
                                                &self.srgb_lut,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    pen_y += line_h;
                }
            }

            // Text decoration (underline, overline, line-through)
            // Drawn per-line so multi-line text gets decorations on every line.
            if let Some(deco) = text_decoration {
                use liquide_compositor::scene::{TextDecorationLine, TextDecorationStyle};

                if deco.line != TextDecorationLine::None {
                    let deco_color = deco.color.unwrap_or(c);
                    let thickness = if deco.thickness > 0.0 {
                        deco.thickness
                    } else {
                        (glyph_height as f32 / 14.0).max(1.0).round()
                    };

                    // Offsets relative to each line's top
                    let mut deco_offsets: Vec<f32> = Vec::with_capacity(3);

                    match deco.line {
                        TextDecorationLine::Underline => {
                            deco_offsets.push(glyph_height as f32 * 0.9);
                        }
                        TextDecorationLine::Overline => {
                            deco_offsets.push(glyph_height as f32 * 0.15);
                        }
                        TextDecorationLine::LineThrough => {
                            deco_offsets.push(glyph_height as f32 * 0.55);
                        }
                        TextDecorationLine::UnderlineOverline => {
                            deco_offsets.push(glyph_height as f32 * 0.9);
                            deco_offsets.push(glyph_height as f32 * 0.15);
                        }
                        TextDecorationLine::None => {}
                    }

                    for line_idx in 0..num_wrapped_lines {
                        let line_top = bounds.y + line_idx as f32 * line_h;
                        for &offset_y in &deco_offsets {
                            let line_y = line_top + offset_y;
                            match deco.style {
                                TextDecorationStyle::Solid => {
                                    let deco_rect =
                                        Rect::new(bounds.x, line_y, bounds.width, thickness);
                                    rasterizer::fill_rect(
                                        fb,
                                        deco_rect,
                                        deco_color,
                                        BlendMode::SrcOver,
                                    );
                                }
                                TextDecorationStyle::Double => {
                                    let deco_rect1 =
                                        Rect::new(bounds.x, line_y, bounds.width, thickness);
                                    let deco_rect2 = Rect::new(
                                        bounds.x,
                                        line_y + thickness * 2.0,
                                        bounds.width,
                                        thickness,
                                    );
                                    rasterizer::fill_rect(
                                        fb,
                                        deco_rect1,
                                        deco_color,
                                        BlendMode::SrcOver,
                                    );
                                    rasterizer::fill_rect(
                                        fb,
                                        deco_rect2,
                                        deco_color,
                                        BlendMode::SrcOver,
                                    );
                                }
                                TextDecorationStyle::Dotted => {
                                    let dot_size = thickness.max(1.0);
                                    let step = dot_size * 3.0;
                                    let mut dx = bounds.x;
                                    while dx < bounds.x + bounds.width {
                                        let dot_rect = Rect::new(dx, line_y, dot_size, thickness);
                                        rasterizer::fill_rect(
                                            fb,
                                            dot_rect,
                                            deco_color,
                                            BlendMode::SrcOver,
                                        );
                                        dx += step;
                                    }
                                }
                                TextDecorationStyle::Dashed => {
                                    let dash_len = thickness * 4.0;
                                    let gap_len = thickness * 2.0;
                                    let step = dash_len + gap_len;
                                    let mut dx = bounds.x;
                                    while dx < bounds.x + bounds.width {
                                        let seg_w = dash_len.min(bounds.x + bounds.width - dx);
                                        let dash_rect = Rect::new(dx, line_y, seg_w, thickness);
                                        rasterizer::fill_rect(
                                            fb,
                                            dash_rect,
                                            deco_color,
                                            BlendMode::SrcOver,
                                        );
                                        dx += step;
                                    }
                                }
                                TextDecorationStyle::Wavy => {
                                    let wave_len = thickness * 4.0;
                                    let amplitude = thickness * 1.5;
                                    let half = wave_len / 2.0;
                                    let mut dx = bounds.x;
                                    let mut up = true;
                                    while dx < bounds.x + bounds.width {
                                        let seg_w = half.min(bounds.x + bounds.width - dx);
                                        let y_off = if up { -amplitude } else { amplitude };
                                        let wave_rect =
                                            Rect::new(dx, line_y + y_off, seg_w, thickness);
                                        rasterizer::fill_rect(
                                            fb,
                                            wave_rect,
                                            deco_color,
                                            BlendMode::SrcOver,
                                        );
                                        dx += half;
                                        up = !up;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
