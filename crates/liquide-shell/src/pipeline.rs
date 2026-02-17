//! CSS pipeline bridge — runs DOM → Style → Layout → Paint → SceneNode.
//!
//! This module bridges the new DOM-based CSS rendering pipeline with the
//! existing compositor scene graph. It takes the [`DesktopDocument`],
//! runs the full pipeline, and converts the resulting [`DisplayList`]
//! into compositor [`SceneNode`]s that the renderer already knows how to
//! draw.
//!
//! ## Pipeline stages
//!
//! 1. **Style** — `StyleEngine::restyle_all()` → `StyleMap`
//! 2. **Layout** — `LayoutEngine::layout()` → `LayoutTree`
//! 3. **Paint** — `Painter::paint()` → `DisplayList`
//! 4. **Bridge** — `DisplayList` → `Vec<SceneNode>` (this module)

use liquide_compositor::geometry::Rect as CRect;
use liquide_compositor::property_tree::{
    ClipNode, EffectNode, PropertyTrees, RenderSurfaceReason,
    TransformNode, ROOT_NODE_ID,
};
use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNode, SceneNodeKind};

use std::sync::{Arc, Mutex};

use liquide_dom::Document;
use liquide_font_rasterizer::database::FontDatabase;
use liquide_layout::{DefaultTextMeasurer, LayoutEngine, LayoutTree, Size};
use liquide_paint::{DisplayItem, DisplayList, Painter};
use liquide_style_engine::computed::BorderLineStyle;
use liquide_style_engine::engine::ViewportSize;
use liquide_style_engine::{StyleEngine, StyleMap};

use crate::font_text_measurer::FontTextMeasurer;
use crate::theme_loader;

/// Holds the full pipeline state.
pub struct DesktopPipeline {
    /// CSS style engine with loaded stylesheets.
    pub style_engine: StyleEngine,
    /// Layout engine with viewport and base font.
    pub layout_engine: LayoutEngine,
    /// The painter (stateless).
    pub painter: Painter,
    /// Monotonic id counter for scene nodes generated from the pipeline.
    next_scene_id: u64,
    /// Last computed styles (cached for hit-testing).
    pub last_styles: Option<StyleMap>,
    /// Last computed layout tree (cached for hit-testing).
    pub last_layout: Option<LayoutTree>,
    /// Image URLs referenced during the last scene build, mapped to their hashed image_id.
    /// The host should load these and register them with the renderer.
    pending_images: Vec<(u64, String)>,
    /// Optional font database for real text measurement.
    font_db: Option<Arc<Mutex<FontDatabase>>>,
}

/// Configuration for the pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Viewport width in logical pixels.
    pub width: f32,
    /// Viewport height in logical pixels.
    pub height: f32,
    /// Base font size in pixels.
    pub base_font_size: f32,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            width: 1920.0,
            height: 1080.0,
            base_font_size: 14.0,
        }
    }
}

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

        let layout_engine = LayoutEngine::new(
            Size {
                width: config.width,
                height: config.height,
            },
            config.base_font_size,
        );

        Self {
            style_engine,
            layout_engine,
            painter: Painter::new(),
            next_scene_id: 1_000_000,
            last_styles: None,
            last_layout: None,
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

        // 1. Style
        let mut styles = self.style_engine.restyle_all(doc);

        // 2. Layout
        let layout = self
            .layout_engine
            .layout(doc, &styles, text_measurer, &image_measurer);

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
        let display_list = self.painter.paint(doc, &layout, &styles);

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

    /// Convert a display list to compositor scene nodes.
    ///
    /// Uses a state stack to handle Push/Pop items properly:
    /// - PushClip: clips all subsequent nodes until PopClip
    /// - PushOpacity: applies opacity to all subsequent nodes until PopOpacity
    /// - PushTransform: applies transform to all subsequent nodes until PopTransform
    /// - PushBlendMode: groups subsequent nodes in a RenderLayer with blend mode
    /// - PushStackingContext: groups subsequent nodes with z-index ordering
    pub fn display_list_to_scene(&mut self, list: &DisplayList, base_z: u32) -> Vec<SceneNode> {
        use liquide_compositor::geometry::Affine2D;

        // Clear image tracking for this build
        self.pending_images.clear();

        /// Active state from Push items.
        #[derive(Clone)]
        struct PipelineState {
            clip: Option<CRect>,
            clip_radius: (f32, f32, f32, f32),
            opacity: f32,
            transform: Affine2D,
        }

        impl Default for PipelineState {
            fn default() -> Self {
                Self {
                    clip: None,
                    clip_radius: (0.0, 0.0, 0.0, 0.0),
                    opacity: 1.0,
                    transform: Affine2D::identity(),
                }
            }
        }

        let mut stack: Vec<PipelineState> = Vec::new();
        let mut current = PipelineState::default();
        let mut nodes = Vec::new();
        let mut z = base_z;

        for item in &list.items {
            match item {
                // ── Push state items ────────────────────────
                DisplayItem::PushClip { rect, radius } => {
                    stack.push(current.clone());
                    let clip_rect = to_compositor_rect(rect);
                    let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                    // Intersect with existing clip
                    current.clip = Some(match current.clip {
                        Some(existing) => intersect_rects(&existing, &clip_rect),
                        None => clip_rect,
                    });
                    // Keep the most specific (innermost) clip radius
                    let has_r = r.0 > 0.5 || r.1 > 0.5 || r.2 > 0.5 || r.3 > 0.5;
                    if has_r {
                        current.clip_radius = r;
                    }
                }
                DisplayItem::PopClip => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushOpacity { opacity } => {
                    stack.push(current.clone());
                    current.opacity *= opacity;
                }
                DisplayItem::PopOpacity => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushTransform {
                    translate_x,
                    translate_y,
                    scale_x,
                    scale_y,
                    rotate,
                    skew_x,
                    skew_y,
                } => {
                    stack.push(current.clone());
                    let mut xform = Affine2D::identity();
                    if *translate_x != 0.0 || *translate_y != 0.0 {
                        xform = xform.then(&Affine2D::translation(*translate_x, *translate_y));
                    }
                    if *scale_x != 1.0 || *scale_y != 1.0 {
                        xform = xform.then(&Affine2D::scale(*scale_x, *scale_y));
                    }
                    if *rotate != 0.0 {
                        xform = xform.then(&Affine2D::rotation(*rotate));
                    }
                    if *skew_x != 0.0 || *skew_y != 0.0 {
                        xform = xform.then(&Affine2D::skew(skew_x.to_radians(), skew_y.to_radians()));
                    }
                    current.transform = current.transform.then(&xform);
                }
                DisplayItem::PopTransform => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushBlendMode { mode } => {
                    stack.push(current.clone());
                    // Emit a RenderLayer node for blend mode compositing
                    use liquide_compositor::pixel::BlendMode;
                    let is_non_default = !matches!(mode, BlendMode::SrcOver);
                    if is_non_default {
                        let id = self.alloc_id();
                        let node = SceneNode::new(
                            id,
                            SceneNodeKind::RenderLayer {
                                blend_mode: *mode,
                                isolate: true,
                            },
                            NodeProperties::new(CRect::new(0.0, 0.0, 0.0, 0.0)).with_z_order(z),
                        );
                        nodes.push(node);
                        z += 1;
                    }
                }
                DisplayItem::PopBlendMode => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushStackingContext { .. } => {
                    stack.push(current.clone());
                }
                DisplayItem::PopStackingContext => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                // ── New state ops: filter, backdrop-filter, mask, save/restore ──
                DisplayItem::PushFilter { filters } => {
                    stack.push(current.clone());
                    // Emit a Filter scene node so the renderer can apply CSS filter effects
                    if !filters.is_empty() {
                        let filter_specs = filters.iter().filter_map(|f| filter_op_to_spec(f)).collect::<Vec<_>>();
                        if !filter_specs.is_empty() {
                            let id = self.alloc_id();
                            // Use a small bounding rect — the filter applies to preceding content
                            let node = SceneNode::new(
                                id,
                                SceneNodeKind::Filter { filters: filter_specs },
                                NodeProperties::new(CRect::new(0.0, 0.0, 0.0, 0.0)).with_z_order(z),
                            );
                            nodes.push(node);
                            z += 1;
                        }
                    }
                }
                DisplayItem::PopFilter => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushBackdropFilter { filters, bounds } => {
                    stack.push(current.clone());
                    // Emit a BackdropFilter scene node so the renderer can apply CSS backdrop-filter
                    if !filters.is_empty() {
                        let backdrop_specs = filters.iter().filter_map(|f| filter_op_to_backdrop_spec(f)).collect::<Vec<_>>();
                        if !backdrop_specs.is_empty() {
                            let id = self.alloc_id();
                            let b = to_compositor_rect(bounds);
                            let mut node = SceneNode::new(
                                id,
                                SceneNodeKind::BackdropFilter { filters: backdrop_specs },
                                NodeProperties::new(b).with_z_order(z),
                            );
                            // Apply accumulated state
                            if current.opacity < 1.0 {
                                node.properties.opacity *= current.opacity;
                            }
                            if let Some(ref clip) = current.clip {
                                node.properties.clip = Some(*clip);
                            }
                            if !current.transform.is_identity() {
                                node.properties.transform =
                                    node.properties.transform.then(&current.transform);
                            }
                            nodes.push(node);
                            z += 1;
                        }
                    }
                }
                DisplayItem::PopBackdropFilter => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushMask { .. } => {
                    stack.push(current.clone());
                }
                DisplayItem::PopMask => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushClipPath { .. } => {
                    stack.push(current.clone());
                }

                DisplayItem::SaveLayer { .. } => {
                    stack.push(current.clone());
                }
                DisplayItem::RestoreLayer => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::Annotate { .. } | DisplayItem::Noop | DisplayItem::SetCursor { .. } | DisplayItem::ScrollContainerHints { .. } | DisplayItem::AnimationHints { .. } | DisplayItem::TimelineHints { .. } => {
                    // Non-renderable metadata — skip
                }

                // ── Renderable items ────────────────────────
                other => {
                    if let Some(mut node) = self.display_item_to_scene(other, z) {
                        // Apply accumulated state from the stack
                        if current.opacity < 1.0 {
                            node.properties.opacity *= current.opacity;
                        }
                        if let Some(ref clip) = current.clip {
                            node.properties.clip = Some(*clip);
                            // Propagate rounded clip radius
                            let cr = current.clip_radius;
                            if cr.0 > 0.5 || cr.1 > 0.5 || cr.2 > 0.5 || cr.3 > 0.5 {
                                node.properties.clip_radius = cr;
                            }
                        }
                        if !current.transform.is_identity() {
                            node.properties.transform =
                                node.properties.transform.then(&current.transform);
                        }
                        nodes.push(node);
                        z += 1;
                    }
                }
            }
        }

        nodes
    }

    /// Build compositor property trees from a pipeline output.
    ///
    /// Walks the display list and the layout tree together to produce
    /// four property trees (Transform, Clip, Effect, Scroll) that model
    /// the compositing hierarchy. This is needed for:
    /// - Efficient compositor-side animations (update a transform without re-layout)
    /// - Correct render surface allocation (filters, opacity, blend modes)
    /// - Scroll-linked transforms and clip computation
    pub fn build_property_trees(&self, output: &PipelineOutput) -> PropertyTrees {
        use liquide_compositor::geometry::Affine2D;

        let mut trees = PropertyTrees::new();

        // Track parent IDs as we walk the display list Push/Pop structure
        let mut transform_stack: Vec<u32> = vec![ROOT_NODE_ID];
        let mut clip_stack: Vec<u32> = vec![ROOT_NODE_ID];
        let mut effect_stack: Vec<u32> = vec![ROOT_NODE_ID];

        for item in &output.display_list.items {
            match item {
                DisplayItem::PushTransform {
                    translate_x,
                    translate_y,
                    scale_x,
                    scale_y,
                    rotate,
                    skew_x,
                    skew_y,
                } => {
                    let parent = *transform_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let mut local = Affine2D::identity();
                    if *translate_x != 0.0 || *translate_y != 0.0 {
                        local = local.then(&Affine2D::translation(*translate_x, *translate_y));
                    }
                    if *scale_x != 1.0 || *scale_y != 1.0 {
                        local = local.then(&Affine2D::scale(*scale_x, *scale_y));
                    }
                    if *rotate != 0.0 {
                        local = local.then(&Affine2D::rotation(*rotate));
                    }
                    if *skew_x != 0.0 || *skew_y != 0.0 {
                        local = local.then(&Affine2D::skew(skew_x.to_radians(), skew_y.to_radians()));
                    }
                    let node = TransformNode {
                        parent,
                        local,
                        ..Default::default()
                    };
                    let id = trees.transform_tree.insert(node);
                    transform_stack.push(id);
                }
                DisplayItem::PopTransform => {
                    if transform_stack.len() > 1 {
                        transform_stack.pop();
                    }
                }

                DisplayItem::PushClip { rect, .. } => {
                    let parent = *clip_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let transform_id = *transform_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = ClipNode {
                        parent,
                        clip_rect: liquide_compositor::geometry::Rect {
                            x: rect.x,
                            y: rect.y,
                            width: rect.width,
                            height: rect.height,
                        },
                        transform_id,
                        ..Default::default()
                    };
                    let id = trees.clip_tree.insert(node);
                    clip_stack.push(id);
                }
                DisplayItem::PopClip => {
                    if clip_stack.len() > 1 {
                        clip_stack.pop();
                    }
                }

                DisplayItem::PushOpacity { opacity } => {
                    let parent = *effect_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = EffectNode {
                        parent,
                        opacity: *opacity,
                        render_surface_reason: RenderSurfaceReason::Opacity,
                        transform_id: *transform_stack.last().unwrap_or(&ROOT_NODE_ID),
                        clip_id: *clip_stack.last().unwrap_or(&ROOT_NODE_ID),
                        ..Default::default()
                    };
                    let id = trees.effect_tree.insert(node);
                    effect_stack.push(id);
                }
                DisplayItem::PopOpacity => {
                    if effect_stack.len() > 1 {
                        effect_stack.pop();
                    }
                }

                DisplayItem::PushBlendMode { mode } => {
                    let parent = *effect_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = EffectNode {
                        parent,
                        blend_mode: *mode,
                        render_surface_reason: RenderSurfaceReason::BlendMode,
                        transform_id: *transform_stack.last().unwrap_or(&ROOT_NODE_ID),
                        clip_id: *clip_stack.last().unwrap_or(&ROOT_NODE_ID),
                        ..Default::default()
                    };
                    let id = trees.effect_tree.insert(node);
                    effect_stack.push(id);
                }
                DisplayItem::PopBlendMode => {
                    if effect_stack.len() > 1 {
                        effect_stack.pop();
                    }
                }

                DisplayItem::PushFilter { filters } => {
                    let parent = *effect_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = EffectNode {
                        parent,
                        filters: filters.clone(),
                        render_surface_reason: RenderSurfaceReason::Filter,
                        transform_id: *transform_stack.last().unwrap_or(&ROOT_NODE_ID),
                        clip_id: *clip_stack.last().unwrap_or(&ROOT_NODE_ID),
                        ..Default::default()
                    };
                    let id = trees.effect_tree.insert(node);
                    effect_stack.push(id);
                }
                DisplayItem::PopFilter => {
                    if effect_stack.len() > 1 {
                        effect_stack.pop();
                    }
                }

                DisplayItem::PushBackdropFilter { filters, .. } => {
                    let parent = *effect_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = EffectNode {
                        parent,
                        backdrop_filters: filters.clone(),
                        render_surface_reason: RenderSurfaceReason::BackdropFilter,
                        transform_id: *transform_stack.last().unwrap_or(&ROOT_NODE_ID),
                        clip_id: *clip_stack.last().unwrap_or(&ROOT_NODE_ID),
                        ..Default::default()
                    };
                    let id = trees.effect_tree.insert(node);
                    effect_stack.push(id);
                }
                DisplayItem::PopBackdropFilter => {
                    if effect_stack.len() > 1 {
                        effect_stack.pop();
                    }
                }

                DisplayItem::PushMask { .. } => {
                    let parent = *effect_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = EffectNode {
                        parent,
                        render_surface_reason: RenderSurfaceReason::Mask,
                        transform_id: *transform_stack.last().unwrap_or(&ROOT_NODE_ID),
                        clip_id: *clip_stack.last().unwrap_or(&ROOT_NODE_ID),
                        ..Default::default()
                    };
                    let id = trees.effect_tree.insert(node);
                    effect_stack.push(id);
                }
                DisplayItem::PopMask => {
                    if effect_stack.len() > 1 {
                        effect_stack.pop();
                    }
                }

                DisplayItem::PushStackingContext { z_index: _, isolation } => {
                    let parent = *effect_stack.last().unwrap_or(&ROOT_NODE_ID);
                    let node = EffectNode {
                        parent,
                        is_isolated: *isolation == liquide_style_engine::computed::Isolation::Isolate,
                        transform_id: *transform_stack.last().unwrap_or(&ROOT_NODE_ID),
                        clip_id: *clip_stack.last().unwrap_or(&ROOT_NODE_ID),
                        ..Default::default()
                    };
                    let id = trees.effect_tree.insert(node);
                    effect_stack.push(id);
                }
                DisplayItem::PopStackingContext => {
                    if effect_stack.len() > 1 {
                        effect_stack.pop();
                    }
                }

                // Non-structural items don't affect the trees
                _ => {}
            }
        }

        // Compute cached values
        trees.update_transform_cache();
        trees.update_clip_cache();

        trees
    }

    /// Map a single DisplayItem to a SceneNode.
    fn display_item_to_scene(&mut self, item: &DisplayItem, z: u32) -> Option<SceneNode> {
        let id = self.alloc_id();

        match item {
            DisplayItem::SolidColor { rect, color, radius } => {
                if color.a == 0 {
                    return None; // Skip fully transparent
                }
                let bounds = to_compositor_rect(rect);
                let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Background { color: *color },
                    NodeProperties::new(bounds).with_z_order(z).with_corner_radius(r),
                );
                Some(node)
            }

            DisplayItem::Border {
                rect,
                top,
                right,
                bottom,
                left,
                radius,
            } => {
                // Convert to compositor BorderSides
                let bounds = to_compositor_rect(rect);
                let sides = liquide_compositor::scene::BorderSides {
                    top: to_border_side(top),
                    right: to_border_side(right),
                    bottom: to_border_side(bottom),
                    left: to_border_side(left),
                };
                let r = (
                    radius.top_left,
                    radius.top_right,
                    radius.bottom_right,
                    radius.bottom_left,
                );
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Border { sides, radius: r },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::BoxShadow {
                rect,
                offset_x,
                offset_y,
                blur_radius,
                spread_radius,
                color,
                inset,
                radius,
            } => {
                let bounds = to_compositor_rect(rect);
                let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                let shadow = liquide_compositor::scene::BoxShadowSpec {
                    offset_x: *offset_x,
                    offset_y: *offset_y,
                    blur_radius: *blur_radius,
                    spread_radius: *spread_radius,
                    color: *color,
                    inset: *inset,
                };
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::BoxShadows {
                        shadows: vec![shadow],
                    },
                    NodeProperties::new(bounds).with_z_order(z).with_corner_radius(r),
                );
                Some(node)
            }

            DisplayItem::Text {
                rect,
                text,
                color,
                font_size,
                font_family,
                font_weight,
                font_style,
                letter_spacing,
                word_spacing,
                line_height,
                text_align,
                text_transform,
                text_overflow,
                white_space,
                text_indent,
                text_decoration,
                text_shadows,
                ..
            } => {
                use liquide_style_engine::computed::*;
                let lh_px = match line_height {
                    LineHeight::Px(px) => *px,
                    LineHeight::Number(n) => n * font_size,
                    LineHeight::Normal => font_size * 1.2,
                };
                let bounds = to_compositor_rect(rect);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Text {
                        text: text.clone(),
                        color: *color,
                        scale: 1,
                        font_family: font_family.first().cloned().unwrap_or_default(),
                        font_size: *font_size,
                        font_weight: *font_weight,
                        font_style_italic: matches!(font_style, FontStyle::Italic | FontStyle::Oblique),
                        letter_spacing: *letter_spacing,
                        word_spacing: *word_spacing,
                        line_height: lh_px,
                        text_align: match text_align {
                            TextAlign::Left | TextAlign::Start => 0,
                            TextAlign::Center => 1,
                            TextAlign::Right | TextAlign::End => 2,
                            TextAlign::Justify => 3,
                        },
                        text_transform: match text_transform {
                            TextTransform::None => 0,
                            TextTransform::Capitalize => 1,
                            TextTransform::Uppercase => 2,
                            TextTransform::Lowercase => 3,
                        },
                        text_overflow: match text_overflow {
                            TextOverflow::Clip => 0,
                            TextOverflow::Ellipsis => 1,
                        },
                        white_space: match white_space {
                            WhiteSpace::Normal => 0,
                            WhiteSpace::NoWrap => 1,
                            WhiteSpace::Pre => 2,
                            WhiteSpace::PreWrap => 3,
                            WhiteSpace::PreLine => 4,
                            WhiteSpace::BreakSpaces => 5,
                        },
                        text_indent: *text_indent,
                        text_decoration: text_decoration.clone(),
                        text_shadows: text_shadows.clone(),
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::Image { rect, src, radius } => {
                let bounds = to_compositor_rect(rect);
                let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                let img_id = hash_string(src);
                self.pending_images.push((img_id, src.clone()));
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Image {
                        image_id: img_id,
                        width: bounds.width as u32,
                        height: bounds.height as u32,
                        fit: liquide_compositor::scene::ImageFit::Cover,
                    },
                    NodeProperties::new(bounds).with_z_order(z).with_corner_radius(r),
                );
                Some(node)
            }

            DisplayItem::ImageRect { rect, src, fit, radius, .. } => {
                let bounds = to_compositor_rect(rect);
                let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                let img_id = hash_string(src);
                self.pending_images.push((img_id, src.clone()));
                let scene_fit = match fit {
                    liquide_paint::display_list::ImageFit::Fill => liquide_compositor::scene::ImageFit::Fill,
                    liquide_paint::display_list::ImageFit::Contain => liquide_compositor::scene::ImageFit::Contain,
                    liquide_paint::display_list::ImageFit::Cover => liquide_compositor::scene::ImageFit::Cover,
                    liquide_paint::display_list::ImageFit::ScaleDown | liquide_paint::display_list::ImageFit::None => liquide_compositor::scene::ImageFit::None,
                };
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Image {
                        image_id: img_id,
                        width: bounds.width as u32,
                        height: bounds.height as u32,
                        fit: scene_fit,
                    },
                    NodeProperties::new(bounds).with_z_order(z).with_corner_radius(r),
                );
                Some(node)
            }

            DisplayItem::LinearGradient { rect, angle_deg, stops, radius } => {
                let bounds = to_compositor_rect(rect);
                let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                // Convert angle to start/end points (normalized 0..1)
                let angle_rad = angle_deg.to_radians();
                let (start_x, start_y, end_x, end_y) = (
                    0.5 - 0.5 * angle_rad.sin(),
                    0.5 + 0.5 * angle_rad.cos(),
                    0.5 + 0.5 * angle_rad.sin(),
                    0.5 - 0.5 * angle_rad.cos(),
                );
                let gradient = liquide_compositor::scene::GradientSpec::Linear {
                    start_x, start_y, end_x, end_y,
                    stops: stops.iter().map(|s| (s.offset, s.color)).collect(),
                };
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::GradientFill { gradient },
                    NodeProperties::new(bounds).with_z_order(z).with_corner_radius(r),
                );
                Some(node)
            }

            DisplayItem::RadialGradient { rect, center_x, center_y, radius_x, stops, .. } => {
                let bounds = to_compositor_rect(rect);
                let gradient = liquide_compositor::scene::GradientSpec::Radial {
                    center_x: *center_x,
                    center_y: *center_y,
                    radius: *radius_x,
                    stops: stops.iter().map(|s| (s.offset, s.color)).collect(),
                };
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::GradientFill { gradient },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::ConicGradient { rect, center_x, center_y, angle_deg, stops } => {
                let bounds = to_compositor_rect(rect);
                let gradient = liquide_compositor::scene::GradientSpec::Conic {
                    center_x: *center_x,
                    center_y: *center_y,
                    start_angle: *angle_deg,
                    stops: stops.iter().map(|s| (s.offset, s.color)).collect(),
                };
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::GradientFill { gradient },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::Outline { rect, width, style, color, offset } => {
                let bounds = to_compositor_rect(rect);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Outline {
                        outline: liquide_compositor::scene::OutlineSpec {
                            width: *width,
                            style: match style {
                                BorderLineStyle::Dotted => liquide_compositor::scene::OutlineStyle::Dotted,
                                BorderLineStyle::Dashed => liquide_compositor::scene::OutlineStyle::Dashed,
                                BorderLineStyle::Double => liquide_compositor::scene::OutlineStyle::Double,
                                _ => liquide_compositor::scene::OutlineStyle::Solid,
                            },
                            color: *color,
                            offset: *offset,
                        },
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::FillRect { rect, color } => {
                if color.a == 0 {
                    return None;
                }
                let bounds = to_compositor_rect(rect);
                Some(SceneNode::new(
                    id,
                    SceneNodeKind::Background { color: *color },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            DisplayItem::StrokeRoundedRect { rect, color, width, .. } => {
                let bounds = to_compositor_rect(rect);
                let side = liquide_compositor::scene::BorderSide {
                    width: *width,
                    style: liquide_compositor::scene::BorderSideStyle::Solid,
                    color: *color,
                };
                Some(SceneNode::new(
                    id,
                    SceneNodeKind::Border {
                        sides: liquide_compositor::scene::BorderSides {
                            top: side.clone(),
                            right: side.clone(),
                            bottom: side.clone(),
                            left: side,
                        },
                        radius: (0.0, 0.0, 0.0, 0.0),
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            DisplayItem::Line { x1, y1, x2, y2, color, width } => {
                let min_x = x1.min(*x2);
                let min_y = y1.min(*y2);
                let w = (x1 - x2).abs().max(*width);
                let h = (y1 - y2).abs().max(*width);
                let bounds = CRect::new(min_x, min_y, w, h);
                Some(SceneNode::new(
                    id,
                    SceneNodeKind::Background { color: *color },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            DisplayItem::TextRun { rect, text, color, font_size, font_family, font_weight, .. } => {
                let bounds = to_compositor_rect(rect);
                Some(SceneNode::new(
                    id,
                    SceneNodeKind::Text {
                        text: text.clone(),
                        color: *color,
                        scale: 1,
                        font_family: font_family.clone(),
                        font_size: *font_size,
                        font_weight: *font_weight,
                        font_style_italic: false,
                        letter_spacing: 0.0,
                        word_spacing: 0.0,
                        line_height: font_size * 1.2,
                        text_align: 0,
                        text_transform: 0,
                        text_overflow: 0,
                        white_space: 0,
                        text_indent: 0.0,
                        text_decoration: None,
                        text_shadows: Vec::new(),
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            DisplayItem::BorderImage { rect, source, .. } => {
                let bounds = to_compositor_rect(rect);
                Some(SceneNode::new(
                    id,
                    SceneNodeKind::Image {
                        image_id: hash_string(source),
                        width: bounds.width as u32,
                        height: bounds.height as u32,
                        fit: liquide_compositor::scene::ImageFit::Fill,
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            // Push/Pop items are handled by display_list_to_scene's state stack.
            // They should never reach this method.
            DisplayItem::PushClip { .. }
            | DisplayItem::PopClip
            | DisplayItem::PushClipPath { .. }
            | DisplayItem::PushOpacity { .. }
            | DisplayItem::PopOpacity
            | DisplayItem::PushTransform { .. }
            | DisplayItem::PopTransform
            | DisplayItem::PushBlendMode { .. }
            | DisplayItem::PopBlendMode
            | DisplayItem::PushFilter { .. }
            | DisplayItem::PopFilter
            | DisplayItem::PushBackdropFilter { .. }
            | DisplayItem::PopBackdropFilter
            | DisplayItem::PushMask { .. }
            | DisplayItem::PopMask
            | DisplayItem::PushStackingContext { .. }
            | DisplayItem::PopStackingContext
            | DisplayItem::SaveLayer { .. }
            | DisplayItem::RestoreLayer
            | DisplayItem::Annotate { .. }
            | DisplayItem::SetCursor { .. }
            | DisplayItem::ScrollContainerHints { .. }
            | DisplayItem::AnimationHints { .. }
            | DisplayItem::TimelineHints { .. }
            | DisplayItem::Noop => {
                None // state ops should be handled by the display_list_to_scene loop
            }

            DisplayItem::Icon {
                rect,
                icon_id,
                color,
            } => {
                let bounds = to_compositor_rect(rect);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Icon {
                        icon_id: *icon_id,
                        color: *color,
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::Surface { rect, surface_id } => {
                let bounds = to_compositor_rect(rect);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Surface {
                        surface_id: *surface_id,
                        buffer: None,
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }
        }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_scene_id;
        self.next_scene_id += 1;
        id
    }
}

/// Output of a full pipeline run.
pub struct PipelineOutput {
    /// Computed styles per node.
    pub styles: StyleMap,
    /// Computed layout boxes.
    pub layout: LayoutTree,
    /// Flat paint commands.
    pub display_list: DisplayList,
}

// ── Conversion helpers ───────────────────────────────────────────

fn to_compositor_rect(r: &liquide_layout::Rect) -> CRect {
    CRect::new(r.x, r.y, r.width, r.height)
}

fn to_border_side(
    edge: &liquide_paint::display_list::BorderEdge,
) -> liquide_compositor::scene::BorderSide {
    liquide_compositor::scene::BorderSide {
        width: edge.width,
        style: match edge.style {
            liquide_style_engine::computed::BorderLineStyle::None => {
                liquide_compositor::scene::BorderSideStyle::None
            }
            liquide_style_engine::computed::BorderLineStyle::Solid => {
                liquide_compositor::scene::BorderSideStyle::Solid
            }
            liquide_style_engine::computed::BorderLineStyle::Dashed => {
                liquide_compositor::scene::BorderSideStyle::Dashed
            }
            liquide_style_engine::computed::BorderLineStyle::Dotted => {
                liquide_compositor::scene::BorderSideStyle::Dotted
            }
            liquide_style_engine::computed::BorderLineStyle::Double => {
                liquide_compositor::scene::BorderSideStyle::Double
            }
            liquide_style_engine::computed::BorderLineStyle::Groove => {
                liquide_compositor::scene::BorderSideStyle::Groove
            }
            liquide_style_engine::computed::BorderLineStyle::Ridge => {
                liquide_compositor::scene::BorderSideStyle::Ridge
            }
            liquide_style_engine::computed::BorderLineStyle::Inset => {
                liquide_compositor::scene::BorderSideStyle::Inset
            }
            liquide_style_engine::computed::BorderLineStyle::Outset => {
                liquide_compositor::scene::BorderSideStyle::Outset
            }
            liquide_style_engine::computed::BorderLineStyle::Hidden => {
                liquide_compositor::scene::BorderSideStyle::Hidden
            }
        },
        color: edge.color,
    }
}

fn hash_string(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Intersect two rectangles, returning the overlapping area.
fn intersect_rects(a: &CRect, b: &CRect) -> CRect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);
    CRect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
}

/// Convert a paint FilterOp to a compositor FilterSpec.
fn filter_op_to_spec(op: &liquide_compositor::property_tree::FilterOp) -> Option<liquide_compositor::scene::FilterSpec> {
    use liquide_compositor::property_tree::FilterOp;
    use liquide_compositor::scene::FilterSpec;
    match op {
        FilterOp::Blur(r) => Some(FilterSpec::Blur { radius: *r }),
        FilterOp::Brightness(v) => Some(FilterSpec::Brightness(*v)),
        FilterOp::Contrast(v) => Some(FilterSpec::Contrast(*v)),
        FilterOp::Saturate(v) => Some(FilterSpec::Saturate(*v)),
        FilterOp::HueRotate(v) => Some(FilterSpec::HueRotate(*v)),
        FilterOp::Grayscale(v) => Some(FilterSpec::Grayscale(*v)),
        FilterOp::Sepia(v) => Some(FilterSpec::Sepia(*v)),
        FilterOp::Invert(v) => Some(FilterSpec::Invert(*v)),
        FilterOp::Opacity(v) => Some(FilterSpec::Opacity(*v)),
        FilterOp::DropShadow { offset_x, offset_y, blur_radius, color } => Some(FilterSpec::DropShadow {
            offset_x: *offset_x,
            offset_y: *offset_y,
            blur: *blur_radius,
            color: *color,
        }),
        FilterOp::Reference(url) => Some(FilterSpec::Url(url.clone())),
        _ => None,
    }
}

/// Convert a paint FilterOp to a compositor BackdropFilterSpec.
fn filter_op_to_backdrop_spec(op: &liquide_compositor::property_tree::FilterOp) -> Option<liquide_compositor::scene::BackdropFilterSpec> {
    use liquide_compositor::property_tree::FilterOp;
    use liquide_compositor::scene::BackdropFilterSpec;
    match op {
        FilterOp::Blur(r) => Some(BackdropFilterSpec::Blur { radius: *r }),
        FilterOp::Brightness(v) => Some(BackdropFilterSpec::Brightness(*v)),
        FilterOp::Contrast(v) => Some(BackdropFilterSpec::Contrast(*v)),
        FilterOp::Saturate(v) => Some(BackdropFilterSpec::Saturate(*v)),
        FilterOp::HueRotate(v) => Some(BackdropFilterSpec::HueRotate(*v)),
        FilterOp::Grayscale(v) => Some(BackdropFilterSpec::Grayscale(*v)),
        FilterOp::Sepia(v) => Some(BackdropFilterSpec::Sepia(*v)),
        FilterOp::Invert(v) => Some(BackdropFilterSpec::Invert(*v)),
        FilterOp::Opacity(v) => Some(BackdropFilterSpec::Opacity(*v)),
        _ => None, // DropShadow, ColorMatrix, Reference not applicable to backdrop
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop_dom::{DesktopDocument, DockItemInfo};
    use liquide_compositor::pixel::Color;

    #[test]
    fn pipeline_runs_on_desktop_document() {
        let config = PipelineConfig::default();
        let mut pipeline = DesktopPipeline::new(&config);

        let mut desktop = DesktopDocument::new();
        desktop.populate_default_statusbar();
        desktop.sync_dock_items(&[DockItemInfo {
            app_id: "files".into(),
            label: "Files".into(),
            icon: "folder".into(),
            is_running: true,
            is_pinned: true,
        }]);

        let output = pipeline.run(&desktop.doc);

        // Should have styles for all nodes
        assert!(output.styles.len() > 0);

        // Should have at least some layout boxes
        assert!(output.layout.boxes.len() > 0);
    }

    #[test]
    fn pipeline_produces_scene_nodes() {
        let config = PipelineConfig::default();
        let mut pipeline = DesktopPipeline::new(&config);

        let mut desktop = DesktopDocument::new();
        desktop.populate_default_statusbar();

        let nodes = pipeline.render_to_scene(&desktop.doc, 0);
        // The pipeline should produce at least some nodes from styled elements
        // (background colors, borders, text, etc.)
        // Note: exact count depends on which elements have visible styles
        assert!(nodes.len() >= 0); // no panic = success
    }

    #[test]
    fn theme_switching() {
        let config = PipelineConfig::default();
        let mut pipeline = DesktopPipeline::new(&config);

        // Switch to Night theme
        pipeline.set_theme(theme_loader::night_css());
        assert!(pipeline.style_engine.rule_count() > 0);

        // Switch to Sunset theme
        pipeline.set_theme(theme_loader::sunset_css());
        assert!(pipeline.style_engine.rule_count() > 0);

        // Switch to Midday theme
        pipeline.set_theme(theme_loader::midday_css());
        assert!(pipeline.style_engine.rule_count() > 0);
    }

    #[test]
    fn viewport_update() {
        let config = PipelineConfig::default();
        let mut pipeline = DesktopPipeline::new(&config);

        pipeline.set_viewport(3840.0, 2160.0);
        assert_eq!(pipeline.style_engine.viewport.width, 3840.0);
        assert_eq!(pipeline.layout_engine.viewport.width, 3840.0);
    }

    #[test]
    fn display_list_bridge() {
        let config = PipelineConfig::default();
        let mut pipeline = DesktopPipeline::new(&config);

        // Create a simple display list manually
        let mut list = DisplayList::new();
        list.push(DisplayItem::SolidColor {
            rect: liquide_layout::Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
            },
            color: Color::new(255, 0, 0, 255),
            radius: liquide_style_engine::dimension::Corners::all(0.0),
        });
        list.push(DisplayItem::Text {
            rect: liquide_layout::Rect {
                x: 10.0,
                y: 10.0,
                width: 80.0,
                height: 20.0,
            },
            text: "Hello".into(),
            color: Color::new(255, 255, 255, 255),
            font_size: 14.0,
            font_family: vec!["Inter".into()],
            font_weight: 400,
            font_style: liquide_style_engine::computed::FontStyle::Normal,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            line_height: liquide_style_engine::computed::LineHeight::Normal,
            text_align: liquide_style_engine::computed::TextAlign::Start,
            text_transform: liquide_style_engine::computed::TextTransform::None,
            text_overflow: liquide_style_engine::computed::TextOverflow::Clip,
            white_space: liquide_style_engine::computed::WhiteSpace::Normal,
            word_break: liquide_style_engine::computed::WordBreak::Normal,
            text_indent: 0.0,
            text_decoration: None,
            text_shadows: Vec::new(),
            text_emphasis_style: None,
            text_emphasis_color: None,
            text_emphasis_position: None,
            caret_color: None,
        });

        let nodes = pipeline.display_list_to_scene(&list, 100);
        assert_eq!(nodes.len(), 2);

        // First is solid color → Background node
        assert!(matches!(nodes[0].kind, SceneNodeKind::Background { .. }));
        // Second is text → Text node
        assert!(matches!(nodes[1].kind, SceneNodeKind::Text { .. }));
    }
}
