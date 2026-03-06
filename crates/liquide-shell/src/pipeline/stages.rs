//! Pipeline execution — construction, configuration, and the Style → Layout → Paint stages.

use std::sync::{Arc, Mutex};

use liquide_compositor::scene::SceneNode;
use liquide_dom::Document;
use liquide_font_rasterizer::database::FontDatabase;
use liquide_layout::{DefaultTextMeasurer, LayoutInput, Size};

use liquide_style_engine::engine::ViewportSize;
use liquide_style_engine::StyleEngine;

use crate::font_text_measurer::FontTextMeasurer;
use crate::theme_loader;

use super::helpers::to_compositor_rect;
use super::{DesktopPipeline, PipelineConfig, PipelineOutput};

impl DesktopPipeline {
    /// Create a new pipeline with the default Liquid Glass theme loaded.
    pub fn new(config: &PipelineConfig) -> Self {
        let viewport = ViewportSize {
            width: config.width,
            height: config.height,
        };

        let mut style_engine = StyleEngine::new(viewport, config.base_font_size);

        // Load the default theme (Night)
        style_engine.add_stylesheet(theme_loader::default_theme_css());

        let layout_engine = liquide_layout::LayoutEngine::new(
            Size {
                width: config.width,
                height: config.height,
            },
            config.base_font_size,
        );

        Self {
            style_engine,
            layout_engine,
            painter: liquide_paint::Painter::new(),
            next_scene_id: 1_000_000,
            last_styles: None,
            last_layout: None,
            last_display_list: None,
            pending_images: Vec::new(),
            font_db: None,
        }
    }

    /// Return the list of image URLs referenced during the last scene build.
    /// Each entry is `(image_id, url)`. The host should load each image and
    /// call `renderer.register_image(image_id, data)` with the decoded bytes.
    pub fn pending_images(&self) -> &[(u64, String)] {
        &self.pending_images
    }

    /// Load an additional stylesheet (e.g. a user theme override).
    pub fn add_stylesheet(&mut self, css: &str) {
        self.style_engine.add_stylesheet(css);
    }

    /// Get the list of @font-face rules parsed from loaded stylesheets.
    /// The caller (e.g. DesktopCompositor) can iterate these and load fonts
    /// into the FontDatabase.
    pub fn font_faces(&self) -> &[liquide_style_engine::engine::PreparedFontFace] {
        self.style_engine.font_faces()
    }

    /// Replace styles with a named theme preset.
    pub fn set_theme(&mut self, preset_css: &str) {
        self.style_engine =
            StyleEngine::new(self.style_engine.viewport, self.style_engine.base_font_size);
        self.style_engine.add_stylesheet(preset_css);
    }

    /// Update viewport dimensions (e.g. on monitor resolution change).
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.style_engine
            .set_viewport(ViewportSize { width, height });
        self.layout_engine.viewport = Size { width, height };
    }

    /// Set preferred color scheme used by style media queries.
    pub fn set_preferred_color_scheme(&mut self, scheme: &str) {
        self.style_engine.set_preferred_color_scheme(scheme);
    }

    /// Set the font database for real text measurement.
    ///
    /// When set, the pipeline will use real glyph metrics from loaded
    /// fonts instead of the approximate `char_width = font_size * 0.6`
    /// fallback.
    pub fn set_font_db(&mut self, db: Arc<Mutex<FontDatabase>>) {
        self.font_db = Some(db);
    }

    /// Run the full pipeline: Style → Layout → Paint.
    ///
    /// Returns the style map, layout tree, and display list.
    pub fn run(&mut self, doc: &Document) -> PipelineOutput {
        // Use real font metrics when a font database is available.
        let font_measurer: Option<FontTextMeasurer> =
            self.font_db.as_ref().map(|db| FontTextMeasurer::new(Arc::clone(db)));
        let text_measurer: &dyn liquide_layout::TextMeasurer = match &font_measurer {
            Some(fm) => fm,
            None => &DefaultTextMeasurer,
        };
        let image_measurer = liquide_layout::DefaultImageMeasurer;

        let has_style_work = !doc.dirty.style.is_empty();
        let has_layout_work = !doc.dirty.layout.is_empty();
        let has_paint_work = !doc.dirty.paint.is_empty();

        // 1. Style
        let mut styles = if has_style_work {
            if let Some(mut cached) = self.last_styles.clone() {
                let changed: Vec<liquide_dom::NodeId> = doc.dirty.style.iter().copied().collect();
                self.style_engine.invalidate(doc, &changed, &mut cached);
                cached
            } else {
                self.style_engine.restyle_all(doc)
            }
        } else if let Some(cached) = self.last_styles.clone() {
            cached
        } else {
            self.style_engine.restyle_all(doc)
        };

        // 2. Layout
        let recompute_layout = has_style_work || has_layout_work || self.last_layout.is_none();
        let layout = if has_style_work || self.last_layout.is_none() {
            self.layout_engine
                .layout(doc, &styles, text_measurer, &image_measurer)
        } else if has_layout_work {
            let mut layout = self.last_layout.clone().unwrap_or_default();
            let input = LayoutInput::new(doc, &styles, text_measurer, &image_measurer);

            let mut dirty_layout_nodes: Vec<liquide_dom::NodeId> =
                doc.dirty.layout.iter().copied().collect();
            dirty_layout_nodes.sort_by_key(|node_id| doc.ancestors(*node_id).len());

            // If both an ancestor and descendant are dirty, relayout the ancestor only.
            let mut relayout_roots: Vec<liquide_dom::NodeId> = Vec::new();
            for node_id in dirty_layout_nodes {
                let ancestors = doc.ancestors(node_id);
                if relayout_roots
                    .iter()
                    .any(|selected| ancestors.iter().any(|a| a == selected))
                {
                    continue;
                }
                relayout_roots.push(node_id);
            }

            for node_id in relayout_roots {
                layout = self.layout_engine.relayout_subtree(&input, node_id, &layout);
            }

            layout
        } else {
            self.last_layout.clone().unwrap_or_default()
        };

        // 2b. Populate container sizes for the next @container evaluation.
        // Elements with container-type != normal get their resolved dimensions
        // stored in the StyleMap so that `evaluate_container_condition` can use
        // real dimensions instead of falling back to the viewport.
        for layout_box in &layout.boxes {
            if let Some(style) = styles.get(layout_box.node) {
                if style.is_container_query_host() {
                    styles.set_container_size(
                        layout_box.node,
                        layout_box.content_rect.width,
                        layout_box.content_rect.height,
                    );
                }
            }
        }

        // 3. Paint
        let recompute_paint = recompute_layout || has_paint_work || self.last_display_list.is_none();
        let display_list = if recompute_paint {
            self.painter.paint(doc, &layout, &styles)
        } else {
            self.last_display_list.clone().unwrap_or_default()
        };

        self.last_styles = Some(styles.clone());
        self.last_layout = Some(layout.clone());
        self.last_display_list = Some(display_list.clone());

        PipelineOutput {
            styles,
            layout,
            display_list,
        }
    }

    /// Run the full pipeline and convert the result to compositor SceneNodes.
    ///
    /// Glass SceneNodes are generated for elements with `blur-radius` CSS
    /// property. These are placed *before* the element's normal paint output
    /// so the blur effect renders behind the content.
    pub fn render_to_scene(&mut self, doc: &Document, base_z: u32) -> Vec<SceneNode> {
        let (nodes, _output) = self.render_to_scene_with_output(doc, base_z);
        nodes
    }

    /// Like [`render_to_scene`] but also returns the pipeline output
    /// (styles + layout) for downstream use (e.g. hit-testing).
    pub fn render_to_scene_with_output(
        &mut self,
        doc: &Document,
        base_z: u32,
    ) -> (Vec<SceneNode>, PipelineOutput) {
        // Reset scene ID counter each frame so glass/blur nodes get stable IDs.
        // Without this, the blur_worker cache grows unbounded since each frame
        // generates new IDs that never match old cache entries.
        self.next_scene_id = 1_000_000;

        let output = self.run(doc);

        // Collect Glass nodes from elements with x_blur_radius > 0.
        let glass_nodes = self.extract_glass_nodes(&output, base_z);
        let glass_count = glass_nodes.len() as u32;

        // Convert paint output to scene nodes, offset z by glass count.
        let mut nodes = glass_nodes;
        let paint_nodes = self.display_list_to_scene(&output.display_list, base_z + glass_count);
        nodes.extend(paint_nodes);

        (nodes, output)
    }

    /// Generate Glass SceneNodes for DOM elements that have `x_blur_radius > 0`
    /// in their computed style. Uses the layout tree to get the element's rect.
    fn extract_glass_nodes(&mut self, output: &PipelineOutput, base_z: u32) -> Vec<SceneNode> {
        use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNodeKind};

        let mut glass_nodes = Vec::new();
        let mut z = base_z;

        for layout_box in &output.layout.boxes {
            if let Some(style) = output.styles.get(layout_box.node) {
                if style.x_blur_radius > 0.0 {
                    let abs_border = output.layout.absolute_border_rect(layout_box.id);
                    let rect = to_compositor_rect(&abs_border);
                    // Skip zero-area boxes
                    if rect.width <= 0.0 || rect.height <= 0.0 {
                        continue;
                    }

                    let tint_color = style.x_glass_tint.unwrap_or_else(|| {
                        // Fall back to background_color if no glass-tint
                        style.background_color
                    });

                    let id = self.alloc_id();
                    let glass = SceneNode::new(
                        id,
                        SceneNodeKind::Glass(GlassParams {
                            blur_radius: style.x_blur_radius as u32,
                            tint_color,
                            inner_glow: true,
                            parallax: false,
                        }),
                        NodeProperties::new(rect).with_z_order(z),
                    );
                    glass_nodes.push(glass);
                    z += 1;
                }
            }
        }

        glass_nodes
    }

    pub(super) fn alloc_id(&mut self) -> u64 {
        let id = self.next_scene_id;
        self.next_scene_id += 1;
        id
    }
}
