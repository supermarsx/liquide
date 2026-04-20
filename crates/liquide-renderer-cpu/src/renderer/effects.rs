//! Glass, shadows, filters, and backdrop effect rendering for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{FlatNode, NodeId, SceneNodeKind};

use crate::effects::{BoxShadow, ShadowParams};
use crate::lod::LodLevel;
use crate::rasterizer;

use super::{CachedShadow, SoftwareRenderer};

impl SoftwareRenderer {
    /// Render a Glass scene node.
    pub(crate) fn render_glass_node(
        &mut self,
        node: &FlatNode,
        fb: &mut FrameBuffer,
        lod_level: LodLevel,
        quality_factor: f32,
    ) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let SceneNodeKind::Glass(params) = &node.kind {
            if self.blur_enabled && lod_level != LodLevel::Low {
                let radius = params.blur_radius.min(30);
                let lod_radius = (radius as f32 * quality_factor) as u32;
                if lod_radius > 0 {
                    self.render_backdrop_blur(node.id, bounds, lod_radius, fb);
                }
            }

            // Apply tint
            let mut tint = params.tint_color;
            tint.a = (tint.a as f32 * opacity + 0.5) as u8;
            rasterizer::fill_rect(fb, bounds, tint, BlendMode::SrcOver);

            // Inner glow (skip for low LOD)
            if params.inner_glow && lod_level != LodLevel::Low {
                crate::effects::InnerGlow::render_glow(
                    fb,
                    bounds,
                    8.0 * quality_factor,
                    3.0 * quality_factor,
                    Color::new(255, 255, 255, 30),
                );
            }
        }
    }

    /// Render a Shadow scene node.
    pub(crate) fn render_shadow_node(
        &mut self,
        node: &FlatNode,
        fb: &mut FrameBuffer,
        lod_level: LodLevel,
        quality_factor: f32,
    ) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let SceneNodeKind::Shadow {
            spread,
            blur_radius,
            color,
            corner_radius,
        } = &node.kind
        {
            if lod_level == LodLevel::Low {
                return;
            }

            let bx = bounds.x as i32;
            let by = bounds.y as i32;
            let bw = bounds.width as u32;
            let bh = bounds.height as u32;

            let cache_hit = self
                .shadow_cache
                .get(&node.id)
                .is_some_and(|c| c.bx == bx && c.by == by && c.bw == bw && c.bh == bh);

            if cache_hit {
                if let Some(cached) = self.shadow_cache.get(&node.id) {
                    BoxShadow::composite_shadow_mask(fb, &cached.mask);
                }
            } else {
                let shadow_color = Color::new(
                    color.r,
                    color.g,
                    color.b,
                    (color.a as f32 * opacity + 0.5) as u8,
                );
                let lod_blur_radius = (*blur_radius as f32 * quality_factor) as u32;
                let params = ShadowParams {
                    surface_rect: bounds,
                    corner_radius: *corner_radius,
                    spread: *spread,
                    blur_radius: lod_blur_radius,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    shadow_color,
                };
                if let Some(mask) =
                    BoxShadow::generate_shadow_mask(fb.width, fb.height, &params)
                {
                    BoxShadow::composite_shadow_mask(fb, &mask);
                    self.shadow_cache_insert(
                        node.id,
                        CachedShadow {
                            mask,
                            bx,
                            by,
                            bw,
                            bh,
                        },
                    );
                }
            }
        }
    }

    /// Render a BoxShadows scene node.
    pub(crate) fn render_box_shadows_node(
        &mut self,
        node: &FlatNode,
        fb: &mut FrameBuffer,
        lod_level: LodLevel,
        quality_factor: f32,
    ) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let SceneNodeKind::BoxShadows { shadows } = &node.kind {
            if lod_level == LodLevel::Low {
                return;
            }
            for shadow in shadows {
                if shadow.color.a == 0 {
                    continue;
                }
                let mut shadow_color = shadow.color;
                if opacity < 1.0 {
                    shadow_color.a = (shadow_color.a as f32 * opacity + 0.5) as u8;
                }

                if shadow.inset {
                    let blur = shadow.blur_radius.max(1.0);
                    let spread = shadow.spread_radius;
                    let ox = shadow.offset_x;
                    let oy = shadow.offset_y;
                    let ix = bounds.x + spread;
                    let iy = bounds.y + spread;
                    let iw = (bounds.width - spread * 2.0).max(0.0);
                    let ih = (bounds.height - spread * 2.0).max(0.0);
                    let c = shadow_color;
                    let steps = (blur as u32).max(1).min(8);
                    for i in 0..steps {
                        let frac = (steps - i) as f32 / steps as f32;
                        let mut sc = c;
                        sc.a = (c.a as f32 * frac * 0.5) as u8;
                        if sc.a == 0 { continue; }
                        let t = i as f32;
                        // Top edge
                        let top_y = iy + oy.max(0.0) + t;
                        if top_y < iy + ih {
                            rasterizer::fill_rect(
                                fb, Rect::new(ix, top_y, iw, 1.0_f32.min(ih)),
                                sc, BlendMode::SrcOver,
                            );
                        }
                        // Bottom edge
                        let bot_y = iy + ih - 1.0 + oy.min(0.0) - t;
                        if bot_y >= iy {
                            rasterizer::fill_rect(
                                fb, Rect::new(ix, bot_y, iw, 1.0_f32.min(ih)),
                                sc, BlendMode::SrcOver,
                            );
                        }
                        // Left edge
                        let left_x = ix + ox.max(0.0) + t;
                        if left_x < ix + iw {
                            rasterizer::fill_rect(
                                fb, Rect::new(left_x, iy, 1.0_f32.min(iw), ih),
                                sc, BlendMode::SrcOver,
                            );
                        }
                        // Right edge
                        let right_x = ix + iw - 1.0 + ox.min(0.0) - t;
                        if right_x >= ix {
                            rasterizer::fill_rect(
                                fb, Rect::new(right_x, iy, 1.0_f32.min(iw), ih),
                                sc, BlendMode::SrcOver,
                            );
                        }
                    }
                } else {
                    let lod_blur = (shadow.blur_radius * quality_factor) as u32;
                    let shadow_bounds = Rect::new(
                        bounds.x + shadow.offset_x - shadow.spread_radius,
                        bounds.y + shadow.offset_y - shadow.spread_radius,
                        bounds.width + shadow.spread_radius * 2.0,
                        bounds.height + shadow.spread_radius * 2.0,
                    );
                    let params = ShadowParams {
                        surface_rect: shadow_bounds,
                        corner_radius: 0.0,
                        spread: shadow.spread_radius,
                        blur_radius: lod_blur,
                        offset_x: shadow.offset_x,
                        offset_y: shadow.offset_y,
                        shadow_color,
                    };
                    if let Some(mask) =
                        BoxShadow::generate_shadow_mask(fb.width, fb.height, &params)
                    {
                        BoxShadow::composite_shadow_mask(fb, &mask);
                    }
                }
            }
        }
    }

    /// Render a BackdropFilter scene node.
    pub(crate) fn render_backdrop_filter_node(
        &mut self,
        node: &FlatNode,
        fb: &mut FrameBuffer,
        lod_level: LodLevel,
        quality_factor: f32,
    ) {
        let bounds = node.absolute_bounds;

        if let SceneNodeKind::BackdropFilter { filters } = &node.kind {
            use liquide_compositor::scene::BackdropFilterSpec;
            for filter in filters {
                match filter {
                    BackdropFilterSpec::Blur { radius } => {
                        let r = (*radius as u32).min(40);
                        if r > 0 && self.blur_enabled && lod_level != LodLevel::Low {
                            let lod_r = (r as f32 * quality_factor) as u32;
                            if lod_r > 0 {
                                self.render_backdrop_blur(node.id, bounds, lod_r, fb);
                            }
                        }
                    }
                    BackdropFilterSpec::Brightness(b) => {
                        crate::filter::PixelFilter::Brightness(*b).apply(fb, bounds);
                    }
                    BackdropFilterSpec::Contrast(c) => {
                        crate::filter::PixelFilter::Contrast(*c).apply(fb, bounds);
                    }
                    BackdropFilterSpec::Saturate(s) => {
                        crate::filter::PixelFilter::Saturate(*s).apply(fb, bounds);
                    }
                    BackdropFilterSpec::HueRotate(deg) => {
                        crate::filter::PixelFilter::HueRotate(*deg).apply(fb, bounds);
                    }
                    BackdropFilterSpec::Grayscale(amount) => {
                        if *amount >= 0.99 {
                            crate::filter::PixelFilter::Grayscale.apply(fb, bounds);
                        } else {
                            crate::filter::PixelFilter::Saturate(1.0 - amount)
                                .apply(fb, bounds);
                        }
                    }
                    BackdropFilterSpec::Sepia(amount) => {
                        if *amount >= 0.99 {
                            crate::filter::PixelFilter::Sepia.apply(fb, bounds);
                        } else if *amount > 0.001 {
                            crate::filter::PixelFilter::ColorMatrix(
                                partial_sepia_matrix(*amount),
                            )
                            .apply(fb, bounds);
                        }
                    }
                    BackdropFilterSpec::Invert(amount) => {
                        if *amount >= 0.99 {
                            crate::filter::PixelFilter::Invert.apply(fb, bounds);
                        } else if *amount > 0.001 {
                            crate::filter::PixelFilter::ColorMatrix(
                                partial_invert_matrix(*amount),
                            )
                            .apply(fb, bounds);
                        }
                    }
                    BackdropFilterSpec::Opacity(o) => {
                        crate::filter::PixelFilter::Opacity(*o).apply(fb, bounds);
                    }
                }
            }
        }
    }

    /// Render a Filter scene node.
    pub(crate) fn render_filter_node(
        &mut self,
        node: &FlatNode,
        fb: &mut FrameBuffer,
    ) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let SceneNodeKind::Filter { filters } = &node.kind {
            use liquide_compositor::scene::FilterSpec;
            for filter in filters {
                match filter {
                    FilterSpec::Blur { radius } => {
                        let r = (*radius as u32).min(40);
                        if r > 0 {
                            crate::blur::blur_region(fb, bounds, r);
                        }
                    }
                    FilterSpec::Brightness(b) => {
                        crate::filter::PixelFilter::Brightness(*b).apply(fb, bounds);
                    }
                    FilterSpec::Contrast(c) => {
                        crate::filter::PixelFilter::Contrast(*c).apply(fb, bounds);
                    }
                    FilterSpec::Saturate(s) => {
                        crate::filter::PixelFilter::Saturate(*s).apply(fb, bounds);
                    }
                    FilterSpec::HueRotate(deg) => {
                        crate::filter::PixelFilter::HueRotate(*deg).apply(fb, bounds);
                    }
                    FilterSpec::Grayscale(amount) => {
                        if *amount >= 0.99 {
                            crate::filter::PixelFilter::Grayscale.apply(fb, bounds);
                        } else {
                            crate::filter::PixelFilter::Saturate(1.0 - amount)
                                .apply(fb, bounds);
                        }
                    }
                    FilterSpec::Sepia(amount) => {
                        if *amount >= 0.99 {
                            crate::filter::PixelFilter::Sepia.apply(fb, bounds);
                        } else if *amount > 0.001 {
                            crate::filter::PixelFilter::ColorMatrix(
                                partial_sepia_matrix(*amount),
                            )
                            .apply(fb, bounds);
                        }
                    }
                    FilterSpec::Invert(amount) => {
                        if *amount >= 0.99 {
                            crate::filter::PixelFilter::Invert.apply(fb, bounds);
                        } else if *amount > 0.001 {
                            crate::filter::PixelFilter::ColorMatrix(
                                partial_invert_matrix(*amount),
                            )
                            .apply(fb, bounds);
                        }
                    }
                    FilterSpec::Opacity(o) => {
                        crate::filter::PixelFilter::Opacity(*o).apply(fb, bounds);
                    }
                    FilterSpec::DropShadow {
                        offset_x,
                        offset_y,
                        blur,
                        color,
                    } => {
                        let shadow_color = Color::new(
                            color.r,
                            color.g,
                            color.b,
                            (color.a as f32 * opacity + 0.5) as u8,
                        );
                        let params = ShadowParams {
                            surface_rect: bounds,
                            corner_radius: 0.0,
                            spread: 0.0,
                            blur_radius: (*blur as u32).min(40),
                            offset_x: *offset_x,
                            offset_y: *offset_y,
                            shadow_color,
                        };
                        if let Some(mask) =
                            BoxShadow::generate_shadow_mask(fb.width, fb.height, &params)
                        {
                            BoxShadow::composite_shadow_mask(fb, &mask);
                        }
                    }
                    FilterSpec::Url(_) => {} // SVG filter refs unsupported
                }
            }
        }
    }

    /// Submit an async backdrop blur for a region.
    ///
    /// Blits any cached result and submits a new blur request if needed.
    /// Used by Glass, BlurBackdrop, BlurCache, and LockScreen nodes.
    pub(crate) fn render_backdrop_blur(
        &mut self,
        node_id: NodeId,
        bounds: Rect,
        radius: u32,
        fb: &mut FrameBuffer,
    ) {
        let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
        let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
        let x1 = (bounds.right().ceil() as u32).min(fb.width);
        let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
        let w = x1.saturating_sub(x0);
        let h = y1.saturating_sub(y0);

        if w == 0 || h == 0 {
            return;
        }

        // Blit cached blur result if available.
        let has_cache = if let Some(cached) = self.blur_worker.get_cached(node_id, w, h) {
            for row in 0..h {
                let src_off = (row * w * 4) as usize;
                let dst_off = fb.pixel_offset(x0, y0 + row);
                let bytes = (w * 4) as usize;
                if src_off + bytes <= cached.pixels.len()
                    && dst_off + bytes <= fb.pixels_mut().expect("CPU framebuffer required").len()
                {
                    fb.pixels_mut().expect("CPU framebuffer required")[dst_off..dst_off + bytes]
                        .copy_from_slice(&cached.pixels[src_off..src_off + bytes]);
                }
            }
            true
        } else {
            false
        };

        // Submit new blur request if worker doesn't have one pending.
        if !has_cache || !self.blur_worker.has_pending(node_id) {
            let mut snapshot = vec![0u8; (w * h * 4) as usize];
            for row in 0..h {
                let src_off = fb.pixel_offset(x0, y0 + row);
                let dst_off = (row * w * 4) as usize;
                let bytes = (w * 4) as usize;
                snapshot[dst_off..dst_off + bytes]
                    .copy_from_slice(&fb.pixels_mut().expect("CPU framebuffer required")[src_off..src_off + bytes]);
            }
            self.blur_worker
                .request_blur(node_id, snapshot, w, h, radius);
        }
    }
}

/// Build a 5×4 color matrix for partial sepia (amount 0..1).
///
/// Interpolates between the identity matrix and the full sepia matrix.
fn partial_sepia_matrix(amount: f32) -> [f32; 20] {
    let a = amount;
    let b = 1.0 - amount;
    #[rustfmt::skip]
    let m = [
        b + a * 0.393, a * 0.769, a * 0.189, 0.0, 0.0,
        a * 0.349, b + a * 0.686, a * 0.168, 0.0, 0.0,
        a * 0.272, a * 0.534, b + a * 0.131, 0.0, 0.0,
        0.0, 0.0, 0.0, 1.0, 0.0,
    ];
    m
}

/// Build a 5×4 color matrix for partial invert (amount 0..1).
///
/// At amount=0 it's identity; at amount=1 it's full inversion.
fn partial_invert_matrix(amount: f32) -> [f32; 20] {
    let f = 1.0 - 2.0 * amount;
    #[rustfmt::skip]
    let m = [
        f,   0.0, 0.0, 0.0, amount,
        0.0, f,   0.0, 0.0, amount,
        0.0, 0.0, f,   0.0, amount,
        0.0, 0.0, 0.0, 1.0, 0.0,
    ];
    m
}
