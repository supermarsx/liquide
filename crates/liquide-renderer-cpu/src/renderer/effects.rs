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

        if let SceneNodeKind::Glass(params) = node.kind_ref() {
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
        } = node.kind_ref()
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
                if let Some(mask) = BoxShadow::generate_shadow_mask(fb.width, fb.height, &params) {
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

        if let SceneNodeKind::BoxShadows { shadows } = node.kind_ref() {
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
                        if sc.a == 0 {
                            continue;
                        }
                        let t = i as f32;
                        // Top edge
                        let top_y = iy + oy.max(0.0) + t;
                        if top_y < iy + ih {
                            rasterizer::fill_rect(
                                fb,
                                Rect::new(ix, top_y, iw, 1.0_f32.min(ih)),
                                sc,
                                BlendMode::SrcOver,
                            );
                        }
                        // Bottom edge
                        let bot_y = iy + ih - 1.0 + oy.min(0.0) - t;
                        if bot_y >= iy {
                            rasterizer::fill_rect(
                                fb,
                                Rect::new(ix, bot_y, iw, 1.0_f32.min(ih)),
                                sc,
                                BlendMode::SrcOver,
                            );
                        }
                        // Left edge
                        let left_x = ix + ox.max(0.0) + t;
                        if left_x < ix + iw {
                            rasterizer::fill_rect(
                                fb,
                                Rect::new(left_x, iy, 1.0_f32.min(iw), ih),
                                sc,
                                BlendMode::SrcOver,
                            );
                        }
                        // Right edge
                        let right_x = ix + iw - 1.0 + ox.min(0.0) - t;
                        if right_x >= ix {
                            rasterizer::fill_rect(
                                fb,
                                Rect::new(right_x, iy, 1.0_f32.min(iw), ih),
                                sc,
                                BlendMode::SrcOver,
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

        if let SceneNodeKind::BackdropFilter { filters } = node.kind_ref() {
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
                            crate::filter::PixelFilter::Saturate(1.0 - amount).apply(fb, bounds);
                        }
                    }
                    BackdropFilterSpec::Sepia(amount) => {
                        if *amount >= 0.99 {
                            crate::filter::PixelFilter::Sepia.apply(fb, bounds);
                        } else if *amount > 0.001 {
                            crate::filter::PixelFilter::ColorMatrix(partial_sepia_matrix(*amount))
                                .apply(fb, bounds);
                        }
                    }
                    BackdropFilterSpec::Invert(amount) => {
                        if *amount >= 0.99 {
                            crate::filter::PixelFilter::Invert.apply(fb, bounds);
                        } else if *amount > 0.001 {
                            crate::filter::PixelFilter::ColorMatrix(partial_invert_matrix(*amount))
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
    pub(crate) fn render_filter_node(&mut self, node: &FlatNode, fb: &mut FrameBuffer) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let SceneNodeKind::Filter { filters } = node.kind_ref() {
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
                            crate::filter::PixelFilter::Saturate(1.0 - amount).apply(fb, bounds);
                        }
                    }
                    FilterSpec::Sepia(amount) => {
                        if *amount >= 0.99 {
                            crate::filter::PixelFilter::Sepia.apply(fb, bounds);
                        } else if *amount > 0.001 {
                            crate::filter::PixelFilter::ColorMatrix(partial_sepia_matrix(*amount))
                                .apply(fb, bounds);
                        }
                    }
                    FilterSpec::Invert(amount) => {
                        if *amount >= 0.99 {
                            crate::filter::PixelFilter::Invert.apply(fb, bounds);
                        } else if *amount > 0.001 {
                            crate::filter::PixelFilter::ColorMatrix(partial_invert_matrix(*amount))
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
    ///
    /// The blur cache is keyed on STABLE attributes — pixel-snapped geometry,
    /// blur radius, and a hash of the underlying backdrop content — rather than
    /// the scene-node id. Scene-node ids are rebuilt every frame in the shell,
    /// so a node-id key never hit and the blur was recomputed (or dropped to a
    /// tint-only fill) on every frame, causing glass blur to flicker / not
    /// render. With a content+geometry key, a steady glass surface over steady
    /// content hits the cache and shows a stable blur; when the geometry, radius
    /// or underlying content actually changes the key changes and the blur is
    /// recomputed — exactly the correct invalidation behaviour.
    pub(crate) fn render_backdrop_blur(
        &mut self,
        _node_id: NodeId,
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

        // Snapshot the backdrop first so its content can participate in the
        // stable cache key (a steady backdrop yields a steady key → cache hit).
        let mut snapshot = vec![0u8; (w * h * 4) as usize];
        for row in 0..h {
            let src_off = fb.pixel_offset(x0, y0 + row);
            let dst_off = (row * w * 4) as usize;
            let bytes = (w * 4) as usize;
            snapshot[dst_off..dst_off + bytes].copy_from_slice(
                &fb.pixels_mut().expect("CPU framebuffer required")[src_off..src_off + bytes],
            );
        }

        let key = Self::stable_blur_key(x0, y0, w, h, radius, &snapshot);

        // Blit cached blur result if available.
        let has_cache = if let Some(cached) = self.blur_worker.get_cached(key, w, h) {
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
        if !has_cache || !self.blur_worker.has_pending(key) {
            self.blur_worker.request_blur(key, snapshot, w, h, radius);
        }
    }

    /// Derive a stable blur-cache key from the region geometry, blur radius and
    /// a hash of the underlying backdrop pixels.
    ///
    /// Independent of the (per-frame-churning) scene-node id: two frames whose
    /// glass surface sits at the same pixel-snapped rect, with the same radius,
    /// over the same backdrop content produce the same key and therefore reuse
    /// the cached blur instead of recomputing it.
    ///
    /// The content is hashed in a sub-sampled stride to keep the per-frame cost
    /// negligible for full-screen regions while still changing the key whenever
    /// the backdrop visibly changes.
    fn stable_blur_key(x0: u32, y0: u32, w: u32, h: u32, radius: u32, snapshot: &[u8]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        x0.hash(&mut hasher);
        y0.hash(&mut hasher);
        w.hash(&mut hasher);
        h.hash(&mut hasher);
        radius.hash(&mut hasher);
        snapshot.len().hash(&mut hasher);
        // Sub-sample: hash at most ~4096 evenly spaced bytes so the cost is
        // bounded regardless of region size. A single u8 step ensures small
        // regions are hashed in full.
        let step = (snapshot.len() / 4096).max(1);
        let mut i = 0;
        while i < snapshot.len() {
            snapshot[i].hash(&mut hasher);
            i += step;
        }
        hasher.finish()
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

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::geometry::Affine2D;
    use liquide_compositor::pixel::PixelFormat;
    use liquide_compositor::scene::GlassParams;
    use std::thread;
    use std::time::Duration;

    /// Paint a deterministic non-flat backdrop so the blur snapshot is non-trivial.
    fn paint_backdrop(fb: &mut FrameBuffer) {
        for y in 0..fb.height {
            for x in 0..fb.width {
                let c = Color::new((x * 7) as u8, (y * 5) as u8, ((x + y) * 3) as u8, 255);
                fb.set_pixel(x, y, c);
            }
        }
    }

    fn glass_node(id: NodeId, bounds: Rect, radius: u32) -> FlatNode {
        FlatNode {
            id,
            kind: SceneNodeKind::Glass(GlassParams {
                blur_radius: radius,
                tint_color: Color::new(255, 255, 255, 0), // no tint, isolate the blur
                inner_glow: false,
                parallax: false,
            })
            .into(),
            absolute_bounds: bounds,
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Spin until the worker has delivered enough blurs to reach `target`
    /// cached entries (bounded).
    fn await_blur_count(renderer: &mut SoftwareRenderer, target: usize) {
        for _ in 0..200 {
            renderer.poll_blur_results();
            if renderer.blur_cache_len() >= target {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// A steady glass surface over steady content must HIT the blur cache even
    /// though the scene-node id changes every frame. The stable key is derived
    /// from geometry + radius + backdrop content, not the node id.
    #[test]
    fn steady_glass_surface_hits_blur_cache_despite_node_id_churn() {
        let mut renderer = SoftwareRenderer::new();
        let bounds = Rect::new(8.0, 8.0, 48.0, 32.0);

        // Frame 1: node id 1 — submits a blur request (no cache yet).
        let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
        paint_backdrop(&mut fb);
        renderer.render_glass_node(&glass_node(1, bounds, 12), &mut fb, LodLevel::High, 1.0);

        // Worker computes the blur off-thread; wait for it.
        await_blur_count(&mut renderer, 1);
        assert_eq!(
            renderer.blur_cache_len(),
            1,
            "expected exactly one cached blur after the first frame"
        );

        // Frame 2: DIFFERENT node id (churn), same geometry/radius/backdrop.
        let mut fb2 = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
        paint_backdrop(&mut fb2);
        renderer.render_glass_node(&glass_node(99999, bounds, 12), &mut fb2, LodLevel::High, 1.0);

        // The stable key matched → cache stayed at one entry (no new key added).
        renderer.poll_blur_results();
        assert_eq!(
            renderer.blur_cache_len(),
            1,
            "node-id churn must not add a second cache entry for a steady surface"
        );

        // And the cached blur was actually blitted: the glass region is no longer
        // a verbatim copy of the sharp backdrop (blur softened it).
        let sharp = {
            let mut s = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
            paint_backdrop(&mut s);
            s
        };
        let mut differs = false;
        for y in 9..39 {
            for x in 9..55 {
                if fb2.get_pixel(x, y) != sharp.get_pixel(x, y) {
                    differs = true;
                }
            }
        }
        assert!(differs, "cached blur should have been composited into the glass region");
    }

    /// When the underlying backdrop content changes, the stable key changes, so
    /// the blur is recomputed (a second cache entry appears) — correct
    /// invalidation, not a stale blur.
    #[test]
    fn changed_backdrop_content_produces_a_new_blur_key() {
        let mut renderer = SoftwareRenderer::new();
        let bounds = Rect::new(8.0, 8.0, 48.0, 32.0);

        let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
        paint_backdrop(&mut fb);
        renderer.render_glass_node(&glass_node(1, bounds, 12), &mut fb, LodLevel::High, 1.0);
        await_blur_count(&mut renderer, 1);
        assert_eq!(renderer.blur_cache_len(), 1);

        // Same node id, same geometry/radius, but a DIFFERENT backdrop.
        let mut fb2 = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
        for y in 0..fb2.height {
            for x in 0..fb2.width {
                fb2.set_pixel(x, y, Color::new(200, 30, 90, 255));
            }
        }
        renderer.render_glass_node(&glass_node(1, bounds, 12), &mut fb2, LodLevel::High, 1.0);
        await_blur_count(&mut renderer, 2);
        assert_eq!(
            renderer.blur_cache_len(),
            2,
            "a changed backdrop must produce a new key → a fresh blur entry"
        );
    }
}
