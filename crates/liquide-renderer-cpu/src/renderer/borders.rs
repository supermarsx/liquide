//! Border rendering for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::FlatNode;

use crate::rasterizer;

use super::SoftwareRenderer;

impl SoftwareRenderer {
    /// Render a Border scene node.
    pub(crate) fn render_border_node(
        &mut self,
        node: &FlatNode,
        fb: &mut FrameBuffer,
    ) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let liquide_compositor::scene::SceneNodeKind::Border { sides, radius } = &node.kind {
            use liquide_compositor::scene::BorderSideStyle;

            let (r_tl, r_tr, r_br, r_bl) = *radius;
            let has_radius = r_tl > 0.5 || r_tr > 0.5 || r_br > 0.5 || r_bl > 0.5;

            if !has_radius {
                // Fast path: straight edges (fill_rect per side)
                let draw_border_side =
                    |fb: &mut FrameBuffer,
                     side_rect: Rect,
                     side: &liquide_compositor::scene::BorderSide,
                     op: f32,
                     horizontal: bool| {
                        if side.width <= 0.0
                            || side.style == BorderSideStyle::None
                            || side.style == BorderSideStyle::Hidden
                        {
                            return;
                        }
                        let mut c = side.color;
                        if op < 1.0 {
                            c.a = (c.a as f32 * op + 0.5) as u8;
                        }
                        if c.a == 0 {
                            return;
                        }

                        match side.style {
                            BorderSideStyle::Solid => {
                                rasterizer::fill_rect(fb, side_rect, c, BlendMode::SrcOver);
                            }
                            BorderSideStyle::Dashed => {
                                let dash_len = (side.width * 3.0).max(3.0);
                                let gap_len = dash_len;
                                if horizontal {
                                    let mut dx = side_rect.x;
                                    let end = side_rect.x + side_rect.width;
                                    while dx < end {
                                        let seg_w = dash_len.min(end - dx);
                                        rasterizer::fill_rect(
                                            fb,
                                            Rect::new(dx, side_rect.y, seg_w, side_rect.height),
                                            c,
                                            BlendMode::SrcOver,
                                        );
                                        dx += dash_len + gap_len;
                                    }
                                } else {
                                    let mut dy = side_rect.y;
                                    let end = side_rect.y + side_rect.height;
                                    while dy < end {
                                        let seg_h = dash_len.min(end - dy);
                                        rasterizer::fill_rect(
                                            fb,
                                            Rect::new(side_rect.x, dy, side_rect.width, seg_h),
                                            c,
                                            BlendMode::SrcOver,
                                        );
                                        dy += dash_len + gap_len;
                                    }
                                }
                            }
                            BorderSideStyle::Dotted => {
                                let dot_size = side.width;
                                let spacing = dot_size * 2.0;
                                if horizontal {
                                    let mut dx = side_rect.x + dot_size * 0.5;
                                    let end = side_rect.x + side_rect.width;
                                    let cy = side_rect.y + side_rect.height * 0.5;
                                    while dx < end {
                                        let r = (dot_size * 0.5).max(0.5);
                                        rasterizer::fill_rect(
                                            fb,
                                            Rect::new(dx - r, cy - r, r * 2.0, r * 2.0),
                                            c,
                                            BlendMode::SrcOver,
                                        );
                                        dx += spacing;
                                    }
                                } else {
                                    let mut dy = side_rect.y + dot_size * 0.5;
                                    let end = side_rect.y + side_rect.height;
                                    let cx = side_rect.x + side_rect.width * 0.5;
                                    while dy < end {
                                        let r = (dot_size * 0.5).max(0.5);
                                        rasterizer::fill_rect(
                                            fb,
                                            Rect::new(cx - r, dy - r, r * 2.0, r * 2.0),
                                            c,
                                            BlendMode::SrcOver,
                                        );
                                        dy += spacing;
                                    }
                                }
                            }
                            BorderSideStyle::Double => {
                                let line_w = (side.width / 3.0).max(1.0);
                                if horizontal {
                                    rasterizer::fill_rect(
                                        fb,
                                        Rect::new(
                                            side_rect.x,
                                            side_rect.y,
                                            side_rect.width,
                                            line_w,
                                        ),
                                        c,
                                        BlendMode::SrcOver,
                                    );
                                    rasterizer::fill_rect(
                                        fb,
                                        Rect::new(
                                            side_rect.x,
                                            side_rect.y + side_rect.height - line_w,
                                            side_rect.width,
                                            line_w,
                                        ),
                                        c,
                                        BlendMode::SrcOver,
                                    );
                                } else {
                                    rasterizer::fill_rect(
                                        fb,
                                        Rect::new(
                                            side_rect.x,
                                            side_rect.y,
                                            line_w,
                                            side_rect.height,
                                        ),
                                        c,
                                        BlendMode::SrcOver,
                                    );
                                    rasterizer::fill_rect(
                                        fb,
                                        Rect::new(
                                            side_rect.x + side_rect.width - line_w,
                                            side_rect.y,
                                            line_w,
                                            side_rect.height,
                                        ),
                                        c,
                                        BlendMode::SrcOver,
                                    );
                                }
                            }
                            BorderSideStyle::Groove | BorderSideStyle::Ridge => {
                                let is_groove = side.style == BorderSideStyle::Groove;
                                let light = Color::new(
                                    (c.r as u16 * 3 / 4 + 64).min(255) as u8,
                                    (c.g as u16 * 3 / 4 + 64).min(255) as u8,
                                    (c.b as u16 * 3 / 4 + 64).min(255) as u8,
                                    c.a,
                                );
                                let dark = Color::new(c.r / 2, c.g / 2, c.b / 2, c.a);
                                let (outer_c, inner_c) = if is_groove {
                                    (dark, light)
                                } else {
                                    (light, dark)
                                };
                                let half = (side.width / 2.0).max(1.0);
                                if horizontal {
                                    rasterizer::fill_rect(
                                        fb,
                                        Rect::new(
                                            side_rect.x,
                                            side_rect.y,
                                            side_rect.width,
                                            half,
                                        ),
                                        outer_c,
                                        BlendMode::SrcOver,
                                    );
                                    rasterizer::fill_rect(
                                        fb,
                                        Rect::new(
                                            side_rect.x,
                                            side_rect.y + half,
                                            side_rect.width,
                                            (side_rect.height - half).max(0.0),
                                        ),
                                        inner_c,
                                        BlendMode::SrcOver,
                                    );
                                } else {
                                    rasterizer::fill_rect(
                                        fb,
                                        Rect::new(
                                            side_rect.x,
                                            side_rect.y,
                                            half,
                                            side_rect.height,
                                        ),
                                        outer_c,
                                        BlendMode::SrcOver,
                                    );
                                    rasterizer::fill_rect(
                                        fb,
                                        Rect::new(
                                            side_rect.x + half,
                                            side_rect.y,
                                            (side_rect.width - half).max(0.0),
                                            side_rect.height,
                                        ),
                                        inner_c,
                                        BlendMode::SrcOver,
                                    );
                                }
                            }
                            BorderSideStyle::Inset | BorderSideStyle::Outset => {
                                let is_inset = side.style == BorderSideStyle::Inset;
                                let light = Color::new(
                                    (c.r as u16 * 3 / 4 + 64).min(255) as u8,
                                    (c.g as u16 * 3 / 4 + 64).min(255) as u8,
                                    (c.b as u16 * 3 / 4 + 64).min(255) as u8,
                                    c.a,
                                );
                                let dark = Color::new(c.r / 2, c.g / 2, c.b / 2, c.a);
                                let use_dark = is_inset;
                                let final_c = if use_dark { dark } else { light };
                                rasterizer::fill_rect(
                                    fb,
                                    side_rect,
                                    final_c,
                                    BlendMode::SrcOver,
                                );
                            }
                            BorderSideStyle::None | BorderSideStyle::Hidden => {}
                        }
                    };

                // Top border
                draw_border_side(
                    fb,
                    Rect::new(bounds.x, bounds.y, bounds.width, sides.top.width),
                    &sides.top,
                    opacity,
                    true,
                );
                // Bottom border
                draw_border_side(
                    fb,
                    Rect::new(
                        bounds.x,
                        bounds.bottom() - sides.bottom.width,
                        bounds.width,
                        sides.bottom.width,
                    ),
                    &sides.bottom,
                    opacity,
                    true,
                );
                // Left border (between top and bottom)
                let side_h = (bounds.height - sides.top.width - sides.bottom.width).max(0.0);
                draw_border_side(
                    fb,
                    Rect::new(
                        bounds.x,
                        bounds.y + sides.top.width,
                        sides.left.width,
                        side_h,
                    ),
                    &sides.left,
                    opacity,
                    false,
                );
                // Right border (between top and bottom)
                draw_border_side(
                    fb,
                    Rect::new(
                        bounds.right() - sides.right.width,
                        bounds.y + sides.top.width,
                        sides.right.width,
                        side_h,
                    ),
                    &sides.right,
                    opacity,
                    false,
                );
            } else {
                // Rounded border: SDF-based per-pixel rendering
                let outer = bounds;
                let inner = Rect::new(
                    bounds.x + sides.left.width,
                    bounds.y + sides.top.width,
                    (bounds.width - sides.left.width - sides.right.width).max(0.0),
                    (bounds.height - sides.top.width - sides.bottom.width).max(0.0),
                );

                // Inner radii: shrink by the larger adjacent border width
                let ir_tl = (r_tl - sides.left.width.max(sides.top.width)).max(0.0);
                let ir_tr = (r_tr - sides.right.width.max(sides.top.width)).max(0.0);
                let ir_br = (r_br - sides.right.width.max(sides.bottom.width)).max(0.0);
                let ir_bl = (r_bl - sides.left.width.max(sides.bottom.width)).max(0.0);

                let x0 = (outer.x.max(0.0) as u32).min(fb.width);
                let y0 = (outer.y.max(0.0) as u32).min(fb.height);
                let x1 = (outer.right().ceil() as u32).min(fb.width);
                let y1 = (outer.bottom().ceil() as u32).min(fb.height);

                if x0 >= x1 || y0 >= y1 {
                    return;
                }

                // Centre for CSS trapezoidal side selection
                let hx = outer.width * 0.5;
                let hy = outer.height * 0.5;
                let cx = outer.x + hx;
                let cy = outer.y + hy;

                // Pre-resolve each side
                let resolve_side = |side: &liquide_compositor::scene::BorderSide| {
                    if side.width <= 0.0
                        || side.style == BorderSideStyle::None
                        || side.style == BorderSideStyle::Hidden
                    {
                        return (false, Color::new(0, 0, 0, 0));
                    }
                    let mut c = side.color;
                    if opacity < 1.0 {
                        c.a = (c.a as f32 * opacity + 0.5) as u8;
                    }
                    if c.a == 0 {
                        return (false, Color::new(0, 0, 0, 0));
                    }
                    (true, c.premultiply())
                };
                let (top_vis, top_pm) = resolve_side(&sides.top);
                let (right_vis, right_pm) = resolve_side(&sides.right);
                let (bottom_vis, bottom_pm) = resolve_side(&sides.bottom);
                let (left_vis, left_pm) = resolve_side(&sides.left);

                if !top_vis && !right_vis && !bottom_vis && !left_vis {
                    return;
                }

                let inv_hx = if hx > 0.0 { 1.0 / hx } else { 0.0 };
                let inv_hy = if hy > 0.0 { 1.0 / hy } else { 0.0 };

                for y in y0..y1 {
                    let fy = y as f32 + 0.5;
                    for x in x0..x1 {
                        let fx = x as f32 + 0.5;

                        // Outer SDF coverage (per-corner radii)
                        let outer_d = rasterizer::sdf_rounded_rect_per_corner(
                            fx, fy, &outer, r_tl, r_tr, r_br, r_bl,
                        );
                        let outer_cov = (-outer_d + 0.5).clamp(0.0, 1.0);
                        if outer_cov <= 0.0 {
                            continue;
                        }

                        // Inner SDF coverage (shrunk radii)
                        let inner_cov = if inner.width > 0.0 && inner.height > 0.0 {
                            let inner_d = rasterizer::sdf_rounded_rect_per_corner(
                                fx, fy, &inner, ir_tl, ir_tr, ir_br, ir_bl,
                            );
                            (-inner_d + 0.5).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };

                        let border_cov = (outer_cov - inner_cov).clamp(0.0, 1.0);
                        if border_cov <= 0.0 {
                            continue;
                        }

                        // CSS trapezoidal side selection via diagonals
                        let rx = (fx - cx) * inv_hx;
                        let ry = (fy - cy) * inv_hy;
                        let abs_rx = rx.abs();

                        let (vis, pm) = if ry < -abs_rx {
                            (top_vis, top_pm)
                        } else if ry > abs_rx {
                            (bottom_vis, bottom_pm)
                        } else if rx < 0.0 {
                            (left_vis, left_pm)
                        } else {
                            (right_vis, right_pm)
                        };

                        if !vis {
                            continue;
                        }

                        let mut src = pm;
                        if border_cov < 1.0 {
                            src.a = (src.a as f32 * border_cov + 0.5) as u8;
                            src.r = (src.r as f32 * border_cov + 0.5) as u8;
                            src.g = (src.g as f32 * border_cov + 0.5) as u8;
                            src.b = (src.b as f32 * border_cov + 0.5) as u8;
                        }

                        if src.a == 0 {
                            continue;
                        }

                        let dst = fb.get_pixel(x, y);
                        let blended = crate::blend::blend(dst, src, BlendMode::SrcOver);
                        fb.set_pixel(x, y, blended);
                    }
                }
            }
        }
    }
}
