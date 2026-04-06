//! Image and surface rendering for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{FlatNode, SceneNodeKind};

use crate::rasterizer;

use crate::texture_cache::image_texture_key;

use super::SoftwareRenderer;

impl SoftwareRenderer {
    /// Render an Image scene node.
    pub(crate) fn render_image_node(
        &mut self,
        node: &FlatNode,
        fb: &mut FrameBuffer,
    ) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let SceneNodeKind::Image {
            image_id,
            width,
            height,
            fit,
        } = &node.kind
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
                            Rect::new(
                                bounds.x + offset_x,
                                bounds.y + offset_y,
                                scaled_w,
                                scaled_h,
                            ),
                        )
                    }
                    liquide_compositor::scene::ImageFit::Cover => {
                        let scale = (dst_w / src_w).max(dst_h / src_h);
                        let scaled_w = src_w * scale;
                        let scaled_h = src_h * scale;
                        let crop_x = ((scaled_w - dst_w) / 2.0) / scale;
                        let crop_y = ((scaled_h - dst_h) / 2.0) / scale;
                        (
                            Rect::new(
                                crop_x,
                                crop_y,
                                src_w - crop_x * 2.0,
                                src_h - crop_y * 2.0,
                            ),
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
    pub(crate) fn render_background_fill_node(
        &mut self,
        node: &FlatNode,
        fb: &mut FrameBuffer,
    ) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let SceneNodeKind::BackgroundFill { background } = &node.kind {
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
                                    self.draw_scaled_texture(
                                        fb, &texture, src, img_rect, opacity,
                                    );
                                }
                                BackgroundRepeat::Repeat
                                | BackgroundRepeat::Space
                                | BackgroundRepeat::Round => {
                                    let mut ty = bounds.y;
                                    while ty < bounds.y + bounds.height {
                                        let mut tx = bounds.x;
                                        while tx < bounds.x + bounds.width {
                                            let tile = Rect::new(
                                                tx,
                                                ty,
                                                img_rect.width,
                                                img_rect.height,
                                            );
                                            self.draw_scaled_texture(
                                                fb, &texture, src, tile, opacity,
                                            );
                                            tx += img_rect.width;
                                        }
                                        ty += img_rect.height;
                                    }
                                }
                                BackgroundRepeat::RepeatX => {
                                    let mut tx = bounds.x;
                                    while tx < bounds.x + bounds.width {
                                        let tile = Rect::new(
                                            tx,
                                            img_rect.y,
                                            img_rect.width,
                                            img_rect.height,
                                        );
                                        self.draw_scaled_texture(
                                            fb, &texture, src, tile, opacity,
                                        );
                                        tx += img_rect.width;
                                    }
                                }
                                BackgroundRepeat::RepeatY => {
                                    let mut ty = bounds.y;
                                    while ty < bounds.y + bounds.height {
                                        let tile = Rect::new(
                                            img_rect.x,
                                            ty,
                                            img_rect.width,
                                            img_rect.height,
                                        );
                                        self.draw_scaled_texture(
                                            fb, &texture, src, tile, opacity,
                                        );
                                        ty += img_rect.height;
                                    }
                                }
                            }
                        }
                    }
                    BackgroundImage::Url(_) => {} // External URLs unsupported
                }
            }
        }
    }

    /// Render a BorderImage scene node.
    pub(crate) fn render_border_image_node(
        &mut self,
        node: &FlatNode,
        fb: &mut FrameBuffer,
    ) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        if let SceneNodeKind::BorderImage { spec } = &node.kind {
            use liquide_compositor::scene::BackgroundImage;
            match &spec.source {
                BackgroundImage::ImageId(image_id) => {
                    let texture_key = image_texture_key(*image_id);
                    if let Some(texture) = self.texture_cache.get_by_key(texture_key) {
                        let src =
                            Rect::new(0.0, 0.0, texture.width as f32, texture.height as f32);
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
