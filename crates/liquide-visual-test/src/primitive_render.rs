//! Headless primitive-render harness for capability goldens (test-harden).
//!
//! The chrome-driven [`crate::capture`] path renders the whole desktop; it is
//! perfect for chrome-surface goldens but gives no control over an isolated
//! renderer primitive (a single bordered box, one gradient rect, an isolated
//! opacity group, a percent-translated element). This module fills that gap: it
//! drives the SAME real pipeline the desktop uses — `liquide_shell::Shell`
//! (style-engine → layout → paint) → flatten → `SoftwareRenderer` raster — but
//! over an author-supplied HTML/CSS fragment mounted onto a clean canvas.
//!
//! Why this path (and not a hand-built paint Scene): the audit-fix capabilities
//! span MULTIPLE stages — `transform: translate(%)` is a STYLE-ENGINE parse fix,
//! border-style / gradient-dither / group-opacity are RENDERER fixes. Driving
//! the whole `Shell` pipeline exercises all of them end-to-end, so a regression
//! in any stage shows up in the pixels (real teeth), and the goldens track the
//! shipping cascade, not a renderer-only shortcut.
//!
//! Determinism: a fixed-size opaque canvas is mounted at `position:fixed; inset:0`
//! with a flat known background, the fragment is mounted inside it, the scene is
//! built at `dt=0` (no animation advance), and the atlas is block-drained via the
//! capture render mode so text is fully present before read-back. The font DB is
//! the renderer's (the real desktop capture path likewise measures with the
//! default measurer and paints with the loaded font DB — this harness matches
//! that relationship exactly).

use liquide_compositor::damage::{DamageClass, DamageSet};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::FlatNode;
use liquide_components::TemplateNode;
use liquide_font_rasterizer::FontDatabase;
use liquide_renderer_cpu::{Renderer, SoftwareRenderer};
use liquide_shell::Shell;

use crate::capture::Frame;
use crate::scenarios::{crate_test_assets_dir, workspace_assets_dir};

/// Tile size for the damage grid (mirrors the live render tile size).
const TILE: u32 = 64;

/// Build a [`FontDatabase`] from the pinned deterministic test font so glyph
/// shaping/advances are byte-stable across machines (same font the chrome
/// scenarios pin). Falls back to the embedded bitmap font if the file is absent.
fn test_font_db() -> FontDatabase {
    let mut db = FontDatabase::new();
    // The pinned Apache-2.0 Inter test font lives under this crate's test-assets.
    let fonts_root = crate_test_assets_dir();
    let _ = db.load_default_fonts(&fonts_root);
    db
}

/// Mirror the live theme-load order (variables → components → split fragments)
/// so any primitive that leans on a CSS variable resolves identically to the
/// desktop. The active theme is intentionally NOT loaded: primitive fragments
/// supply their own inline styles and a flat canvas background, so theme chrome
/// rules cannot perturb the isolated primitive under test.
fn load_base_css(shell: &mut Shell) {
    let themes = workspace_assets_dir().join("themes");
    for base in ["variables.css", "components.css"] {
        if let Ok(css) = std::fs::read_to_string(themes.join(base)) {
            shell.add_base_layer_stylesheet(&css);
        }
    }
}

/// Render an HTML/CSS fragment on a clean opaque canvas through the real
/// `Shell` pipeline and return the captured [`Frame`] (RGBA8, top-down).
///
/// * `width`/`height` — the surface size (also the canvas size).
/// * `canvas_bg` — a CSS color string painted as the flat canvas background
///   (e.g. `"#202020"`), so primitive content composites over a KNOWN backdrop.
/// * `fragment` — the author's primitive subtree, mounted inside the canvas.
///
/// The canvas is `position:fixed; inset:0` so it covers the whole surface and
/// the desktop wallpaper underneath never bleeds into the assertion region.
pub fn render_fragment(
    width: u32,
    height: u32,
    canvas_bg: &str,
    fragment: TemplateNode,
) -> Frame {
    let mut shell = Shell::new(width as f32, height as f32);
    load_base_css(&mut shell);

    // The canvas: a fixed full-surface opaque box that hosts the fragment.
    let canvas = TemplateNode::el("div")
        .id("primitive-canvas")
        .style("position", "fixed")
        .style("left", "0")
        .style("top", "0")
        .style("width", &format!("{width}px"))
        .style("height", &format!("{height}px"))
        .style("background", canvas_bg)
        .style("z-index", "100000")
        .style("overflow", "hidden")
        .child(fragment);
    shell.mount_template("primitive-canvas", &canvas);

    // Build the scene (dt = 0 → no animation advance, deterministic).
    let scene = shell.build_scene();
    let mut flat: Vec<FlatNode> = Vec::new();
    scene.flatten_into(&mut flat);

    // Raster into a BGRA framebuffer; block-drain glyphs (capture mode) so text
    // is fully present, then a second pass to settle the atlas-backed glyphs.
    let mut renderer = SoftwareRenderer::with_font_db(test_font_db());
    let mut fb = FrameBuffer::new(width, height, PixelFormat::Bgra8);
    let grid_w = width.div_ceil(TILE);
    let grid_h = height.div_ceil(TILE);
    let full = DamageSet::full(TILE, grid_w, grid_h, DamageClass::UiPrimitive);
    let _ = renderer.render(&flat, &mut fb, &full);
    // Re-render so any glyphs uploaded on the first pass paint this pass.
    let _ = renderer.render(&flat, &mut fb, &full);

    frame_from_fb(&fb)
}

/// Convert a BGRA8 [`FrameBuffer`] to a packed RGBA8 [`Frame`].
fn frame_from_fb(fb: &FrameBuffer) -> Frame {
    let w = fb.width as usize;
    let h = fb.height as usize;
    let src = fb.pixels();
    // FrameBuffer is tightly packed at width*4 (no padded stride in this path).
    let stride = w * 4;
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let s = &src[y * stride + x * 4..];
            let d = &mut rgba[(y * w + x) * 4..];
            d[0] = s[2]; // R <- B
            d[1] = s[1]; // G
            d[2] = s[0]; // B <- R
            d[3] = s[3]; // A
        }
    }
    Frame {
        width: fb.width,
        height: fb.height,
        rgba,
    }
}

/// A horizontal span of "ink" columns: for a frame row band, the min and max
/// x where a pixel differs from `bg` by more than `tol` on any channel.
///
/// Used by the text-shaping width tooth: the painted ink extent of a kerned
/// string is TIGHTER than the sum of naive per-codepoint advances.
#[must_use]
pub fn ink_x_extent(frame: &Frame, bg: [u8; 4], tol: u8) -> Option<(u32, u32)> {
    let mut min_x: Option<u32> = None;
    let mut max_x: Option<u32> = None;
    for y in 0..frame.height {
        for x in 0..frame.width {
            let p = frame.pixel(x, y).unwrap_or([0, 0, 0, 0]);
            let differs = p
                .iter()
                .zip(bg.iter())
                .any(|(&a, &b)| a.abs_diff(b) > tol);
            if differs {
                min_x = Some(min_x.map_or(x, |m| m.min(x)));
                max_x = Some(max_x.map_or(x, |m| m.max(x)));
            }
        }
    }
    match (min_x, max_x) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    }
}

/// Count distinct quantized luminance levels present in `frame` (a banding
/// tell-tale: a smooth dithered gradient spreads ink across MANY levels; a
/// banded one collapses to a few flat plateaus).
#[must_use]
pub fn distinct_luma_levels(frame: &Frame) -> usize {
    let mut seen = [false; 256];
    for px in frame.rgba.chunks_exact(4) {
        // Rec.601 luma.
        let luma = (0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32)
            .round()
            .clamp(0.0, 255.0) as usize;
        seen[luma] = true;
    }
    seen.iter().filter(|&&b| b).count()
}
