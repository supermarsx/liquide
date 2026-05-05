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
            // Check if this is a skeleton node (window being dragged)
            let is_skeleton = self.is_skeleton_node(node.id);

            if is_skeleton {
                // Skeleton mode: Only render a simple border outline
                if *border_width > 0.0 {
                    let mut bc = *border_color;
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
                let mut bg = *background;
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
                    let mut bc = *border_color;
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

                // Close button
                if button_state.close {
                    let close_x = bounds.x + bounds.width - btn_w - btn_right_margin;
                    let close_bg = if button_state.close_hovered {
                        button_colors.close_bg_hover
                    } else {
                        button_colors.close_bg
                    };
                    let close_bounds = Rect::new(close_x, btn_y, btn_w, btn_h);
                    rasterizer::fill_rounded_rect(
                        fb,
                        close_bounds,
                        button_layout.button_corner_radius,
                        &Fill::Solid(close_bg),
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
                    // X icon
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

                // Maximize button
                if button_state.maximize {
                    let max_x = bounds.x + bounds.width - btn_w * 2.0 - btn_right_margin;
                    let btn_bg = if button_state.maximize_hovered {
                        button_colors.maximize_bg_hover
                    } else {
                        button_colors.maximize_bg
                    };
                    let max_bounds = Rect::new(max_x, btn_y, btn_w, btn_h);
                    rasterizer::fill_rounded_rect(
                        fb,
                        max_bounds,
                        button_layout.button_corner_radius,
                        &Fill::Solid(btn_bg),
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
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

                // Minimize button
                if button_state.minimize {
                    let min_x = bounds.x + bounds.width - btn_w * 3.0 - btn_right_margin;
                    let btn_bg = if button_state.minimize_hovered {
                        button_colors.minimize_bg_hover
                    } else {
                        button_colors.minimize_bg
                    };
                    let min_bounds = Rect::new(min_x, btn_y, btn_w, btn_h);
                    rasterizer::fill_rounded_rect(
                        fb,
                        min_bounds,
                        button_layout.button_corner_radius,
                        &Fill::Solid(btn_bg),
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
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

                // Always-on-top button
                if button_state.always_on_top {
                    let aot_x = bounds.x + bounds.width - btn_w * 4.0 - btn_right_margin;
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
                    let aot_bounds = Rect::new(aot_x, btn_y, btn_w, btn_h);
                    rasterizer::fill_rounded_rect(
                        fb,
                        aot_bounds,
                        button_layout.button_corner_radius,
                        &Fill::Solid(btn_bg),
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
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

                // Title text (centered in title bar)
                if let Some(title_text) = title {
                    if !title_text.is_empty() {
                        let mut tc = *title_color;
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
