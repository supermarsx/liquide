//! Text rendering for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::BlendMode;
use liquide_compositor::scene::FlatNode;

use crate::glyph::GlyphKey;
use crate::rasterizer;

use super::{SoftwareRenderer, WordSplitter};

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
            #[allow(unused_assignments)]
            let mut pen_x = bounds.x + text_indent;
            let mut pen_y = bounds.y;
            let line_h = if *line_height > 0.0 {
                *line_height
            } else {
                glyph_height as f32 * 1.2
            };

            // Pre-warm common glyphs when enabled for this renderer.
            // Prewarm only upright runs: the prewarm path requests glyphs with
            // italic=false, so an italic-flagged font_id must not be prewarmed
            // (it would seed upright glyphs under an italic key).
            let prewarm_key = (font_id, size_px);
            if self.common_glyph_prewarm_enabled()
                && !font_family.is_empty()
                && !*font_style_italic
                && !self.prewarmed_fonts.contains(&prewarm_key)
            {
                self.prewarmed_fonts.insert(prewarm_key);
                self.prewarm_glyphs(font_id, size_px, glyph_height, font_family, *font_weight);
            }

            // First pass: request missing glyphs
            for ch in render_text.chars() {
                if ch == '\n' || ch == '\r' {
                    continue;
                }
                let glyph_id = ch as u32;
                let key = GlyphKey {
                    font_id,
                    glyph_id,
                    size_px,
                    subpixel: false,
                };
                if self.glyph_atlas.get(&key).is_none() {
                    self.has_pending_glyphs = true;
                    self.font_worker.request_glyph_with_font(
                        key,
                        ch,
                        glyph_height,
                        font_family.clone(),
                        *font_weight,
                        *font_style_italic,
                    );
                }
            }

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
                let estimated_advance = glyph_height as f32 * 0.55;

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

                // Helper closure: measure a char advance using atlas or estimate
                let char_advance = |ch: char| -> f32 {
                    let key = GlyphKey {
                        font_id,
                        glyph_id: ch as u32,
                        size_px,
                        subpixel: false,
                    };
                    let base = if let Some(cached) = self.glyph_atlas.get(&key) {
                        cached.advance
                    } else {
                        estimated_advance
                    };
                    let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                    base + *letter_spacing + extra
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
                            let word_width: f32 = word.chars().map(&char_advance).sum();

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
                                for ch in word.chars() {
                                    let cw = char_advance(ch);
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

                // Render text shadows BEFORE the main text (CSS: shadows behind text)
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
                        let mut s_first = true;
                        for s_line in &wrapped_lines {
                            // Measure line for alignment
                            let mut lw = 0.0f32;
                            if s_first {
                                lw += text_indent;
                            }
                            for ch in s_line.chars() {
                                if ch == '\r' {
                                    continue;
                                }
                                let key = GlyphKey {
                                    font_id,
                                    glyph_id: ch as u32,
                                    size_px,
                                    subpixel: false,
                                };
                                let base = if let Some(cached) = self.glyph_atlas.get(&key) {
                                    cached.advance
                                } else {
                                    estimated_advance
                                };
                                let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                                lw += base + *letter_spacing + extra;
                            }
                            let ax = match text_align {
                                1 => ((bounds.width - lw) / 2.0).max(0.0),
                                2 => (bounds.width - lw).max(0.0),
                                _ => 0.0,
                            };
                            let mut s_pen_x = bounds.x + ax + sx;
                            if s_first {
                                s_pen_x += text_indent;
                            }
                            for ch in s_line.chars() {
                                if ch == '\r' {
                                    continue;
                                }
                                let key = GlyphKey {
                                    font_id,
                                    glyph_id: ch as u32,
                                    size_px,
                                    subpixel: false,
                                };
                                if let Some(cached) = self.glyph_atlas.get(&key) {
                                    let pos = liquide_compositor::geometry::Point::new(
                                        s_pen_x,
                                        s_pen_y + glyph_height as f32,
                                    );
                                    let advance = cached.advance;
                                    self.glyph_atlas.blit_glyph(fb, cached, pos, shadow_c, clip);
                                    let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                                    s_pen_x += advance + *letter_spacing + extra;
                                } else {
                                    let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                                    s_pen_x += estimated_advance + *letter_spacing + extra;
                                }
                            }
                            s_pen_y += line_h;
                            s_first = false;
                        }
                    }
                }

                let mut is_first_line = true;
                for line_text in &wrapped_lines {
                    // Measure line width for alignment
                    let mut line_width = 0.0f32;
                    if is_first_line {
                        line_width += text_indent;
                    }
                    for ch in line_text.chars() {
                        if ch == '\r' {
                            continue;
                        }
                        let key = GlyphKey {
                            font_id,
                            glyph_id: ch as u32,
                            size_px,
                            subpixel: false,
                        };
                        if let Some(cached) = self.glyph_atlas.get(&key) {
                            let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                            line_width += cached.advance + *letter_spacing + extra;
                        } else {
                            let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                            line_width += estimated_advance + *letter_spacing + extra;
                        }
                    }

                    // Text-align offset: 0=left, 1=center, 2=right, 3=justify
                    let align_x = match text_align {
                        1 => ((bounds.width - line_width) / 2.0).max(0.0),
                        2 => (bounds.width - line_width).max(0.0),
                        _ => 0.0,
                    };

                    pen_x = bounds.x + align_x;
                    if is_first_line {
                        pen_x += text_indent;
                    }

                    // Text overflow: ellipsis (1)
                    let max_x = bounds.x + bounds.width;
                    let use_ellipsis = *text_overflow == 1 && line_width > bounds.width;

                    for ch in line_text.chars() {
                        if ch == '\r' {
                            continue;
                        }

                        // Ellipsis check
                        if use_ellipsis && pen_x + glyph_height as f32 * 0.6 > max_x {
                            let ellipsis_key = GlyphKey {
                                font_id,
                                glyph_id: '\u{2026}' as u32,
                                size_px,
                                subpixel: false,
                            };
                            if let Some(cached) = self.glyph_atlas.get(&ellipsis_key) {
                                let pos = liquide_compositor::geometry::Point::new(
                                    pen_x,
                                    pen_y + glyph_height as f32,
                                );
                                self.glyph_atlas.blit_glyph(fb, cached, pos, c, clip);
                            }
                            break;
                        }

                        let key = GlyphKey {
                            font_id,
                            glyph_id: ch as u32,
                            size_px,
                            subpixel: false,
                        };
                        if let Some(cached) = self.glyph_atlas.get(&key) {
                            let pos = liquide_compositor::geometry::Point::new(
                                pen_x,
                                pen_y + glyph_height as f32,
                            );
                            let advance = cached.advance;
                            self.glyph_atlas.blit_glyph(fb, cached, pos, c, clip);

                            // text-emphasis: draw the mark centered over (or
                            // under) this character. Skip whitespace — emphasis
                            // marks are not drawn on separators (CSS Text Deco 3).
                            if let Some(emph) = text_emphasis {
                                if !ch.is_whitespace() {
                                    if let Some(mark_ch) = emph.mark.chars().next() {
                                        let mark_key = GlyphKey {
                                            font_id,
                                            glyph_id: mark_ch as u32,
                                            size_px: emphasis_size_px,
                                            subpixel: false,
                                        };
                                        if let Some(mark_glyph) =
                                            self.glyph_atlas.get(&mark_key)
                                        {
                                            let mut mc = emph.color.unwrap_or(c);
                                            if opacity < 1.0 {
                                                mc.a = (mc.a as f32 * opacity + 0.5) as u8;
                                            }
                                            // Center the mark over the character cell.
                                            let mark_x = pen_x
                                                + (advance - mark_glyph.advance) * 0.5;
                                            use liquide_compositor::scene::TextEmphasisPosition;
                                            let mark_y = match emph.position {
                                                TextEmphasisPosition::Over => {
                                                    // Marks sit above the text top.
                                                    // The glyph cell occupies
                                                    // [pen_y, pen_y + glyph_height];
                                                    // place the mark baseline at the
                                                    // line top so it renders just above.
                                                    pen_y
                                                }
                                                TextEmphasisPosition::Under => {
                                                    // Below the descender: baseline
                                                    // one mark-height under the cell.
                                                    pen_y
                                                        + glyph_height as f32
                                                        + emphasis_height as f32
                                                }
                                            };
                                            let mark_pos =
                                                liquide_compositor::geometry::Point::new(
                                                    mark_x, mark_y,
                                                );
                                            self.glyph_atlas
                                                .blit_glyph(fb, mark_glyph, mark_pos, mc, clip);
                                        }
                                    }
                                }
                            }

                            let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                            pen_x += advance + *letter_spacing + extra;
                        } else {
                            let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                            pen_x += estimated_advance + *letter_spacing + extra;
                        }
                    }
                    pen_y += line_h;
                    is_first_line = false;
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
