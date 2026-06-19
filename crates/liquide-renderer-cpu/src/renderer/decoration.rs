//! Window decoration rendering for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::BlendMode;
use liquide_compositor::scene::FlatNode;

use crate::rasterizer::{self, Fill};

use super::SoftwareRenderer;

impl SoftwareRenderer {
    /// Render a Decoration scene node.
    pub(crate) fn render_decoration_node(&mut self, node: &FlatNode, fb: &mut FrameBuffer) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let liquide_compositor::scene::SceneNodeKind::Decoration {
            title,
            title_color,
            background,
            border_color,
            border_width,
            corner_radius,
            button_state,
            button_colors,
            button_layout,
        } = node.kind_ref()
        {
            // CSS-resolved frame colors (titlebar bg / border / title text)
            // override the legacy ShellTheme-sourced node fields when present
            // (t112-b2 full-CSS frame colors). When absent, the legacy fields
            // (`background` / `border_color` / `title_color`) are used unchanged.
            let frame = button_layout.frame_colors;
            let frame_title_bar_bg = frame.map(|f| f.title_bar_bg).unwrap_or(*background);
            let frame_border = frame.map(|f| f.border).unwrap_or(*border_color);
            let frame_title_text = frame.map(|f| f.title_text).unwrap_or(*title_color);

            // Check if this is a skeleton node (window being dragged)
            let is_skeleton = self.is_skeleton_node(node.id);

            if is_skeleton {
                // Skeleton mode: Only render a simple border outline
                if *border_width > 0.0 {
                    let mut bc = frame_border;
                    if opacity < 1.0 {
                        bc.a = (bc.a as f32 * opacity + 0.5) as u8;
                    }
                    // Make border more visible during drag
                    bc.a = bc.a.saturating_add(40);
                    rasterizer::stroke_rounded_rect(
                        fb,
                        bounds,
                        *corner_radius,
                        *border_width * 1.5,
                        bc,
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
                }
            } else {
                // Normal mode: Full decoration with title bar, buttons, etc.
                // Title bar background as a rounded rect (top corners only)
                let mut bg = frame_title_bar_bg;
                if opacity < 1.0 {
                    bg.a = (bg.a as f32 * opacity + 0.5) as u8;
                }
                rasterizer::fill_rounded_rect(
                    fb,
                    bounds,
                    *corner_radius,
                    &Fill::Solid(bg),
                    BlendMode::SrcOver,
                    &self.srgb_lut,
                );

                // Border stroke around the window bounds
                if *border_width > 0.0 {
                    let mut bc = frame_border;
                    if opacity < 1.0 {
                        bc.a = (bc.a as f32 * opacity + 0.5) as u8;
                    }
                    rasterizer::stroke_rounded_rect(
                        fb,
                        bounds,
                        *corner_radius,
                        *border_width,
                        bc,
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
                }

                // --- Window control buttons ---
                let title_bar_h = button_layout.title_bar_height;
                let btn_w = button_layout.button_width;
                let btn_h = button_layout.button_height;
                let btn_y = bounds.y + (title_bar_h - btn_h) / 2.0;
                let btn_right_margin = button_layout.button_right_margin;
                let rects = button_layout.button_rects;

                // Resolve a button's paint rect: prefer the per-button CSS box
                // (exact paint↔hit parity, t112-b2) when present, otherwise fall
                // back to the legacy fixed-stride model. `stride` is the index
                // from the right edge used by the fallback (close=1, max=2, …).
                let resolve_rect = |css: Option<Rect>, stride: f32| -> Rect {
                    css.unwrap_or_else(|| {
                        let x = bounds.x + bounds.width - btn_w * stride - btn_right_margin;
                        Rect::new(x, btn_y, btn_w, btn_h)
                    })
                };

                // Close button
                if button_state.close {
                    let close_bounds = resolve_rect(rects.close, 1.0);
                    let close_x = close_bounds.x;
                    let btn_y = close_bounds.y;
                    let btn_w = close_bounds.width;
                    let btn_h = close_bounds.height;
                    let close_bg = if button_state.close_hovered {
                        button_colors.close_bg_hover
                    } else {
                        button_colors.close_bg
                    };
                    rasterizer::fill_rounded_rect(
                        fb,
                        close_bounds,
                        button_layout.button_corner_radius,
                        &Fill::Solid(close_bg),
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
                    // X icon — macOS traffic-light glyph-on-hover (t172-e2): the
                    // dot reads as a solid colored circle at rest; the ×/−/+
                    // glyph only appears while the button (group) is hovered. The
                    // renderer owns the glyph (there is no CSS `::before` seam for
                    // the decoration node — see the module note / executor
                    // report), so gating it on the hover state IS the
                    // glyph-on-hover behavior, and the hovered repaint produces a
                    // real pixel delta (t176 hover damage tests).
                    if button_state.close_hovered {
                        let cx = close_x + btn_w / 2.0;
                        let cy_btn = btn_y + btn_h / 2.0;
                        let icon_color = button_colors.close_icon;
                        let arm = 4.0_f32;
                        let thickness = 1.5_f32;
                        for i in 0..((arm * 2.0) as i32) {
                            let t = i as f32 - arm;
                            rasterizer::fill_rect(
                                fb,
                                Rect::new(
                                    cx + t - thickness / 2.0,
                                    cy_btn + t - thickness / 2.0,
                                    thickness,
                                    thickness,
                                ),
                                icon_color,
                                BlendMode::SrcOver,
                            );
                        }
                        for i in 0..((arm * 2.0) as i32) {
                            let t = i as f32 - arm;
                            rasterizer::fill_rect(
                                fb,
                                Rect::new(
                                    cx - t - thickness / 2.0,
                                    cy_btn + t - thickness / 2.0,
                                    thickness,
                                    thickness,
                                ),
                                icon_color,
                                BlendMode::SrcOver,
                            );
                        }
                    }
                }

                // Maximize button
                if button_state.maximize {
                    let max_bounds = resolve_rect(rects.maximize, 2.0);
                    let max_x = max_bounds.x;
                    let btn_y = max_bounds.y;
                    let btn_w = max_bounds.width;
                    let btn_h = max_bounds.height;
                    let btn_bg = if button_state.maximize_hovered {
                        button_colors.maximize_bg_hover
                    } else {
                        button_colors.maximize_bg
                    };
                    rasterizer::fill_rounded_rect(
                        fb,
                        max_bounds,
                        button_layout.button_corner_radius,
                        &Fill::Solid(btn_bg),
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
                    // Maximize glyph (square outline) — shown only on hover
                    // (macOS glyph-on-hover, t172-e2).
                    if button_state.maximize_hovered {
                        let cx = max_x + btn_w / 2.0;
                        let cy_btn = btn_y + btn_h / 2.0;
                        let icon_color = button_colors.maximize_icon;
                        let half = 4.0_f32;
                        let stroke = 1.5_f32;
                        // Top edge
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(cx - half, cy_btn - half, half * 2.0, stroke),
                            icon_color,
                            BlendMode::SrcOver,
                        );
                        // Bottom edge
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(cx - half, cy_btn + half - stroke, half * 2.0, stroke),
                            icon_color,
                            BlendMode::SrcOver,
                        );
                        // Left edge
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(cx - half, cy_btn - half, stroke, half * 2.0),
                            icon_color,
                            BlendMode::SrcOver,
                        );
                        // Right edge
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(cx + half - stroke, cy_btn - half, stroke, half * 2.0),
                            icon_color,
                            BlendMode::SrcOver,
                        );
                    }
                }

                // Minimize button
                if button_state.minimize {
                    let min_bounds = resolve_rect(rects.minimize, 3.0);
                    let min_x = min_bounds.x;
                    let btn_y = min_bounds.y;
                    let btn_w = min_bounds.width;
                    let btn_h = min_bounds.height;
                    let btn_bg = if button_state.minimize_hovered {
                        button_colors.minimize_bg_hover
                    } else {
                        button_colors.minimize_bg
                    };
                    rasterizer::fill_rounded_rect(
                        fb,
                        min_bounds,
                        button_layout.button_corner_radius,
                        &Fill::Solid(btn_bg),
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
                    // Minimize glyph (horizontal dash) — shown only on hover
                    // (macOS glyph-on-hover, t172-e2).
                    if button_state.minimize_hovered {
                        let cx = min_x + btn_w / 2.0;
                        let cy_btn = btn_y + btn_h / 2.0;
                        let icon_color = button_colors.minimize_icon;
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(cx - 5.0, cy_btn + 2.0, 10.0, 1.5),
                            icon_color,
                            BlendMode::SrcOver,
                        );
                    }
                }

                // Always-on-top button
                if button_state.always_on_top {
                    let aot_bounds = resolve_rect(rects.always_on_top, 4.0);
                    let aot_x = aot_bounds.x;
                    let btn_y = aot_bounds.y;
                    let btn_w = aot_bounds.width;
                    let btn_h = aot_bounds.height;
                    let btn_bg = if button_state.is_topmost {
                        if button_state.always_on_top_hovered {
                            button_colors.pin_bg_active_hover
                        } else {
                            button_colors.pin_bg_active
                        }
                    } else if button_state.always_on_top_hovered {
                        button_colors.pin_bg_hover
                    } else {
                        button_colors.pin_bg
                    };
                    rasterizer::fill_rounded_rect(
                        fb,
                        aot_bounds,
                        button_layout.button_corner_radius,
                        &Fill::Solid(btn_bg),
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
                    // Pin glyph — the pin is not a macOS traffic light; keep its
                    // glyph visible while the window is pinned (active/topmost) or
                    // while hovered, so an always-on-top window still shows its
                    // state at rest, but an idle un-pinned dot stays clean like the
                    // traffic lights (t172-e2 glyph-on-hover).
                    if button_state.is_topmost || button_state.always_on_top_hovered {
                        let cx = aot_x + btn_w / 2.0;
                        let cy_btn = btn_y + btn_h / 2.0;
                        let icon_color = if button_state.is_topmost {
                            button_colors.pin_icon_active
                        } else {
                            button_colors.pin_icon
                        };
                        // Pin head
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(cx - 3.0, cy_btn - 5.0, 6.0, 4.0),
                            icon_color,
                            BlendMode::SrcOver,
                        );
                        // Pin shaft
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(cx - 0.75, cy_btn - 1.0, 1.5, 6.0),
                            icon_color,
                            BlendMode::SrcOver,
                        );
                        // Pin point
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(cx - 0.5, cy_btn + 5.0, 1.0, 2.0),
                            icon_color,
                            BlendMode::SrcOver,
                        );
                    }
                }

                // Title text (centered in title bar)
                if let Some(title_text) = title {
                    if !title_text.is_empty() {
                        let mut tc = frame_title_text;
                        if opacity < 1.0 {
                            tc.a = (tc.a as f32 * opacity + 0.5) as u8;
                        }
                        let char_w = 8_i32;
                        let text_w = title_text.len() as i32 * char_w;
                        let text_x = bounds.x as i32 + (bounds.width as i32 - text_w) / 2;
                        let text_y = bounds.y as i32 + (title_bar_h as i32 - 16) / 2;
                        crate::bitmap_font::draw_text(fb, title_text, text_x, text_y, tc, 1);
                    }
                }
            } // end else (normal decoration rendering)
        }
    }
}

#[cfg(test)]
mod glyph_on_hover_tests {
    //! t172-e2: macOS glyph-on-hover. At rest a traffic-light dot is a solid
    //! colored circle with NO glyph; the ×/−/+ glyph appears only while the
    //! button is hovered. We render the SAME decoration node twice — once at rest,
    //! once with the close button hovered — holding the close background IDENTICAL
    //! across both (so the only possible delta inside the close box is the glyph
    //! ink) and assert: zero glyph-ink pixels at rest, > 0 on hover.

    use liquide_compositor::framebuffer::FrameBuffer;
    use liquide_compositor::geometry::{Affine2D, Rect};
    use liquide_compositor::pixel::{Color, PixelFormat};
    use liquide_compositor::scene::{
        DecorationButtonRects, DecorationButtons, DecorationColors, DecorationLayout, FlatNode,
        SceneNodeKind,
    };

    use crate::renderer::SoftwareRenderer;
    use std::sync::Arc;

    const W: u32 = 200;
    const H: u32 = 60;

    /// The close dot's painted box (small round dot near the left edge).
    fn close_box() -> Rect {
        Rect::new(12.0, 22.0, 14.0, 14.0)
    }

    fn decoration_node(close_hovered: bool) -> FlatNode {
        // Identical close bg in BOTH states so the ONLY in-box delta is the glyph.
        let icon = Color::new(255, 255, 255, 255);
        let flat_red = Color::new(255, 95, 87, 255);
        let colors = DecorationColors {
            close_bg: flat_red,
            close_bg_hover: flat_red,
            close_icon: icon,
            ..DecorationColors::default()
        };
        let layout = DecorationLayout {
            title_bar_height: 36.0,
            button_width: 14.0,
            button_height: 14.0,
            button_right_margin: 0.0,
            button_corner_radius: 7.0,
            button_rects: DecorationButtonRects {
                close: Some(close_box()),
                ..DecorationButtonRects::default()
            },
            frame_colors: None,
        };
        let buttons = DecorationButtons {
            close: true,
            maximize: false,
            minimize: false,
            always_on_top: false,
            is_topmost: false,
            close_hovered,
            maximize_hovered: false,
            minimize_hovered: false,
            always_on_top_hovered: false,
        };
        FlatNode {
            id: 1,
            kind: Arc::new(SceneNodeKind::Decoration {
                title: None,
                title_color: Color::new(255, 255, 255, 255),
                background: Color::new(40, 40, 42, 255),
                border_color: Color::new(0, 0, 0, 0),
                border_width: 0.0,
                corner_radius: 8.0,
                button_state: buttons,
                button_colors: colors,
                button_layout: layout,
            }),
            absolute_bounds: Rect::new(0.0, 0.0, W as f32, H as f32),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Count pixels inside the close box that match the glyph ink color (white).
    fn glyph_ink_pixels(fb: &FrameBuffer) -> usize {
        let b = close_box();
        let mut n = 0;
        for y in (b.y as u32)..((b.y + b.height) as u32) {
            for x in (b.x as u32)..((b.x + b.width) as u32) {
                let off = fb.pixel_offset(x, y);
                let px = &fb.pixels()[off..off + 4];
                // BGRA8: white ink is high in all of B,G,R; the red dot is low in
                // B and G. The × glyph is the only white-ish ink in the box.
                if px[0] > 200 && px[1] > 200 && px[2] > 200 {
                    n += 1;
                }
            }
        }
        n
    }

    fn render(node: &FlatNode) -> FrameBuffer {
        let mut rnd = SoftwareRenderer::new();
        let mut fb = FrameBuffer::new(W, H, PixelFormat::Bgra8);
        rnd.render_decoration_node(node, &mut fb);
        fb
    }

    #[test]
    fn glyph_absent_at_rest_present_on_hover() {
        let rest = render(&decoration_node(false));
        let hover = render(&decoration_node(true));

        let rest_ink = glyph_ink_pixels(&rest);
        let hover_ink = glyph_ink_pixels(&hover);

        assert_eq!(
            rest_ink, 0,
            "at rest the close dot must be a solid circle with NO glyph ink, got {rest_ink} px"
        );
        assert!(
            hover_ink > 0,
            "on hover the × glyph must appear inside the close dot (got {hover_ink} px)"
        );
    }
}
