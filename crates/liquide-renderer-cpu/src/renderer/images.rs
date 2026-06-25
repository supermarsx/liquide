//! Image and surface rendering for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{BackgroundRepeat, FlatNode, SceneNodeKind};

use crate::rasterizer;

use crate::texture_cache::{CachedTexture, PatternCacheKey, PatternRepeatMode, image_texture_key};

use super::SoftwareRenderer;

impl SoftwareRenderer {
    /// Render an Image scene node.
    pub(crate) fn render_image_node(&mut self, node: &FlatNode, fb: &mut FrameBuffer) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let SceneNodeKind::Image {
            image_id,
            width,
            height,
            fit,
        } = node.kind_ref()
        {
            let texture_key = image_texture_key(*image_id);

            if let Some(texture) = self.texture_cache.get_by_key(texture_key) {
                let src_w = texture.width as f32;
                let src_h = texture.height as f32;
                let dst_w = bounds.width;
                let dst_h = bounds.height;

                let (src_rect, dst_rect) = match fit {
                    liquide_compositor::scene::ImageFit::Fill => {
                        (Rect::new(0.0, 0.0, src_w, src_h), bounds)
                    }
                    liquide_compositor::scene::ImageFit::Contain => {
                        let scale = (dst_w / src_w).min(dst_h / src_h);
                        let scaled_w = src_w * scale;
                        let scaled_h = src_h * scale;
                        let offset_x = (dst_w - scaled_w) / 2.0;
                        let offset_y = (dst_h - scaled_h) / 2.0;
                        (
                            Rect::new(0.0, 0.0, src_w, src_h),
                            Rect::new(bounds.x + offset_x, bounds.y + offset_y, scaled_w, scaled_h),
                        )
                    }
                    liquide_compositor::scene::ImageFit::Cover => {
                        // A Cover image MUST leave no uncovered edge of its
                        // destination. When the image is a desktop-background-
                        // scale surface (it spans almost the whole framebuffer
                        // and its bounds hug the framebuffer edges), snap the
                        // destination out to the framebuffer so a layout-origin
                        // quirk (e.g. a desktop-background box that starts a few
                        // px in from x=0) cannot leave a black strip along an
                        // edge. Cover then re-crops against the full-screen dst,
                        // still fully covering with no bars. Small / inset
                        // images (icons, thumbnails) keep their exact bounds.
                        let cover_dst = Self::cover_destination_rect(bounds, fb);
                        let cdw = cover_dst.width;
                        let cdh = cover_dst.height;
                        let scale = (cdw / src_w).max(cdh / src_h);
                        let scaled_w = src_w * scale;
                        let scaled_h = src_h * scale;
                        let crop_x = ((scaled_w - cdw) / 2.0) / scale;
                        let crop_y = ((scaled_h - cdh) / 2.0) / scale;
                        (
                            Rect::new(crop_x, crop_y, src_w - crop_x * 2.0, src_h - crop_y * 2.0),
                            cover_dst,
                        )
                    }
                    liquide_compositor::scene::ImageFit::None => {
                        let offset_x = (dst_w - src_w) / 2.0;
                        let offset_y = (dst_h - src_h) / 2.0;
                        (
                            Rect::new(0.0, 0.0, src_w, src_h),
                            Rect::new(
                                bounds.x + offset_x,
                                bounds.y + offset_y,
                                src_w.min(dst_w),
                                src_h.min(dst_h),
                            ),
                        )
                    }
                    liquide_compositor::scene::ImageFit::Sized { width, height } => {
                        // CSS background-size: <w> <h> — scale the whole source
                        // image to the explicit logical size, anchored at the
                        // node's top-left.
                        (
                            Rect::new(0.0, 0.0, src_w, src_h),
                            Rect::new(bounds.x, bounds.y, *width, *height),
                        )
                    }
                };

                self.draw_scaled_texture(fb, &texture, src_rect, dst_rect, opacity);
            } else {
                // Fallback: render placeholder when image not loaded
                let placeholder_color = Color::new(
                    128,
                    128,
                    128,
                    if opacity < 1.0 {
                        (64.0 * opacity + 0.5) as u8
                    } else {
                        64
                    },
                );
                rasterizer::fill_rect(fb, bounds, placeholder_color, BlendMode::SrcOver);

                let cx = bounds.x + bounds.width / 2.0;
                let cy = bounds.y + bounds.height / 2.0;
                let dot_size = 4.0_f32.min(bounds.width / 4.0).min(bounds.height / 4.0);
                if dot_size > 0.5 {
                    let indicator = Color::new(
                        180,
                        180,
                        180,
                        if opacity < 1.0 {
                            (80.0 * opacity + 0.5) as u8
                        } else {
                            80
                        },
                    );
                    rasterizer::fill_rect(
                        fb,
                        Rect::new(cx - dot_size, cy - dot_size, dot_size * 2.0, dot_size * 2.0),
                        indicator,
                        BlendMode::SrcOver,
                    );
                }
            }
            let _ = (width, height);
        }
    }

    /// Destination rect for a Cover-fit image, snapped out to the framebuffer
    /// when the image is a near-full-screen desktop background.
    ///
    /// A Cover image is meant to fully cover its box with no uncovered edge. If
    /// the box itself starts a few pixels in from the framebuffer origin (a
    /// layout-origin quirk for the desktop-background element), Cover dutifully
    /// fills only the box and leaves a black strip along the framebuffer edge.
    /// For a background-scale image we extend the destination to whichever
    /// framebuffer edges the box already hugs, eliminating the strip while
    /// keeping genuinely small / inset images at their exact bounds.
    fn cover_destination_rect(bounds: Rect, fb: &FrameBuffer) -> Rect {
        let fb_w = fb.width as f32;
        let fb_h = fb.height as f32;
        if fb_w <= 0.0 || fb_h <= 0.0 {
            return bounds;
        }

        // Only background-scale images qualify: the box must span most of the
        // framebuffer in both axes. Smaller images keep their exact bounds.
        let covers_most = bounds.width >= fb_w * 0.85 && bounds.height >= fb_h * 0.85;
        if !covers_most {
            return bounds;
        }

        // Snap an edge out to the framebuffer only when the box already hugs it
        // within a small slack (the observed layout strip is ~50px). This keeps
        // a deliberately offset large image from being yanked to the corner.
        let slack = (fb_w.max(fb_h) * 0.1).max(64.0);
        let mut left = bounds.x;
        let mut top = bounds.y;
        let mut right = bounds.right();
        let mut bottom = bounds.bottom();
        if left > 0.0 && left <= slack {
            left = 0.0;
        }
        if top > 0.0 && top <= slack {
            top = 0.0;
        }
        if right < fb_w && right >= fb_w - slack {
            right = fb_w;
        }
        if bottom < fb_h && bottom >= fb_h - slack {
            bottom = fb_h;
        }
        Rect::new(left, top, right - left, bottom - top)
    }

    /// Render a BackgroundFill scene node.
    pub(crate) fn render_background_fill_node(&mut self, node: &FlatNode, fb: &mut FrameBuffer) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let SceneNodeKind::BackgroundFill { background } = node.kind_ref() {
            // Solid color first
            if let Some(bg_color) = background.color {
                let mut c = bg_color;
                if opacity < 1.0 {
                    c.a = (c.a as f32 * opacity + 0.5) as u8;
                }
                if c.a > 0 {
                    rasterizer::fill_rect(fb, bounds, c, BlendMode::SrcOver);
                }
            }
            // Background image (gradient or texture)
            if let Some(ref img) = background.image {
                use liquide_compositor::scene::{
                    BackgroundImage, BackgroundRepeat, BackgroundSize,
                };
                let compute_image_rect = |img_w: f32, img_h: f32| -> Rect {
                    let (bw, bh) = match background.size {
                        BackgroundSize::Auto => (img_w, img_h),
                        BackgroundSize::Cover => {
                            let scale = (bounds.width / img_w).max(bounds.height / img_h);
                            (img_w * scale, img_h * scale)
                        }
                        BackgroundSize::Contain => {
                            let scale = (bounds.width / img_w).min(bounds.height / img_h);
                            (img_w * scale, img_h * scale)
                        }
                        BackgroundSize::Explicit { width, height } => (width, height),
                    };
                    let (pos_x, pos_y) = background.position;
                    let bx = bounds.x + (bounds.width - bw) * (pos_x / 100.0);
                    let by = bounds.y + (bounds.height - bh) * (pos_y / 100.0);
                    Rect::new(bx, by, bw, bh)
                };

                match img {
                    BackgroundImage::Gradient(gradient) => {
                        let img_rect = compute_image_rect(bounds.width, bounds.height);
                        self.render_gradient(fb, img_rect, gradient, opacity, node.corner_radius);
                    }
                    BackgroundImage::ImageId(image_id) => {
                        let texture_key = image_texture_key(*image_id);
                        if let Some(texture) = self.texture_cache.get_by_key(texture_key) {
                            let tw = texture.width as f32;
                            let th = texture.height as f32;
                            let img_rect = compute_image_rect(tw, th);
                            let src = Rect::new(0.0, 0.0, tw, th);
                            match background.repeat {
                                BackgroundRepeat::NoRepeat => {
                                    self.draw_scaled_texture(fb, &texture, src, img_rect, opacity);
                                }
                                BackgroundRepeat::Repeat
                                | BackgroundRepeat::Space
                                | BackgroundRepeat::Round => {
                                    self.render_repeated_background_texture(
                                        fb,
                                        &texture,
                                        texture_key,
                                        src,
                                        img_rect,
                                        bounds,
                                        background.repeat,
                                        opacity,
                                    );
                                }
                                BackgroundRepeat::RepeatX => {
                                    self.render_repeated_background_texture(
                                        fb,
                                        &texture,
                                        texture_key,
                                        src,
                                        img_rect,
                                        bounds,
                                        background.repeat,
                                        opacity,
                                    );
                                }
                                BackgroundRepeat::RepeatY => {
                                    self.render_repeated_background_texture(
                                        fb,
                                        &texture,
                                        texture_key,
                                        src,
                                        img_rect,
                                        bounds,
                                        background.repeat,
                                        opacity,
                                    );
                                }
                            }
                        }
                    }
                    BackgroundImage::Url(_) => {} // External URLs unsupported
                }
            }
        }
    }

    fn render_repeated_background_texture(
        &mut self,
        fb: &mut FrameBuffer,
        texture: &CachedTexture,
        source_key: u64,
        src: Rect,
        img_rect: Rect,
        bounds: Rect,
        repeat: BackgroundRepeat,
        opacity: f32,
    ) {
        if Self::repeated_background_is_pixel_aligned(img_rect, bounds, repeat) {
            if let Some(key) = Self::pattern_cache_key(source_key, texture, img_rect, repeat) {
                if let Some(tile) = self.realized_pattern_tile(key, texture, src) {
                    self.draw_repeated_cached_pattern_tile(
                        fb, &tile, texture, src, img_rect, bounds, repeat, opacity,
                    );
                    return;
                }
            }
        }

        self.draw_repeated_background_legacy(fb, texture, src, img_rect, bounds, repeat, opacity);
    }

    fn pattern_cache_key(
        source_key: u64,
        texture: &CachedTexture,
        img_rect: Rect,
        repeat: BackgroundRepeat,
    ) -> Option<PatternCacheKey> {
        if texture.width == 0 || texture.height == 0 {
            return None;
        }

        let tile_width = Self::realized_tile_dimension(img_rect.width)?;
        let tile_height = Self::realized_tile_dimension(img_rect.height)?;
        let scale_x = img_rect.width / texture.width as f32;
        let scale_y = img_rect.height / texture.height as f32;
        if !scale_x.is_finite() || !scale_y.is_finite() || scale_x <= 0.0 || scale_y <= 0.0 {
            return None;
        }

        Some(PatternCacheKey::new(
            source_key,
            tile_width,
            tile_height,
            Self::pattern_repeat_mode(repeat),
            scale_x,
            scale_y,
        ))
    }

    fn realized_tile_dimension(value: f32) -> Option<u32> {
        if !value.is_finite() || value < 1.0 || value > u32::MAX as f32 {
            return None;
        }
        let rounded = value.round();
        if (value - rounded).abs() > 0.001 {
            return None;
        }
        Some(rounded as u32)
    }

    fn repeated_background_is_pixel_aligned(
        img_rect: Rect,
        bounds: Rect,
        repeat: BackgroundRepeat,
    ) -> bool {
        match repeat {
            // Space/Round change the per-tile geometry (gaps / scale-to-fit), so
            // they cannot use the simple pixel-aligned realized-tile fast path —
            // route them to the legacy tiler which honours their CSS semantics.
            BackgroundRepeat::Space | BackgroundRepeat::Round => false,
            BackgroundRepeat::Repeat => {
                Self::is_pixel_aligned(bounds.x) && Self::is_pixel_aligned(bounds.y)
            }
            BackgroundRepeat::RepeatX => {
                Self::is_pixel_aligned(bounds.x) && Self::is_pixel_aligned(img_rect.y)
            }
            BackgroundRepeat::RepeatY => {
                Self::is_pixel_aligned(img_rect.x) && Self::is_pixel_aligned(bounds.y)
            }
            BackgroundRepeat::NoRepeat => {
                Self::is_pixel_aligned(img_rect.x) && Self::is_pixel_aligned(img_rect.y)
            }
        }
    }

    fn is_pixel_aligned(value: f32) -> bool {
        value.is_finite() && (value - value.round()).abs() <= 0.001
    }

    /// CSS `background-repeat: round` per-axis sizing: choose the whole tile
    /// count nearest to `extent / natural` (min 1) and the adjusted tile size
    /// that makes that count fill `extent` exactly. Returns `(tile_size, count)`.
    fn round_axis(extent: f32, natural: f32) -> (f32, u32) {
        if extent <= 0.0 || !natural.is_finite() || natural <= 0.0 {
            return (0.0, 0);
        }
        let count = (extent / natural).round().max(1.0);
        (extent / count, count as u32)
    }

    /// CSS `background-repeat: space` per-axis layout: lay out as many natural
    /// tiles as fit (min 1), distributing the leftover space as EQUAL gaps
    /// between them. Returns `(gap, count)`. With 0 or 1 tile the gap is 0.
    fn space_axis(extent: f32, natural: f32) -> (f32, u32) {
        if extent <= 0.0 || !natural.is_finite() || natural <= 0.0 {
            return (0.0, 0);
        }
        let count = (extent / natural).floor().max(1.0) as u32;
        if count <= 1 {
            return (0.0, count);
        }
        let leftover = (extent - count as f32 * natural).max(0.0);
        (leftover / (count - 1) as f32, count)
    }

    fn pattern_repeat_mode(repeat: BackgroundRepeat) -> PatternRepeatMode {
        match repeat {
            BackgroundRepeat::Repeat => PatternRepeatMode::Repeat,
            BackgroundRepeat::RepeatX => PatternRepeatMode::RepeatX,
            BackgroundRepeat::RepeatY => PatternRepeatMode::RepeatY,
            BackgroundRepeat::NoRepeat => PatternRepeatMode::NoRepeat,
            BackgroundRepeat::Space => PatternRepeatMode::Space,
            BackgroundRepeat::Round => PatternRepeatMode::Round,
        }
    }

    fn realized_pattern_tile(
        &mut self,
        key: PatternCacheKey,
        texture: &CachedTexture,
        src_rect: Rect,
    ) -> Option<CachedTexture> {
        if let Some(tile) = self.texture_cache.get_pattern(&key) {
            return Some(tile);
        }

        let (tile_width, tile_height) = key.tile_dimensions();
        let pixels = Self::realize_scaled_tile(texture, src_rect, tile_width, tile_height)?;
        self.texture_cache
            .insert_pattern(key, pixels, tile_width, tile_height);
        self.texture_cache.get_pattern(&key)
    }

    fn realize_scaled_tile(
        texture: &CachedTexture,
        src_rect: Rect,
        tile_width: u32,
        tile_height: u32,
    ) -> Option<Vec<u8>> {
        if tile_width == 0 || tile_height == 0 {
            return None;
        }

        let len = tile_width
            .checked_mul(tile_height)?
            .checked_mul(4)
            .map(|value| value as usize)?;
        let mut pixels = vec![0u8; len];

        let src_x0 = src_rect.x.max(0.0) as u32;
        let src_y0 = src_rect.y.max(0.0) as u32;
        let src_x1 = (src_rect.right().min(texture.width as f32)) as u32;
        let src_y1 = (src_rect.bottom().min(texture.height as f32)) as u32;
        if src_x1 <= src_x0 || src_y1 <= src_y0 {
            return None;
        }

        let src_w = (src_x1 - src_x0) as f32;
        let src_h = (src_y1 - src_y0) as f32;
        let dst_w = tile_width as f32;
        let dst_h = tile_height as f32;

        // Match `draw_scaled_texture`'s sampler so the realized-tile cache path
        // and the direct-scale (legacy/no-repeat) path stay byte-identical
        // (t87-crisp #2): bilinear when scaled, nearest at 1:1.
        let scaled = (src_w - dst_w).abs() > 0.5 || (src_h - dst_h).abs() > 0.5;

        let clamp_x = |v: f32| -> u32 { (v as i32).clamp(src_x0 as i32, src_x1 as i32 - 1) as u32 };
        let clamp_y = |v: f32| -> u32 { (v as i32).clamp(src_y0 as i32, src_y1 as i32 - 1) as u32 };
        let texel = |x: u32, y: u32| -> [f32; 4] {
            let i = ((y * texture.width + x) * 4) as usize;
            if i + 3 >= texture.data.len() {
                return [0.0; 4];
            }
            [
                texture.data[i] as f32,
                texture.data[i + 1] as f32,
                texture.data[i + 2] as f32,
                texture.data[i + 3] as f32,
            ]
        };

        for dst_y in 0..tile_height {
            let rel_y = dst_y as f32 / dst_h;
            for dst_x in 0..tile_width {
                let rel_x = dst_x as f32 / dst_w;
                let dst_idx = ((dst_y * tile_width + dst_x) * 4) as usize;

                if scaled {
                    let fx = src_x0 as f32 + rel_x * src_w - 0.5;
                    let fy = src_y0 as f32 + rel_y * src_h - 0.5;
                    let x0 = fx.floor();
                    let y0 = fy.floor();
                    let tx = fx - x0;
                    let ty = fy - y0;
                    let xa = clamp_x(x0);
                    let xb = clamp_x(x0 + 1.0);
                    let ya = clamp_y(y0);
                    let yb = clamp_y(y0 + 1.0);
                    let c00 = texel(xa, ya);
                    let c10 = texel(xb, ya);
                    let c01 = texel(xa, yb);
                    let c11 = texel(xb, yb);
                    for k in 0..4 {
                        let top = c00[k] + (c10[k] - c00[k]) * tx;
                        let bot = c01[k] + (c11[k] - c01[k]) * tx;
                        pixels[dst_idx + k] = (top + (bot - top) * ty + 0.5) as u8;
                    }
                } else {
                    let src_x = (src_x0 as f32 + rel_x * src_w) as u32;
                    let src_y = (src_y0 as f32 + rel_y * src_h) as u32;
                    let src_idx = ((src_y * texture.width + src_x) * 4) as usize;
                    if src_idx + 3 >= texture.data.len() {
                        continue;
                    }
                    pixels[dst_idx] = texture.data[src_idx];
                    pixels[dst_idx + 1] = texture.data[src_idx + 1];
                    pixels[dst_idx + 2] = texture.data[src_idx + 2];
                    pixels[dst_idx + 3] = texture.data[src_idx + 3];
                }
            }
        }

        Some(pixels)
    }

    fn draw_repeated_cached_pattern_tile(
        &mut self,
        fb: &mut FrameBuffer,
        tile_texture: &CachedTexture,
        source_texture: &CachedTexture,
        src: Rect,
        img_rect: Rect,
        bounds: Rect,
        repeat: BackgroundRepeat,
        opacity: f32,
    ) {
        match repeat {
            BackgroundRepeat::Repeat | BackgroundRepeat::Space | BackgroundRepeat::Round => {
                let mut ty = bounds.y;
                while ty < bounds.y + bounds.height {
                    let mut tx = bounds.x;
                    while tx < bounds.x + bounds.width {
                        let tile = Rect::new(tx, ty, img_rect.width, img_rect.height);
                        self.draw_cached_pattern_or_scaled(
                            fb,
                            tile_texture,
                            source_texture,
                            src,
                            tile,
                            opacity,
                        );
                        tx += img_rect.width;
                    }
                    ty += img_rect.height;
                }
            }
            BackgroundRepeat::RepeatX => {
                let mut tx = bounds.x;
                while tx < bounds.x + bounds.width {
                    let tile = Rect::new(tx, img_rect.y, img_rect.width, img_rect.height);
                    self.draw_cached_pattern_or_scaled(
                        fb,
                        tile_texture,
                        source_texture,
                        src,
                        tile,
                        opacity,
                    );
                    tx += img_rect.width;
                }
            }
            BackgroundRepeat::RepeatY => {
                let mut ty = bounds.y;
                while ty < bounds.y + bounds.height {
                    let tile = Rect::new(img_rect.x, ty, img_rect.width, img_rect.height);
                    self.draw_cached_pattern_or_scaled(
                        fb,
                        tile_texture,
                        source_texture,
                        src,
                        tile,
                        opacity,
                    );
                    ty += img_rect.height;
                }
            }
            BackgroundRepeat::NoRepeat => {
                self.draw_scaled_texture(fb, source_texture, src, img_rect, opacity);
            }
        }
    }

    fn draw_cached_pattern_or_scaled(
        &mut self,
        fb: &mut FrameBuffer,
        tile_texture: &CachedTexture,
        source_texture: &CachedTexture,
        src: Rect,
        tile: Rect,
        opacity: f32,
    ) {
        if Self::can_blit_cached_pattern_tile(fb, tile_texture, tile) {
            Self::blit_cached_pattern_tile(
                fb,
                tile_texture,
                tile.x.round() as u32,
                tile.y.round() as u32,
                opacity,
            );
        } else {
            self.draw_scaled_texture(fb, source_texture, src, tile, opacity);
        }
    }

    fn can_blit_cached_pattern_tile(
        fb: &FrameBuffer,
        tile_texture: &CachedTexture,
        tile: Rect,
    ) -> bool {
        Self::is_pixel_aligned(tile.x)
            && Self::is_pixel_aligned(tile.y)
            && Self::realized_tile_dimension(tile.width) == Some(tile_texture.width)
            && Self::realized_tile_dimension(tile.height) == Some(tile_texture.height)
            && tile.x >= 0.0
            && tile.y >= 0.0
            && tile.right() <= fb.width as f32
            && tile.bottom() <= fb.height as f32
    }

    fn blit_cached_pattern_tile(
        fb: &mut FrameBuffer,
        tile_texture: &CachedTexture,
        dst_x: u32,
        dst_y: u32,
        opacity: f32,
    ) {
        // SIMD per-row SrcOver of the realized tile onto the framebuffer (t-prim
        // #3). The premultiplied native-byte source row is byte-identical to the
        // scalar path; a transparent (a==0) src pixel stays a SrcOver no-op,
        // matching the old `continue`. Non-BGRA/RGBA formats fall back to scalar.
        use liquide_compositor::pixel::PixelFormat;
        let fmt = fb.format;
        let simd_eligible = matches!(fmt, PixelFormat::Bgra8 | PixelFormat::Rgba8);
        let to_native = |c: Color| -> [u8; 4] {
            match fmt {
                PixelFormat::Rgba8 => [c.r, c.g, c.b, c.a],
                _ => c.to_bgra_bytes(),
            }
        };
        let sample = |x: u32, y: u32| -> Option<Color> {
            let src_idx = ((y * tile_texture.width + x) * 4) as usize;
            if src_idx + 3 >= tile_texture.data.len() {
                return None;
            }
            let mut src_color = Color::new(
                tile_texture.data[src_idx],
                tile_texture.data[src_idx + 1],
                tile_texture.data[src_idx + 2],
                tile_texture.data[src_idx + 3],
            );
            if src_color.a == 0 {
                return None;
            }
            if opacity < 1.0 {
                src_color.a = (src_color.a as f32 * opacity + 0.5) as u8;
            }
            Some(src_color.premultiply())
        };

        let tw = tile_texture.width;
        if simd_eligible && dst_x + tw <= fb.width {
            let run = tw as usize;
            let mut src_row = vec![0u8; run * 4];
            for y in 0..tile_texture.height {
                let py = dst_y + y;
                for x in 0..tw {
                    let bytes = match sample(x, y) {
                        Some(pm) => to_native(pm),
                        None => [0, 0, 0, 0],
                    };
                    src_row[x as usize * 4..x as usize * 4 + 4].copy_from_slice(&bytes);
                }
                if let Some(row) = fb.row_mut(py) {
                    let start = dst_x as usize * 4;
                    let end = (dst_x + tw) as usize * 4;
                    crate::blend::blend_scanline(
                        &mut row[start..end],
                        &src_row[..run * 4],
                        BlendMode::SrcOver,
                    );
                }
            }
        } else {
            for y in 0..tile_texture.height {
                for x in 0..tw {
                    let Some(src_color) = sample(x, y) else {
                        continue;
                    };
                    let px = dst_x + x;
                    let py = dst_y + y;
                    let dst_color = fb.get_pixel(px, py);
                    let blended = crate::blend::blend(dst_color, src_color, BlendMode::SrcOver);
                    fb.set_pixel(px, py, blended);
                }
            }
        }
    }

    fn draw_repeated_background_legacy(
        &mut self,
        fb: &mut FrameBuffer,
        texture: &CachedTexture,
        src: Rect,
        img_rect: Rect,
        bounds: Rect,
        repeat: BackgroundRepeat,
        opacity: f32,
    ) {
        match repeat {
            BackgroundRepeat::NoRepeat => {
                self.draw_scaled_texture(fb, texture, src, img_rect, opacity);
            }
            BackgroundRepeat::Repeat => {
                let mut ty = bounds.y;
                while ty < bounds.y + bounds.height {
                    let mut tx = bounds.x;
                    while tx < bounds.x + bounds.width {
                        let tile = Rect::new(tx, ty, img_rect.width, img_rect.height);
                        self.draw_scaled_texture(fb, texture, src, tile, opacity);
                        tx += img_rect.width;
                    }
                    ty += img_rect.height;
                }
            }
            BackgroundRepeat::Round => {
                // CSS `round`: scale each tile (independently per axis) so a WHOLE
                // number of tiles fills the box with no gaps and no clipping.
                let (tw, nx) = Self::round_axis(bounds.width, img_rect.width);
                let (th, ny) = Self::round_axis(bounds.height, img_rect.height);
                if tw <= 0.0 || th <= 0.0 {
                    return;
                }
                for iy in 0..ny {
                    let ty = bounds.y + iy as f32 * th;
                    for ix in 0..nx {
                        let tx = bounds.x + ix as f32 * tw;
                        let tile = Rect::new(tx, ty, tw, th);
                        self.draw_scaled_texture(fb, texture, src, tile, opacity);
                    }
                }
            }
            BackgroundRepeat::Space => {
                // CSS `space`: tiles keep their natural size; whole tiles are laid
                // out with EQUAL gaps between them (first/last flush to the edges).
                // A single tile (or one that overflows) is centred / left flush.
                let (gx, nx) = Self::space_axis(bounds.width, img_rect.width);
                let (gy, ny) = Self::space_axis(bounds.height, img_rect.height);
                for iy in 0..ny {
                    let ty = bounds.y + iy as f32 * (img_rect.height + gy);
                    for ix in 0..nx {
                        let tx = bounds.x + ix as f32 * (img_rect.width + gx);
                        let tile = Rect::new(tx, ty, img_rect.width, img_rect.height);
                        self.draw_scaled_texture(fb, texture, src, tile, opacity);
                    }
                }
            }
            BackgroundRepeat::RepeatX => {
                let mut tx = bounds.x;
                while tx < bounds.x + bounds.width {
                    let tile = Rect::new(tx, img_rect.y, img_rect.width, img_rect.height);
                    self.draw_scaled_texture(fb, texture, src, tile, opacity);
                    tx += img_rect.width;
                }
            }
            BackgroundRepeat::RepeatY => {
                let mut ty = bounds.y;
                while ty < bounds.y + bounds.height {
                    let tile = Rect::new(img_rect.x, ty, img_rect.width, img_rect.height);
                    self.draw_scaled_texture(fb, texture, src, tile, opacity);
                    ty += img_rect.height;
                }
            }
        }
    }

    /// Render a BorderImage scene node.
    pub(crate) fn render_border_image_node(&mut self, node: &FlatNode, fb: &mut FrameBuffer) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let SceneNodeKind::BorderImage { spec } = node.kind_ref() {
            use liquide_compositor::scene::BackgroundImage;
            match &spec.source {
                BackgroundImage::ImageId(image_id) => {
                    let texture_key = image_texture_key(*image_id);
                    if let Some(texture) = self.texture_cache.get_by_key(texture_key) {
                        // ── CSS border-image 9-slice (t-prim #5) ──────────────
                        //
                        // Expand the border box by `outset`, slice the SOURCE into
                        // 9 regions by the `slice` percentages, and map them to the
                        // destination: the 4 CORNERS are drawn 1:1 (never
                        // stretched), the 4 EDGES are stretched or tiled per the
                        // repeat mode, and the CENTER is dropped (no `fill`).
                        let sw = texture.width as f32;
                        let sh = texture.height as f32;
                        if sw <= 0.0 || sh <= 0.0 {
                            return;
                        }

                        let (ot, or, ob, ol) = spec.outset;
                        let outer = Rect::new(
                            bounds.x - ol,
                            bounds.y - ot,
                            bounds.width + ol + or,
                            bounds.height + ot + ob,
                        );

                        // Source slice insets (percent of source dims → pixels).
                        let (st_p, sr_p, sb_p, sl_p) = spec.slice;
                        let s_top = (st_p / 100.0).clamp(0.0, 1.0) * sh;
                        let s_right = (sr_p / 100.0).clamp(0.0, 1.0) * sw;
                        let s_bottom = (sb_p / 100.0).clamp(0.0, 1.0) * sh;
                        let s_left = (sl_p / 100.0).clamp(0.0, 1.0) * sw;
                        let s_cw = (sw - s_left - s_right).max(0.0);
                        let s_ch = (sh - s_top - s_bottom).max(0.0);

                        // Destination border widths (pixels), clamped so opposite
                        // pairs never exceed the box (CSS shrinks them together).
                        let (mut wt, mut wr, mut wb, mut wl) = spec.width;
                        let sx = if wl + wr > outer.width && wl + wr > 0.0 {
                            outer.width / (wl + wr)
                        } else {
                            1.0
                        };
                        let sy = if wt + wb > outer.height && wt + wb > 0.0 {
                            outer.height / (wt + wb)
                        } else {
                            1.0
                        };
                        wt *= sy;
                        wb *= sy;
                        wl *= sx;
                        wr *= sx;
                        let d_cw = (outer.width - wl - wr).max(0.0);
                        let d_ch = (outer.height - wt - wb).max(0.0);

                        let tex = &texture;
                        // src/dst region pairs. (corners stretch-mapped to corner
                        // boxes, which for a 1:1 slice→width is exact; for unequal
                        // slice vs width they scale, matching the CSS spec.)
                        // Corner regions:
                        let corners = [
                            // (src, dst)
                            (
                                Rect::new(0.0, 0.0, s_left, s_top),
                                Rect::new(outer.x, outer.y, wl, wt),
                            ),
                            (
                                Rect::new(sw - s_right, 0.0, s_right, s_top),
                                Rect::new(outer.right() - wr, outer.y, wr, wt),
                            ),
                            (
                                Rect::new(0.0, sh - s_bottom, s_left, s_bottom),
                                Rect::new(outer.x, outer.bottom() - wb, wl, wb),
                            ),
                            (
                                Rect::new(sw - s_right, sh - s_bottom, s_right, s_bottom),
                                Rect::new(outer.right() - wr, outer.bottom() - wb, wr, wb),
                            ),
                        ];
                        for (s, d) in corners {
                            if s.width > 0.0 && s.height > 0.0 && d.width > 0.0 && d.height > 0.0 {
                                self.draw_scaled_texture(fb, tex, s, d, opacity);
                            }
                        }

                        // Edges. Top/bottom tile/stretch horizontally; left/right
                        // vertically. `repeat` selects stretch vs tile (round/space
                        // approximated as repeat-to-fit here).
                        let repeat = spec.repeat;
                        // Top edge.
                        self.draw_border_image_edge(
                            fb,
                            tex,
                            Rect::new(s_left, 0.0, s_cw, s_top),
                            Rect::new(outer.x + wl, outer.y, d_cw, wt),
                            repeat,
                            true,
                            opacity,
                        );
                        // Bottom edge.
                        self.draw_border_image_edge(
                            fb,
                            tex,
                            Rect::new(s_left, sh - s_bottom, s_cw, s_bottom),
                            Rect::new(outer.x + wl, outer.bottom() - wb, d_cw, wb),
                            repeat,
                            true,
                            opacity,
                        );
                        // Left edge.
                        self.draw_border_image_edge(
                            fb,
                            tex,
                            Rect::new(0.0, s_top, s_left, s_ch),
                            Rect::new(outer.x, outer.y + wt, wl, d_ch),
                            repeat,
                            false,
                            opacity,
                        );
                        // Right edge.
                        self.draw_border_image_edge(
                            fb,
                            tex,
                            Rect::new(sw - s_right, s_top, s_right, s_ch),
                            Rect::new(outer.right() - wr, outer.y + wt, wr, d_ch),
                            repeat,
                            false,
                            opacity,
                        );
                        // Center is intentionally NOT filled (no `fill` keyword in
                        // the scene spec).
                    }
                }
                BackgroundImage::Gradient(gradient) => {
                    self.render_gradient(fb, bounds, gradient, opacity, node.corner_radius);
                }
                _ => {}
            }
        }
    }

    /// Draw one border-image edge region, tiling or stretching per `repeat`.
    ///
    /// `horizontal` selects the tiling axis (true = top/bottom edges tile along
    /// x; false = left/right edges tile along y). `Stretch` scales the single
    /// source slice to fill the whole edge; `Repeat`/`Round`/`Space` lay out
    /// whole tiles to fit (Round/Space approximated as repeat-to-fill).
    #[allow(clippy::too_many_arguments)]
    fn draw_border_image_edge(
        &mut self,
        fb: &mut FrameBuffer,
        texture: &CachedTexture,
        src: Rect,
        dst: Rect,
        repeat: liquide_compositor::scene::BorderImageRepeat,
        horizontal: bool,
        opacity: f32,
    ) {
        use liquide_compositor::scene::BorderImageRepeat;
        if src.width <= 0.0 || src.height <= 0.0 || dst.width <= 0.0 || dst.height <= 0.0 {
            return;
        }
        match repeat {
            BorderImageRepeat::Stretch => {
                self.draw_scaled_texture(fb, texture, src, dst, opacity);
            }
            BorderImageRepeat::Repeat
            | BorderImageRepeat::Round
            | BorderImageRepeat::Space => {
                // Natural tile size in destination space: the source slice keeps
                // its aspect along the tiling axis, the cross axis fills the edge.
                if horizontal {
                    // Tile size = source slice width scaled by the edge's height
                    // ratio so tiles aren't squashed; for Round, snap to a whole
                    // count that fills the edge exactly.
                    let scale = dst.height / src.height;
                    let mut tile_w = (src.width * scale).max(1.0);
                    let count = match repeat {
                        BorderImageRepeat::Round => {
                            let n = (dst.width / tile_w).round().max(1.0);
                            tile_w = dst.width / n;
                            n as u32
                        }
                        _ => (dst.width / tile_w).ceil().max(1.0) as u32,
                    };
                    for i in 0..count {
                        let tx = dst.x + i as f32 * tile_w;
                        let w = tile_w.min(dst.right() - tx);
                        if w <= 0.0 {
                            break;
                        }
                        // Crop source proportionally if the last tile is clipped.
                        let src_w = src.width * (w / tile_w);
                        let s = Rect::new(src.x, src.y, src_w, src.height);
                        let d = Rect::new(tx, dst.y, w, dst.height);
                        self.draw_scaled_texture(fb, texture, s, d, opacity);
                    }
                } else {
                    let scale = dst.width / src.width;
                    let mut tile_h = (src.height * scale).max(1.0);
                    let count = match repeat {
                        BorderImageRepeat::Round => {
                            let n = (dst.height / tile_h).round().max(1.0);
                            tile_h = dst.height / n;
                            n as u32
                        }
                        _ => (dst.height / tile_h).ceil().max(1.0) as u32,
                    };
                    for i in 0..count {
                        let ty = dst.y + i as f32 * tile_h;
                        let h = tile_h.min(dst.bottom() - ty);
                        if h <= 0.0 {
                            break;
                        }
                        let src_h = src.height * (h / tile_h);
                        let s = Rect::new(src.x, src.y, src.width, src_h);
                        let d = Rect::new(dst.x, ty, dst.width, h);
                        self.draw_scaled_texture(fb, texture, s, d, opacity);
                    }
                }
            }
        }
    }

    /// Draw a texture to the framebuffer with scaling.
    pub(crate) fn draw_scaled_texture(
        &mut self,
        fb: &mut FrameBuffer,
        texture: &crate::texture_cache::CachedTexture,
        src_rect: Rect,
        dst_rect: Rect,
        opacity: f32,
    ) {
        let src_x0 = src_rect.x.max(0.0) as u32;
        let src_y0 = src_rect.y.max(0.0) as u32;
        let src_x1 = (src_rect.right().min(texture.width as f32)) as u32;
        let src_y1 = (src_rect.bottom().min(texture.height as f32)) as u32;

        let dst_x0 = dst_rect.x.max(0.0);
        let dst_y0 = dst_rect.y.max(0.0);
        let dst_x1 = dst_rect.right().min(fb.width as f32);
        let dst_y1 = dst_rect.bottom().min(fb.height as f32);

        if dst_x0 >= dst_x1 || dst_y0 >= dst_y1 {
            return;
        }

        let src_w = (src_x1 - src_x0) as f32;
        let src_h = (src_y1 - src_y0) as f32;
        let dst_w = dst_x1 - dst_x0;
        let dst_h = dst_y1 - dst_y0;

        if src_w <= 0.0 || src_h <= 0.0 || dst_w <= 0.0 || dst_h <= 0.0 {
            return;
        }

        // Confine the destination window to the per-thread write-scissor (t80).
        // The source mapping below is anchored to `dst_x0/dst_y0`, so skipping
        // edge pixels does not shift any survivor — a full-screen wallpaper image
        // on a partial-damage frame now writes only inside the damage rect
        // instead of re-blitting the whole screen (the t79 regression).
        let (sc_x0, sc_y0, sc_x1, sc_y1) = rasterizer::scissor_clamp_window(
            dst_x0 as u32,
            dst_y0 as u32,
            dst_x1.ceil() as u32,
            dst_y1.ceil() as u32,
        );
        // Choose sampler: nearest for 1:1 blits (crisp UI sprites/icons at native
        // size), bilinear when the source is scaled to a different destination
        // size (wallpaper, scaled photos/icons). Nearest-neighbor on a non-integer
        // scale produces visibly jagged diagonals and duplicated/dropped rows —
        // the single most visible "not crisp" artifact on the desktop wallpaper
        // (t83-crisp #2). Compare the integer span ratios with a small epsilon so
        // exact 1:1 (and integer-multiple-free fractional) cases are detected.
        let scaled = (src_w - dst_w).abs() > 0.5 || (src_h - dst_h).abs() > 0.5;

        // ── SIMD blit fast path (t-primitives #3) ───────────────────────────
        //
        // The largest per-frame pixel-bandwidth op (the full-screen wallpaper /
        // image / pattern blit) previously SrcOver-blended one pixel at a time.
        // We now build a PREMULTIPLIED source scanline in the framebuffer's
        // native byte order — the SAME bytes `set_pixel(.., blend(..))` would
        // ultimately rely on — and hand the whole row to the SIMD
        // `blend_scanline_src_over` kernel (via `crate::blend::blend_scanline`),
        // which is certified byte-identical to the scalar `crate::blend::blend`.
        //
        // The per-pixel SAMPLE math (bilinear / nearest, opacity, premultiply) is
        // unchanged; only the blend is vectorised. Gated to 4-byte BGRA/RGBA
        // (alpha at byte 3, what the SIMD kernel assumes); any other format takes
        // the original scalar `set_pixel` loop, bit-for-bit unchanged.
        use liquide_compositor::pixel::PixelFormat;
        let fmt = fb.format;
        let simd_eligible = matches!(fmt, PixelFormat::Bgra8 | PixelFormat::Rgba8);
        let to_native = |c: Color| -> [u8; 4] {
            match fmt {
                PixelFormat::Rgba8 => [c.r, c.g, c.b, c.a],
                _ => c.to_bgra_bytes(),
            }
        };

        // Sample a single destination pixel → premultiplied straight Color, or
        // `None` when the scalar path would `continue` (skip the write entirely).
        let sample_px = |dst_x: u32, dst_y: u32| -> Option<Color> {
            let rel_x = (dst_x as f32 - dst_x0) / dst_w;
            let rel_y = (dst_y as f32 - dst_y0) / dst_h;
            let mut src_color = if scaled {
                let fx = src_x0 as f32 + rel_x * src_w - 0.5;
                let fy = src_y0 as f32 + rel_y * src_h - 0.5;
                let x0 = fx.floor();
                let y0 = fy.floor();
                let tx = fx - x0;
                let ty = fy - y0;
                let clamp_x =
                    |v: f32| -> u32 { (v as i32).clamp(src_x0 as i32, src_x1 as i32 - 1) as u32 };
                let clamp_y =
                    |v: f32| -> u32 { (v as i32).clamp(src_y0 as i32, src_y1 as i32 - 1) as u32 };
                let xa = clamp_x(x0);
                let xb = clamp_x(x0 + 1.0);
                let ya = clamp_y(y0);
                let yb = clamp_y(y0 + 1.0);
                let texel = |x: u32, y: u32| -> [f32; 4] {
                    let i = ((y * texture.width + x) * 4) as usize;
                    if i + 3 >= texture.data.len() {
                        return [0.0; 4];
                    }
                    [
                        texture.data[i] as f32,
                        texture.data[i + 1] as f32,
                        texture.data[i + 2] as f32,
                        texture.data[i + 3] as f32,
                    ]
                };
                let c00 = texel(xa, ya);
                let c10 = texel(xb, ya);
                let c01 = texel(xa, yb);
                let c11 = texel(xb, yb);
                let mut out = [0.0f32; 4];
                for k in 0..4 {
                    let top = c00[k] + (c10[k] - c00[k]) * tx;
                    let bot = c01[k] + (c11[k] - c01[k]) * tx;
                    out[k] = top + (bot - top) * ty;
                }
                Color::new(
                    (out[0] + 0.5) as u8,
                    (out[1] + 0.5) as u8,
                    (out[2] + 0.5) as u8,
                    (out[3] + 0.5) as u8,
                )
            } else {
                let src_x = (src_x0 as f32 + rel_x * src_w) as u32;
                let src_y = (src_y0 as f32 + rel_y * src_h) as u32;
                let src_idx = ((src_y * texture.width + src_x) * 4) as usize;
                if src_idx + 3 >= texture.data.len() {
                    return None;
                }
                Color::new(
                    texture.data[src_idx],
                    texture.data[src_idx + 1],
                    texture.data[src_idx + 2],
                    texture.data[src_idx + 3],
                )
            };
            if opacity < 1.0 {
                src_color.a = (src_color.a as f32 * opacity + 0.5) as u8;
            }
            Some(src_color.premultiply())
        };

        if simd_eligible {
            let run = (sc_x1 - sc_x0) as usize;
            let mut src_row = vec![0u8; run * 4];
            for dst_y in sc_y0..sc_y1 {
                for (i, dst_x) in (sc_x0..sc_x1).enumerate() {
                    // Default to alpha-0 (a SrcOver no-op == the scalar skip /
                    // `continue`, which left dst untouched).
                    let bytes = match sample_px(dst_x, dst_y) {
                        Some(pm) => to_native(pm),
                        None => [0, 0, 0, 0],
                    };
                    src_row[i * 4..i * 4 + 4].copy_from_slice(&bytes);
                }
                if let Some(row) = fb.row_mut(dst_y) {
                    let start = sc_x0 as usize * 4;
                    let end = sc_x1 as usize * 4;
                    crate::blend::blend_scanline(
                        &mut row[start..end],
                        &src_row[..run * 4],
                        BlendMode::SrcOver,
                    );
                }
            }
        } else {
            // Scalar fallback (non-BGRA/RGBA formats): byte-for-byte unchanged.
            for dst_y in sc_y0..sc_y1 {
                for dst_x in sc_x0..sc_x1 {
                    let Some(src_color) = sample_px(dst_x, dst_y) else {
                        continue;
                    };
                    let dst_color = fb.get_pixel(dst_x, dst_y);
                    let blended = crate::blend::blend(dst_color, src_color, BlendMode::SrcOver);
                    fb.set_pixel(dst_x, dst_y, blended);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::Renderer;
    use liquide_compositor::damage::{DamageClass, DamageSet};
    use liquide_compositor::geometry::Affine2D;
    use liquide_compositor::pixel::PixelFormat;
    use liquide_compositor::scene::{BackgroundImage, BackgroundSize, BackgroundSpec};

    // A genuine FULL-frame damage set (clip = None), matching the real capture
    // path these image tests model. (t80: previously this only marked tile
    // (0,0); the per-frame write-scissor now confines partial frames, so a
    // single-tile "full_damage" would clip a full-bleed wallpaper — which is the
    // correct partial-frame behaviour but not what these full-coverage tests
    // intend. The grid is sized generously to cover every test framebuffer.)
    fn full_damage() -> DamageSet {
        DamageSet::full(64, 32, 32, DamageClass::UiPrimitive)
    }

    fn background_node(
        image_id: u64,
        repeat: BackgroundRepeat,
        size: BackgroundSize,
        bounds: Rect,
    ) -> FlatNode {
        FlatNode {
            id: image_id,
            kind: SceneNodeKind::BackgroundFill {
                background: BackgroundSpec {
                    color: None,
                    image: Some(BackgroundImage::ImageId(image_id)),
                    size,
                    position: (0.0, 0.0),
                    repeat,
                },
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

    fn checker_rgba() -> Vec<u8> {
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ]
    }

    fn render_background(renderer: &mut SoftwareRenderer, node: &FlatNode) -> FrameBuffer {
        let mut fb = FrameBuffer::new(12, 12, PixelFormat::Bgra8);
        let damage = full_damage();
        renderer
            .render(std::slice::from_ref(node), &mut fb, &damage)
            .unwrap();
        fb
    }

    // t87-crisp #2: scaling an image to a different size must use BILINEAR
    // interpolation, not nearest-neighbor. Anti-fake-green: if the sampler
    // reverts to nearest, the center pixels collapse back to pure source colors
    // and these assertions fail.
    #[test]
    fn scaled_texture_is_bilinear_not_nearest() {
        let mut renderer = SoftwareRenderer::new();
        // 2x2 checker: TL red, TR green, BL blue, BR white.
        renderer.register_image_rgba(91, checker_rgba(), 2, 2);
        let texture = renderer
            .texture_cache
            .get_by_key(crate::texture_cache::image_texture_key(91))
            .expect("texture registered");

        // Draw the 2x2 source scaled up to an 8x8 destination.
        let mut fb = FrameBuffer::new(8, 8, PixelFormat::Bgra8);
        let src = Rect::new(0.0, 0.0, 2.0, 2.0);
        let dst = Rect::new(0.0, 0.0, 8.0, 8.0);
        renderer.draw_scaled_texture(&mut fb, &texture, src, dst, 1.0);

        // A pixel straddling the boundary between two source texels must be a
        // genuine blend of them — distinct from BOTH neighbours. Pixel (3,0) sits
        // near the horizontal midpoint of the top row (red -> green), so its
        // red and green channels must both be partial (interpolated), which
        // nearest-neighbor can never produce.
        let mid = fb.get_pixel(3, 0);
        assert!(
            mid.r > 0 && mid.r < 255,
            "expected interpolated red, got {} (nearest-neighbor regressed?)",
            mid.r
        );
        assert!(
            mid.g > 0 && mid.g < 255,
            "expected interpolated green, got {} (nearest-neighbor regressed?)",
            mid.g
        );

        // The exact center (3,3)/(4,4) straddles all four texels; it must not
        // equal any single source color exactly.
        let center = fb.get_pixel(4, 4);
        let pure = |c: &Color, r: u8, g: u8, b: u8| c.r == r && c.g == g && c.b == b;
        assert!(
            !pure(&center, 255, 0, 0)
                && !pure(&center, 0, 255, 0)
                && !pure(&center, 0, 0, 255)
                && !pure(&center, 255, 255, 255),
            "center pixel {:?} equals a pure source texel — not interpolated",
            center
        );
    }

    // t87-crisp #2 tooth: a 1:1 blit (no scaling) must stay NEAREST (exact,
    // crisp) — never smeared by the bilinear branch.
    #[test]
    fn unscaled_texture_is_exact_nearest() {
        let mut renderer = SoftwareRenderer::new();
        renderer.register_image_rgba(92, checker_rgba(), 2, 2);
        let texture = renderer
            .texture_cache
            .get_by_key(crate::texture_cache::image_texture_key(92))
            .expect("texture registered");

        let mut fb = FrameBuffer::new(2, 2, PixelFormat::Bgra8);
        let src = Rect::new(0.0, 0.0, 2.0, 2.0);
        let dst = Rect::new(0.0, 0.0, 2.0, 2.0);
        renderer.draw_scaled_texture(&mut fb, &texture, src, dst, 1.0);

        // Source colors must survive byte-exact at 1:1.
        let tl = fb.get_pixel(0, 0);
        assert_eq!((tl.r, tl.g, tl.b), (255, 0, 0));
        let br = fb.get_pixel(1, 1);
        assert_eq!((br.r, br.g, br.b), (255, 255, 255));
    }

    #[test]
    fn repeated_background_reuses_realized_pattern_tile() {
        let mut renderer = SoftwareRenderer::new();
        renderer.register_image_rgba(7, checker_rgba(), 2, 2);
        let node = background_node(
            7,
            BackgroundRepeat::Repeat,
            BackgroundSize::Explicit {
                width: 2.0,
                height: 2.0,
            },
            Rect::new(0.0, 0.0, 12.0, 12.0),
        );

        let _ = render_background(&mut renderer, &node);
        assert_eq!(renderer.texture_cache.pattern_len(), 1);
        let size_after_first_render = renderer.texture_cache.stats().size_bytes;

        let _ = render_background(&mut renderer, &node);
        assert_eq!(renderer.texture_cache.pattern_len(), 1);
        assert_eq!(
            renderer.texture_cache.stats().size_bytes,
            size_after_first_render
        );
    }

    #[test]
    fn repeated_background_cache_differentiates_identity_size_and_repeat() {
        let mut renderer = SoftwareRenderer::new();
        renderer.register_image_rgba(10, checker_rgba(), 2, 2);
        renderer.register_image_rgba(11, checker_rgba(), 2, 2);

        let repeat_10 = background_node(
            10,
            BackgroundRepeat::Repeat,
            BackgroundSize::Explicit {
                width: 2.0,
                height: 2.0,
            },
            Rect::new(0.0, 0.0, 12.0, 12.0),
        );
        let repeat_11 = background_node(
            11,
            BackgroundRepeat::Repeat,
            BackgroundSize::Explicit {
                width: 2.0,
                height: 2.0,
            },
            Rect::new(0.0, 0.0, 12.0, 12.0),
        );
        let larger_tile = background_node(
            10,
            BackgroundRepeat::Repeat,
            BackgroundSize::Explicit {
                width: 4.0,
                height: 2.0,
            },
            Rect::new(0.0, 0.0, 12.0, 12.0),
        );
        let repeat_x = background_node(
            10,
            BackgroundRepeat::RepeatX,
            BackgroundSize::Explicit {
                width: 2.0,
                height: 2.0,
            },
            Rect::new(0.0, 0.0, 12.0, 12.0),
        );

        let _ = render_background(&mut renderer, &repeat_10);
        let _ = render_background(&mut renderer, &repeat_11);
        let _ = render_background(&mut renderer, &larger_tile);
        let _ = render_background(&mut renderer, &repeat_x);
        assert_eq!(renderer.texture_cache.pattern_len(), 4);

        renderer.register_image_rgba(10, vec![32u8; 16], 2, 2);
        assert_eq!(renderer.texture_cache.pattern_len(), 1);
    }

    #[test]
    fn image_fit_sized_scales_to_explicit_size_not_bounds() {
        use liquide_compositor::geometry::Affine2D;
        use liquide_compositor::scene::ImageFit;

        let mut renderer = SoftwareRenderer::new();
        // 2x2 fully-opaque white texture.
        renderer.register_image_rgba(55, vec![255u8; 16], 2, 2);

        // Image node bounds are 20x20, but Sized forces an 8x8 draw rect at the
        // node's top-left. So pixels inside 8x8 are painted, pixels beyond are not.
        let node = FlatNode {
            id: 55,
            kind: SceneNodeKind::Image {
                image_id: 55,
                width: 2,
                height: 2,
                fit: ImageFit::Sized {
                    width: 8.0,
                    height: 8.0,
                },
            }
            .into(),
            absolute_bounds: Rect::new(0.0, 0.0, 20.0, 20.0),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        };

        let mut fb = FrameBuffer::new(24, 24, PixelFormat::Bgra8);
        let damage = full_damage();
        renderer
            .render(std::slice::from_ref(&node), &mut fb, &damage)
            .unwrap();

        // Inside the 8x8 explicit size: painted.
        assert!(
            fb.get_pixel(2, 2).a > 0,
            "pixel inside explicit 8x8 size should be painted"
        );
        // Beyond 8px but within the 20px bounds: NOT painted (Sized != Fill).
        assert_eq!(
            fb.get_pixel(15, 15).a,
            0,
            "pixel beyond the explicit size must not be painted"
        );
    }

    #[test]
    fn registered_image_rasterizes_with_cover_fit() {
        use liquide_compositor::geometry::Affine2D;
        use liquide_compositor::scene::ImageFit;

        // 2x2 source: distinct opaque colors per quadrant so we can confirm real
        // texels (not the unloaded gray placeholder) land on screen.
        // RGBA: red, green, blue, white.
        let src = vec![
            255, 0, 0, 255, // (0,0) red
            0, 255, 0, 255, // (1,0) green
            0, 0, 255, 255, // (0,1) blue
            255, 255, 255, 255, // (1,1) white
        ];
        let mut renderer = SoftwareRenderer::new();
        renderer.register_image_rgba(77, src, 2, 2);

        // Wide bounds (40x20) with a square source forces Cover to crop
        // horizontally and fill the whole rect — every pixel must be a real
        // texel (fully opaque), never the gray placeholder.
        let node = FlatNode {
            id: 77,
            kind: SceneNodeKind::Image {
                image_id: 77,
                width: 2,
                height: 2,
                fit: ImageFit::Cover,
            }
            .into(),
            absolute_bounds: Rect::new(0.0, 0.0, 40.0, 20.0),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        };

        let mut fb = FrameBuffer::new(40, 20, PixelFormat::Bgra8);
        let damage = full_damage();
        renderer
            .render(std::slice::from_ref(&node), &mut fb, &damage)
            .unwrap();

        // Cover fills the entire rect with opaque texels.
        for (x, y) in [(0, 0), (39, 0), (0, 19), (39, 19), (20, 10)] {
            let p = fb.get_pixel(x, y);
            assert_eq!(
                p.a, 255,
                "Cover must fill pixel ({x},{y}) with an opaque texel"
            );
        }
        // The gray placeholder (rgb 128, a 64) must NOT appear — confirm at least
        // one painted pixel is a real source color, not the 128/128/128 dot.
        let center = fb.get_pixel(20, 10);
        assert!(
            !(center.r == 128 && center.g == 128 && center.b == 128),
            "registered image must rasterize real texels, not the unloaded placeholder"
        );
    }

    #[test]
    fn cover_background_with_left_inset_bounds_covers_x0_no_strip() {
        use liquide_compositor::geometry::Affine2D;
        use liquide_compositor::scene::ImageFit;

        // Fully-opaque 4x4 red texture.
        let mut renderer = SoftwareRenderer::new();
        renderer.register_image_rgba(88, vec![255u8; 4 * 4 * 4], 4, 4);

        // Wallpaper-scale Cover node, but its layout box starts at x=50 (the
        // observed desktop-background origin quirk) on a 200x120 framebuffer.
        // Cover must still cover the framebuffer edge-to-edge — no black strip.
        let node = FlatNode {
            id: 88,
            kind: SceneNodeKind::Image {
                image_id: 88,
                width: 4,
                height: 4,
                fit: ImageFit::Cover,
            }
            .into(),
            absolute_bounds: Rect::new(50.0, 0.0, 200.0, 120.0),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        };

        let mut fb = FrameBuffer::new(200, 120, PixelFormat::Bgra8);
        let damage = full_damage();
        renderer
            .render(std::slice::from_ref(&node), &mut fb, &damage)
            .unwrap();

        // Every framebuffer pixel, including the left edge column the box did
        // NOT originally span, must be an opaque texel (the wallpaper), never
        // an uncovered (transparent/black) strip.
        for y in [0u32, 60, 119] {
            for x in [0u32, 1, 25, 49, 100, 199] {
                let p = fb.get_pixel(x, y);
                assert_eq!(
                    p.a, 255,
                    "Cover wallpaper must cover pixel ({x},{y}) — no uncovered left strip"
                );
            }
        }
    }

    #[test]
    fn cover_small_inset_image_keeps_its_bounds() {
        use liquide_compositor::geometry::Affine2D;
        use liquide_compositor::scene::ImageFit;

        // A small Cover image well inside the framebuffer must NOT be snapped to
        // the edges (only background-scale images are).
        let mut renderer = SoftwareRenderer::new();
        renderer.register_image_rgba(89, vec![255u8; 4 * 4 * 4], 4, 4);

        let node = FlatNode {
            id: 89,
            kind: SceneNodeKind::Image {
                image_id: 89,
                width: 4,
                height: 4,
                fit: ImageFit::Cover,
            }
            .into(),
            absolute_bounds: Rect::new(40.0, 40.0, 20.0, 20.0),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        };

        let mut fb = FrameBuffer::new(200, 120, PixelFormat::Bgra8);
        let damage = full_damage();
        renderer
            .render(std::slice::from_ref(&node), &mut fb, &damage)
            .unwrap();

        // Inside the box: painted.
        assert_eq!(
            fb.get_pixel(50, 50).a,
            255,
            "small image paints its own box"
        );
        // Far corner: untouched (the small image was not snapped to the edges).
        assert_eq!(
            fb.get_pixel(0, 0).a,
            0,
            "a small inset Cover image must not be expanded to the framebuffer"
        );
    }

    #[test]
    fn contain_fit_letterboxes_preserving_aspect() {
        // t144: object-fit: contain on a SQUARE source inside a WIDE box must
        // letterbox — the image is centered, scaled to fit the smaller axis
        // (height), leaving uncovered (transparent) bands on the left and right.
        use liquide_compositor::geometry::Affine2D;
        use liquide_compositor::scene::ImageFit;

        // 4x4 fully-opaque white square.
        let mut renderer = SoftwareRenderer::new();
        renderer.register_image_rgba(201, vec![255u8; 4 * 4 * 4], 4, 4);

        // 40 wide x 20 tall box. Contain scales the 1:1 source by min(40/4, 20/4)
        // = 5 → 20x20 centered → painted band x in [10,30), transparent outside.
        let node = FlatNode {
            id: 201,
            kind: SceneNodeKind::Image {
                image_id: 201,
                width: 4,
                height: 4,
                fit: ImageFit::Contain,
            }
            .into(),
            absolute_bounds: Rect::new(0.0, 0.0, 40.0, 20.0),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        };
        let mut fb = FrameBuffer::new(40, 20, PixelFormat::Bgra8);
        renderer
            .render(std::slice::from_ref(&node), &mut fb, &full_damage())
            .unwrap();

        // Center is painted (opaque white texel).
        assert_eq!(fb.get_pixel(20, 10).a, 255, "contain fills the centered area");
        // The left/right letterbox bands are uncovered (transparent) — contain
        // never stretches to fill the wide box.
        assert_eq!(
            fb.get_pixel(2, 10).a,
            0,
            "contain must leave a transparent left letterbox band"
        );
        assert_eq!(
            fb.get_pixel(37, 10).a,
            0,
            "contain must leave a transparent right letterbox band"
        );
    }

    #[test]
    fn cover_fit_crops_to_fill_without_distortion_no_transparent_edge() {
        // t144: object-fit: cover on a SQUARE source inside a WIDE box must crop
        // (scale to the LARGER axis) so the box is fully covered — every pixel an
        // opaque texel, no transparent letterbox.
        use liquide_compositor::geometry::Affine2D;
        use liquide_compositor::scene::ImageFit;

        let mut renderer = SoftwareRenderer::new();
        renderer.register_image_rgba(202, vec![255u8; 4 * 4 * 4], 4, 4);

        // Small inset box (NOT background-scale, so cover keeps its exact bounds):
        // 40 wide x 20 tall at (60,60) on a larger framebuffer.
        let node = FlatNode {
            id: 202,
            kind: SceneNodeKind::Image {
                image_id: 202,
                width: 4,
                height: 4,
                fit: ImageFit::Cover,
            }
            .into(),
            absolute_bounds: Rect::new(60.0, 60.0, 40.0, 20.0),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        };
        let mut fb = FrameBuffer::new(200, 200, PixelFormat::Bgra8);
        renderer
            .render(std::slice::from_ref(&node), &mut fb, &full_damage())
            .unwrap();

        // Every corner + center of the box is an opaque texel — cover leaves no
        // uncovered edge.
        for (x, y) in [(60u32, 60u32), (99, 60), (60, 79), (99, 79), (80, 70)] {
            assert_eq!(
                fb.get_pixel(x, y).a,
                255,
                "cover must fully cover pixel ({x},{y}) with an opaque texel"
            );
        }
        // Just outside the box stays untouched (the small image was not snapped).
        assert_eq!(fb.get_pixel(50, 70).a, 0, "outside the box is untouched");
    }

    #[test]
    fn unregistered_image_paints_placeholder_not_texels() {
        use liquide_compositor::geometry::Affine2D;
        use liquide_compositor::scene::ImageFit;

        // No register_image: the id is unknown, so the renderer must draw the
        // gray placeholder rather than nothing/garbage.
        let mut renderer = SoftwareRenderer::new();
        let node = FlatNode {
            id: 4242,
            kind: SceneNodeKind::Image {
                image_id: 4242,
                width: 2,
                height: 2,
                fit: ImageFit::Cover,
            }
            .into(),
            absolute_bounds: Rect::new(0.0, 0.0, 8.0, 8.0),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        };
        let mut fb = FrameBuffer::new(8, 8, PixelFormat::Bgra8);
        let damage = full_damage();
        renderer
            .render(std::slice::from_ref(&node), &mut fb, &damage)
            .unwrap();
        // Placeholder is a faint, semi-transparent gray fill (a=64) blended over
        // the black framebuffer — painted but neutral-gray and NOT opaque, which
        // distinguishes "image not loaded" from a real (opaque) wallpaper texel.
        let p = fb.get_pixel(1, 1);
        assert!(p.r > 0, "placeholder must be painted (non-zero)");
        assert!(
            p.a < 255,
            "placeholder must be semi-transparent, not an opaque texel"
        );
        assert_eq!(p.r, p.g, "placeholder must be neutral gray");
        assert_eq!(p.g, p.b, "placeholder must be neutral gray");
    }

    #[test]
    fn repeated_background_cached_output_matches_legacy_scaling() {
        let image_id = 99;
        let node = background_node(
            image_id,
            BackgroundRepeat::Repeat,
            BackgroundSize::Explicit {
                width: 4.0,
                height: 4.0,
            },
            Rect::new(0.0, 0.0, 8.0, 8.0),
        );

        let mut cached_renderer = SoftwareRenderer::new();
        cached_renderer.register_image_rgba(image_id, checker_rgba(), 2, 2);
        let mut cached_fb = FrameBuffer::new(8, 8, PixelFormat::Bgra8);
        let damage = full_damage();
        cached_renderer
            .render(std::slice::from_ref(&node), &mut cached_fb, &damage)
            .unwrap();

        let mut legacy_renderer = SoftwareRenderer::new();
        legacy_renderer.register_image_rgba(image_id, checker_rgba(), 2, 2);
        let texture = legacy_renderer
            .texture_cache
            .get_by_key(image_texture_key(image_id))
            .unwrap();
        let mut legacy_fb = FrameBuffer::new(8, 8, PixelFormat::Bgra8);
        legacy_renderer.draw_repeated_background_legacy(
            &mut legacy_fb,
            &texture,
            Rect::new(0.0, 0.0, 2.0, 2.0),
            Rect::new(0.0, 0.0, 4.0, 4.0),
            Rect::new(0.0, 0.0, 8.0, 8.0),
            BackgroundRepeat::Repeat,
            1.0,
        );

        assert_eq!(cached_renderer.texture_cache.pattern_len(), 1);
        assert_eq!(cached_fb.pixels(), legacy_fb.pixels());
    }

    // ── t-prim #3: SIMD blit must be byte-identical to the scalar SrcOver ──

    // Independent SCALAR reference for `draw_scaled_texture` (the pre-SIMD path).
    // Mirrors the sample math exactly but always uses the per-pixel
    // get_pixel/blend/set_pixel loop, so it cannot share a bug with the SIMD path.
    fn scalar_blit_reference(
        fb: &mut FrameBuffer,
        texture: &CachedTexture,
        src_rect: Rect,
        dst_rect: Rect,
        opacity: f32,
    ) {
        let src_x0 = src_rect.x.max(0.0) as u32;
        let src_y0 = src_rect.y.max(0.0) as u32;
        let src_x1 = (src_rect.right().min(texture.width as f32)) as u32;
        let src_y1 = (src_rect.bottom().min(texture.height as f32)) as u32;
        let dst_x0 = dst_rect.x.max(0.0);
        let dst_y0 = dst_rect.y.max(0.0);
        let dst_x1 = dst_rect.right().min(fb.width as f32);
        let dst_y1 = dst_rect.bottom().min(fb.height as f32);
        if dst_x0 >= dst_x1 || dst_y0 >= dst_y1 {
            return;
        }
        let src_w = (src_x1 - src_x0) as f32;
        let src_h = (src_y1 - src_y0) as f32;
        let dst_w = dst_x1 - dst_x0;
        let dst_h = dst_y1 - dst_y0;
        if src_w <= 0.0 || src_h <= 0.0 || dst_w <= 0.0 || dst_h <= 0.0 {
            return;
        }
        let scaled = (src_w - dst_w).abs() > 0.5 || (src_h - dst_h).abs() > 0.5;
        for dst_y in dst_y0 as u32..dst_y1.ceil() as u32 {
            for dst_x in dst_x0 as u32..dst_x1.ceil() as u32 {
                if dst_x >= fb.width || dst_y >= fb.height {
                    continue;
                }
                let rel_x = (dst_x as f32 - dst_x0) / dst_w;
                let rel_y = (dst_y as f32 - dst_y0) / dst_h;
                let mut src_color = if scaled {
                    let fx = src_x0 as f32 + rel_x * src_w - 0.5;
                    let fy = src_y0 as f32 + rel_y * src_h - 0.5;
                    let x0 = fx.floor();
                    let y0 = fy.floor();
                    let tx = fx - x0;
                    let ty = fy - y0;
                    let cx = |v: f32| -> u32 {
                        (v as i32).clamp(src_x0 as i32, src_x1 as i32 - 1) as u32
                    };
                    let cy = |v: f32| -> u32 {
                        (v as i32).clamp(src_y0 as i32, src_y1 as i32 - 1) as u32
                    };
                    let texel = |x: u32, y: u32| -> [f32; 4] {
                        let i = ((y * texture.width + x) * 4) as usize;
                        if i + 3 >= texture.data.len() {
                            return [0.0; 4];
                        }
                        [
                            texture.data[i] as f32,
                            texture.data[i + 1] as f32,
                            texture.data[i + 2] as f32,
                            texture.data[i + 3] as f32,
                        ]
                    };
                    let c00 = texel(cx(x0), cy(y0));
                    let c10 = texel(cx(x0 + 1.0), cy(y0));
                    let c01 = texel(cx(x0), cy(y0 + 1.0));
                    let c11 = texel(cx(x0 + 1.0), cy(y0 + 1.0));
                    let mut out = [0.0f32; 4];
                    for k in 0..4 {
                        let top = c00[k] + (c10[k] - c00[k]) * tx;
                        let bot = c01[k] + (c11[k] - c01[k]) * tx;
                        out[k] = top + (bot - top) * ty;
                    }
                    Color::new(
                        (out[0] + 0.5) as u8,
                        (out[1] + 0.5) as u8,
                        (out[2] + 0.5) as u8,
                        (out[3] + 0.5) as u8,
                    )
                } else {
                    let src_x = (src_x0 as f32 + rel_x * src_w) as u32;
                    let src_y = (src_y0 as f32 + rel_y * src_h) as u32;
                    let src_idx = ((src_y * texture.width + src_x) * 4) as usize;
                    if src_idx + 3 >= texture.data.len() {
                        continue;
                    }
                    Color::new(
                        texture.data[src_idx],
                        texture.data[src_idx + 1],
                        texture.data[src_idx + 2],
                        texture.data[src_idx + 3],
                    )
                };
                if opacity < 1.0 {
                    src_color.a = (src_color.a as f32 * opacity + 0.5) as u8;
                }
                src_color = src_color.premultiply();
                let dst_color = fb.get_pixel(dst_x, dst_y);
                let blended = crate::blend::blend(dst_color, src_color, BlendMode::SrcOver);
                fb.set_pixel(dst_x, dst_y, blended);
            }
        }
    }

    fn gradient_dst(w: u32, h: u32) -> FrameBuffer {
        // A non-trivial dst so SrcOver over partial alpha differences would show.
        let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        for y in 0..h {
            for x in 0..w {
                fb.set_pixel(
                    x,
                    y,
                    Color::new((x * 7 % 256) as u8, (y * 11 % 256) as u8, 90, 255),
                );
            }
        }
        fb
    }

    fn semi_texture(renderer: &mut SoftwareRenderer, id: u64, w: u32, h: u32) -> CachedTexture {
        // RGBA with varying alpha so premultiply + SrcOver matter.
        let mut data = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                data.push((x * 17 % 256) as u8);
                data.push((y * 23 % 256) as u8);
                data.push(((x + y) * 5 % 256) as u8);
                data.push((40 + (x * y) % 200) as u8); // alpha 40..239
            }
        }
        renderer.register_image_rgba(id, data, w, h);
        renderer
            .texture_cache
            .get_by_key(image_texture_key(id))
            .unwrap()
    }

    #[test]
    fn simd_blit_byte_identical_to_scalar_unscaled() {
        let mut r = SoftwareRenderer::new();
        let tex = semi_texture(&mut r, 301, 16, 9);
        let src = Rect::new(0.0, 0.0, 16.0, 9.0);
        let dst = Rect::new(2.0, 1.0, 16.0, 9.0); // 1:1 → nearest

        let mut simd_fb = gradient_dst(24, 16);
        r.draw_scaled_texture(&mut simd_fb, &tex, src, dst, 1.0);

        let mut scalar_fb = gradient_dst(24, 16);
        scalar_blit_reference(&mut scalar_fb, &tex, src, dst, 1.0);

        assert_eq!(
            simd_fb.pixels(),
            scalar_fb.pixels(),
            "SIMD blit (nearest) must equal the scalar SrcOver byte-for-byte"
        );
    }

    #[test]
    fn simd_blit_byte_identical_to_scalar_scaled_and_opacity() {
        let mut r = SoftwareRenderer::new();
        let tex = semi_texture(&mut r, 302, 8, 8);
        let src = Rect::new(0.0, 0.0, 8.0, 8.0);
        let dst = Rect::new(0.0, 0.0, 30.0, 22.0); // scaled → bilinear

        let mut simd_fb = gradient_dst(32, 24);
        r.draw_scaled_texture(&mut simd_fb, &tex, src, dst, 0.6);

        let mut scalar_fb = gradient_dst(32, 24);
        scalar_blit_reference(&mut scalar_fb, &tex, src, dst, 0.6);

        assert_eq!(
            simd_fb.pixels(),
            scalar_fb.pixels(),
            "SIMD blit (bilinear + opacity) must equal the scalar SrcOver byte-for-byte"
        );
    }

    // ── t-prim #4: background-repeat space / round ──────────────────────

    #[test]
    fn background_repeat_space_leaves_gaps() {
        // 4x4 opaque tile, box 15 wide → floor(15/4)=3 tiles=12px, 3px spread over
        // 2 gaps = 1.5px each (≥1px → at least one fully-empty column).
        let mut r = SoftwareRenderer::new();
        r.register_image_rgba(401, vec![255u8; 4 * 4 * 4], 4, 4);
        let node = background_node(
            401,
            BackgroundRepeat::Space,
            BackgroundSize::Explicit { width: 4.0, height: 4.0 },
            Rect::new(0.0, 0.0, 15.0, 4.0),
        );
        let mut fb = FrameBuffer::new(15, 4, PixelFormat::Bgra8);
        r.render(std::slice::from_ref(&node), &mut fb, &full_damage())
            .unwrap();

        // First tile flush at x=0 painted.
        assert_eq!(fb.get_pixel(0, 1).a, 255, "space: first tile flush-left");
        // There must be at least one fully-unpainted (gap) column between tiles.
        let gap_cols = (0..15).filter(|&x| (0..4).all(|y| fb.get_pixel(x, y).a == 0)).count();
        assert!(
            gap_cols > 0,
            "background-repeat: space must leave gap columns between tiles (found {gap_cols})"
        );
    }

    #[test]
    fn background_repeat_space_differs_from_plain_repeat() {
        // Teeth: plain Repeat fills edge-to-edge with NO gaps; Space must differ.
        let mut r = SoftwareRenderer::new();
        r.register_image_rgba(402, vec![255u8; 4 * 4 * 4], 4, 4);
        let mk = |rep| {
            background_node(
                402,
                rep,
                BackgroundSize::Explicit { width: 4.0, height: 4.0 },
                Rect::new(0.0, 0.0, 15.0, 4.0),
            )
        };
        let mut rep_fb = FrameBuffer::new(15, 4, PixelFormat::Bgra8);
        r.render(std::slice::from_ref(&mk(BackgroundRepeat::Repeat)), &mut rep_fb, &full_damage())
            .unwrap();
        let mut space_fb = FrameBuffer::new(15, 4, PixelFormat::Bgra8);
        r.render(std::slice::from_ref(&mk(BackgroundRepeat::Space)), &mut space_fb, &full_damage())
            .unwrap();
        assert_ne!(
            rep_fb.pixels(),
            space_fb.pixels(),
            "space must differ from plain repeat (gaps)"
        );
        let rep_gaps = (0..15).filter(|&x| (0..4).all(|y| rep_fb.get_pixel(x, y).a == 0)).count();
        assert_eq!(rep_gaps, 0, "plain repeat must leave no gaps");
    }

    #[test]
    fn background_repeat_round_scales_to_whole_tiles() {
        // Box 18 wide, natural tile 4 → 18/4=4.5 → round to 4 tiles, each 4.5px.
        // Result fills edge-to-edge with NO gaps (unlike plain repeat which would
        // leave a clipped partial tile but still no transparent gaps; the key
        // round property is exact whole-tile fit). Assert full coverage + the
        // right edge is painted (a whole tile ends exactly at the box edge).
        let mut r = SoftwareRenderer::new();
        r.register_image_rgba(403, vec![255u8; 4 * 4 * 4], 4, 4);
        let node = background_node(
            403,
            BackgroundRepeat::Round,
            BackgroundSize::Explicit { width: 4.0, height: 4.0 },
            Rect::new(0.0, 0.0, 18.0, 4.0),
        );
        let mut fb = FrameBuffer::new(18, 4, PixelFormat::Bgra8);
        r.render(std::slice::from_ref(&node), &mut fb, &full_damage())
            .unwrap();
        // Full coverage, no gaps, including the far edge.
        for x in [0u32, 8, 17] {
            assert_eq!(fb.get_pixel(x, 1).a, 255, "round must fully cover x={x}");
        }
        let gap_cols = (0..18).filter(|&x| (0..4).all(|y| fb.get_pixel(x, y).a == 0)).count();
        assert_eq!(gap_cols, 0, "round leaves no gaps (whole-tile fit)");
    }

    #[test]
    fn round_axis_picks_whole_tile_count() {
        // 20 / 6 = 3.33 → nearest whole count is 3, each tile resized to ~6.67px.
        let (size, count) = SoftwareRenderer::round_axis(20.0, 6.0);
        assert_eq!(count, 3);
        assert!((size * count as f32 - 20.0).abs() < 1e-3, "tiles fill exactly: {size}");
        // 22 / 4 = 5.5 → rounds up to 6 (round-half-away-from-zero), exact fit.
        let (size6, count6) = SoftwareRenderer::round_axis(22.0, 4.0);
        assert_eq!(count6, 6);
        assert!((size6 * count6 as f32 - 22.0).abs() < 1e-3);
    }

    #[test]
    fn space_axis_distributes_even_gaps() {
        // 22 / 4 → 5 tiles (20px), 2px leftover over 4 gaps = 0.5px each.
        let (gap, count) = SoftwareRenderer::space_axis(22.0, 4.0);
        assert_eq!(count, 5);
        assert!((gap - 0.5).abs() < 1e-4, "even gap: {gap}");
        // A single tile (box smaller than 2 tiles) → no gap.
        let (g1, c1) = SoftwareRenderer::space_axis(5.0, 4.0);
        assert_eq!(c1, 1);
        assert_eq!(g1, 0.0);
    }

    // ── t-prim #5: border-image 9-slice keeps corners unstretched ───────

    fn border_image_node(image_id: u64, bounds: Rect, slice: f32, width: f32) -> FlatNode {
        use liquide_compositor::scene::{BorderImageRepeat, BorderImageSpec};
        FlatNode {
            id: image_id,
            kind: SceneNodeKind::BorderImage {
                spec: BorderImageSpec {
                    source: BackgroundImage::ImageId(image_id),
                    slice: (slice, slice, slice, slice),
                    width: (width, width, width, width),
                    outset: (0.0, 0.0, 0.0, 0.0),
                    repeat: BorderImageRepeat::Stretch,
                },
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

    // Build a 9x9 source as a 3x3 grid of SOLID 3x3 colour blocks. With a 33.33%
    // slice each corner slice is a full solid 3x3 block, so corner sampling is a
    // pure colour regardless of sub-pixel slice rounding (robust test).
    //
    //   TL(red)    T(yellow)  TR(green)
    //   L(cyan)    C(white)   R(magenta)
    //   BL(blue)   B(gray)    BR(dark)
    fn nine_slice_source() -> Vec<u8> {
        let block = |bx: u32, by: u32| -> [u8; 4] {
            match (bx, by) {
                (0, 0) => [255, 0, 0, 255],     // TL red
                (1, 0) => [255, 255, 0, 255],   // T yellow
                (2, 0) => [0, 255, 0, 255],     // TR green
                (0, 1) => [0, 255, 255, 255],   // L cyan
                (1, 1) => [255, 255, 255, 255], // C white
                (2, 1) => [255, 0, 255, 255],   // R magenta
                (0, 2) => [0, 0, 255, 255],     // BL blue
                (1, 2) => [128, 128, 128, 255], // B gray
                _ => [20, 20, 20, 255],         // BR dark
            }
        };
        let mut data = Vec::with_capacity(9 * 9 * 4);
        for y in 0..9u32 {
            for x in 0..9u32 {
                data.extend_from_slice(&block(x / 3, y / 3));
            }
        }
        data
    }

    #[test]
    fn border_image_nine_slice_paints_corners_and_skips_center() {
        let mut r = SoftwareRenderer::new();
        r.register_image_rgba(501, nine_slice_source(), 9, 9);
        // Box 30x30, slice ~33.33% (1 of 3 texels), border width 6px.
        let node = border_image_node(501, Rect::new(0.0, 0.0, 30.0, 30.0), 33.3334, 6.0);
        let mut fb = FrameBuffer::new(30, 30, PixelFormat::Bgra8);
        r.render(std::slice::from_ref(&node), &mut fb, &full_damage())
            .unwrap();

        // Corners carry their distinct source texel colours (corners are NOT
        // stretched away — they occupy the 6px corner boxes).
        let tl = fb.get_pixel(1, 1);
        assert_eq!((tl.r, tl.g, tl.b), (255, 0, 0), "TL corner = red, got {tl:?}");
        let tr = fb.get_pixel(28, 1);
        assert_eq!((tr.r, tr.g, tr.b), (0, 255, 0), "TR corner = green, got {tr:?}");
        let bl = fb.get_pixel(1, 28);
        assert_eq!((bl.r, bl.g, bl.b), (0, 0, 255), "BL corner = blue, got {bl:?}");

        // The CENTER is NOT filled (no fill keyword) → transparent.
        let center = fb.get_pixel(15, 15);
        assert_eq!(center.a, 0, "border-image center must stay unfilled, got {center:?}");
    }

    #[test]
    fn border_image_nine_slice_differs_from_plain_stretch() {
        // Teeth: a plain full-image stretch would put the WHITE center texel in the
        // middle and would NOT keep the corner colours in the corners. Compare the
        // 9-slice render to a naive stretch of the whole image; they must differ,
        // and the 9-slice corner must be the corner colour (not an interpolation).
        let mut r = SoftwareRenderer::new();
        r.register_image_rgba(502, nine_slice_source(), 9, 9);
        let node = border_image_node(502, Rect::new(0.0, 0.0, 30.0, 30.0), 33.3334, 6.0);
        let mut sliced = FrameBuffer::new(30, 30, PixelFormat::Bgra8);
        r.render(std::slice::from_ref(&node), &mut sliced, &full_damage())
            .unwrap();

        // Naive full-image stretch reference.
        let tex = r.texture_cache.get_by_key(image_texture_key(502)).unwrap();
        let mut stretched = FrameBuffer::new(30, 30, PixelFormat::Bgra8);
        r.draw_scaled_texture(
            &mut stretched,
            &tex,
            Rect::new(0.0, 0.0, 9.0, 9.0),
            Rect::new(0.0, 0.0, 30.0, 30.0),
            1.0,
        );

        assert_ne!(
            sliced.pixels(),
            stretched.pixels(),
            "9-slice must differ from a plain stretch"
        );
        // Stretch puts white (center texel, bilinearly mixed) in the middle;
        // 9-slice leaves the middle transparent.
        assert_eq!(sliced.get_pixel(15, 15).a, 0);
        assert!(stretched.get_pixel(15, 15).a > 0, "stretch fills the middle");
    }
}
