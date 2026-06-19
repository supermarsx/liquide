//! Glass, shadows, filters, and backdrop effect rendering for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{FlatNode, NodeId, SceneNodeKind};

use crate::effects::{BoxShadow, ShadowParams};
use crate::lod::LodLevel;
use crate::rasterizer;

use super::{CachedShadow, SoftwareRenderer};

#[cfg(test)]
thread_local! {
    /// Number of times `render_box_shadows_node` actually REGENERATED a chrome
    /// box-shadow mask via `generate_shadow_mask` (full-frame path). A translate
    /// (t179) or an exact-key hit does NOT increment this — so a drag of N move
    /// frames over an identical shape should bump it exactly once. Tests reset it
    /// and assert it to prove reuse vs regeneration.
    pub(crate) static BOX_SHADOW_GENERATE_COUNT: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

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
                    self.render_backdrop_blur(node.id, bounds, lod_radius, node.corner_radius, fb);
                }
            }

            // Apply tint (confined to the active damage region, t76). Honour the
            // node's corner radius: glass surfaces (launcher, menus, dock band,
            // notification center) carry border-radius, and a hard rectangular
            // tint paints square corners over the rounded chrome — the hallmark
            // "cheap glass" look (t83-crisp #3). When any corner is rounded, fill
            // a rounded rect (per-corner SDF AA) instead of a sharp rectangle.
            let mut tint = params.tint_color;
            tint.a = (tint.a as f32 * opacity + 0.5) as u8;
            let (r_tl, r_tr, r_br, r_bl) = node.corner_radius;
            let rounded = r_tl > 0.0 || r_tr > 0.0 || r_br > 0.0 || r_bl > 0.0;
            if rounded {
                self.fill_rounded_rect_per_corner_clipped(
                    fb,
                    bounds,
                    tint,
                    r_tl,
                    r_tr,
                    r_br,
                    r_bl,
                    BlendMode::SrcOver,
                    self.raster_clip,
                );
            } else if let Some(tint_rect) = rasterizer::clip_rect(bounds, self.raster_clip) {
                rasterizer::fill_rect(fb, tint_rect, tint, BlendMode::SrcOver);
            }

            // Inner glow (skip for low LOD). Confine its WRITES to the active
            // damage clip + write-scissor (t119 #2) so a partial-damage on-glass
            // frame does not re-run the glow over the FULL glass bounds. The glow
            // is positional (each pixel computed independently from its position vs
            // `bounds`), so a clipped iteration window is byte-identical to the
            // full pass inside the damage rect — and untouched outside it.
            if params.inner_glow && lod_level != LodLevel::Low {
                crate::effects::InnerGlow::render_glow_clipped(
                    fb,
                    bounds,
                    8.0 * quality_factor,
                    3.0 * quality_factor,
                    Color::new(255, 255, 255, 30),
                    self.raster_clip,
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
            } else if let Some(mask) = BoxShadow::generate_shadow_mask(fb.width, fb.height, &params)
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

                    // On a partial-damage frame `generate_shadow_mask` confines the
                    // mask to (shadow rect ∩ scissor + blur margin) — byte-identical
                    // inside the damage rect but CLIP-SPECIFIC, so it must NOT be
                    // cached (a later frame with a different clip would wrongly reuse
                    // it). Compute fresh + composite, exactly the Shadow-node
                    // discipline. The FULL-frame path (no scissor) caches the mask by
                    // a signature of all its inputs so a steady chrome drop-shadow
                    // (statusbar/dock) reuses the once-computed SDF + blur instead of
                    // regenerating it every full frame (the t173 ~35-50 ms culprit).
                    if crate::rasterizer::write_scissor().is_some() {
                        if let Some(mask) =
                            BoxShadow::generate_shadow_mask(fb.width, fb.height, &params)
                        {
                            BoxShadow::composite_shadow_mask(fb, &mask);
                        }
                        continue;
                    }

                    let key = box_shadow_mask_key(fb.width, fb.height, &params);
                    if self.box_shadow_cache_get(key).is_some() {
                        // Exact-key HIT: same shape AND same position — reuse as-is
                        // (the steady-chrome / identical-frame path, unchanged). The
                        // re-get is needed because the `.is_some()` borrow above ends
                        // before the mutable `fb` borrow in `composite`.
                        let mask = self.box_shadow_cache_get(key).expect("just checked");
                        BoxShadow::composite_shadow_mask(fb, mask);
                    } else {
                        // Exact-key MISS. A MOVING window keeps the same shadow
                        // SHAPE but a new position, so the position-bearing `key`
                        // changes every drag frame while the SHAPE is identical
                        // (t175 culprit). Compute the shape signature + the mask's
                        // would-be origin/unclamped state, and try to TRANSLATE a
                        // cached same-shape mask to the new spot instead of
                        // regenerating the SDF + blur (t179 fast path).
                        let shape_key = box_shadow_shape_key(fb.width, fb.height, &params);
                        let (dst_x0, dst_y0, dst_unclamped) =
                            box_shadow_mask_origin(fb.width, fb.height, &params);
                        if dst_unclamped
                            && self.box_shadow_cache_translate(
                                key,
                                shape_key,
                                dst_unclamped,
                                dst_x0,
                                dst_y0,
                            )
                        {
                            // Translated into the cache under `key`; composite it.
                            let mask = self.box_shadow_cache_get(key).unwrap();
                            BoxShadow::composite_shadow_mask(fb, mask);
                        } else if let Some(mask) =
                            BoxShadow::generate_shadow_mask(fb.width, fb.height, &params)
                        {
                            // No same-shape entry to translate (or this position
                            // clamps at an fb edge): regenerate fresh, exactly as
                            // t175 did, and cache it under both the exact `key` and
                            // its shape signature for future translates.
                            #[cfg(test)]
                            BOX_SHADOW_GENERATE_COUNT.with(|c| c.set(c.get() + 1));
                            BoxShadow::composite_shadow_mask(fb, &mask);
                            self.box_shadow_cache_insert(key, shape_key, dst_unclamped, mask);
                        }
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
                                self.render_backdrop_blur(
                                    node.id,
                                    bounds,
                                    lod_r,
                                    node.corner_radius,
                                    fb,
                                );
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
        corner_radius: (f32, f32, f32, f32),
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
        // Per-corner radius: when the glass surface is rounded, the blurred
        // backdrop must NOT be written as a sharp rectangle over the rounded
        // chrome (t83-crisp #3). Mask the corner pixels by the rounded-rect SDF
        // coverage — fully inside the radius keeps the bulk fast-path copy, the
        // anti-aliased corner band lerps each blurred pixel against the existing
        // backdrop by coverage, and pixels outside the radius keep the background.
        let (r_tl, r_tr, r_br, r_bl) = corner_radius;
        let rounded = r_tl > 0.0 || r_tr > 0.0 || r_br > 0.0 || r_bl > 0.0;

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
                if src_off + bytes > cached.pixels.len()
                    || dst_off + bytes > fb.pixels_mut().expect("CPU framebuffer required").len()
                {
                    continue;
                }
                if !rounded {
                    fb.pixels_mut().expect("CPU framebuffer required")[dst_off..dst_off + bytes]
                        .copy_from_slice(&cached.pixels[src_off..src_off + bytes]);
                    continue;
                }
                // Rounded: blend per-pixel by SDF corner coverage. Interior
                // pixels (coverage == 1) overwrite; corner-band pixels lerp; pixels
                // outside the radius (coverage == 0) are untouched (background
                // shows through). Coverage is sampled against the full `bounds` so
                // the rounded geometry matches the tint fill above exactly.
                let fy = dy as f32 + 0.5;
                for i in 0..span {
                    let px = cx0 + i as u32;
                    let fx = px as f32 + 0.5;
                    let d = rasterizer::sdf_rounded_rect_per_corner(
                        fx, fy, &bounds, r_tl, r_tr, r_br, r_bl,
                    );
                    let coverage = (-d + 0.5).clamp(0.0, 1.0);
                    if coverage <= 0.0 {
                        continue;
                    }
                    let s = src_off + i * 4;
                    let blur_px = Color::new(
                        cached.pixels[s],
                        cached.pixels[s + 1],
                        cached.pixels[s + 2],
                        cached.pixels[s + 3],
                    );
                    let out = if coverage >= 1.0 {
                        blur_px
                    } else {
                        let bg = fb.get_pixel(px, dy);
                        let lerp = |a: u8, b: u8| -> u8 {
                            (a as f32 + (b as f32 - a as f32) * coverage + 0.5) as u8
                        };
                        Color::new(
                            lerp(bg.r, blur_px.r),
                            lerp(bg.g, blur_px.g),
                            lerp(bg.b, blur_px.b),
                            lerp(bg.a, blur_px.a),
                        )
                    };
                    fb.set_pixel(px, dy, out);
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

/// Derive a stable signature for a chrome box-shadow mask from EVERY input that
/// affects the generated mask: the framebuffer dimensions used to clamp it, the
/// surface geometry, corner radius, spread, blur radius, offset, and colour.
///
/// Two frames whose chrome shadow has identical inputs produce the same key and
/// reuse the cached mask; any change to ANY field (size, radius, blur, spread,
/// colour, offset, fb dims) changes the key and forces a fresh full compute — so
/// a stale mask can never paint. `f32` fields are hashed by their exact bit
/// pattern (`to_bits`): the cache must distinguish values that produce a
/// different mask, which is exactly bit-distinctness of the geometry inputs.
fn box_shadow_mask_key(fb_width: u32, fb_height: u32, params: &ShadowParams) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    fb_width.hash(&mut h);
    fb_height.hash(&mut h);
    params.surface_rect.x.to_bits().hash(&mut h);
    params.surface_rect.y.to_bits().hash(&mut h);
    params.surface_rect.width.to_bits().hash(&mut h);
    params.surface_rect.height.to_bits().hash(&mut h);
    params.corner_radius.to_bits().hash(&mut h);
    params.spread.to_bits().hash(&mut h);
    params.blur_radius.hash(&mut h);
    params.offset_x.to_bits().hash(&mut h);
    params.offset_y.to_bits().hash(&mut h);
    params.shadow_color.r.hash(&mut h);
    params.shadow_color.g.hash(&mut h);
    params.shadow_color.b.hash(&mut h);
    params.shadow_color.a.hash(&mut h);
    h.finish()
}

/// Derive a POSITION-INDEPENDENT signature of a chrome box-shadow mask's SHAPE:
/// every input [`box_shadow_mask_key`] hashes EXCEPT `surface_rect.x`/`.y`. Two
/// shadows of the same width/height/corner-radius/spread/blur/colour/offset (at
/// the same fb dims) share this key regardless of where the surface sits.
///
/// A moving window keeps an identical shadow shape while its position — and hence
/// the position-bearing exact `box_shadow_mask_key` — changes every drag frame.
/// Matching on this shape key lets the renderer REUSE the cached mask by an
/// integer translate (t179) rather than regenerating the SDF + blur. Because the
/// translate is only ever applied between two UNCLAMPED positions (mask rect
/// fully inside the fb at both), and the SDF coverage depends only on
/// `(sample − surface_centre)` — both of which shift by the same integer on a
/// move — the translated mask is BYTE-IDENTICAL to a fresh compute. Any
/// shape-affecting change yields a different shape key → no translate → a fresh
/// compute, so a stale shape can never paint. `f32` fields hash by exact bits,
/// matching `box_shadow_mask_key`.
fn box_shadow_shape_key(fb_width: u32, fb_height: u32, params: &ShadowParams) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    fb_width.hash(&mut h);
    fb_height.hash(&mut h);
    // NOTE: surface_rect.x / .y are DELIBERATELY excluded — they are the only
    // position-bearing inputs, and the whole point of this key is to be invariant
    // to them so a move reuses the shape.
    params.surface_rect.width.to_bits().hash(&mut h);
    params.surface_rect.height.to_bits().hash(&mut h);
    params.corner_radius.to_bits().hash(&mut h);
    params.spread.to_bits().hash(&mut h);
    params.blur_radius.hash(&mut h);
    params.offset_x.to_bits().hash(&mut h);
    params.offset_y.to_bits().hash(&mut h);
    params.shadow_color.r.hash(&mut h);
    params.shadow_color.g.hash(&mut h);
    params.shadow_color.b.hash(&mut h);
    params.shadow_color.a.hash(&mut h);
    h.finish()
}

/// Compute the framebuffer origin `(x0, y0)` a freshly-generated mask WOULD have,
/// and whether that mask rect is fully inside `[0,fb)` (UNCLAMPED).
///
/// This mirrors EXACTLY the geometry `BoxShadow::generate_shadow_mask` derives on
/// the FULL-frame (no write-scissor) path: `shadow_rect` = surface expanded by
/// `spread + blur_radius` and shifted by the offset, then clamped to the
/// framebuffer. The mask is "unclamped" when every edge of `shadow_rect` lies
/// within `[0,fb)` so none of the `.max(0)`/`.min(fb)` clamps truncated it — the
/// precondition for a byte-identical integer translate (t178/t179). `x0`/`y0`
/// match `generate_shadow_mask`'s `(shadow_rect.x.max(0) as u32).min(fb)` exactly
/// (an unclamped value rounds toward zero identically to the floor of a
/// non-negative coordinate).
fn box_shadow_mask_origin(fb_width: u32, fb_height: u32, params: &ShadowParams) -> (u32, u32, bool) {
    let expand = params.spread + params.blur_radius as f32;
    let sr = params.surface_rect;
    let rx = sr.x - expand + params.offset_x;
    let ry = sr.y - expand + params.offset_y;
    let rw = sr.width + expand * 2.0;
    let rh = sr.height + expand * 2.0;
    let right = rx + rw;
    let bottom = ry + rh;

    let x0 = (rx.max(0.0) as u32).min(fb_width);
    let y0 = (ry.max(0.0) as u32).min(fb_height);
    let x1 = (right.ceil() as u32).min(fb_width);
    let y1 = (bottom.ceil() as u32).min(fb_height);

    // Unclamped: no edge was truncated by a clamp. The low edges must be >= 0 (so
    // `.max(0)` was a no-op) AND <= fb (so `.min(fb)` was a no-op); the high edges'
    // ceil must be <= fb (so `.min(fb)` was a no-op). Equivalent to "the computed
    // [x0,x1)×[y0,y1) rect equals the unclamped rect".
    let unclamped = rx >= 0.0
        && ry >= 0.0
        && (rx as u32) == x0
        && (ry as u32) == y0
        && right.ceil() <= fb_width as f32
        && bottom.ceil() <= fb_height as f32
        && right.ceil() as u32 == x1
        && bottom.ceil() as u32 == y1
        && x1 > x0
        && y1 > y0;

    (x0, y0, unclamped)
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
        renderer.render_glass_node(
            &glass_node(99999, bounds, 12),
            &mut fb2,
            LodLevel::High,
            1.0,
        );

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
        assert!(
            differs,
            "cached blur should have been composited into the glass region"
        );
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
    fn run_blur(
        bounds: Rect,
        radius: u32,
        clip: Option<Rect>,
        fb_w: u32,
        fb_h: u32,
    ) -> FrameBuffer {
        let mut renderer = SoftwareRenderer::new();
        renderer.deterministic_blur = true; // synchronous, byte-stable
        renderer.raster_clip = clip;

        // Paint the backdrop input BEFORE installing the write-scissor. In
        // production the backdrop outside the current damage rect is the
        // PRESERVED content from prior frames (the framebuffer is not cleared on
        // a partial frame), so the blur can still sample it; the scissor only
        // confines the CURRENT pass's writes. Painting the backdrop under the
        // scissor would (correctly, per t84) confine it to the damage rect and
        // starve the blur's sample margin — an artifact of an empty test fb, not
        // production. So we seed the full backdrop first, then scissor the blur.
        let mut fb = FrameBuffer::new(fb_w, fb_h, PixelFormat::Bgra8);
        paint_backdrop(&mut fb);

        let prev = crate::rasterizer::set_write_scissor(clip);
        renderer.render_backdrop_blur(1, bounds, radius, (0.0, 0.0, 0.0, 0.0), &mut fb);
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
            (
                Rect::new(8.0, 8.0, 200.0, 160.0),
                4,
                Rect::new(40.0, 40.0, 24.0, 24.0),
            ),
            (
                Rect::new(8.0, 8.0, 200.0, 160.0),
                12,
                Rect::new(90.0, 70.0, 32.0, 32.0),
            ),
            (
                Rect::new(8.0, 8.0, 200.0, 160.0),
                20,
                Rect::new(120.0, 100.0, 16.0, 16.0),
            ),
            // Damage ON the top-left glass edge (margin must clamp to the edge).
            (
                Rect::new(8.0, 8.0, 200.0, 160.0),
                12,
                Rect::new(8.0, 8.0, 20.0, 20.0),
            ),
            // Damage ON the bottom-right glass edge.
            (
                Rect::new(8.0, 8.0, 200.0, 160.0),
                16,
                Rect::new(180.0, 140.0, 28.0, 28.0),
            ),
            // Odd-dim glass + large radius → safe fallback to full bounds; must
            // still be byte-identical inside the damage rect.
            (
                Rect::new(8.0, 8.0, 201.0, 161.0),
                12,
                Rect::new(90.0, 70.0, 30.0, 30.0),
            ),
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
            for &(dx, dy, dw, dh) in &[
                (4.0, 4.0, 18.0, 18.0),     // top-left corner
                (226.0, 4.0, 18.0, 18.0),   // top-right corner
                (4.0, 186.0, 18.0, 18.0),   // bottom-left corner
                (226.0, 186.0, 18.0, 18.0), // bottom-right corner
                (60.0, 50.0, 13.0, 27.0),   // odd-sized interior
                (121.0, 99.0, 7.0, 7.0),    // odd-offset tiny interior
                (10.0, 90.0, 200.0, 11.0),  // wide thin strip
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
        full_r.render_backdrop_blur(1, bounds, radius, (0.0, 0.0, 0.0, 0.0), &mut fb_full);
        let full_px = full_r.last_blur_source_px.get();
        assert_eq!(
            full_px,
            400 * 400,
            "full path should snapshot the whole glass"
        );

        // Clipped: the blur source must be a small window around the damage rect.
        let mut clip_r = SoftwareRenderer::new();
        clip_r.deterministic_blur = true;
        clip_r.raster_clip = Some(dmg);
        let prev = crate::rasterizer::set_write_scissor(Some(dmg));
        let mut fb_clip = FrameBuffer::new(400, 400, PixelFormat::Bgra8);
        paint_backdrop(&mut fb_clip);
        clip_r.render_backdrop_blur(1, bounds, radius, (0.0, 0.0, 0.0, 0.0), &mut fb_clip);
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

    /// Render JUST the glass inner-glow (blur radius 0 → no blur; tint alpha 0 →
    /// no tint) over a non-flat backdrop, optionally clipped to `clip`, returning
    /// the framebuffer. Mirrors the production wiring (raster_clip + write-scissor).
    fn run_inner_glow(clip: Option<Rect>, fb_w: u32, fb_h: u32, bounds: Rect) -> FrameBuffer {
        let mut renderer = SoftwareRenderer::new();
        renderer.raster_clip = clip;
        let node = FlatNode {
            id: 1,
            kind: SceneNodeKind::Glass(GlassParams {
                blur_radius: 0,                            // isolate: no blur
                tint_color: Color::new(0, 0, 0, 0),        // isolate: no tint
                inner_glow: true,
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
        };
        let mut fb = FrameBuffer::new(fb_w, fb_h, PixelFormat::Bgra8);
        paint_backdrop(&mut fb);
        let prev = crate::rasterizer::set_write_scissor(clip);
        renderer.render_glass_node(&node, &mut fb, LodLevel::High, 1.0);
        crate::rasterizer::set_write_scissor(prev);
        fb
    }

    /// t119 #2 — the glass INNER GLOW must respect the damage clip: a partial
    /// frame must NOT re-run the glow over the FULL glass bounds. The clipped run
    /// (a) writes NOTHING outside the damage rect, and (b) is BYTE-IDENTICAL to the
    /// full (unclipped) glow INSIDE the damage rect (the glow is positional, each
    /// pixel independent of the iteration window). Before t119 the glow ignored
    /// `raster_clip` and ran the full bounds every frame.
    #[test]
    fn inner_glow_respects_damage_clip_and_is_byte_identical_inside() {
        let fb_w = 220u32;
        let fb_h = 200u32;
        // Big glass surface; the glow rides its edges (the inset border band).
        let bounds = Rect::new(10.0, 10.0, 200.0, 180.0);

        let full = run_inner_glow(None, fb_w, fb_h, bounds);
        let full_iter = crate::effects::LAST_GLOW_ITER_PX.with(std::cell::Cell::get);

        // A damage rect over the glass's TOP-LEFT corner band (where the glow is
        // strongest) — small relative to the full glass.
        let dmg = Rect::new(10.0, 10.0, 40.0, 40.0);
        let clipped = run_inner_glow(Some(dmg), fb_w, fb_h, bounds);
        let clipped_iter = crate::effects::LAST_GLOW_ITER_PX.with(std::cell::Cell::get);

        // COST (the t119 #2 point): the clipped run must ITERATE only ~O(damage)
        // pixels, not the full glass bounds. Catches a regression that reverts the
        // iteration-window confinement (the compositor scissor would keep the
        // OUTPUT correct but pay full per-pixel SDF cost). 40×40 damage = 1600 px;
        // full glass = 200×180 = 36 000 px.
        assert!(
            clipped_iter <= (dmg.width * dmg.height) as usize + 16,
            "clipped inner-glow iterated {clipped_iter} px, expected ~O(damage) (<= {} px)",
            (dmg.width * dmg.height) as usize + 16
        );
        assert!(
            clipped_iter * 4 < full_iter,
            "clipped inner-glow iteration {clipped_iter} px must be a small fraction \
             of the full {full_iter} px — the iteration-window confine regressed"
        );

        let dx0 = dmg.x as u32;
        let dy0 = dmg.y as u32;
        let dx1 = dmg.right().ceil() as u32;
        let dy1 = dmg.bottom().ceil() as u32;

        // Baseline: the sharp (glow-free) backdrop, to detect writes.
        let mut sharp = FrameBuffer::new(fb_w, fb_h, PixelFormat::Bgra8);
        paint_backdrop(&mut sharp);

        // (a) clipped glow wrote NOTHING outside the damage rect, AND
        // (b) byte-identical to the full glow INSIDE the damage rect.
        let mut glow_pixels_inside = 0u32;
        let mut full_glowed_outside = 0u32;
        for y in 0..fb_h {
            for x in 0..fb_w {
                let inside = x >= dx0 && x < dx1 && y >= dy0 && y < dy1;
                if inside {
                    assert_eq!(
                        clipped.get_pixel(x, y),
                        full.get_pixel(x, y),
                        "clipped inner-glow differs from full glow INSIDE damage at ({x},{y})"
                    );
                    if clipped.get_pixel(x, y) != sharp.get_pixel(x, y) {
                        glow_pixels_inside += 1;
                    }
                } else {
                    assert_eq!(
                        clipped.get_pixel(x, y),
                        sharp.get_pixel(x, y),
                        "clipped inner-glow wrote OUTSIDE the damage rect at ({x},{y})"
                    );
                    if full.get_pixel(x, y) != sharp.get_pixel(x, y) {
                        full_glowed_outside += 1;
                    }
                }
            }
        }
        // The glow must actually paint inside the damage rect (not an all-no-op),
        // and the FULL glow must paint OUTSIDE it (proving the clip is what
        // suppressed those writes — the confinement is real, not vacuous).
        assert!(
            glow_pixels_inside > 0,
            "the inner glow must paint pixels inside the damage rect (got 0)"
        );
        assert!(
            full_glowed_outside > 50,
            "the full inner glow must paint many pixels outside the damage rect \
             (only {full_glowed_outside}); otherwise the clip test is vacuous"
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

    // ── Chrome BoxShadows mask cache correctness ──────────────────────────────

    use liquide_compositor::scene::BoxShadowSpec;

    fn box_shadows_node(id: NodeId, bounds: Rect, shadow: BoxShadowSpec) -> FlatNode {
        FlatNode {
            id,
            kind: SceneNodeKind::BoxShadows {
                shadows: vec![shadow],
            }
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

    fn default_shadow() -> BoxShadowSpec {
        BoxShadowSpec {
            offset_x: 0.0,
            offset_y: 4.0,
            blur_radius: 12.0,
            spread_radius: 2.0,
            color: Color::new(0, 0, 0, 160),
            inset: false,
        }
    }

    /// Render a chrome BoxShadows node on a FRESH renderer (cold cache) over a
    /// painted backdrop. This is the ground-truth "fresh full compute" the cached
    /// path must match.
    fn render_box_shadows_fresh(bounds: Rect, shadow: BoxShadowSpec, w: u32, h: u32) -> FrameBuffer {
        let mut renderer = SoftwareRenderer::new();
        let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        paint_backdrop(&mut fb);
        renderer.render_box_shadows_node(
            &box_shadows_node(1, bounds, shadow),
            &mut fb,
            LodLevel::High,
            1.0,
        );
        fb
    }

    /// The cache key must change iff a mask-affecting input changes. Distinct keys
    /// for distinct geometry/radius/blur/spread/colour/offset/fb-size; equal key
    /// for identical inputs. This is the field-coverage tooth at the unit level:
    /// dropping a field from `box_shadow_mask_key` collapses two distinct cases to
    /// the same key → RED here AND a stale mask downstream.
    #[test]
    fn box_shadow_mask_key_covers_every_input() {
        let base = || ShadowParams {
            surface_rect: Rect::new(10.0, 20.0, 200.0, 30.0),
            corner_radius: 0.0,
            spread: 2.0,
            blur_radius: 12,
            offset_x: 0.0,
            offset_y: 4.0,
            shadow_color: Color::new(0, 0, 0, 160),
        };
        let k0 = box_shadow_mask_key(1920, 1080, &base());
        assert_eq!(k0, box_shadow_mask_key(1920, 1080, &base()), "stable");

        // Each mutation must change the key.
        let mut p = base();
        p.surface_rect.x += 1.0;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "x must be keyed");
        let mut p = base();
        p.surface_rect.y += 1.0;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "y must be keyed");
        let mut p = base();
        p.surface_rect.width += 1.0;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "width must be keyed");
        let mut p = base();
        p.surface_rect.height += 1.0;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "height must be keyed");
        let mut p = base();
        p.corner_radius += 1.0;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "corner must be keyed");
        let mut p = base();
        p.spread += 1.0;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "spread must be keyed");
        let mut p = base();
        p.blur_radius += 1;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "blur must be keyed");
        let mut p = base();
        p.offset_x += 1.0;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "offset_x must be keyed");
        let mut p = base();
        p.offset_y += 1.0;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "offset_y must be keyed");
        let mut p = base();
        p.shadow_color.r ^= 0xFF;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "color.r must be keyed");
        let mut p = base();
        p.shadow_color.g ^= 0xFF;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "color.g must be keyed");
        let mut p = base();
        p.shadow_color.b ^= 0xFF;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "color.b must be keyed");
        let mut p = base();
        p.shadow_color.a ^= 0xFF;
        assert_ne!(k0, box_shadow_mask_key(1920, 1080, &p), "color.a must be keyed");
        assert_ne!(k0, box_shadow_mask_key(1921, 1080, &base()), "fb_w must be keyed");
        assert_ne!(k0, box_shadow_mask_key(1920, 1081, &base()), "fb_h must be keyed");
    }

    /// A steady chrome shadow rendered TWICE on the same renderer (2nd frame hits
    /// the cache) must paint BYTE-IDENTICALLY to a fresh full compute. Proves the
    /// cache HIT path reuses the correct mask (not a stale/empty one).
    #[test]
    fn cached_box_shadow_equals_fresh_when_inputs_unchanged() {
        let bounds = Rect::new(6.0, 40.0, 200.0, 36.0);
        let shadow = default_shadow();
        let w = 240u32;
        let h = 120u32;

        let mut renderer = SoftwareRenderer::new();

        // Frame 1: cold → computes + caches.
        let mut fb1 = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        paint_backdrop(&mut fb1);
        renderer.render_box_shadows_node(
            &box_shadows_node(1, bounds, shadow.clone()),
            &mut fb1,
            LodLevel::High,
            1.0,
        );

        // Frame 2: DIFFERENT node id (id churns in the shell), identical inputs →
        // must HIT the cache and paint the same as a cold compute.
        let mut fb2 = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        paint_backdrop(&mut fb2);
        renderer.render_box_shadows_node(
            &box_shadows_node(99999, bounds, shadow.clone()),
            &mut fb2,
            LodLevel::High,
            1.0,
        );

        let fresh = render_box_shadows_fresh(bounds, shadow.clone(), w, h);
        assert_eq!(
            fb2.pixels().to_vec(),
            fresh.pixels().to_vec(),
            "cache-hit frame must equal a fresh full compute"
        );
        // Sanity: the shadow actually painted something (not a vacuous all-equal).
        let mut sharp = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        paint_backdrop(&mut sharp);
        assert_ne!(
            fb2.pixels().to_vec(),
            sharp.pixels().to_vec(),
            "the cached shadow must actually paint (test would be vacuous otherwise)"
        );
        // And frame 1 == frame 2 (cache hit identical to cold).
        assert_eq!(fb1.pixels().to_vec(), fb2.pixels().to_vec());
    }

    /// THE TEETH: after caching a shadow, mutating EACH mask-affecting input must
    /// force a recompute that matches a FRESH full compute of the NEW input — i.e.
    /// the cache never serves the old mask for new inputs. A cache key missing any
    /// field would serve the stale mask → mismatch vs fresh → RED.
    #[test]
    fn mutating_any_input_recomputes_and_matches_fresh_never_stale() {
        let bounds = Rect::new(6.0, 40.0, 200.0, 36.0);
        let base = default_shadow();
        let w = 260u32;
        let h = 130u32;

        // Mutators: one per mask-affecting field (geometry mutated via bounds,
        // the rest via the shadow spec). Each returns (mutated_bounds, mutated_spec).
        type Mut = (&'static str, fn(Rect, BoxShadowSpec) -> (Rect, BoxShadowSpec));
        let mutators: &[Mut] = &[
            ("bounds.x", |b, s| (Rect::new(b.x + 5.0, b.y, b.width, b.height), s)),
            ("bounds.y", |b, s| (Rect::new(b.x, b.y + 5.0, b.width, b.height), s)),
            ("bounds.width", |b, s| (Rect::new(b.x, b.y, b.width + 7.0, b.height), s)),
            ("bounds.height", |b, s| (Rect::new(b.x, b.y, b.width, b.height + 7.0), s)),
            ("blur", |b, mut s| { s.blur_radius += 4.0; (b, s) }),
            ("spread", |b, mut s| { s.spread_radius += 3.0; (b, s) }),
            ("offset_x", |b, mut s| { s.offset_x += 6.0; (b, s) }),
            ("offset_y", |b, mut s| { s.offset_y += 6.0; (b, s) }),
            ("color", |b, mut s| { s.color = Color::new(20, 90, 200, 200); (b, s) }),
        ];

        for (name, mutate) in mutators {
            // Fresh renderer; render the BASE shadow first so it is cached.
            let mut renderer = SoftwareRenderer::new();
            let mut warm = FrameBuffer::new(w, h, PixelFormat::Bgra8);
            paint_backdrop(&mut warm);
            renderer.render_box_shadows_node(
                &box_shadows_node(1, bounds, base.clone()),
                &mut warm,
                LodLevel::High,
                1.0,
            );

            // Now render the MUTATED shadow on the SAME (warm) renderer.
            let (mb, ms) = mutate(bounds, base.clone());
            let mut got = FrameBuffer::new(w, h, PixelFormat::Bgra8);
            paint_backdrop(&mut got);
            renderer.render_box_shadows_node(
                &box_shadows_node(2, mb, ms.clone()),
                &mut got,
                LodLevel::High,
                1.0,
            );

            // Ground truth: a cold renderer computing the mutated shadow fresh.
            let fresh = render_box_shadows_fresh(mb, ms, w, h);

            assert_eq!(
                got.pixels().to_vec(),
                fresh.pixels().to_vec(),
                "mutating `{name}` served a STALE cached mask (got != fresh full compute)"
            );

            // And the mutation actually changed the output vs the base shadow —
            // otherwise the test couldn't distinguish stale from fresh.
            let base_fresh = render_box_shadows_fresh(bounds, base.clone(), w, h);
            assert_ne!(
                fresh.pixels().to_vec(),
                base_fresh.pixels().to_vec(),
                "mutator `{name}` did not change the mask — test is vacuous for it"
            );
        }
    }

    /// A scissored (partial-damage) frame must NOT poison the cache: the
    /// clip-confined mask is never stored, so a later FULL frame still computes +
    /// caches the true full mask. Guards against caching a clip-specific mask.
    #[test]
    fn scissored_frame_does_not_cache_a_clip_specific_mask() {
        let bounds = Rect::new(6.0, 40.0, 200.0, 36.0);
        let shadow = default_shadow();
        let w = 240u32;
        let h = 120u32;

        let mut renderer = SoftwareRenderer::new();

        // Scissored frame first (confined mask — must not be cached).
        let mut fb_clip = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        paint_backdrop(&mut fb_clip);
        let dmg = Rect::new(20.0, 30.0, 30.0, 30.0);
        let prev = crate::rasterizer::set_write_scissor(Some(dmg));
        renderer.render_box_shadows_node(
            &box_shadows_node(1, bounds, shadow.clone()),
            &mut fb_clip,
            LodLevel::High,
            1.0,
        );
        crate::rasterizer::set_write_scissor(prev);

        // Now a FULL frame: must equal a cold fresh full compute (the scissored
        // frame must not have cached its confined mask under the same key).
        let mut fb_full = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        paint_backdrop(&mut fb_full);
        renderer.render_box_shadows_node(
            &box_shadows_node(2, bounds, shadow.clone()),
            &mut fb_full,
            LodLevel::High,
            1.0,
        );

        let fresh = render_box_shadows_fresh(bounds, shadow.clone(), w, h);
        assert_eq!(
            fb_full.pixels().to_vec(),
            fresh.pixels().to_vec(),
            "full frame after a scissored frame must equal a fresh full compute"
        );
    }

    // ── t179: position-translate fast path for a MOVING window's drop-shadow ───

    /// Build the `ShadowParams` EXACTLY as `render_box_shadows_node` does for a
    /// `(bounds, spec)` pair on the full-frame path (surface_rect folds in
    /// offset/spread; offset/spread carried separately). Tests that probe the
    /// cache helpers (`box_shadow_mask_origin`, the two keys, `cache_translate`)
    /// MUST use this so they match the real path's params byte-for-byte.
    fn params_for(bounds: Rect, s: &BoxShadowSpec) -> ShadowParams {
        let shadow_bounds = Rect::new(
            bounds.x + s.offset_x - s.spread_radius,
            bounds.y + s.offset_y - s.spread_radius,
            bounds.width + s.spread_radius * 2.0,
            bounds.height + s.spread_radius * 2.0,
        );
        ShadowParams {
            surface_rect: shadow_bounds,
            corner_radius: 0.0,
            spread: s.spread_radius,
            blur_radius: s.blur_radius as u32,
            offset_x: s.offset_x,
            offset_y: s.offset_y,
            shadow_color: s.color,
        }
    }

    /// Render a chrome BoxShadows node on the given (possibly warm) renderer and
    /// return the framebuffer; the generate-counter reflects whether a fresh SDF +
    /// blur compute happened on this call (callers reset it as needed).
    fn render_box_shadows_on(
        renderer: &mut SoftwareRenderer,
        id: NodeId,
        bounds: Rect,
        shadow: BoxShadowSpec,
        w: u32,
        h: u32,
    ) -> FrameBuffer {
        let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        paint_backdrop(&mut fb);
        renderer.render_box_shadows_node(
            &box_shadows_node(id, bounds, shadow),
            &mut fb,
            LodLevel::High,
            1.0,
        );
        fb
    }

    /// (a) The TRANSLATE path must produce output BYTE-IDENTICAL to a fresh full
    /// compute at the new position, across several integer deltas AND shapes, for
    /// the unclamped interior case. This is the whole correctness basis (t178): an
    /// integer move of an unclamped mask reuses the same pixels at a new origin.
    #[test]
    fn translate_path_is_byte_identical_to_fresh_at_new_position() {
        let w = 320u32;
        let h = 200u32;
        // A few shapes (vary size / blur / spread / radius / colour / offset).
        let shapes: &[(Rect, BoxShadowSpec)] = &[
            (Rect::new(60.0, 60.0, 120.0, 40.0), default_shadow()),
            (
                Rect::new(80.0, 50.0, 90.0, 60.0),
                BoxShadowSpec {
                    offset_x: 3.0,
                    offset_y: -2.0,
                    blur_radius: 6.0,
                    spread_radius: 0.0,
                    color: Color::new(10, 40, 90, 200),
                    inset: false,
                },
            ),
            (
                Rect::new(70.0, 70.0, 100.0, 50.0),
                BoxShadowSpec {
                    offset_x: 0.0,
                    offset_y: 5.0,
                    blur_radius: 16.0,
                    spread_radius: 4.0,
                    color: Color::new(0, 0, 0, 120),
                    inset: false,
                },
            ),
        ];
        // Integer deltas (incl. negative + diagonal); all stay interior/unclamped.
        let deltas: &[(f32, f32)] = &[(1.0, 0.0), (0.0, 1.0), (7.0, -3.0), (-5.0, 9.0), (12.0, 12.0)];

        for (base_bounds, shadow) in shapes {
            for &(dx, dy) in deltas {
                // Warm a renderer at the BASE position (caches the shape).
                let mut renderer = SoftwareRenderer::new();
                let _ = render_box_shadows_on(&mut renderer, 1, *base_bounds, shadow.clone(), w, h);

                // Move by an integer delta (different node id, as the shell churns ids).
                let moved = Rect::new(
                    base_bounds.x + dx,
                    base_bounds.y + dy,
                    base_bounds.width,
                    base_bounds.height,
                );
                // Both positions must be unclamped for this case to apply.
                let mp = params_for(moved, shadow);
                let (_, _, moved_unclamped) = box_shadow_mask_origin(w, h, &mp);
                assert!(moved_unclamped, "test setup: moved position must be unclamped");

                BOX_SHADOW_GENERATE_COUNT.with(|c| c.set(0));
                let got = render_box_shadows_on(&mut renderer, 2, moved, shadow.clone(), w, h);
                assert_eq!(
                    BOX_SHADOW_GENERATE_COUNT.with(|c| c.get()),
                    0,
                    "an unclamped integer move must TRANSLATE (no fresh generate), \
                     delta=({dx},{dy})"
                );

                let fresh = render_box_shadows_fresh(moved, shadow.clone(), w, h);
                assert_eq!(
                    got.pixels().to_vec(),
                    fresh.pixels().to_vec(),
                    "translated mask must be BYTE-IDENTICAL to a fresh compute at the \
                     new position (delta=({dx},{dy}))"
                );
            }
        }
    }

    /// (b) A SEQUENCE of move frames must reuse the cached shape via translate: the
    /// fresh-generate counter increments ONCE (the first frame) then stays put as
    /// the window slides — not once per frame (the t175 regression this fixes).
    #[test]
    fn move_sequence_reuses_via_translate_generates_once() {
        let w = 400u32;
        let h = 240u32;
        let shadow = default_shadow();
        let start = Rect::new(40.0, 50.0, 140.0, 44.0);

        let mut renderer = SoftwareRenderer::new();
        BOX_SHADOW_GENERATE_COUNT.with(|c| c.set(0));

        // Frame 0: cold → one fresh generate.
        let _ = render_box_shadows_on(&mut renderer, 1, start, shadow.clone(), w, h);
        assert_eq!(
            BOX_SHADOW_GENERATE_COUNT.with(|c| c.get()),
            1,
            "the first (cold) frame must generate exactly once"
        );

        // Slide the window one pixel/frame for many frames — each must translate.
        for i in 1..=30u32 {
            let b = Rect::new(start.x + i as f32, start.y + (i / 2) as f32, start.width, start.height);
            // Count ONLY the generates attributable to this move frame (a fresh
            // compute on the cold ground-truth renderer below also bumps the shared
            // counter, so measure the delta around the move render specifically).
            let before = BOX_SHADOW_GENERATE_COUNT.with(|c| c.get());
            let got = render_box_shadows_on(&mut renderer, 100 + i as u64, b, shadow.clone(), w, h);
            assert_eq!(
                BOX_SHADOW_GENERATE_COUNT.with(|c| c.get()),
                before,
                "move frame {i} regenerated instead of translating"
            );
            // ...and still correct vs a fresh compute at that spot.
            let fresh = render_box_shadows_fresh(b, shadow.clone(), w, h);
            assert_eq!(
                got.pixels().to_vec(),
                fresh.pixels().to_vec(),
                "move frame {i} (translate) diverged from a fresh compute"
            );
        }
    }

    /// (c) A SHAPE change (resize / blur / colour) must NOT translate — it must
    /// regenerate and match a fresh compute (no stale shape). Each shape mutation
    /// yields a new shape key, so the translate lookup misses and a fresh generate
    /// runs.
    #[test]
    fn shape_change_regenerates_never_translates_a_stale_shape() {
        let w = 320u32;
        let h = 200u32;
        let base_bounds = Rect::new(60.0, 60.0, 120.0, 44.0);
        let base = default_shadow();

        type Mut = (&'static str, fn(Rect, BoxShadowSpec) -> (Rect, BoxShadowSpec));
        let mutators: &[Mut] = &[
            ("resize_w", |b, s| (Rect::new(b.x, b.y, b.width + 10.0, b.height), s)),
            ("resize_h", |b, s| (Rect::new(b.x, b.y, b.width, b.height + 10.0), s)),
            ("blur", |b, mut s| { s.blur_radius += 5.0; (b, s) }),
            ("spread", |b, mut s| { s.spread_radius += 3.0; (b, s) }),
            ("color", |b, mut s| { s.color = Color::new(30, 120, 60, 210); (b, s) }),
            ("offset_x", |b, mut s| { s.offset_x += 6.0; (b, s) }),
        ];

        for (name, mutate) in mutators {
            let mut renderer = SoftwareRenderer::new();
            // Warm with the base shape.
            let _ = render_box_shadows_on(&mut renderer, 1, base_bounds, base.clone(), w, h);

            // Mutate the SHAPE (also move it, so a buggy translate would have a
            // same-position fallback to fall into — it must NOT).
            let (mb0, ms) = mutate(base_bounds, base.clone());
            let mb = Rect::new(mb0.x + 8.0, mb0.y + 4.0, mb0.width, mb0.height);

            BOX_SHADOW_GENERATE_COUNT.with(|c| c.set(0));
            let got = render_box_shadows_on(&mut renderer, 2, mb, ms.clone(), w, h);
            assert_eq!(
                BOX_SHADOW_GENERATE_COUNT.with(|c| c.get()),
                1,
                "shape change `{name}` must REGENERATE (a translate would paint a stale shape)"
            );

            let fresh = render_box_shadows_fresh(mb, ms.clone(), w, h);
            assert_eq!(
                got.pixels().to_vec(),
                fresh.pixels().to_vec(),
                "shape change `{name}` did not match a fresh compute (stale shape served?)"
            );
        }
    }

    /// (d) A position whose mask rect CLAMPS at an fb edge must regenerate (never
    /// translate a cached interior shape onto a clamped spot — the dimensions
    /// differ, translation would be wrong).
    #[test]
    fn clamped_position_regenerates_does_not_translate() {
        let w = 200u32;
        let h = 140u32;
        let shadow = default_shadow();

        let mut renderer = SoftwareRenderer::new();
        // Warm with an interior (unclamped) shape.
        let interior = Rect::new(70.0, 60.0, 60.0, 30.0);
        let ip = params_for(interior, &shadow);
        assert!(box_shadow_mask_origin(w, h, &ip).2, "warm shape must be unclamped");
        let _ = render_box_shadows_on(&mut renderer, 1, interior, shadow.clone(), w, h);

        // Move SAME shape hard against the top-left so its expanded shadow rect
        // crosses x<0 / y<0 → clamped.
        let clamped = Rect::new(2.0, 2.0, 60.0, 30.0);
        let cp = params_for(clamped, &shadow);
        assert!(
            !box_shadow_mask_origin(w, h, &cp).2,
            "test setup: clamped position must report clamped"
        );

        BOX_SHADOW_GENERATE_COUNT.with(|c| c.set(0));
        let got = render_box_shadows_on(&mut renderer, 2, clamped, shadow.clone(), w, h);
        assert_eq!(
            BOX_SHADOW_GENERATE_COUNT.with(|c| c.get()),
            1,
            "a clamped position must REGENERATE (translation across a clamp is wrong)"
        );
        let fresh = render_box_shadows_fresh(clamped, shadow.clone(), w, h);
        assert_eq!(
            got.pixels().to_vec(),
            fresh.pixels().to_vec(),
            "clamped-position render must equal a fresh compute"
        );
    }

    /// TEETH 1: forcing a translate of a SHAPE-CHANGED entry paints the WRONG mask.
    /// We mimic the bug by translating the cached (base-shape) mask to a position
    /// computed for a DIFFERENT shape, and assert it DIVERGES from the correct
    /// fresh compute — proving the shape-key guard is load-bearing. (If translate
    /// ignored the shape, the real path would do exactly this and be RED.)
    #[test]
    fn teeth_translate_across_shape_change_is_wrong() {
        let w = 320u32;
        let h = 200u32;
        let base_bounds = Rect::new(60.0, 60.0, 120.0, 44.0);
        let base = default_shadow();

        // Cache the base shape.
        let mut renderer = SoftwareRenderer::new();
        let _ = render_box_shadows_on(&mut renderer, 1, base_bounds, base.clone(), w, h);

        // A bigger shape at a moved position. Compute ITS key/shape/origin.
        let big = BoxShadowSpec { blur_radius: base.blur_radius + 8.0, ..base.clone() };
        let moved = Rect::new(base_bounds.x + 6.0, base_bounds.y + 4.0, base_bounds.width, base_bounds.height);
        let mp = params_for(moved, &big);
        let key = box_shadow_mask_key(w, h, &mp);
        let (ox, oy, unclamped) = box_shadow_mask_origin(w, h, &mp);
        assert!(unclamped);

        // Force a translate keyed by the BASE shape (the bug): reuse the cached
        // base mask under the big-shape's exact key. The base shape key must use
        // the BASE shadow's inputs (the cached entry's shape), not the big one.
        let base_shape_key = box_shadow_shape_key(w, h, &params_for(base_bounds, &base));
        assert!(
            renderer.box_shadow_cache_translate(key, base_shape_key, unclamped, ox, oy),
            "forced translate of the base shape should succeed (bug simulation)"
        );
        let mut wrong = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        paint_backdrop(&mut wrong);
        BoxShadow::composite_shadow_mask(&mut wrong, renderer.box_shadow_cache_get(key).unwrap());

        // The correct render for the BIG shape at `moved`.
        let fresh = render_box_shadows_fresh(moved, big.clone(), w, h);
        assert_ne!(
            wrong.pixels().to_vec(),
            fresh.pixels().to_vec(),
            "translating a base-shape mask under a big-shape key must be WRONG — \
             this is why the shape-key guard exists"
        );
    }

    /// TEETH 2: forcing a translate ACROSS a clamp boundary diverges from a fresh
    /// compute — proving the unclamped guard is load-bearing. We translate an
    /// interior cached mask onto a clamped spot and show it differs from the (true)
    /// clamped fresh compute.
    #[test]
    fn teeth_translate_across_clamp_is_wrong() {
        let w = 200u32;
        let h = 140u32;
        let shadow = default_shadow();

        let mut renderer = SoftwareRenderer::new();
        let interior = Rect::new(70.0, 60.0, 60.0, 30.0);
        let _ = render_box_shadows_on(&mut renderer, 1, interior, shadow.clone(), w, h);

        // Clamped destination (same shape, hard against the corner).
        let clamped = Rect::new(2.0, 2.0, 60.0, 30.0);
        let cp = params_for(clamped, &shadow);
        let shape_key = box_shadow_shape_key(w, h, &cp);
        let key = box_shadow_mask_key(w, h, &cp);
        // The clamped origin (where a fresh clamped mask would START).
        let (ox, oy, dst_unclamped) = box_shadow_mask_origin(w, h, &cp);
        assert!(!dst_unclamped, "destination must be clamped");

        // Force the bug: translate the interior mask to the clamped origin under
        // the clamped key, BYPASSING the dst_unclamped guard (passing `true`).
        assert!(
            renderer.box_shadow_cache_translate(key, shape_key, true, ox, oy),
            "forced translate onto a clamped spot should succeed (bug simulation)"
        );
        let mut wrong = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        paint_backdrop(&mut wrong);
        BoxShadow::composite_shadow_mask(&mut wrong, renderer.box_shadow_cache_get(key).unwrap());

        let fresh = render_box_shadows_fresh(clamped, shadow.clone(), w, h);
        assert_ne!(
            wrong.pixels().to_vec(),
            fresh.pixels().to_vec(),
            "translating an interior mask onto a clamped spot must DIVERGE from a \
             fresh clamped compute — this is why the unclamped guard exists"
        );
    }

    /// Drag-frame box-shadow cost: regenerate (t175 path, position keyed) vs the
    /// t179 translate fast path, over a realistic large window shadow. Prints both
    /// per-frame times. Not an assertion of a hard threshold (CI timing is noisy),
    /// but the translate path is structurally O(copy) vs O(SDF + Gaussian blur) and
    /// is expected to save ~5-10 ms/drag frame (t173). `#[ignore]` so it never gates
    /// CI; run with `--ignored --nocapture` to see the numbers.
    #[test]
    #[ignore]
    fn bench_drag_frame_translate_vs_regenerate() {
        use std::time::Instant;
        let w = 1920u32;
        let h = 1080u32;
        // A large window-sized drop shadow (the t173 culprit shape).
        let shadow = BoxShadowSpec {
            offset_x: 0.0,
            offset_y: 8.0,
            blur_radius: 24.0,
            spread_radius: 2.0,
            color: Color::new(0, 0, 0, 140),
            inset: false,
        };
        let bounds = Rect::new(400.0, 300.0, 900.0, 600.0);
        let frames = 60u32;

        // BEFORE (t175): every drag frame is an exact-key MISS that REGENERATES,
        // because position is in the key. Emulate by clearing the cache each frame
        // (so the translate path can never find a same-shape entry).
        let mut renderer = SoftwareRenderer::new();
        let t0 = Instant::now();
        for i in 0..frames {
            renderer.clear_shadow_cache();
            let b = Rect::new(bounds.x + i as f32, bounds.y, bounds.width, bounds.height);
            let _ = render_box_shadows_on(&mut renderer, 1, b, shadow.clone(), w, h);
        }
        let regen = t0.elapsed().as_secs_f64() * 1000.0 / frames as f64;

        // AFTER (t179): the cache persists, so frame 0 generates and every
        // subsequent move frame TRANSLATES.
        let mut renderer = SoftwareRenderer::new();
        let _ = render_box_shadows_on(&mut renderer, 0, bounds, shadow.clone(), w, h); // warm
        let t1 = Instant::now();
        for i in 1..=frames {
            let b = Rect::new(bounds.x + i as f32, bounds.y, bounds.width, bounds.height);
            let _ = render_box_shadows_on(&mut renderer, 100 + i as u64, b, shadow.clone(), w, h);
        }
        let translate = t1.elapsed().as_secs_f64() * 1000.0 / frames as f64;

        println!(
            "drag-frame box-shadow: regenerate (t175) = {regen:.3} ms/frame, \
             translate (t179) = {translate:.3} ms/frame, saved ~{:.3} ms/frame",
            regen - translate
        );
        assert!(
            translate < regen,
            "translate path must be faster than regenerate (got translate={translate:.3} \
             >= regen={regen:.3})"
        );
    }

    /// The shape key must be invariant to position (x/y) and sensitive to every
    /// OTHER mask input — the dual of `box_shadow_mask_key_covers_every_input`.
    #[test]
    fn box_shadow_shape_key_is_position_invariant_and_shape_sensitive() {
        let base = || ShadowParams {
            surface_rect: Rect::new(10.0, 20.0, 200.0, 30.0),
            corner_radius: 0.0,
            spread: 2.0,
            blur_radius: 12,
            offset_x: 0.0,
            offset_y: 4.0,
            shadow_color: Color::new(0, 0, 0, 160),
        };
        let k0 = box_shadow_shape_key(1920, 1080, &base());
        // Position must NOT affect the shape key.
        let mut p = base();
        p.surface_rect.x += 37.0;
        assert_eq!(k0, box_shadow_shape_key(1920, 1080, &p), "x must NOT affect shape key");
        let mut p = base();
        p.surface_rect.y -= 11.0;
        assert_eq!(k0, box_shadow_shape_key(1920, 1080, &p), "y must NOT affect shape key");
        // Every other field must.
        let mut p = base();
        p.surface_rect.width += 1.0;
        assert_ne!(k0, box_shadow_shape_key(1920, 1080, &p), "width");
        let mut p = base();
        p.surface_rect.height += 1.0;
        assert_ne!(k0, box_shadow_shape_key(1920, 1080, &p), "height");
        let mut p = base();
        p.corner_radius += 1.0;
        assert_ne!(k0, box_shadow_shape_key(1920, 1080, &p), "corner");
        let mut p = base();
        p.spread += 1.0;
        assert_ne!(k0, box_shadow_shape_key(1920, 1080, &p), "spread");
        let mut p = base();
        p.blur_radius += 1;
        assert_ne!(k0, box_shadow_shape_key(1920, 1080, &p), "blur");
        let mut p = base();
        p.offset_x += 1.0;
        assert_ne!(k0, box_shadow_shape_key(1920, 1080, &p), "offset_x");
        let mut p = base();
        p.offset_y += 1.0;
        assert_ne!(k0, box_shadow_shape_key(1920, 1080, &p), "offset_y");
        let mut p = base();
        p.shadow_color.a ^= 0xFF;
        assert_ne!(k0, box_shadow_shape_key(1920, 1080, &p), "color");
        assert_ne!(k0, box_shadow_shape_key(1921, 1080, &base()), "fb_w");
        assert_ne!(k0, box_shadow_shape_key(1920, 1081, &base()), "fb_h");
    }
}
