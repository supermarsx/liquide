//! Windowed-renderer asset/font helpers.
//!
//! These functions seed a SEPARATE renderer (the dev-mode devtools window, and
//! any future per-window renderer) with the SAME packaged fonts and asset root
//! as the main desktop renderer, so its text lays out and paints with identical
//! faces/advances instead of degrading to the divergent 8x16 bitmap font (t167).
//!
//! NOTE: a full per-window render pipeline (`WindowRenderManager` wrapping a
//! `liquide-render-thread` `RenderCoordinator` with per-window chrome/content
//! worker threads) used to live here. It was fully built and unit-tested but the
//! event loop NEVER registered a window with it — all rendering goes through the
//! single main `render_full_job` worker. It was removed (wire-or-remove); only
//! these live font/asset helpers survive.

use std::path::PathBuf;

use liquide_font_rasterizer::FontDatabase;

/// Resolve the assets directory for windowed renderers, mirroring
/// [`super::DesktopSession::resolve_asset_root`] (which is private to that type).
///
/// Kept here (rather than reused from `mod.rs`) only because the separate
/// devtools window needs it independently of a live `DesktopSession`; the
/// resolution order is identical so both pick up the SAME font set as the main
/// desktop renderer.
pub(super) fn resolve_asset_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("LIQUIDE_ASSETS_DIR") {
        let candidate = PathBuf::from(dir);
        if candidate.is_dir() {
            return candidate;
        }
    }
    let cwd_relative = PathBuf::from("assets");
    if cwd_relative.is_dir() {
        return cwd_relative;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidate = exe_dir.join("assets");
            if candidate.is_dir() {
                return candidate;
            }
        }
    }
    let manifest_relative = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets");
    if manifest_relative.is_dir() {
        return manifest_relative;
    }
    cwd_relative
}

/// Build a fully-populated font database for a windowed renderer.
///
/// Loads the same packaged TrueType faces as the main desktop renderer (falling
/// back to the embedded face when none are on disk). Without this, a windowed
/// renderer built via `SoftwareRenderer::new()` has a 0-face database and every
/// glyph falls to the 8x16 bitmap font, whose advances diverge from the
/// rustybuzz layout advances — producing jumbled / overlapping window text
/// (t167). The result is guaranteed non-empty.
pub(super) fn build_window_font_database() -> FontDatabase {
    FontDatabase::with_default_fonts(resolve_asset_root())
}

#[cfg(test)]
mod tests {
    use std::thread;

    use liquide_compositor::geometry::{Affine2D, Rect};
    use liquide_compositor::pixel::Color;
    use liquide_compositor::scene::{FlatNode, SceneNodeKind, WordBreak};
    use liquide_render_thread::{ContentThread, content_thread};
    use liquide_renderer_cpu::SoftwareRenderer;

    use super::*;

    /// A single-line text FlatNode shaped like an app/settings window label.
    fn settings_text_node(text: &str) -> FlatNode {
        FlatNode {
            id: 42,
            kind: std::sync::Arc::new(SceneNodeKind::Text {
                text: text.to_string(),
                color: Color::WHITE,
                scale: 1,
                font_family: "Inter".to_string(),
                font_size: 14.0,
                font_weight: 400,
                font_style_italic: false,
                letter_spacing: 0.0,
                word_spacing: 0.0,
                line_height: 0.0,
                text_align: 0,
                text_transform: 0,
                text_overflow: 0,
                white_space: 1, // nowrap
                word_break: WordBreak::Normal,
                text_indent: 0.0,
                text_decoration: None,
                text_shadows: Vec::new(),
                text_emphasis: None,
            }),
            absolute_bounds: Rect::new(10.0, 10.0, 280.0, 24.0),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Drive one app/settings text frame through a real `content_worker` thread
    /// (the per-window content render path) with `renderer`, returning the
    /// rendered BGRA pixels of a frame that has had its glyphs drained.
    ///
    /// Two frames are requested: glyph rasterization is async, so the FIRST frame
    /// issues the glyph requests and the SECOND (capture path) block-drains them
    /// into the atlas and paints real ink.
    fn render_settings_text_via_content_thread(
        renderer: Box<SoftwareRenderer>,
        text: &str,
        w: u32,
        h: u32,
    ) -> (Vec<u8>, u32) {
        let (mut handle, rx, comp_tx) = ContentThread::new(w, h);
        let worker = thread::spawn(move || {
            content_thread::content_worker(rx, comp_tx, renderer);
        });

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut last_pixels: Option<std::sync::Arc<Vec<u8>>> = None;
        let mut stride = w * 4;
        for _ in 0..2 {
            handle
                .request_frame(Vec::new(), vec![settings_text_node(text)])
                .expect("request frame");
            // Wait for this frame's completion (resets state to Idle).
            loop {
                if let Some(c) = handle.try_recv_completion() {
                    stride = c.stride;
                    if let Some(px) = c.pixels {
                        last_pixels = Some(px);
                    }
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "content worker did not complete a frame in time"
                );
                std::thread::yield_now();
            }
        }

        handle.shutdown().ok();
        let _ = worker.join();
        (
            last_pixels.map(|p| (*p).clone()).unwrap_or_default(),
            stride,
        )
    }

    /// Painted ink width (rightmost − leftmost near-white column) of the label
    /// band in a BGRA frame.
    fn label_ink_width(px: &[u8], stride: u32, h: u32) -> u32 {
        let row_bytes = stride as usize;
        let mut min_x: Option<u32> = None;
        let mut max_x: Option<u32> = None;
        for y in 8..h.min(40) {
            for x in 0..(stride / 4) {
                let idx = y as usize * row_bytes + (x as usize) * 4;
                if idx + 4 <= px.len() {
                    let l = (px[idx] as u32 + px[idx + 1] as u32 + px[idx + 2] as u32) / 3;
                    if l > 120 {
                        min_x = Some(min_x.map_or(x, |m| m.min(x)));
                        max_x = Some(max_x.map_or(x, |m| m.max(x)));
                    }
                }
            }
        }
        match (min_x, max_x) {
            (Some(a), Some(b)) => b - a + 1,
            _ => 0,
        }
    }

    #[test]
    fn per_window_font_db_is_non_empty() {
        // The per-window chrome/content render threads are seeded from this DB.
        // Before the fix they used `SoftwareRenderer::new()` (0 faces) so app /
        // settings window text dropped to the 8x16 bitmap font (t167).
        let db = build_window_font_database();
        assert!(
            db.face_count() >= 1,
            "per-window font DB must be non-empty (got {} faces)",
            db.face_count()
        );
        assert!(db.resolve("sans-serif", 400, false).is_some());
        assert!(db.resolve("Inter", 400, false).is_some());
    }

    #[test]
    fn settings_window_content_text_uses_real_font_not_bitmap_fallback() {
        // RED if a content thread reverts to `SoftwareRenderer::new()`. We drive
        // the SAME settings-style label through the real content render path with
        // (a) the SEEDED renderer (built from `build_window_font_database`) and
        // (b) an empty-DB renderer, then compare the painted ink WIDTH.
        //
        // The bitmap fallback advances every glyph by a uniform ~half-em, so its
        // string width differs from a real proportional font (Inter @ 14px). A
        // matching width would mean the seeded path is still empty-DB.
        let w = 320;
        let h = 48;
        let label = "General Network Display";

        let seeded = Box::new(SoftwareRenderer::with_font_db(build_window_font_database()));
        let (seeded_px, seeded_stride) =
            render_settings_text_via_content_thread(seeded, label, w, h);
        let seeded_w = label_ink_width(&seeded_px, seeded_stride, h);

        let empty = Box::new(SoftwareRenderer::new());
        let (empty_px, empty_stride) = render_settings_text_via_content_thread(empty, label, w, h);
        let empty_w = label_ink_width(&empty_px, empty_stride, h);

        assert!(
            seeded_w > 0,
            "the seeded content renderer must paint real glyph ink (width 0)"
        );
        assert!(
            empty_w > 0,
            "sanity: the empty-DB bitmap path must paint something (width 0)"
        );
        assert!(
            seeded_w.abs_diff(empty_w) >= 3,
            "seeded content-thread text width ({seeded_w}px) must differ from the \
             empty-DB bitmap width ({empty_w}px); equal means the per-window \
             content thread is still empty-DB (fix not effective / reverted)"
        );
    }
}
