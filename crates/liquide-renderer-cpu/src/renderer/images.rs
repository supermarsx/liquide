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
                        let scale = (dst_w / src_w).max(dst_h / src_h);
                        let scaled_w = src_w * scale;
                        let scaled_h = src_h * scale;
                        let crop_x = ((scaled_w - dst_w) / 2.0) / scale;
                        let crop_y = ((scaled_h - dst_h) / 2.0) / scale;
                        (
                            Rect::new(crop_x, crop_y, src_w - crop_x * 2.0, src_h - crop_y * 2.0),
                            bounds,
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
            BackgroundRepeat::Repeat | BackgroundRepeat::Space | BackgroundRepeat::Round => {
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

        for dst_y in 0..tile_height {
            let rel_y = dst_y as f32 / dst_h;
            let src_y = (src_y0 as f32 + rel_y * src_h) as u32;
            for dst_x in 0..tile_width {
                let rel_x = dst_x as f32 / dst_w;
                let src_x = (src_x0 as f32 + rel_x * src_w) as u32;
                let src_idx = ((src_y * texture.width + src_x) * 4) as usize;
                if src_idx + 3 >= texture.data.len() {
                    continue;
                }

                let dst_idx = ((dst_y * tile_width + dst_x) * 4) as usize;
                pixels[dst_idx] = texture.data[src_idx];
                pixels[dst_idx + 1] = texture.data[src_idx + 1];
                pixels[dst_idx + 2] = texture.data[src_idx + 2];
                pixels[dst_idx + 3] = texture.data[src_idx + 3];
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
        for y in 0..tile_texture.height {
            for x in 0..tile_texture.width {
                let src_idx = ((y * tile_texture.width + x) * 4) as usize;
                if src_idx + 3 >= tile_texture.data.len() {
                    continue;
                }

                let mut src_color = Color::new(
                    tile_texture.data[src_idx],
                    tile_texture.data[src_idx + 1],
                    tile_texture.data[src_idx + 2],
                    tile_texture.data[src_idx + 3],
                );
                if src_color.a == 0 {
                    continue;
                }

                if opacity < 1.0 {
                    src_color.a = (src_color.a as f32 * opacity + 0.5) as u8;
                }

                src_color = src_color.premultiply();
                let px = dst_x + x;
                let py = dst_y + y;
                let dst_color = fb.get_pixel(px, py);
                let blended = crate::blend::blend(dst_color, src_color, BlendMode::SrcOver);
                fb.set_pixel(px, py, blended);
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
            BackgroundRepeat::Repeat | BackgroundRepeat::Space | BackgroundRepeat::Round => {
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
                        let src = Rect::new(0.0, 0.0, texture.width as f32, texture.height as f32);
                        self.draw_scaled_texture(fb, &texture, src, bounds, opacity);
                    }
                }
                BackgroundImage::Gradient(gradient) => {
                    self.render_gradient(fb, bounds, gradient, opacity, node.corner_radius);
                }
                _ => {}
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

        // Nearest-neighbor scaling
        for dst_y in (dst_y0 as u32)..(dst_y1.ceil() as u32) {
            for dst_x in (dst_x0 as u32)..(dst_x1.ceil() as u32) {
                let rel_x = (dst_x as f32 - dst_x0) / dst_w;
                let rel_y = (dst_y as f32 - dst_y0) / dst_h;
                let src_x = (src_x0 as f32 + rel_x * src_w) as u32;
                let src_y = (src_y0 as f32 + rel_y * src_h) as u32;

                let src_idx = ((src_y * texture.width + src_x) * 4) as usize;
                if src_idx + 3 >= texture.data.len() {
                    continue;
                }

                let mut src_color = Color::new(
                    texture.data[src_idx],
                    texture.data[src_idx + 1],
                    texture.data[src_idx + 2],
                    texture.data[src_idx + 3],
                );

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::Renderer;
    use liquide_compositor::damage::{DamageClass, DamageSet, DamageTile};
    use liquide_compositor::geometry::Affine2D;
    use liquide_compositor::pixel::PixelFormat;
    use liquide_compositor::scene::{BackgroundImage, BackgroundSize, BackgroundSpec};

    fn full_damage() -> DamageSet {
        let mut damage = DamageSet::new(64);
        damage.add(DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::UiPrimitive,
        });
        damage
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
}
