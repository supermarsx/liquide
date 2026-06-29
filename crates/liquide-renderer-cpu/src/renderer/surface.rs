//! Surface-cache raster helpers (t2-surface-cache, executor E2 — RENDERER side).
//!
//! This module provides the two renderer-side primitives the composite-only loop
//! (E3, `render_thread.rs`) calls on a cache MISS, plus the glass backdrop
//! signature the cache keys on:
//!
//! 1. [`SoftwareRenderer::render_subtree_to_surface`] — raster an opaque owner's
//!    (window / wallpaper / chrome-layer) origin-translated subtree into an
//!    offscreen [`FrameBuffer`] drawn from the [`FrameMemoryPool`], then capture it
//!    into a cacheable [`SurfaceBuffer`]. The result, blitted back at the
//!    footprint origin, is **byte-identical** to rastering that subtree directly
//!    into the destination framebuffer at the same position.
//!
//! 2. [`SoftwareRenderer::glass_backdrop_sig`] + [`SoftwareRenderer::render_glass_in_place_and_capture`]
//!    — the GLASS / backdrop-blur correctness path. The compositor composites
//!    back-to-front, so when a glass surface is reached the destination
//!    framebuffer under its footprint already IS its backdrop. The signature is a
//!    CRC32C over `footprint.expand(blur_radius)`; a cached glass surface may be
//!    re-blitted only while `(own_sig, backdrop_sig)` is unchanged, otherwise the
//!    glass is re-blurred IN PLACE over the live backdrop (byte-identical to the
//!    direct full-frame glass path, since it reuses the very same
//!    `render_glass_node` / `render_backdrop_blur` code) and the cache refreshed
//!    via `capture_region`.
//!
//! Byte-identity rationale (the gate):
//! - **Opaque subtree:** the offscreen target is SEEDED with the live backdrop
//!   region, the subtree is rendered over it with an INTEGER pixel translation
//!   (so anti-aliasing / sub-pixel coverage is unchanged), and the result is
//!   captured. Because this reproduces the exact same integer blend operations,
//!   in the same order, over the same backdrop bytes, copying the captured
//!   surface back (opaque blit / memcpy) at the footprint origin yields pixels
//!   bit-for-bit equal to a direct raster.
//! - **Glass:** re-blur runs the unchanged glass paint path over the live
//!   backdrop already composited into the destination FB, so it is identical to
//!   the full-frame path by construction; the captured result is what the cache
//!   stores and re-blits while the backdrop CRC matches. The blur cache is keyed
//!   on the backdrop snapshot bytes (`stable_blur_key`), so a changed backdrop
//!   can never reuse a stale blur.
//!
//! Every blit performed by E3 on the reuse path goes through the existing
//! `blit_*_stride_clipped` helpers, which clamp to the Tier-1 write-scissor
//! (`scissor_clamp_window`) — so no surface write can escape `damage ∩ footprint`.

use liquide_compositor::damage::{crc32c, DamageSet};
use liquide_compositor::framebuffer::{FrameBuffer, FrameMemoryPool};
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::{FlatNode, SceneNodeKind, SurfaceBuffer};
use liquide_compositor::RenderMode;
use std::sync::Arc;

use super::SoftwareRenderer;

/// The integer pixel window `(x0, y0, w, h)` a `footprint` maps to, clamped to
/// the framebuffer. Uses the SAME rounding as [`FrameBuffer::capture_region`]
/// (floor origin, ceil far edge) so the surface dimensions match what the blit
/// back at `(x0, y0)` will cover.
#[inline]
#[must_use]
fn footprint_window(footprint: Rect, fb_w: u32, fb_h: u32) -> (u32, u32, u32, u32) {
    let x0 = (footprint.x.floor().max(0.0) as u32).min(fb_w);
    let y0 = (footprint.y.floor().max(0.0) as u32).min(fb_h);
    let x1 = (footprint.right().ceil().max(0.0) as u32).min(fb_w);
    let y1 = (footprint.bottom().ceil().max(0.0) as u32).min(fb_h);
    (x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0))
}

/// A `1x1` transparent surface — the degenerate (empty / off-screen footprint)
/// return value, mirroring [`FrameBuffer::capture_region`] so callers always get
/// a paintable buffer and never index an empty slice.
#[inline]
#[must_use]
fn transparent_surface(format: PixelFormat) -> SurfaceBuffer {
    let bpp = format.bytes_per_pixel();
    SurfaceBuffer {
        pixels: Arc::new(vec![0u8; bpp as usize]),
        width: 1,
        height: 1,
        stride: bpp,
        format,
    }
}

/// Copy the `[ox, ox+w) x [oy, oy+h)` region of `src` into the top-left of
/// `dst`. Both are assumed `Bgra8`-class (4 bpp) CPU framebuffers; out-of-range
/// rows are skipped (leaving the destination zero-filled), never panicking.
fn seed_from_backdrop(dst: &mut FrameBuffer, src: &FrameBuffer, ox: u32, oy: u32, w: u32, h: u32) {
    let bpp = src.format.bytes_per_pixel() as usize;
    let src_stride = src.stride as usize;
    let dst_stride = dst.stride as usize;
    let row_bytes = w as usize * bpp;
    let src_px = src.pixels();
    let Some(dst_px) = dst.pixels_mut() else {
        return;
    };
    for row in 0..h as usize {
        let s = (oy as usize + row) * src_stride + ox as usize * bpp;
        let d = row * dst_stride;
        if s + row_bytes <= src_px.len() && d + row_bytes <= dst_px.len() {
            dst_px[d..d + row_bytes].copy_from_slice(&src_px[s..s + row_bytes]);
        }
    }
}

/// Clone `nodes` with their absolute bounds + clip shifted by `(-dx, -dy)`.
/// `dx`/`dy` are INTEGER pixel offsets, so fractional positions (and thus the AA
/// coverage of every edge) are preserved exactly — the property that makes the
/// offscreen raster byte-identical to the in-place raster.
fn translate_nodes(nodes: &[FlatNode], dx: f32, dy: f32) -> Vec<FlatNode> {
    nodes
        .iter()
        .map(|n| {
            let mut c = n.clone(); // `kind` is an `Arc` — cloning is an atomic incr.
            c.absolute_bounds.x -= dx;
            c.absolute_bounds.y -= dy;
            if let Some(clip) = c.clip.as_mut() {
                clip.x -= dx;
                clip.y -= dy;
            }
            c
        })
        .collect()
}

impl SoftwareRenderer {
    /// Raster an OPAQUE owner's origin-translated `nodes` subtree into an
    /// offscreen surface, returning a cacheable [`SurfaceBuffer`].
    ///
    /// `footprint` is the owner's painted footprint in `dest_fb` (screen) coords
    /// — `bounds ∪ shadow/effect margin`. The offscreen target is acquired from
    /// `pool` (bucketed by size, so a re-raster does not allocate a fresh
    /// megabyte buffer) and returned to it before this call returns.
    ///
    /// The returned surface, blitted back at the integer footprint origin
    /// (`floor(footprint.x/y)`) via the existing `Surface` node / opaque blit, is
    /// byte-identical to rastering `nodes` directly into `dest_fb` at the same
    /// position. See the module docs for the proof. `mode` selects the glyph
    /// drain / determinism policy exactly like [`SoftwareRenderer::render_live`];
    /// the capture/golden path passes [`RenderMode::Capture`].
    ///
    /// E3 calls this on an OPAQUE cache MISS; on a HIT it re-blits the stored
    /// surface (no call here).
    #[must_use]
    pub fn render_subtree_to_surface(
        &mut self,
        nodes: &[FlatNode],
        footprint: Rect,
        dest_fb: &FrameBuffer,
        pool: &mut FrameMemoryPool,
        mode: RenderMode,
    ) -> SurfaceBuffer {
        let format = dest_fb.format;
        let (ox, oy, w, h) = footprint_window(footprint, dest_fb.width, dest_fb.height);
        if w == 0 || h == 0 {
            return transparent_surface(format);
        }

        // Offscreen target, seeded with the live backdrop so a subtree with
        // semi-transparent edges (rounded-corner AA, drop shadow) composites over
        // the real backdrop — making the captured surface reproduce the direct
        // raster bit-for-bit when copied back.
        let mut off = pool.acquire(w, h, format);
        seed_from_backdrop(&mut off, dest_fb, ox, oy, w, h);

        // Render the origin-translated subtree FULLY (empty damage => no raster
        // clip / write-scissor, no damage-bbox cull), so every footprint pixel is
        // produced. Integer translation keeps sub-pixel AA identical.
        let translated = translate_nodes(nodes, ox as f32, oy as f32);
        let empty = DamageSet::new(64);
        let _ = self.render_with_mode(&translated, &mut off, &empty, mode);

        let surface = off.capture_region(Rect::new(0.0, 0.0, w as f32, h as f32));
        pool.release(off);
        surface
    }

    /// The blur radius the renderer will actually sample over for a glass node —
    /// the value E3/E4 must expand the backdrop footprint by so the CRC covers
    /// every backdrop pixel the blur reads. Matches `render_glass_node`'s
    /// `params.blur_radius.min(30)` cap. Returns `0` for non-glass nodes.
    ///
    /// This is a CONSERVATIVE (upper-bound) radius: a LOD pass may shrink the
    /// effective blur, but never widen it, so keying on this radius never
    /// under-covers the sampled backdrop.
    #[must_use]
    pub fn glass_blur_radius(node: &FlatNode) -> u32 {
        match node.kind_ref() {
            SceneNodeKind::Glass(params) => params.blur_radius.min(30),
            _ => 0,
        }
    }

    /// The backdrop signature for a glass surface: `crc32c` over the destination
    /// framebuffer region under `footprint` expanded by `blur_radius` on every
    /// side (the exact set of backdrop pixels the blur samples).
    ///
    /// **Contract:** call this in composite order AFTER everything beneath the
    /// glass has been composited into `fb` — then `fb` under the footprint IS the
    /// glass's backdrop. A cached glass surface is valid iff its stored
    /// `(own_sig, backdrop_sig)` equals `(own_sig, glass_backdrop_sig(...))`; any
    /// change to a backdrop pixel within the expanded footprint flips this CRC and
    /// forces a re-blur. Determinism: the captured region is a tight, deterministic
    /// copy and `crc32c` is the same SSE4.2/scalar-identical routine the damage
    /// tracker uses.
    #[must_use]
    pub fn glass_backdrop_sig(fb: &FrameBuffer, footprint: Rect, blur_radius: u32) -> u32 {
        let region = fb.capture_region(footprint.expand(blur_radius as f32));
        crc32c(region.pixels.as_slice())
    }

    /// Re-blur a glass surface IN PLACE over the live backdrop already composited
    /// into `fb`, then capture the result into a refreshed [`SurfaceBuffer`].
    ///
    /// `glass_nodes` are the glass owner's nodes (the `Glass` node plus any tint /
    /// content / inner-glow children), positioned in screen coords. They are
    /// painted through the UNCHANGED glass path (`render_glass_node` /
    /// `render_backdrop_blur`) so the output is byte-identical to a full-frame
    /// repaint of that glass over the same backdrop. `damage` confines the paint
    /// (and is clamped further by the Tier-1 write-scissor) to `damage ∩
    /// footprint`; pass full / footprint damage to refresh the whole surface.
    ///
    /// E3 calls this on a glass cache MISS (own_sig changed OR backdrop_sig
    /// changed). The returned surface is stored keyed on the
    /// `(own_sig, backdrop_sig)` that was true for THIS backdrop; on a later HIT
    /// E3 re-blits it instead of re-blurring.
    #[must_use]
    pub fn render_glass_in_place_and_capture(
        &mut self,
        glass_nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        footprint: Rect,
        damage: &DamageSet,
        mode: RenderMode,
    ) -> SurfaceBuffer {
        let _ = self.render_with_mode(glass_nodes, fb, damage, mode);
        fb.capture_region(footprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::pixel::Color;
    use liquide_compositor::scene::GlassParams;

    fn renderer() -> SoftwareRenderer {
        // No font DB needed: the surface/glass tests paint fills + glass, not text.
        SoftwareRenderer::new()
    }

    fn flat(
        id: u64,
        kind: SceneNodeKind,
        bounds: Rect,
        opacity: f32,
        corner: (f32, f32, f32, f32),
    ) -> FlatNode {
        FlatNode {
            id,
            kind: Arc::new(kind),
            absolute_bounds: bounds,
            absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
            clip: None,
            opacity,
            z_order: 0,
            corner_radius: corner,
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Paint a deterministic, NON-uniform backdrop so blur / blend results are
    /// content-dependent (a flat backdrop would hide many bugs).
    fn paint_backdrop(fb: &mut FrameBuffer) {
        for y in 0..fb.height {
            for x in 0..fb.width {
                let r = ((x * 7 + y * 3) % 256) as u8;
                let g = ((x * 3 + y * 11) % 256) as u8;
                let b = ((x + y * 5) % 256) as u8;
                fb.set_pixel(x, y, Color::new(r, g, b, 255));
            }
        }
    }

    fn region_bytes(fb: &FrameBuffer, x0: u32, y0: u32, w: u32, h: u32) -> Vec<u8> {
        let bpp = fb.format.bytes_per_pixel() as usize;
        let stride = fb.stride as usize;
        let px = fb.pixels();
        let mut out = Vec::with_capacity(w as usize * h as usize * bpp);
        for row in 0..h as usize {
            let s = (y0 as usize + row) * stride + x0 as usize * bpp;
            out.extend_from_slice(&px[s..s + w as usize * bpp]);
        }
        out
    }

    fn blit_surface_opaque(fb: &mut FrameBuffer, surf: &SurfaceBuffer, dst_x: u32, dst_y: u32) {
        crate::rasterizer::blit_opaque_stride_clipped(
            fb,
            &surf.pixels,
            surf.width,
            surf.height,
            surf.stride as usize,
            dst_x,
            dst_y,
            None,
        );
    }

    // ── 1. Opaque raster-to-surface == direct raster, byte for byte ──────────

    #[test]
    fn raster_to_surface_blit_is_byte_identical_to_direct() {
        let mut r = renderer();
        let w = 160u32;
        let h = 120u32;

        // A subtree with a SEMI-TRANSPARENT, ROUNDED fill: exercises alpha
        // blending against the backdrop AND anti-aliased rounded corners — the
        // pixels where a naive offscreen-over-transparent approach would diverge.
        let footprint = Rect::new(30.0, 24.0, 80.0, 60.0);
        let subtree = vec![flat(
            1,
            SceneNodeKind::Background {
                color: Color::new(220, 60, 90, 160),
            },
            footprint,
            1.0,
            (14.0, 14.0, 14.0, 14.0),
        )];

        // Common backdrop.
        let mut base = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        paint_backdrop(&mut base);

        // DIRECT: render the subtree straight into a clone of the backdrop.
        let mut fb_direct = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        seed_from_backdrop(&mut fb_direct, &base, 0, 0, w, h);
        let empty = DamageSet::new(64);
        let _ = r.render_with_mode(&subtree, &mut fb_direct, &empty, RenderMode::Capture);

        // OFFSCREEN: raster to a surface, then blit it back over a fresh clone.
        let mut pool = FrameMemoryPool::new();
        let surf = r.render_subtree_to_surface(
            &subtree,
            footprint,
            &base,
            &mut pool,
            RenderMode::Capture,
        );
        let (ox, oy, fw, fh) = footprint_window(footprint, w, h);
        let mut fb_surface = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        seed_from_backdrop(&mut fb_surface, &base, 0, 0, w, h);
        blit_surface_opaque(&mut fb_surface, &surf, ox, oy);

        let direct = region_bytes(&fb_direct, ox, oy, fw, fh);
        let via_surface = region_bytes(&fb_surface, ox, oy, fw, fh);
        assert_eq!(
            direct, via_surface,
            "surface raster+blit must be byte-identical to direct raster"
        );

        // Teeth: the subtree actually drew something (region differs from the raw
        // backdrop), so the equality above is non-trivial.
        let backdrop = region_bytes(&base, ox, oy, fw, fh);
        assert_ne!(
            direct, backdrop,
            "subtree must have painted over the backdrop (else the test is vacuous)"
        );
    }

    // ── 2. Backdrop CRC: changes inside the expanded footprint flip it; ──────
    //       changes outside it do not.

    #[test]
    fn glass_backdrop_sig_tracks_only_the_expanded_footprint() {
        let mut fb = FrameBuffer::new(200, 200, PixelFormat::Bgra8);
        paint_backdrop(&mut fb);
        let footprint = Rect::new(80.0, 80.0, 40.0, 40.0);
        let radius = 12u32;

        let sig0 = SoftwareRenderer::glass_backdrop_sig(&fb, footprint, radius);

        // A change INSIDE footprint.expand(radius) must flip the CRC.
        fb.set_pixel(85, 85, Color::new(0, 0, 0, 255));
        let sig_inside = SoftwareRenderer::glass_backdrop_sig(&fb, footprint, radius);
        assert_ne!(sig0, sig_inside, "a backdrop change inside the footprint must flip the CRC");

        // Restore, then change a pixel FAR outside the expanded footprint: the
        // CRC must be unchanged (else glass would needlessly re-blur — the
        // footprint expansion would be wrong).
        paint_backdrop(&mut fb);
        let sig_restored = SoftwareRenderer::glass_backdrop_sig(&fb, footprint, radius);
        assert_eq!(sig0, sig_restored, "restoring the backdrop must restore the CRC");
        fb.set_pixel(5, 5, Color::new(0, 0, 0, 255)); // far outside [68,132)^2
        let sig_outside = SoftwareRenderer::glass_backdrop_sig(&fb, footprint, radius);
        assert_eq!(
            sig0, sig_outside,
            "a backdrop change OUTSIDE the expanded footprint must not flip the CRC"
        );
    }

    fn glass_scene(footprint: Rect) -> Vec<FlatNode> {
        vec![flat(
            7,
            SceneNodeKind::Glass(GlassParams {
                blur_radius: 12,
                tint_color: Color::new(255, 255, 255, 60),
                inner_glow: false,
                parallax: false,
            }),
            footprint,
            1.0,
            (8.0, 8.0, 8.0, 8.0),
        )]
    }

    // ── 3. Cached glass blit == fresh re-blur when the backdrop is unchanged. ─

    #[test]
    fn glass_cached_blit_equals_reblur_when_backdrop_unchanged() {
        let mut r = renderer();
        let w = 200u32;
        let h = 200u32;
        let footprint = Rect::new(60.0, 60.0, 80.0, 50.0);
        let glass = glass_scene(footprint);
        let radius = SoftwareRenderer::glass_blur_radius(&glass[0]);

        let mut base = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        paint_backdrop(&mut base);

        let empty = DamageSet::new(64);

        // MISS: re-blur in place over the live backdrop, capture the cache entry.
        let mut fb_make = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        seed_from_backdrop(&mut fb_make, &base, 0, 0, w, h);
        let sig_make = SoftwareRenderer::glass_backdrop_sig(&fb_make, footprint, radius);
        let cached =
            r.render_glass_in_place_and_capture(&glass, &mut fb_make, footprint, &empty, RenderMode::Capture);

        // HIT path: backdrop is the SAME -> sig matches -> blit the cached surface.
        let mut fb_reuse = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        seed_from_backdrop(&mut fb_reuse, &base, 0, 0, w, h);
        let sig_reuse = SoftwareRenderer::glass_backdrop_sig(&fb_reuse, footprint, radius);
        assert_eq!(sig_make, sig_reuse, "identical backdrop must yield identical CRC");
        let (ox, oy, fw, fh) = footprint_window(footprint, w, h);
        blit_surface_opaque(&mut fb_reuse, &cached, ox, oy);

        // FRESH re-blur over the same backdrop, for comparison.
        let mut fb_fresh = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        seed_from_backdrop(&mut fb_fresh, &base, 0, 0, w, h);
        let _ =
            r.render_glass_in_place_and_capture(&glass, &mut fb_fresh, footprint, &empty, RenderMode::Capture);

        let via_cache = region_bytes(&fb_reuse, ox, oy, fw, fh);
        let via_reblur = region_bytes(&fb_fresh, ox, oy, fw, fh);
        assert_eq!(
            via_cache, via_reblur,
            "cached glass blit must equal a fresh re-blur when the backdrop is unchanged"
        );
        // Non-vacuous: glass actually altered the backdrop.
        let backdrop = region_bytes(&base, ox, oy, fw, fh);
        assert_ne!(via_reblur, backdrop, "glass must have changed the backdrop pixels");
    }

    // ── 4. A backdrop change flips the CRC AND a stale cached blit is wrong. ──
    //       (Teeth: forcing the stale blit over a changed backdrop diverges from
    //        the correct re-blur — proving the CRC is what prevents the bug.)

    #[test]
    fn glass_backdrop_change_flips_sig_and_stale_blit_is_wrong() {
        let mut r = renderer();
        let w = 200u32;
        let h = 200u32;
        let footprint = Rect::new(60.0, 60.0, 80.0, 50.0);
        let glass = glass_scene(footprint);
        let radius = SoftwareRenderer::glass_blur_radius(&glass[0]);
        let (ox, oy, fw, fh) = footprint_window(footprint, w, h);
        let empty = DamageSet::new(64);

        // Original backdrop -> cached glass surface + its backdrop sig.
        let mut base0 = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        paint_backdrop(&mut base0);
        let sig0 = SoftwareRenderer::glass_backdrop_sig(&base0, footprint, radius);
        let mut fb_make = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        seed_from_backdrop(&mut fb_make, &base0, 0, 0, w, h);
        let cached =
            r.render_glass_in_place_and_capture(&glass, &mut fb_make, footprint, &empty, RenderMode::Capture);

        // Now CHANGE the backdrop under the glass (a window below repainted).
        let mut base1 = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        seed_from_backdrop(&mut base1, &base0, 0, 0, w, h);
        for y in 70..100 {
            for x in 70..120 {
                base1.set_pixel(x, y, Color::new(10, 200, 30, 255));
            }
        }

        // The CRC must catch it.
        let sig1 = SoftwareRenderer::glass_backdrop_sig(&base1, footprint, radius);
        assert_ne!(
            sig0, sig1,
            "a backdrop change under the glass must flip the backdrop CRC (forcing a re-blur)"
        );

        // CORRECT result: re-blur over the NEW backdrop.
        let mut fb_correct = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        seed_from_backdrop(&mut fb_correct, &base1, 0, 0, w, h);
        let _ = r.render_glass_in_place_and_capture(
            &glass,
            &mut fb_correct,
            footprint,
            &empty,
            RenderMode::Capture,
        );
        let correct = region_bytes(&fb_correct, ox, oy, fw, fh);

        // STALE result: blit the OLD cached glass over the NEW backdrop (what
        // would happen if the CRC check were skipped).
        let mut fb_stale = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        seed_from_backdrop(&mut fb_stale, &base1, 0, 0, w, h);
        blit_surface_opaque(&mut fb_stale, &cached, ox, oy);
        let stale = region_bytes(&fb_stale, ox, oy, fw, fh);

        assert_ne!(
            stale, correct,
            "a stale cached glass blitted over a changed backdrop must differ from the correct \
             re-blur — proving the backdrop CRC is load-bearing"
        );
    }
}
