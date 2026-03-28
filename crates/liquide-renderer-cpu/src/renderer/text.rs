//! Text rendering for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::BlendMode;
use liquide_compositor::scene::FlatNode;

use crate::glyph::GlyphKey;
use crate::rasterizer;

use super::{SoftwareRenderer, WordSplitter};

impl SoftwareRenderer {
    /// Render a Text scene node.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_text_node(
        &mut self,
        node: &FlatNode,
        fb: &mut FrameBuffer,
    ) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let liquide_compositor::scene::SceneNodeKind::Text {
            text,
            color,
            scale,
            font_family,
            font_size,
            font_weight,
            font_style_italic: _,
            letter_spacing,
            word_spacing,
            line_height,
            text_align,
            text_transform,
            text_overflow,
            white_space: _,
            text_indent,
            text_decoration,
            text_shadows,
        } = &node.kind
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

            // Encode font_weight and letter_spacing into the font_id
            let family_hash = if font_family.is_empty() {
                0_u32
            } else {
                let mut h: u32 = 5381;
                for b in font_family.bytes() {
                    h = h.wrapping_mul(33).wrapping_add(b as u32);
                }
                h & 0xFFFF
            };
            let font_id = (((*font_weight as u32) & 0xFF) << 16) | family_hash;

            let size_px = glyph_height as u16;
            #[allow(unused_assignments)]
            let mut pen_x = bounds.x + text_indent;
            let mut pen_y = bounds.y;
            let line_h = if *line_height > 0.0 {
                *line_height
            } else {
                glyph_height as f32 * 1.2
            };

            // Pre-warm common glyphs
            let prewarm_key = (font_id, size_px);
            if !font_family.is_empty() && !self.prewarmed_fonts.contains(&prewarm_key) {
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
                    );
                }
            }

            // Track how many wrapped lines for per-line decoration
            let num_wrapped_lines: usize;

            // Render using the atlas
            {
                let estimated_advance = glyph_height as f32 * 0.55;

                // Word-wrap aware line splitting
                let white_space_val = node.kind.text_white_space().unwrap_or(0);
                let allows_wrap = matches!(white_space_val, 0 | 3 | 4 | 5);
                let max_line_width = bounds.width;

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

                // Split into visual lines
                let mut wrapped_lines: Vec<String> = Vec::new();
                for hard_line in render_text.split('\n') {
                    if !allows_wrap || max_line_width <= 0.0 {
                        wrapped_lines.push(hard_line.to_string());
                    } else {
                        let indent = if wrapped_lines.is_empty() { *text_indent } else { 0.0 };
                        let mut current_line = String::new();
                        let mut current_width = indent;

                        for word in WordSplitter::new(hard_line) {
                            let word_width: f32 = word.chars().map(&char_advance).sum();

                            if !current_line.is_empty()
                                && current_width + word_width > max_line_width
                            {
                                wrapped_lines.push(current_line.trim_end().to_string());
                                current_line = String::new();
                                current_width = 0.0;
                            }

                            current_line.push_str(word);
                            current_width += word_width;
                        }
                        if !current_line.is_empty() {
                            wrapped_lines.push(current_line.trim_end().to_string());
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
                            if s_first { lw += text_indent; }
                            for ch in s_line.chars() {
                                if ch == '\r' { continue; }
                                let key = GlyphKey {
                                    font_id, glyph_id: ch as u32, size_px, subpixel: false,
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
                            if s_first { s_pen_x += text_indent; }
                            for ch in s_line.chars() {
                                if ch == '\r' { continue; }
                                let key = GlyphKey {
                                    font_id, glyph_id: ch as u32, size_px, subpixel: false,
                                };
                                if let Some(cached) = self.glyph_atlas.get(&key).cloned() {
                                    let pos = liquide_compositor::geometry::Point::new(
                                        s_pen_x,
                                        s_pen_y + glyph_height as f32,
                                    );
                                    self.glyph_atlas.blit_glyph(fb, &cached, pos, shadow_c);
                                    let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                                    s_pen_x += cached.advance + *letter_spacing + extra;
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
                            if let Some(cached) = self.glyph_atlas.get(&ellipsis_key).cloned() {
                                let pos = liquide_compositor::geometry::Point::new(
                                    pen_x,
                                    pen_y + glyph_height as f32,
                                );
                                self.glyph_atlas.blit_glyph(fb, &cached, pos, c);
                            }
                            break;
                        }

                        let key = GlyphKey {
                            font_id,
                            glyph_id: ch as u32,
                            size_px,
                            subpixel: false,
                        };
                        if let Some(cached) = self.glyph_atlas.get(&key).cloned() {
                            let pos = liquide_compositor::geometry::Point::new(
                                pen_x,
                                pen_y + glyph_height as f32,
                            );
                            self.glyph_atlas.blit_glyph(fb, &cached, pos, c);
                            let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                            pen_x += cached.advance + *letter_spacing + extra;
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
                                        let dot_rect =
                                            Rect::new(dx, line_y, dot_size, thickness);
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
                                        let seg_w =
                                            dash_len.min(bounds.x + bounds.width - dx);
                                        let dash_rect =
                                            Rect::new(dx, line_y, seg_w, thickness);
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
                                        let seg_w =
                                            half.min(bounds.x + bounds.width - dx);
                                        let y_off =
                                            if up { -amplitude } else { amplitude };
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
