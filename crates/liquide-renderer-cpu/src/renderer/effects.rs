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

            // Apply tint (confined to the active damage region, t76).
            let mut tint = params.tint_color;
            tint.a = (tint.a as f32 * opacity + 0.5) as u8;
            if let Some(tint_rect) = rasterizer::clip_rect(bounds, self.raster_clip) {
                rasterizer::fill_rect(fb, tint_rect, tint, BlendMode::SrcOver);
            }

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

            // On a partial-damage frame (write-scissor active) `generate_shadow_mask`
            // confines the mask to (shadow rect ∩ scissor+blur margin) — byte-
            // identical to the full mask inside the damage rect (t82). Such a mask
            // is CLIP-SPECIFIC, so it must NOT be cached (the cache is keyed on
            // bounds only; a later frame with a different clip would wrongly reuse
            // it). The full-frame path (no scissor) keeps the bounds-keyed cache so
            // a steady shadow is generated once and reused.
            if crate::rasterizer::write_scissor().is_some() {
                if let Some(mask) = BoxShadow::generate_shadow_mask(fb.width, fb.height, &params) {
                    BoxShadow::composite_shadow_mask(fb, &mask);
                }
                return;
            }

            let cache_hit = self
                .shadow_cache
                .get(&node.id)
                .is_some_and(|c| c.bx == bx && c.by == by && c.bw == bw && c.bh == bh);

            if cache_hit {
                if let Some(cached) = self.shadow_cache.get(&node.id) {
                    BoxShadow::composite_shadow_mask(fb, &cached.mask);
                }
            } else if let Some(mask) = BoxShadow::generate_shadow_mask(fb.width, fb.height, &params) {
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
        // Full pixel-snapped glass bounds, clamped to the framebuffer. This is
        // the region the blur is logically defined over (its edge handling is
        // clamp-to-edge at THESE bounds).
        let gx0 = (bounds.x.max(0.0) as u32).min(fb.width);
        let gy0 = (bounds.y.max(0.0) as u32).min(fb.height);
        let gx1 = (bounds.right().ceil() as u32).min(fb.width);
        let gy1 = (bounds.bottom().ceil() as u32).min(fb.height);
        let gw = gx1.saturating_sub(gx0);
        let gh = gy1.saturating_sub(gy0);

        if gw == 0 || gh == 0 {
            return;
        }

        // ── Damage-confined blur source window (t82) ──────────────────────────
        //
        // On a partial-damage frame the blur EFFECT (snapshot + convolution) was
        // previously computed over the FULL glass bounds even when the damage is
        // a tiny rect, so any hover over a big glass surface (launcher, menu, dock)
        // paid nearly the full-frame cost regardless of clip size (t78 bench).
        //
        // When `raster_clip` is `Some`, we shrink the SOURCE region the blur is
        // computed over to (glass ∩ damage) EXPANDED by the blur sample radius on
        // all sides, then snapped so the result is BYTE-IDENTICAL to the full
        // computation inside the damage rect:
        //   * margin = `radius` on every side that is interior to the glass bounds.
        //     The separable Gaussian (clamp-to-edge) at output pixel p depends on
        //     source pixels within ±radius of p in each axis; with that margin the
        //     intermediate H-pass values feeding the target rows are themselves
        //     correct, and no clamp differs from the full computation.
        //   * origin snapped DOWN to EVEN coords. The large-radius path
        //     (`compute_blur`, radius ≥ 8) downsamples 2× on a grid anchored at the
        //     buffer origin; an even crop origin makes the cropped downsample an
        //     exact sub-rect of the full downsample, so downsample → half-res blur
        //     → bilinear upsample reproduce the full result bit-for-bit at interior
        //     (≥ radius from a non-true-edge) pixels — which the target span is.
        //   * far edge extended to keep ≥ radius margin (also even-rounded up).
        // Where the window coincides with the true glass edge, clamp-to-edge
        // matches the full computation automatically.
        //
        // The write-back is then confined to (glass ∩ damage), further clamped by
        // the hard write-scissor (t80).
        //
        // Whether a damage-confined blur can be made BYTE-IDENTICAL to the full
        // computation for this node. The large-radius path downsamples 2×; its
        // bilinear upsample uses scale = (dim/2)/dim, which is exactly 0.5 — the
        // value that makes a cropped window reproduce the full result at interior
        // pixels — only when the glass width AND height are even. For odd glass
        // dims (radius ≥ 8) we cannot guarantee identical edges, so we fall back
        // to the FULL bounds rather than ship a visual artifact. The full-res
        // path (radius < 8) is always safe (clamp-to-edge separable Gaussian).
        let downsample_path = radius >= 8;
        let clip_safe = !downsample_path || (gw % 2 == 0 && gh % 2 == 0);

        let (sx0, sy0, sx1, sy1, blit_x0, blit_y0, blit_x1, blit_y1) = match self.raster_clip {
            Some(clip) if clip_safe => {
                let cx0 = (clip.x.max(0.0) as u32).max(gx0).min(gx1);
                let cy0 = (clip.y.max(0.0) as u32).max(gy0).min(gy1);
                let cx1 = (clip.right().ceil().max(0.0) as u32).min(gx1).max(cx0);
                let cy1 = (clip.bottom().ceil().max(0.0) as u32).min(gy1).max(cy0);
                if cx1 <= cx0 || cy1 <= cy0 {
                    // Damage does not touch this glass node — nothing to blur/blit.
                    #[cfg(test)]
                    self.last_blur_source_px.set(0);
                    return;
                }
                // Expand the (glass ∩ damage) target by the sample margin, snap the
                // origin DOWN to even, clamp to the glass bounds. The full-res
                // separable Gaussian reaches ±radius. The large-radius path also
                // downsamples 2× then bilinearly upsamples, so a full-space dst
                // pixel additionally reads one extra HALF-RES neighbour each side
                // (= 2 full px) beyond the half-res kernel's ±radius reach; we add
                // a small slack so the convolved source feeding every target pixel
                // is byte-identical to the full computation.
                let r = if downsample_path { radius + 4 } else { radius };
                let mut wx0 = cx0.saturating_sub(r);
                let mut wy0 = cy0.saturating_sub(r);
                let mut wx1 = (cx1 + r).min(gx1);
                let mut wy1 = (cy1 + r).min(gy1);
                wx0 = wx0.max(gx0);
                wy0 = wy0.max(gy0);
                // Snap origin down to an even offset RELATIVE to the glass origin
                // so the downsample 2× grid phase matches the full computation.
                if (wx0 - gx0) % 2 == 1 {
                    wx0 -= 1;
                }
                if (wy0 - gy0) % 2 == 1 {
                    wy0 -= 1;
                }
                wx0 = wx0.max(gx0);
                wy0 = wy0.max(gy0);
                // Keep the window width/height even (relative to its own origin) so
                // the cropped downsample covers whole 2× blocks like the full one;
                // extend the far edge, clamped to the glass bounds.
                if (wx1 - wx0) % 2 == 1 && wx1 < gx1 {
                    wx1 += 1;
                }
                if (wy1 - wy0) % 2 == 1 && wy1 < gy1 {
                    wy1 += 1;
                }
                (wx0, wy0, wx1, wy1, cx0, cy0, cx1, cy1)
            }
            // clip None, OR clip Some but the node can't be safely confined:
            // compute over the FULL glass bounds (byte-identical to the historic
            // path). The blit covers the full bounds and the write-scissor (t80)
            // still confines the actual writes to the damage rect.
            _ => (gx0, gy0, gx1, gy1, gx0, gy0, gx1, gy1),
        };

        let x0 = sx0;
        let y0 = sy0;
        let x1 = sx1;
        let y1 = sy1;
        let w = x1.saturating_sub(x0);
        let h = y1.saturating_sub(y0);

        if w == 0 || h == 0 {
            return;
        }

        #[cfg(test)]
        self.last_blur_source_px.set((w * h) as usize);

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

        // Deterministic capture path: if the blur for this content/geometry/radius
        // is not already cached, compute it SYNCHRONOUSLY so the glass region is
        // always blurred and identical run-to-run (no dependence on async worker
        // timing — the source of e2e_temporal blur flakiness). The result is
        // byte-identical to the async worker's output (same `compute_blur`).
        if self.deterministic_blur && self.blur_worker.get_cached(key, w, h).is_none() {
            self.blur_worker
                .compute_blur_blocking(key, snapshot.clone(), w, h, radius);
        }

        // Blit cached blur result if available. The blur is computed over the
        // (possibly damage-confined) SOURCE window `[x0,x1)×[y0,y1)` (t82), whose
        // interior — everything inside the (glass ∩ damage) target — is
        // byte-identical to the full-bounds blur. The WRITE-BACK is confined to
        // that target `[blit_x0,blit_x1)×[blit_y0,blit_y1)`, further clamped by
        // the per-thread write-scissor (t80) so no pixel escapes the damage rect
        // (the t79 regression). Each row copies the sub-span out of the cached
        // window buffer at the correct (col, row) offset relative to `(x0, y0)`.
        let has_cache = if let Some(cached) = self.blur_worker.get_cached(key, w, h) {
            for dy in blit_y0..blit_y1 {
                // Clamp this row's [blit_x0, blit_x1) destination span to the
                // scissor, in BOTH axes — a row outside the scissor's y-range must
                // be skipped entirely (the scissor confines writes to the damage
                // rect, not just its column span).
                let (cx0, cy0s, cx1, cy1s) =
                    crate::rasterizer::scissor_clamp_window(blit_x0, dy, blit_x1, dy + 1);
                if cx1 <= cx0 || cy1s <= cy0s {
                    continue;
                }
                let row = (dy - y0) as usize;
                let col0 = (cx0 - x0) as usize;
                let span = (cx1 - cx0) as usize;
                let src_off = (row * w as usize + col0) * 4;
                let dst_off = fb.pixel_offset(cx0, dy);
                let bytes = span * 4;
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

        // Submit new blur request if worker doesn't have one pending. In the
        // deterministic path the result is already cached above, so this is
        // skipped (no redundant async work).
        if !self.deterministic_blur && (!has_cache || !self.blur_worker.has_pending(key)) {
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

    // ── t82: damage-confined backdrop blur ────────────────────────────────────

    /// Run a backdrop blur over `bounds` with `radius`, optionally clipped to
    /// `clip` (matching the production write-scissor + raster_clip wiring), in the
    /// deterministic (synchronous) blur mode so the result is present immediately
    /// and byte-stable. Returns the framebuffer after the blur write-back.
    fn run_blur(bounds: Rect, radius: u32, clip: Option<Rect>, fb_w: u32, fb_h: u32) -> FrameBuffer {
        let mut renderer = SoftwareRenderer::new();
        renderer.deterministic_blur = true; // synchronous, byte-stable
        renderer.raster_clip = clip;
        let prev = crate::rasterizer::set_write_scissor(clip);

        let mut fb = FrameBuffer::new(fb_w, fb_h, PixelFormat::Bgra8);
        paint_backdrop(&mut fb);
        renderer.render_backdrop_blur(1, bounds, radius, &mut fb);

        crate::rasterizer::set_write_scissor(prev);
        fb
    }

    /// CORRECTNESS: the damage-clipped blur must be BYTE-IDENTICAL to the
    /// full-backdrop blur inside the damage rect, across several radii and damage
    /// positions — including a damage rect sitting ON the glass edge, where the
    /// sample margin matters most.
    #[test]
    fn clipped_blur_is_byte_identical_to_full_blur_inside_damage() {
        // Glass at an even origin & even dims (the common, safely-confinable case),
        // plus a deliberately ODD-dim glass to exercise the safe-fallback path.
        let cases = [
            // (bounds, radius, damage rect)
            (Rect::new(8.0, 8.0, 200.0, 160.0), 4, Rect::new(40.0, 40.0, 24.0, 24.0)),
            (Rect::new(8.0, 8.0, 200.0, 160.0), 12, Rect::new(90.0, 70.0, 32.0, 32.0)),
            (Rect::new(8.0, 8.0, 200.0, 160.0), 20, Rect::new(120.0, 100.0, 16.0, 16.0)),
            // Damage ON the top-left glass edge (margin must clamp to the edge).
            (Rect::new(8.0, 8.0, 200.0, 160.0), 12, Rect::new(8.0, 8.0, 20.0, 20.0)),
            // Damage ON the bottom-right glass edge.
            (Rect::new(8.0, 8.0, 200.0, 160.0), 16, Rect::new(180.0, 140.0, 28.0, 28.0)),
            // Odd-dim glass + large radius → safe fallback to full bounds; must
            // still be byte-identical inside the damage rect.
            (Rect::new(8.0, 8.0, 201.0, 161.0), 12, Rect::new(90.0, 70.0, 30.0, 30.0)),
        ];

        for (bounds, radius, dmg) in cases {
            let full = run_blur(bounds, radius, None, 256, 224);
            let clipped = run_blur(bounds, radius, Some(dmg), 256, 224);

            // Compare every pixel inside the damage rect (∩ framebuffer).
            let dx0 = dmg.x as u32;
            let dy0 = dmg.y as u32;
            let dx1 = (dmg.right().ceil() as u32).min(256);
            let dy1 = (dmg.bottom().ceil() as u32).min(224);
            let mut compared = 0u32;
            for y in dy0..dy1 {
                for x in dx0..dx1 {
                    assert_eq!(
                        clipped.get_pixel(x, y),
                        full.get_pixel(x, y),
                        "clipped blur differs from full blur at ({x},{y}) \
                         for bounds={bounds:?} radius={radius} damage={dmg:?}"
                    );
                    compared += 1;
                }
            }
            assert!(compared > 0, "test compared no pixels for {dmg:?}");

            // And the clipped blur must NOT have written ANYTHING outside the
            // damage rect: every pixel outside the damage rect must still equal
            // the sharp backdrop, while the FULL blur softened many of them
            // (proving the region really is glass/blurry and the clip is what
            // suppressed the writes — not an all-no-op).
            let mut sharp = FrameBuffer::new(256, 224, PixelFormat::Bgra8);
            paint_backdrop(&mut sharp);
            let in_damage = |x: u32, y: u32| x >= dx0 && x < dx1 && y >= dy0 && y < dy1;
            let mut full_softened_outside = 0u32;
            for y in (bounds.y as u32)..(bounds.bottom() as u32).min(224) {
                for x in (bounds.x as u32)..(bounds.right() as u32).min(256) {
                    if in_damage(x, y) {
                        continue;
                    }
                    assert_eq!(
                        clipped.get_pixel(x, y),
                        sharp.get_pixel(x, y),
                        "clipped blur wrote OUTSIDE the damage rect at ({x},{y}) \
                         for damage={dmg:?}"
                    );
                    if full.get_pixel(x, y) != sharp.get_pixel(x, y) {
                        full_softened_outside += 1;
                    }
                }
            }
            assert!(
                full_softened_outside > 100,
                "full blur should have softened many pixels outside the damage rect \
                 (only {full_softened_outside}) for {dmg:?}"
            );
        }
    }

    /// ADVERSARIAL SWEEP: every (radius × damage-position) combination over a
    /// fixed glass surface must be byte-identical inside the damage rect — this
    /// catches a too-small margin at any radius/offset, not just hand-picked ones.
    #[test]
    fn clipped_blur_byte_identical_across_radii_and_positions_sweep() {
        let bounds = Rect::new(4.0, 4.0, 240.0, 200.0); // even dims
        let fb_w = 260u32;
        let fb_h = 220u32;
        for &radius in &[1u32, 3, 7, 8, 9, 14, 20, 30] {
            let full = run_blur(bounds, radius, None, fb_w, fb_h);
            // Damage rects of varying size at varying offsets, including the four
            // glass corners (where the margin clamps to the true edge).
            for &( dx, dy, dw, dh) in &[
                (4.0, 4.0, 18.0, 18.0),       // top-left corner
                (226.0, 4.0, 18.0, 18.0),     // top-right corner
                (4.0, 186.0, 18.0, 18.0),     // bottom-left corner
                (226.0, 186.0, 18.0, 18.0),   // bottom-right corner
                (60.0, 50.0, 13.0, 27.0),     // odd-sized interior
                (121.0, 99.0, 7.0, 7.0),      // odd-offset tiny interior
                (10.0, 90.0, 200.0, 11.0),    // wide thin strip
            ] {
                let dmg = Rect::new(dx, dy, dw, dh);
                let clipped = run_blur(bounds, radius, Some(dmg), fb_w, fb_h);
                let ix0 = dx as u32;
                let iy0 = dy as u32;
                let ix1 = (dmg.right().ceil() as u32).min(fb_w);
                let iy1 = (dmg.bottom().ceil() as u32).min(fb_h);
                for y in iy0..iy1 {
                    for x in ix0..ix1 {
                        assert_eq!(
                            clipped.get_pixel(x, y),
                            full.get_pixel(x, y),
                            "sweep mismatch at ({x},{y}) radius={radius} damage={dmg:?}"
                        );
                    }
                }
            }
        }
    }

    /// COST/BEHAVIOR: a small damage rect over a large glass surface must shrink
    /// the blur SOURCE area to ~O(damage + radius border), NOT the full backdrop.
    /// Fails (reverting to full-area) if the damage-confinement is removed.
    #[test]
    fn clipped_blur_source_area_is_proportional_to_damage_not_full_backdrop() {
        let bounds = Rect::new(0.0, 0.0, 400.0, 400.0); // 160 000-px glass
        let radius = 12u32;
        let dmg = Rect::new(180.0, 180.0, 32.0, 32.0); // tiny central damage

        // Full (clip None): the blur source is the whole glass.
        let mut full_r = SoftwareRenderer::new();
        full_r.deterministic_blur = true;
        full_r.raster_clip = None;
        let mut fb_full = FrameBuffer::new(400, 400, PixelFormat::Bgra8);
        paint_backdrop(&mut fb_full);
        full_r.render_backdrop_blur(1, bounds, radius, &mut fb_full);
        let full_px = full_r.last_blur_source_px.get();
        assert_eq!(full_px, 400 * 400, "full path should snapshot the whole glass");

        // Clipped: the blur source must be a small window around the damage rect.
        let mut clip_r = SoftwareRenderer::new();
        clip_r.deterministic_blur = true;
        clip_r.raster_clip = Some(dmg);
        let prev = crate::rasterizer::set_write_scissor(Some(dmg));
        let mut fb_clip = FrameBuffer::new(400, 400, PixelFormat::Bgra8);
        paint_backdrop(&mut fb_clip);
        clip_r.render_backdrop_blur(1, bounds, radius, &mut fb_clip);
        crate::rasterizer::set_write_scissor(prev);
        let clip_px = clip_r.last_blur_source_px.get();

        // Expected upper bound: damage + 2*(margin) on each axis, even-rounded.
        // The downsample path uses margin radius+4, so 32 + 2*(12+4) = 64 → at
        // most ~66*66. The key assertion is that it is a SMALL FRACTION of the
        // full backdrop (here ~3% of 160 000) and would jump to 160 000 on
        // regression to full-area blur.
        let margin = radius as usize + 4; // matches downsample-path margin
        let bound = (32 + 2 * margin + 4).pow(2);
        assert!(
            clip_px <= bound,
            "clipped blur source {clip_px} px exceeds expected ~O(damage+radius) bound {bound}"
        );
        assert!(
            clip_px * 10 < full_px,
            "clipped blur source {clip_px} px is not a small fraction of the full \
             {full_px} px — damage confinement regressed to full-area blur"
        );
    }

    /// CORRECTNESS: a damage-confined box-shadow mask must composite BYTE-
    /// IDENTICALLY to the full-mask path inside the damage rect, and must not
    /// write outside it. Catches a too-small mask margin (blur bleed) or an
    /// over-confinement that drops shadow pixels inside the damage rect.
    #[test]
    fn confined_shadow_mask_is_byte_identical_inside_damage() {
        use crate::effects::{BoxShadow, ShadowParams};
        let surface = Rect::new(40.0, 40.0, 180.0, 140.0);
        for blur_radius in [0u32, 4, 12, 20] {
            let params = ShadowParams {
                surface_rect: surface,
                corner_radius: 16.0,
                spread: 6.0,
                blur_radius,
                offset_x: 0.0,
                offset_y: 0.0,
                shadow_color: Color::new(0, 0, 0, 180),
            };

            // FULL: no scissor → mask over the whole shadow rect.
            let mut full = FrameBuffer::new(280, 240, PixelFormat::Bgra8);
            paint_backdrop(&mut full);
            if let Some(mask) = BoxShadow::generate_shadow_mask(full.width, full.height, &params) {
                BoxShadow::composite_shadow_mask(&mut full, &mask);
            }

            // Damage rects across the shadow (corner, edge, interior of the blur halo).
            for &(dx, dy, dw, dh) in &[
                (30.0, 30.0, 24.0, 24.0),   // top-left shadow halo corner
                (120.0, 100.0, 20.0, 20.0), // interior
                (205.0, 165.0, 30.0, 30.0), // bottom-right halo
            ] {
                let dmg = Rect::new(dx, dy, dw, dh);
                let mut clipped = FrameBuffer::new(280, 240, PixelFormat::Bgra8);
                paint_backdrop(&mut clipped);
                let prev = crate::rasterizer::set_write_scissor(Some(dmg));
                if let Some(mask) =
                    BoxShadow::generate_shadow_mask(clipped.width, clipped.height, &params)
                {
                    BoxShadow::composite_shadow_mask(&mut clipped, &mask);
                }
                crate::rasterizer::set_write_scissor(prev);

                let ix0 = dx as u32;
                let iy0 = dy as u32;
                let ix1 = (dmg.right().ceil() as u32).min(280);
                let iy1 = (dmg.bottom().ceil() as u32).min(240);
                for y in iy0..iy1 {
                    for x in ix0..ix1 {
                        assert_eq!(
                            clipped.get_pixel(x, y),
                            full.get_pixel(x, y),
                            "shadow mismatch at ({x},{y}) blur_radius={blur_radius} damage={dmg:?}"
                        );
                    }
                }
                // Nothing written outside the damage rect.
                let mut sharp = FrameBuffer::new(280, 240, PixelFormat::Bgra8);
                paint_backdrop(&mut sharp);
                for y in 0..240u32 {
                    for x in 0..280u32 {
                        let inside = x >= ix0 && x < ix1 && y >= iy0 && y < iy1;
                        if !inside {
                            assert_eq!(
                                clipped.get_pixel(x, y),
                                sharp.get_pixel(x, y),
                                "shadow wrote OUTSIDE damage at ({x},{y}) damage={dmg:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// A full-damage (clip None) blur must be IDENTICAL to the historic path:
    /// the entire glass region is blurred (this guards the e2e_temporal / capture
    /// golden no-op requirement at the unit level).
    #[test]
    fn clip_none_blurs_entire_glass_region() {
        let bounds = Rect::new(8.0, 8.0, 120.0, 96.0);
        let blurred = run_blur(bounds, 12, None, 160, 128);
        let mut sharp = FrameBuffer::new(160, 128, PixelFormat::Bgra8);
        paint_backdrop(&mut sharp);
        // Many interior glass pixels must differ from the sharp backdrop.
        let mut differing = 0u32;
        for y in 20..100 {
            for x in 20..120 {
                if blurred.get_pixel(x, y) != sharp.get_pixel(x, y) {
                    differing += 1;
                }
            }
        }
        assert!(
            differing > 1000,
            "clip-None must blur the whole glass region (only {differing} px changed)"
        );
    }
}
