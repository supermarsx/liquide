//! Pre-shell loading-screen MINI-PIPELINE.
//!
//! The startup loading screen is the literal first frame — it is rendered
//! *before* the shell's DOM and theme cascade exist. Per the t86 full-CSS
//! migration plan (phase P3, option 3-A), it is therefore driven by a tiny
//! self-contained CSS mini-pipeline rather than hardcoded `Rect::new` /
//! `Color::new` primitives:
//!
//! 1. an embedded `loading.html` template ([`LOADING_HTML`]) is parsed into a
//!    [`Document`],
//! 2. an embedded `loading.css` stylesheet ([`LOADING_CSS`]) is loaded into a
//!    fresh [`DesktopPipeline`] — the SAME style → layout → paint → scene
//!    engine the shell uses, only with a different (pre-shell) bootstrap,
//! 3. the pipeline emits compositor [`SceneNode`]s, which we wrap under a Root.
//!
//! This keeps the loading screen on the single CSS render path (themeable,
//! no parallel imperative track) while staying cheap: it is a one-shot
//! full pass over ~8 boxes, with no font database loaded (layout uses the
//! cheap `DefaultTextMeasurer`; glyphs still rasterize via the renderer's
//! own font DB at paint time). It does NOT load fonts or external assets, so
//! it adds no measurable startup latency.

use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
use liquide_dom::html_parser::parse_html;
use liquide_shell::pipeline::{DesktopPipeline, PipelineConfig};

/// Embedded loading-screen DOM template (pre-shell; see `assets/templates/loading.html`).
pub const LOADING_HTML: &str = include_str!("../../../../assets/templates/loading.html");

/// Embedded loading-screen stylesheet (pre-shell; see `assets/themes/loading.css`).
///
/// Self-contained: it uses NO `var(--…)` custom properties because the
/// variables.css / components.css cascade is not loaded by the mini-pipeline.
pub const LOADING_CSS: &str = include_str!("../../../../assets/themes/loading.css");

/// Build the startup loading-screen scene from the embedded DOM + CSS via the
/// pre-shell mini-pipeline.
///
/// `width`/`height` are the framebuffer dimensions in physical pixels (the
/// loading screen runs at scale 1.0). Returns a `Root` scene node whose
/// children are the pipeline's paint output, z-ordered in emission order.
pub fn build_loading_scene_nodes(width: u32, height: u32) -> SceneNode {
    let w = width as f32;
    let h = height as f32;
    let screen = Rect::new(0.0, 0.0, w, h);

    // Parse the embedded loading DOM. A freshly-parsed document + a freshly
    // built pipeline have empty caches, so `render_to_scene` runs the full
    // Style → Layout → Paint pass (no font DB ⇒ DefaultTextMeasurer).
    let doc = parse_html(LOADING_HTML);

    let mut pipeline = DesktopPipeline::new(&PipelineConfig {
        width: w,
        height: h,
        base_font_size: 14.0,
    });
    // Replace the pipeline's default shell theme with ONLY the self-contained
    // loading stylesheet — the loading screen must not depend on the shell
    // theme cascade (which does not exist yet at first frame).
    pipeline.set_theme(LOADING_CSS);

    // dt_ms = 0.0: the loading screen is static (deterministic 35% progress
    // baked into the CSS); no animation/transition advance on the first frame.
    let (nodes, _animating) = pipeline.render_to_scene(&doc, 0, 0.0);

    let mut root = SceneNode::new(0, SceneNodeKind::Root, NodeProperties::new(screen));
    for node in nodes {
        root.add_child(node);
    }
    root
}
