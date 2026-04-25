//! LayerCompositor — composites a flattened layer tree to an output framebuffer.
//!
//! Walks the draw commands from [`flatten`], blits each layer's pixels with
//! the appropriate transform, opacity, clip, and blend mode. Implements
//! occlusion culling to skip fully hidden layers.

use crate::draw_cmd::{LayerDrawCmd, flatten};
use crate::layer::{BlendMode, Rect};
use crate::tree::LayerTree;

/// Tracks opaque regions on screen to detect fully occluded layers.
///
/// Opaque regions are accumulated back-to-front. If a subsequent layer's
/// screen rect is entirely covered by previously-composited opaque regions,
/// we can skip it entirely.
#[derive(Debug, Clone)]
pub struct OcclusionTracker {
    /// List of opaque rectangles that have been composited so far.
    /// Stored in front-to-back order (most recent first).
    opaque_rects: Vec<Rect>,
    /// Maximum number of rects to track before giving up on occlusion
    /// culling (to bound the O(N*M) check).
    max_rects: usize,
}

impl OcclusionTracker {
    /// Create a new tracker with a default limit.
    #[must_use]
    pub fn new() -> Self {
        Self {
            opaque_rects: Vec::new(),
            max_rects: 256,
        }
    }

    /// Create a tracker with a custom rectangle limit.
    #[must_use]
    pub fn with_limit(max_rects: usize) -> Self {
        Self {
            opaque_rects: Vec::new(),
            max_rects,
        }
    }

    /// Reset the tracker for a new frame.
    pub fn reset(&mut self) {
        self.opaque_rects.clear();
    }

    /// Check whether a rectangle is fully occluded by previously-recorded
    /// opaque regions.
    ///
    /// This uses a simple check: the rect must be fully contained within
    /// a single opaque rect. A more sophisticated implementation would
    /// handle the case where multiple opaque rects together cover the
    /// candidate, but that's O(N!) in the worst case.
    #[must_use]
    pub fn is_fully_occluded(&self, rect: &Rect) -> bool {
        for opaque in &self.opaque_rects {
            if opaque.contains_rect(rect) {
                return true;
            }
        }
        false
    }

    /// Record an opaque region that has been composited.
    pub fn add_opaque_rect(&mut self, rect: Rect) {
        if self.opaque_rects.len() < self.max_rects {
            self.opaque_rects.push(rect);
        }
    }

    /// Number of opaque rects currently tracked.
    #[must_use]
    pub fn rect_count(&self) -> usize {
        self.opaque_rects.len()
    }
}

impl Default for OcclusionTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of compositing a single frame.
#[derive(Debug, Clone, Default)]
pub struct CompositeStats {
    /// Total number of draw commands from flatten.
    pub total_commands: usize,
    /// Number of layers actually drawn (after occlusion culling).
    pub drawn: usize,
    /// Number of layers skipped due to occlusion.
    pub occluded: usize,
    /// Number of layers skipped because they have no pixel data.
    pub skipped_no_pixels: usize,
}

/// Composites a layer tree to an RGBA output buffer.
pub struct LayerCompositor {
    /// Occlusion tracker reused across frames.
    pub occlusion: OcclusionTracker,
}

impl LayerCompositor {
    /// Create a new compositor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            occlusion: OcclusionTracker::new(),
        }
    }

    /// Composite the layer tree into the output buffer.
    ///
    /// `output` must be `width * height * 4` bytes (RGBA).
    /// The buffer is NOT cleared — call `clear_output` first if needed.
    pub fn composite(
        &mut self,
        tree: &LayerTree,
        output: &mut [u8],
        width: u32,
        height: u32,
    ) -> CompositeStats {
        let viewport = Rect::new(0.0, 0.0, width as f32, height as f32);
        let commands = flatten(tree, viewport);
        self.composite_commands(&commands, tree, output, width, height)
    }

    /// Composite from pre-flattened draw commands.
    pub fn composite_commands(
        &mut self,
        commands: &[LayerDrawCmd],
        tree: &LayerTree,
        output: &mut [u8],
        width: u32,
        height: u32,
    ) -> CompositeStats {
        self.occlusion.reset();
        let mut stats = CompositeStats {
            total_commands: commands.len(),
            ..Default::default()
        };

        // We process commands back-to-front for correct alpha compositing.
        // However, occlusion culling works front-to-back. So we do two passes:
        //
        // Pass 1: Walk front-to-back to mark fully occluded layers.
        // Pass 2: Walk back-to-front to composite visible layers.
        //
        // For simplicity and to avoid allocating a second Vec, we use
        // a single back-to-front pass and skip occlusion when it's too
        // expensive.

        let viewport = Rect::new(0.0, 0.0, width as f32, height as f32);

        // Build occlusion map front-to-back.
        let mut occluded = vec![false; commands.len()];
        for i in (0..commands.len()).rev() {
            let cmd = &commands[i];
            let visible_rect = match cmd.clip {
                Some(clip) => match cmd.screen_rect.intersection(&clip) {
                    Some(r) => r,
                    None => {
                        occluded[i] = true;
                        continue;
                    }
                },
                None => cmd.screen_rect,
            };

            if self.occlusion.is_fully_occluded(&visible_rect) {
                occluded[i] = true;
                stats.occluded += 1;
                continue;
            }

            // If this layer is fully opaque and has identity blend, it will
            // cover everything behind it.
            let layer = tree.get(cmd.layer_id);
            let is_fully_opaque = cmd.opacity >= 1.0 - f32::EPSILON
                && cmd.blend_mode == BlendMode::SrcOver
                && layer.map_or(false, |l| l.pixels.is_some());
            if is_fully_opaque {
                if let Some(clip) = cmd.clip {
                    if let Some(clipped) = visible_rect.intersection(&clip) {
                        self.occlusion.add_opaque_rect(clipped);
                    }
                } else {
                    self.occlusion.add_opaque_rect(visible_rect);
                }
            }
        }

        // Composite back-to-front (painters algorithm).
        for (i, cmd) in commands.iter().enumerate() {
            if occluded[i] {
                continue;
            }

            let layer = match tree.get(cmd.layer_id) {
                Some(l) => l,
                None => continue,
            };

            let pixels = match &layer.pixels {
                Some(p) => p,
                None => {
                    stats.skipped_no_pixels += 1;
                    continue;
                }
            };

            blit_layer(cmd, pixels, &layer.bounds, output, width, height, &viewport);
            stats.drawn += 1;
        }

        stats
    }
}

impl Default for LayerCompositor {
    fn default() -> Self {
        Self::new()
    }
}

/// Clear the output buffer to opaque black.
pub fn clear_output(output: &mut [u8]) {
    // RGBA: (0, 0, 0, 255) — opaque black.
    let len = output.len();
    let mut i = 0;
    while i + 3 < len {
        output[i] = 0;
        output[i + 1] = 0;
        output[i + 2] = 0;
        output[i + 3] = 255;
        i += 4;
    }
}

/// Clear the output buffer to a solid RGBA color.
pub fn clear_output_color(output: &mut [u8], r: u8, g: u8, b: u8, a: u8) {
    let len = output.len();
    let mut i = 0;
    while i + 3 < len {
        output[i] = r;
        output[i + 1] = g;
        output[i + 2] = b;
        output[i + 3] = a;
        i += 4;
    }
}

/// Blit a single layer into the output buffer, applying the draw command's
/// transform, opacity, clip, and blend mode.
fn blit_layer(
    cmd: &LayerDrawCmd,
    src_pixels: &[u8],
    src_bounds: &Rect,
    output: &mut [u8],
    out_width: u32,
    out_height: u32,
    viewport: &Rect,
) {
    let src_w = src_bounds.width.ceil() as u32;
    let src_h = src_bounds.height.ceil() as u32;
    if src_w == 0 || src_h == 0 {
        return;
    }

    // Determine the screen-space destination rectangle.
    let dst_rect = &cmd.screen_rect;

    // Clip the destination to viewport and command clip.
    let mut clip = match cmd.clip {
        Some(c) => match dst_rect.intersection(&c) {
            Some(r) => r,
            None => return,
        },
        None => *dst_rect,
    };
    clip = match clip.intersection(viewport) {
        Some(r) => r,
        None => return,
    };

    // Check if the transform is an axis-aligned translation (fast path).
    let [a, b, c, d, tx, ty] = cmd.transform;
    let is_simple = (a - 1.0).abs() < f32::EPSILON
        && b.abs() < f32::EPSILON
        && c.abs() < f32::EPSILON
        && (d - 1.0).abs() < f32::EPSILON;

    if is_simple {
        blit_translated(
            src_pixels,
            src_w,
            src_h,
            tx,
            ty,
            &clip,
            cmd.opacity,
            cmd.blend_mode,
            output,
            out_width,
            out_height,
        );
    } else {
        blit_transformed(
            src_pixels,
            src_w,
            src_h,
            &cmd.transform,
            &clip,
            cmd.opacity,
            cmd.blend_mode,
            output,
            out_width,
            out_height,
        );
    }
}

/// Fast path: blit with translation only (no rotation/scale/skew).
fn blit_translated(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    tx: f32,
    ty: f32,
    clip: &Rect,
    opacity: f32,
    blend_mode: BlendMode,
    output: &mut [u8],
    out_w: u32,
    out_h: u32,
) {
    let dst_x_start = clip.x.floor().max(0.0) as i32;
    let dst_y_start = clip.y.floor().max(0.0) as i32;
    let dst_x_end = clip.right().ceil().min(out_w as f32) as i32;
    let dst_y_end = clip.bottom().ceil().min(out_h as f32) as i32;

    let alpha = (opacity * 255.0).round() as u32;

    for dst_y in dst_y_start..dst_y_end {
        let src_y = (dst_y as f32 - ty).round() as i32;
        if src_y < 0 || src_y >= src_h as i32 {
            continue;
        }

        for dst_x in dst_x_start..dst_x_end {
            let src_x = (dst_x as f32 - tx).round() as i32;
            if src_x < 0 || src_x >= src_w as i32 {
                continue;
            }

            let src_off = (src_y as u32 * src_w + src_x as u32) as usize * 4;
            let dst_off = (dst_y as u32 * out_w + dst_x as u32) as usize * 4;

            if src_off + 3 >= src.len() || dst_off + 3 >= output.len() {
                continue;
            }

            let sr = src[src_off] as u32;
            let sg = src[src_off + 1] as u32;
            let sb = src[src_off + 2] as u32;
            let sa = (src[src_off + 3] as u32 * alpha + 127) / 255;

            if sa == 0 {
                continue;
            }

            blend_pixel(
                sr,
                sg,
                sb,
                sa,
                &mut output[dst_off..dst_off + 4],
                blend_mode,
            );
        }
    }
}

/// Slow path: blit with full affine transform (rotation, scale, skew).
fn blit_transformed(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    transform: &[f32; 6],
    clip: &Rect,
    opacity: f32,
    blend_mode: BlendMode,
    output: &mut [u8],
    out_w: u32,
    out_h: u32,
) {
    let dst_x_start = clip.x.floor().max(0.0) as i32;
    let dst_y_start = clip.y.floor().max(0.0) as i32;
    let dst_x_end = clip.right().ceil().min(out_w as f32) as i32;
    let dst_y_end = clip.bottom().ceil().min(out_h as f32) as i32;

    // Compute inverse transform for sampling source pixels.
    let inv = invert_affine(transform);

    let alpha = (opacity * 255.0).round() as u32;

    for dst_y in dst_y_start..dst_y_end {
        for dst_x in dst_x_start..dst_x_end {
            let fx = dst_x as f32 + 0.5;
            let fy = dst_y as f32 + 0.5;

            // Map screen coord back to source layer space.
            let sx = inv[0] * fx + inv[1] * fy + inv[4];
            let sy = inv[2] * fx + inv[3] * fy + inv[5];

            let src_x = sx.floor() as i32;
            let src_y = sy.floor() as i32;

            if src_x < 0 || src_y < 0 || src_x >= src_w as i32 || src_y >= src_h as i32 {
                continue;
            }

            let src_off = (src_y as u32 * src_w + src_x as u32) as usize * 4;
            let dst_off = (dst_y as u32 * out_w + dst_x as u32) as usize * 4;

            if src_off + 3 >= src.len() || dst_off + 3 >= output.len() {
                continue;
            }

            let sr = src[src_off] as u32;
            let sg = src[src_off + 1] as u32;
            let sb = src[src_off + 2] as u32;
            let sa = (src[src_off + 3] as u32 * alpha + 127) / 255;

            if sa == 0 {
                continue;
            }

            blend_pixel(
                sr,
                sg,
                sb,
                sa,
                &mut output[dst_off..dst_off + 4],
                blend_mode,
            );
        }
    }
}

/// Blend a single source pixel into the destination pixel using the
/// given blend mode. Source alpha is pre-applied to `sa`.
fn blend_pixel(sr: u32, sg: u32, sb: u32, sa: u32, dst: &mut [u8], mode: BlendMode) {
    let dr = dst[0] as u32;
    let dg = dst[1] as u32;
    let db = dst[2] as u32;
    let da = dst[3] as u32;

    match mode {
        BlendMode::Src => {
            dst[0] = (sr * sa / 255) as u8;
            dst[1] = (sg * sa / 255) as u8;
            dst[2] = (sb * sa / 255) as u8;
            dst[3] = sa as u8;
        }
        BlendMode::SrcOver => {
            // Porter-Duff source-over: out = src + dst * (1 - src_alpha)
            let inv_sa = 255 - sa;
            dst[0] = ((sr * sa + dr * inv_sa + 127) / 255) as u8;
            dst[1] = ((sg * sa + dg * inv_sa + 127) / 255) as u8;
            dst[2] = ((sb * sa + db * inv_sa + 127) / 255) as u8;
            dst[3] = ((sa * 255 + da * inv_sa + 127) / 255) as u8;
        }
        BlendMode::Multiply => {
            let br = (sr * dr + 127) / 255;
            let bg = (sg * dg + 127) / 255;
            let bb = (sb * db + 127) / 255;
            let inv_sa = 255 - sa;
            dst[0] = ((br * sa + dr * inv_sa + 127) / 255) as u8;
            dst[1] = ((bg * sa + dg * inv_sa + 127) / 255) as u8;
            dst[2] = ((bb * sa + db * inv_sa + 127) / 255) as u8;
            dst[3] = ((sa * 255 + da * (255 - sa) + 127) / 255) as u8;
        }
        BlendMode::Screen => {
            let br = sr + dr - (sr * dr + 127) / 255;
            let bg = sg + dg - (sg * dg + 127) / 255;
            let bb = sb + db - (sb * db + 127) / 255;
            let inv_sa = 255 - sa;
            dst[0] = ((br * sa + dr * inv_sa + 127) / 255).min(255) as u8;
            dst[1] = ((bg * sa + dg * inv_sa + 127) / 255).min(255) as u8;
            dst[2] = ((bb * sa + db * inv_sa + 127) / 255).min(255) as u8;
            dst[3] = ((sa * 255 + da * (255 - sa) + 127) / 255) as u8;
        }
        BlendMode::Overlay => {
            fn overlay_channel(s: u32, d: u32) -> u32 {
                if d < 128 {
                    (2 * s * d + 127) / 255
                } else {
                    255 - (2 * (255 - s) * (255 - d) + 127) / 255
                }
            }
            let br = overlay_channel(sr, dr);
            let bg = overlay_channel(sg, dg);
            let bb = overlay_channel(sb, db);
            let inv_sa = 255 - sa;
            dst[0] = ((br * sa + dr * inv_sa + 127) / 255) as u8;
            dst[1] = ((bg * sa + dg * inv_sa + 127) / 255) as u8;
            dst[2] = ((bb * sa + db * inv_sa + 127) / 255) as u8;
            dst[3] = ((sa * 255 + da * (255 - sa) + 127) / 255) as u8;
        }
        BlendMode::Darken => {
            let br = sr.min(dr);
            let bg = sg.min(dg);
            let bb = sb.min(db);
            let inv_sa = 255 - sa;
            dst[0] = ((br * sa + dr * inv_sa + 127) / 255) as u8;
            dst[1] = ((bg * sa + dg * inv_sa + 127) / 255) as u8;
            dst[2] = ((bb * sa + db * inv_sa + 127) / 255) as u8;
            dst[3] = ((sa * 255 + da * (255 - sa) + 127) / 255) as u8;
        }
        BlendMode::Lighten => {
            let br = sr.max(dr);
            let bg = sg.max(dg);
            let bb = sb.max(db);
            let inv_sa = 255 - sa;
            dst[0] = ((br * sa + dr * inv_sa + 127) / 255) as u8;
            dst[1] = ((bg * sa + dg * inv_sa + 127) / 255) as u8;
            dst[2] = ((bb * sa + db * inv_sa + 127) / 255) as u8;
            dst[3] = ((sa * 255 + da * (255 - sa) + 127) / 255) as u8;
        }
        BlendMode::Difference => {
            let br = if sr > dr { sr - dr } else { dr - sr };
            let bg = if sg > dg { sg - dg } else { dg - sg };
            let bb = if sb > db { sb - db } else { db - sb };
            let inv_sa = 255 - sa;
            dst[0] = ((br * sa + dr * inv_sa + 127) / 255) as u8;
            dst[1] = ((bg * sa + dg * inv_sa + 127) / 255) as u8;
            dst[2] = ((bb * sa + db * inv_sa + 127) / 255) as u8;
            dst[3] = ((sa * 255 + da * (255 - sa) + 127) / 255) as u8;
        }
    }
}

/// Invert a 2D affine transform.
/// Returns the identity if the determinant is zero (degenerate transform).
fn invert_affine(t: &[f32; 6]) -> [f32; 6] {
    let [a, b, c, d, tx, ty] = *t;
    let det = a * d - b * c;
    if det.abs() < 1e-10 {
        return [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    }
    let inv_det = 1.0 / det;
    [
        d * inv_det,
        -b * inv_det,
        -c * inv_det,
        a * inv_det,
        (b * ty - d * tx) * inv_det,
        (c * tx - a * ty) * inv_det,
    ]
}
