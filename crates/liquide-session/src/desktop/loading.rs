//! Loading overlay scene — shown during first-frame startup.
//!
//! The loading screen is the literal first frame, rendered *before* the shell
//! DOM / theme cascade exists. As of the t86 full-CSS migration (phase P3) it
//! is no longer assembled from hardcoded `Rect::new` / `Color::new`
//! primitives: it is built from an embedded `loading.html` + `loading.css`
//! routed through a pre-shell CSS MINI-PIPELINE. See
//! [`super::loading_pipeline`] for the bootstrap details.

use liquide_compositor::scene::SceneNode;

use super::DesktopCompositor;
use super::loading_pipeline;

impl DesktopCompositor {
    /// Build a loading overlay scene — shown during first-frame startup.
    ///
    /// Runs the embedded loading DOM + CSS through the pre-shell mini-pipeline
    /// (same style/layout/paint engine as the shell, different bootstrap) and
    /// returns the resulting scene. This keeps the loading screen on the single
    /// CSS render path instead of the old imperative primitive assembly.
    pub(super) fn build_loading_scene(&self) -> SceneNode {
        loading_pipeline::build_loading_scene_nodes(self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::loading_pipeline::{self, LOADING_CSS};
    use liquide_compositor::scene::{SceneNode, SceneNodeKind};

    /// Recursively collect every node's bounds + kind for structural assertions.
    fn flatten(node: &SceneNode, out: &mut Vec<(f32, f32, f32, f32)>) {
        let b = &node.properties.bounds;
        out.push((b.x, b.y, b.width, b.height));
        for child in &node.children {
            flatten(child, out);
        }
    }

    /// Count the leaf paint nodes (non-Root) in the scene.
    fn paint_node_count(node: &SceneNode) -> usize {
        let mut n = match node.kind {
            SceneNodeKind::Root => 0,
            _ => 1,
        };
        for child in &node.children {
            n += paint_node_count(child);
        }
        n
    }

    /// The loading scene must be produced by the DOM/CSS mini-pipeline, not the
    /// old hardcoded primitives. A pipeline-produced scene emits MANY paint
    /// nodes (panel, accent, title, subtitle, progress track + fill, status,
    /// plus the full-viewport backdrop) parented under a Root. If this reverts
    /// to a single hardcoded fill — or the pipeline fails to emit the template
    /// structure — this fails.
    #[test]
    fn loading_scene_is_built_from_dom_css_pipeline() {
        let scene = loading_pipeline::build_loading_scene_nodes(1280, 800);

        assert!(
            matches!(scene.kind, SceneNodeKind::Root),
            "loading scene root must be a Root node"
        );

        // The template has 7 styled elements (screen, panel, accent, title,
        // subtitle, progress, fill, status) + text runs; a real pipeline run
        // emits a background fill for each painted box plus text glyphs. Far
        // more than the 0–1 nodes a stub/blank scene would have.
        let count = paint_node_count(&scene);
        assert!(
            count >= 6,
            "expected the mini-pipeline to emit the loading template structure \
             (>=6 paint nodes), got {count} — did it revert to a hardcoded/blank scene?"
        );

        // The full-viewport backdrop (loading-screen) must cover the framebuffer.
        let mut bounds = Vec::new();
        flatten(&scene, &mut bounds);
        let has_full_viewport = bounds
            .iter()
            .any(|(x, y, w, h)| *x <= 1.0 && *y <= 1.0 && *w >= 1279.0 && *h >= 799.0);
        assert!(
            has_full_viewport,
            "expected a full-viewport backdrop node at ~(0,0,1280,800) from \
             loading-screen CSS; bounds were {bounds:?}"
        );
    }

    /// TOOTH: a CSS change must move the rendered loading screen. We render the
    /// progress fill at its real CSS width (140), then render again with the
    /// fill width doubled in the stylesheet, and assert a node's width tracked
    /// the CSS edit. This fails if the scene is NOT actually driven by the CSS
    /// (e.g. if someone re-hardcodes the geometry while leaving the asset).
    #[test]
    fn css_width_change_moves_the_loading_scene() {
        use liquide_compositor::scene::SceneNode;
        use liquide_dom::html_parser::parse_html;
        use liquide_shell::pipeline::{DesktopPipeline, PipelineConfig};

        fn widths_for_css(css: &str) -> Vec<f32> {
            let doc = parse_html(loading_pipeline::LOADING_HTML);
            let mut p = DesktopPipeline::new(&PipelineConfig {
                width: 1280.0,
                height: 800.0,
                base_font_size: 14.0,
            });
            p.set_theme(css);
            let (nodes, _) = p.render_to_scene(&doc, 0, 0.0);
            let mut ws = Vec::new();
            fn collect(n: &SceneNode, out: &mut Vec<f32>) {
                out.push(n.properties.bounds.width);
                for c in &n.children {
                    collect(c, out);
                }
            }
            for n in &nodes {
                collect(n, &mut ws);
            }
            ws
        }

        let baseline = widths_for_css(LOADING_CSS);

        // Double the progress-fill width in the stylesheet.
        let widened = LOADING_CSS.replace(
            "loading-progress-fill {\n  display: flex;\n  width: 140;",
            "loading-progress-fill {\n  display: flex;\n  width: 280;",
        );
        assert_ne!(
            widened, LOADING_CSS,
            "test setup: the loading-progress-fill width rule was not found \
             verbatim in loading.css (asset changed?) — update this test"
        );
        let changed = widths_for_css(&widened);

        // Some node must now report the new 280 width that did NOT exist before.
        let had_280_before = baseline.iter().any(|w| (*w - 280.0).abs() < 0.5);
        let has_280_after = changed.iter().any(|w| (*w - 280.0).abs() < 0.5);
        assert!(
            !had_280_before && has_280_after,
            "a CSS width edit must move the emitted scene (CSS-driven proof). \
             before={baseline:?} after={changed:?}"
        );
    }
}
