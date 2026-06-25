//! Border rendering for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::FlatNode;

use crate::rasterizer;

use super::SoftwareRenderer;

impl SoftwareRenderer {
    /// Render a Border scene node.
    pub(crate) fn render_border_node(&mut self, node: &FlatNode, fb: &mut FrameBuffer) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let liquide_compositor::scene::SceneNodeKind::Border { sides, radius } = node.kind_ref()
        {
            use liquide_compositor::scene::BorderSideStyle;

            let (r_tl, r_tr, r_br, r_bl) = *radius;
            let has_radius = r_tl > 0.5 || r_tr > 0.5 || r_br > 0.5 || r_bl > 0.5;

            if !has_radius {
                // Fast path: straight edges (fill_rect per side).
                // `near_side` = top or left (the light-lit sides for `outset`).
                let draw_border_side = |fb: &mut FrameBuffer,
                                        side_rect: Rect,
                                        side: &liquide_compositor::scene::BorderSide,
                                        op: f32,
                                        horizontal: bool,
                                        near_side: bool| {
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
                            // CSS dotted borders draw ROUND dots (a chain of
                            // filled circles of diameter = border width), not
                            // squares. Each dot is rasterised with an AA SDF disc
                            // so the curvature is visible even at small sizes.
                            let dot_size = side.width;
                            let spacing = dot_size * 2.0;
                            let r = (dot_size * 0.5).max(0.5);
                            if horizontal {
                                let mut dx = side_rect.x + dot_size * 0.5;
                                let end = side_rect.x + side_rect.width;
                                let cy = side_rect.y + side_rect.height * 0.5;
                                while dx < end {
                                    fill_dot(fb, dx, cy, r, c);
                                    dx += spacing;
                                }
                            } else {
                                let mut dy = side_rect.y + dot_size * 0.5;
                                let end = side_rect.y + side_rect.height;
                                let cx = side_rect.x + side_rect.width * 0.5;
                                while dy < end {
                                    fill_dot(fb, cx, dy, r, c);
                                    dy += spacing;
                                }
                            }
                        }
                        BorderSideStyle::Double => {
                            let line_w = (side.width / 3.0).max(1.0);
                            if horizontal {
                                rasterizer::fill_rect(
                                    fb,
                                    Rect::new(side_rect.x, side_rect.y, side_rect.width, line_w),
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
                                    Rect::new(side_rect.x, side_rect.y, line_w, side_rect.height),
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
                                    Rect::new(side_rect.x, side_rect.y, side_rect.width, half),
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
                                    Rect::new(side_rect.x, side_rect.y, half, side_rect.height),
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
                            // CSS inset/outset shade each side as a flat colour,
                            // but the TOP/LEFT (near) sides and BOTTOM/RIGHT (far)
                            // sides get OPPOSITE shades to fake a 3D bevel:
                            //  - outset: near = light, far = dark
                            //  - inset:  near = dark,  far = light
                            let is_inset = side.style == BorderSideStyle::Inset;
                            let light = Color::new(
                                (c.r as u16 * 3 / 4 + 64).min(255) as u8,
                                (c.g as u16 * 3 / 4 + 64).min(255) as u8,
                                (c.b as u16 * 3 / 4 + 64).min(255) as u8,
                                c.a,
                            );
                            let dark = Color::new(c.r / 2, c.g / 2, c.b / 2, c.a);
                            // near_light XOR is_inset.
                            let near_light = !is_inset;
                            let final_c = if near_side == near_light { light } else { dark };
                            rasterizer::fill_rect(fb, side_rect, final_c, BlendMode::SrcOver);
                        }
                        BorderSideStyle::None | BorderSideStyle::Hidden => {}
                    }
                };

                // Top border (near side)
                draw_border_side(
                    fb,
                    Rect::new(bounds.x, bounds.y, bounds.width, sides.top.width),
                    &sides.top,
                    opacity,
                    true,
                    true,
                );
                // Bottom border (far side)
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
                    false,
                );
                // Left border (near side, between top and bottom)
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
                    true,
                );
                // Right border (far side, between top and bottom)
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
                // Confine to the per-thread write-scissor (t80).
                let (x0, y0, x1, y1) = rasterizer::scissor_clamp_window(x0, y0, x1, y1);

                if x0 >= x1 || y0 >= y1 {
                    return;
                }

                // Centre for CSS trapezoidal side selection
                let hx = outer.width * 0.5;
                let hy = outer.height * 0.5;
                let cx = outer.x + hx;
                let cy = outer.y + hy;

                // Pre-resolve each side. Unlike the straight path, the rounded
                // path must KEEP the per-side style and the (opacity-applied,
                // un-premultiplied) colour so the per-pixel loop below can render
                // dashed/dotted/double/bevel patterns — not just solid.
                let resolve_side = |side: &liquide_compositor::scene::BorderSide| {
                    if side.width <= 0.0
                        || side.style == BorderSideStyle::None
                        || side.style == BorderSideStyle::Hidden
                    {
                        return (false, BorderSideStyle::None, Color::new(0, 0, 0, 0), 0.0_f32);
                    }
                    let mut c = side.color;
                    if opacity < 1.0 {
                        c.a = (c.a as f32 * opacity + 0.5) as u8;
                    }
                    if c.a == 0 {
                        return (false, BorderSideStyle::None, Color::new(0, 0, 0, 0), 0.0);
                    }
                    (true, side.style, c, side.width)
                };
                let (top_vis, top_style, top_c, top_w) = resolve_side(&sides.top);
                let (right_vis, right_style, right_c, right_w) = resolve_side(&sides.right);
                let (bottom_vis, bottom_style, bottom_c, bottom_w) = resolve_side(&sides.bottom);
                let (left_vis, left_style, left_c, left_w) = resolve_side(&sides.left);

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

                        // `is_h` marks the top/bottom sides (a horizontal band),
                        // used to compute a perimeter arc-length for dash/dot
                        // phase and to pick the band thickness.
                        let (vis, style, base_c, side_w, is_h) = if ry < -abs_rx {
                            (top_vis, top_style, top_c, top_w, true)
                        } else if ry > abs_rx {
                            (bottom_vis, bottom_style, bottom_c, bottom_w, true)
                        } else if rx < 0.0 {
                            (left_vis, left_style, left_c, left_w, false)
                        } else {
                            (right_vis, right_style, right_c, right_w, false)
                        };

                        if !vis {
                            continue;
                        }

                        // Depth into the band measured from the OUTER edge, as a
                        // fraction in [0,1] (0 = outer edge, 1 = inner edge).
                        // `-outer_d` is the inward distance past the outer edge.
                        let depth_px = (-outer_d).max(0.0);
                        let band_t = if side_w > 0.0 {
                            (depth_px / side_w).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };

                        // Arc-length-ish coordinate along the side, in pixels, for
                        // dash/dot phase. Use the pixel coordinate parallel to the
                        // side (x for top/bottom, y for left/right). Continuous and
                        // monotonic along each side — good enough for an even
                        // dash/dot cadence on rounded boxes.
                        let along = if is_h { fx } else { fy };

                        // `style_cov` modulates the band coverage (0 in gaps),
                        // `style_c` is the (possibly bevel-shaded) colour.
                        let (style_cov, style_c) = match style {
                            BorderSideStyle::Solid => (1.0, base_c),
                            BorderSideStyle::Dashed => {
                                let dash_len = (side_w * 3.0).max(3.0);
                                let period = dash_len * 2.0;
                                let phase = along.rem_euclid(period);
                                let on = phase < dash_len;
                                (if on { 1.0 } else { 0.0 }, base_c)
                            }
                            BorderSideStyle::Dotted => {
                                // Round dots: a dot every `2*width` along the side,
                                // circular via distance to the dot centre in
                                // (along, depth-from-mid) space.
                                let dia = side_w.max(1.0);
                                let period = dia * 2.0;
                                let r = dia * 0.5;
                                // Distance from the nearest dot centre along the side.
                                let mut da = along.rem_euclid(period) - r;
                                // Centre dots within their cell (phase 0..period).
                                da = da.abs();
                                // Perpendicular distance from the band centre line.
                                let dp = (band_t - 0.5).abs() * side_w;
                                let dist = (da * da + dp * dp).sqrt();
                                let cov = (-(dist - r) + 0.5).clamp(0.0, 1.0);
                                (cov, base_c)
                            }
                            BorderSideStyle::Double => {
                                // Two lines: outer third and inner third painted,
                                // middle third blank.
                                if band_t < 1.0 / 3.0 || band_t > 2.0 / 3.0 {
                                    (1.0, base_c)
                                } else {
                                    (0.0, base_c)
                                }
                            }
                            BorderSideStyle::Groove
                            | BorderSideStyle::Ridge
                            | BorderSideStyle::Inset
                            | BorderSideStyle::Outset => {
                                let light = Color::new(
                                    (base_c.r as u16 * 3 / 4 + 64).min(255) as u8,
                                    (base_c.g as u16 * 3 / 4 + 64).min(255) as u8,
                                    (base_c.b as u16 * 3 / 4 + 64).min(255) as u8,
                                    base_c.a,
                                );
                                let dark = Color::new(
                                    base_c.r / 2,
                                    base_c.g / 2,
                                    base_c.b / 2,
                                    base_c.a,
                                );
                                // Decide which half (outer vs inner) gets light.
                                // outer_light reflects a top/left-lit 3D bevel.
                                let outer_light = match style {
                                    BorderSideStyle::Outset => true,
                                    BorderSideStyle::Inset => false,
                                    // Groove looks carved (dark outer, light inner
                                    // on top/left); Ridge is the inverse. Flip for
                                    // bottom/right sides for the lit-from-top-left
                                    // look.
                                    BorderSideStyle::Ridge => true,
                                    _ /* Groove */ => false,
                                };
                                let chosen = if (band_t < 0.5) == outer_light {
                                    light
                                } else {
                                    dark
                                };
                                (1.0, chosen)
                            }
                            BorderSideStyle::None | BorderSideStyle::Hidden => (0.0, base_c),
                        };

                        let cov = border_cov * style_cov;
                        if cov <= 0.0 {
                            continue;
                        }

                        let mut src = style_c.premultiply();
                        if cov < 1.0 {
                            src.a = (src.a as f32 * cov + 0.5) as u8;
                            src.r = (src.r as f32 * cov + 0.5) as u8;
                            src.g = (src.g as f32 * cov + 0.5) as u8;
                            src.b = (src.b as f32 * cov + 0.5) as u8;
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

/// Fill an antialiased disc centred at `(cx, cy)` with radius `r` in colour `c`.
///
/// Used by the dotted border style so dots render as round circles (CSS dotted
/// semantics) rather than squares. Coverage is a 1px-wide SDF ramp.
fn fill_dot(fb: &mut FrameBuffer, cx: f32, cy: f32, r: f32, c: Color) {
    if c.a == 0 || r <= 0.0 {
        return;
    }
    let x0 = ((cx - r).floor().max(0.0) as u32).min(fb.width);
    let y0 = ((cy - r).floor().max(0.0) as u32).min(fb.height);
    let x1 = ((cx + r).ceil().max(0.0) as u32).min(fb.width);
    let y1 = ((cy + r).ceil().max(0.0) as u32).min(fb.height);
    let (x0, y0, x1, y1) = rasterizer::scissor_clamp_window(x0, y0, x1, y1);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let pm = c.premultiply();
    for y in y0..y1 {
        let fy = y as f32 + 0.5;
        for x in x0..x1 {
            let fx = x as f32 + 0.5;
            let d = ((fx - cx) * (fx - cx) + (fy - cy) * (fy - cy)).sqrt() - r;
            let cov = (-d + 0.5).clamp(0.0, 1.0);
            if cov <= 0.0 {
                continue;
            }
            let mut src = pm;
            if cov < 1.0 {
                src.a = (src.a as f32 * cov + 0.5) as u8;
                src.r = (src.r as f32 * cov + 0.5) as u8;
                src.g = (src.g as f32 * cov + 0.5) as u8;
                src.b = (src.b as f32 * cov + 0.5) as u8;
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

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::geometry::Affine2D;
    use liquide_compositor::pixel::PixelFormat;
    use liquide_compositor::scene::{
        BorderSide, BorderSideStyle, BorderSides, FlatNode, SceneNodeKind,
    };

    fn side(style: BorderSideStyle, width: f32) -> BorderSide {
        BorderSide {
            width,
            style,
            color: Color::new(255, 0, 0, 255),
        }
    }

    fn border_node(style: BorderSideStyle, width: f32, radius: f32) -> FlatNode {
        let sides = BorderSides {
            top: side(style, width),
            right: side(style, width),
            bottom: side(style, width),
            left: side(style, width),
        };
        FlatNode {
            id: 1,
            kind: SceneNodeKind::Border {
                sides,
                radius: (radius, radius, radius, radius),
            }
            .into(),
            absolute_bounds: Rect::new(0.0, 0.0, 64.0, 64.0),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (radius, radius, radius, radius),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        }
    }

    fn render(style: BorderSideStyle, width: f32, radius: f32) -> FrameBuffer {
        let mut r = SoftwareRenderer::new();
        let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
        let node = border_node(style, width, radius);
        r.render_border_node(&node, &mut fb);
        fb
    }

    // Count painted (non-transparent) pixels along the TOP border band's centre
    // row, i.e. how much of the top edge is "on".
    fn painted_on_top_row(fb: &FrameBuffer, row: u32) -> usize {
        (0..fb.width)
            .filter(|&x| fb.get_pixel(x, row).a > 0)
            .count()
    }

    // ── Dashed: the top edge has GAPS (solid does not) ──────────────────

    #[test]
    fn straight_dashed_has_gaps_solid_does_not() {
        let solid = render(BorderSideStyle::Solid, 6.0, 0.0);
        let dashed = render(BorderSideStyle::Dashed, 6.0, 0.0);
        let solid_on = painted_on_top_row(&solid, 2);
        let dashed_on = painted_on_top_row(&dashed, 2);
        assert!(
            solid_on as f32 >= dashed_on as f32 * 1.3,
            "dashed must leave gaps along the edge (solid_on={solid_on}, dashed_on={dashed_on})"
        );
        assert!(dashed_on > 0, "dashed must still paint some dashes");
    }

    #[test]
    fn rounded_dashed_has_gaps_unlike_rounded_solid() {
        // Teeth target: a corner radius previously forced SOLID for every style.
        let solid = render(BorderSideStyle::Solid, 6.0, 16.0);
        let dashed = render(BorderSideStyle::Dashed, 6.0, 16.0);
        // Sample a straight portion of the top edge (centre, away from corners).
        let solid_on = (16..48).filter(|&x| solid.get_pixel(x, 2).a > 0).count();
        let dashed_on = (16..48).filter(|&x| dashed.get_pixel(x, 2).a > 0).count();
        assert!(
            solid_on > dashed_on,
            "rounded dashed must have gaps the rounded solid does not \
             (solid_on={solid_on}, dashed_on={dashed_on})"
        );
        assert!(dashed_on > 0, "rounded dashed must paint dashes");
    }

    // ── Dotted: ROUND dots, not square ──────────────────────────────────

    #[test]
    fn straight_dotted_is_round_not_square() {
        // A round dot is narrower at its top/bottom rows than at its middle row.
        // A square would have IDENTICAL coverage on every row of the band.
        let dotted = render(BorderSideStyle::Dotted, 10.0, 0.0);
        let mid = painted_on_top_row(&dotted, 5); // centre of the 10px band
        let edge = painted_on_top_row(&dotted, 0); // top row of the band
        assert!(
            mid > edge,
            "round dots taper: centre row ({mid}) must paint MORE than the edge row ({edge}); \
             equal counts would mean squares"
        );
        assert!(edge < mid, "a square dotted border would fail this (teeth)");
    }

    #[test]
    fn rounded_dotted_is_round_not_solid() {
        let dotted = render(BorderSideStyle::Dotted, 10.0, 18.0);
        // Straight portion of the top edge: must have gaps between dots.
        let on = (18..46).filter(|&x| dotted.get_pixel(x, 5).a > 0).count();
        let span = 46 - 18;
        assert!(on > 0 && on < span, "rounded dotted must have round-dot gaps, not solid (on={on}/{span})");
    }

    // ── Double: TWO lines with a gap between them ────────────────────────

    #[test]
    fn straight_double_draws_two_lines() {
        let dbl = render(BorderSideStyle::Double, 9.0, 0.0);
        // Down a column through the top band (x=32): on, off (middle), on.
        let col: Vec<bool> = (0..9).map(|y| dbl.get_pixel(32, y).a > 0).collect();
        let groups = col
            .iter()
            .collect::<Vec<_>>()
            .split(|&&on| !on)
            .filter(|g| !g.is_empty())
            .count();
        assert_eq!(groups, 2, "double border must be two separated lines: {col:?}");
    }

    #[test]
    fn rounded_double_draws_two_lines() {
        let dbl = render(BorderSideStyle::Double, 9.0, 16.0);
        // Column at the centre of the top edge.
        let col: Vec<bool> = (0..9).map(|y| dbl.get_pixel(32, y).a > 0).collect();
        let groups = col
            .iter()
            .collect::<Vec<_>>()
            .split(|&&on| !on)
            .filter(|g| !g.is_empty())
            .count();
        assert_eq!(groups, 2, "rounded double must also be two lines: {col:?}");
    }

    // ── Groove/Ridge/Inset/Outset: bevel = two different shades ─────────

    #[test]
    fn straight_outset_top_and_bottom_sides_bevel_opposite_shades() {
        // CSS inset/outset shade each side flat, but TOP/LEFT and BOTTOM/RIGHT get
        // OPPOSITE shades (the 3D bevel). Outset → top light, bottom dark.
        let outset = render(BorderSideStyle::Outset, 8.0, 0.0);
        let top = outset.get_pixel(32, 2);
        let bottom = outset.get_pixel(32, 61);
        assert!(top.a > 0 && bottom.a > 0, "outset bands must paint");
        assert_ne!(
            (top.r, top.g, top.b),
            (bottom.r, bottom.g, bottom.b),
            "outset must bevel: top {top:?} vs bottom {bottom:?}"
        );
        // Top is the lit (lighter) side for outset; bottom is the shaded (darker).
        assert!(
            top.r >= bottom.r && (top.g > bottom.g || top.b > bottom.b),
            "outset top side must be lighter than bottom (top={top:?}, bottom={bottom:?})"
        );
    }

    #[test]
    fn straight_inset_bevel_is_inverse_of_outset() {
        // Teeth: inset must flip the bevel vs outset (top dark, bottom light).
        let inset = render(BorderSideStyle::Inset, 8.0, 0.0);
        let top = inset.get_pixel(32, 2);
        let bottom = inset.get_pixel(32, 61);
        assert!(
            bottom.r >= top.r && (bottom.g > top.g || bottom.b > top.b),
            "inset top must be darker than bottom (top={top:?}, bottom={bottom:?})"
        );
    }

    #[test]
    fn rounded_groove_has_two_shades_not_flat_solid() {
        let groove = render(BorderSideStyle::Groove, 8.0, 16.0);
        // Centre of top edge: outer band pixel vs inner band pixel differ.
        let outer = groove.get_pixel(32, 1);
        let inner = groove.get_pixel(32, 6);
        assert!(outer.a > 0 && inner.a > 0, "groove band must paint");
        assert_ne!(
            (outer.r, outer.g, outer.b),
            (inner.r, inner.g, inner.b),
            "rounded groove must show a light/dark bevel, not flat solid"
        );
    }

    // ── Teeth: forcing a style back to "solid" must break the style tests ─

    #[test]
    fn rounded_solid_top_edge_is_continuous() {
        // Anchor: rounded SOLID is fully continuous along the straight top edge.
        // (If the dashed/dotted fixes regressed solid into gaps, this fails.)
        let solid = render(BorderSideStyle::Solid, 6.0, 16.0);
        let on = (20..44).all(|x| solid.get_pixel(x, 2).a > 0);
        assert!(on, "rounded solid top edge must be continuous (no spurious gaps)");
    }
}
