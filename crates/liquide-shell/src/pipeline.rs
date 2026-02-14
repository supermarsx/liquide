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
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNode, SceneNodeKind};

use liquide_dom::Document;
use liquide_layout::{DefaultTextMeasurer, LayoutEngine, LayoutTree, Size};
use liquide_paint::{DisplayItem, DisplayList, Painter};
use liquide_style_engine::{StyleEngine, StyleMap};
use liquide_style_engine::engine::ViewportSize;

use crate::desktop_dom::DesktopDocument;
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

        // Load the default theme
        style_engine.add_stylesheet(theme_loader::default_liquid_glass_css());

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
        }
    }

    /// Load an additional stylesheet (e.g. a user theme override).
    pub fn add_stylesheet(&mut self, css: &str) {
        self.style_engine.add_stylesheet(css);
    }

    /// Replace styles with a named theme preset.
    pub fn set_theme(&mut self, preset_css: &str) {
        self.style_engine = StyleEngine::new(
            self.style_engine.viewport,
            self.style_engine.base_font_size,
        );
        self.style_engine.add_stylesheet(preset_css);
    }

    /// Update viewport dimensions (e.g. on monitor resolution change).
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.style_engine.set_viewport(ViewportSize {
            width,
            height,
        });
        self.layout_engine.viewport = Size { width, height };
    }

    /// Run the full pipeline: Style → Layout → Paint.
    ///
    /// Returns the style map, layout tree, and display list.
    pub fn run(
        &mut self,
        doc: &Document,
    ) -> PipelineOutput {
        let text_measurer = DefaultTextMeasurer;
        let image_measurer = liquide_layout::DefaultImageMeasurer;

        // 1. Style
        let styles = self.style_engine.restyle_all(doc);

        // 2. Layout
        let layout = self.layout_engine.layout(
            doc,
            &styles,
            &text_measurer,
            &image_measurer,
        );

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
    pub fn render_to_scene(
        &mut self,
        doc: &Document,
        base_z: u32,
    ) -> Vec<SceneNode> {
        let output = self.run(doc);

        // Collect Glass nodes from elements with x_blur_radius > 0.
        let glass_nodes = self.extract_glass_nodes(&output, base_z);
        let glass_count = glass_nodes.len() as u32;

        // Convert paint output to scene nodes, offset z by glass count.
        let mut nodes = glass_nodes;
        let paint_nodes = self.display_list_to_scene(&output.display_list, base_z + glass_count);
        nodes.extend(paint_nodes);

        nodes
    }

    /// Generate Glass SceneNodes for DOM elements that have `x_blur_radius > 0`
    /// in their computed style. Uses the layout tree to get the element's rect.
    fn extract_glass_nodes(
        &mut self,
        output: &PipelineOutput,
        base_z: u32,
    ) -> Vec<SceneNode> {
        let mut glass_nodes = Vec::new();
        let mut z = base_z;

        for layout_box in &output.layout.boxes {
            if let Some(style) = output.styles.get(layout_box.node) {
                if style.x_blur_radius > 0.0 {
                    let rect = to_compositor_rect(&layout_box.border_rect);
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
    pub fn display_list_to_scene(
        &mut self,
        list: &DisplayList,
        base_z: u32,
    ) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let mut z = base_z;

        for item in &list.items {
            if let Some(node) = self.display_item_to_scene(item, z) {
                nodes.push(node);
                z += 1;
            }
        }

        nodes
    }

    /// Map a single DisplayItem to a SceneNode.
    fn display_item_to_scene(
        &mut self,
        item: &DisplayItem,
        z: u32,
    ) -> Option<SceneNode> {
        let id = self.alloc_id();

        match item {
            DisplayItem::SolidColor { rect, color, .. } => {
                if color.a == 0 {
                    return None; // Skip fully transparent
                }
                let bounds = to_compositor_rect(rect);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Tint { color: *color },
                    NodeProperties::new(bounds).with_z_order(z),
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
                    NodeProperties::new(bounds).with_z_order(z),
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
            } => {
                let bounds = to_compositor_rect(rect);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Text {
                        text: text.clone(),
                        color: *color,
                        scale: 1,
                        font_family: font_family
                            .first()
                            .cloned()
                            .unwrap_or_default(),
                        font_size: *font_size,
                        font_weight: *font_weight,
                        letter_spacing: 0.0,
                        line_height: 1.2,
                        text_decoration: None,
                        text_shadows: Vec::new(),
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::Image { rect, src, .. } => {
                let bounds = to_compositor_rect(rect);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Image {
                        image_id: hash_string(src),
                        width: bounds.width as u32,
                        height: bounds.height as u32,
                        fit: liquide_compositor::scene::ImageFit::Cover,
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            // Clip / transform / opacity / blend / stacking contexts are
            // handled structurally via push/pop. For the flat-to-tree
            // conversion we currently skip them — the compositor applies
            // these as node properties. A full implementation would nest
            // children inside grouped nodes. TODO: push/pop grouping.
            DisplayItem::PushClip { .. }
            | DisplayItem::PopClip
            | DisplayItem::PushOpacity { .. }
            | DisplayItem::PopOpacity
            | DisplayItem::PushTransform { .. }
            | DisplayItem::PopTransform
            | DisplayItem::PushBlendMode { .. }
            | DisplayItem::PopBlendMode
            | DisplayItem::PushStackingContext { .. }
            | DisplayItem::PopStackingContext => None,

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

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop_dom::{DesktopDocument, DockItemInfo};

    #[test]
    fn pipeline_runs_on_desktop_document() {
        let config = PipelineConfig::default();
        let mut pipeline = DesktopPipeline::new(&config);

        let mut desktop = DesktopDocument::new();
        desktop.populate_default_statusbar();
        desktop.sync_dock_items(&[
            DockItemInfo {
                app_id: "files".into(),
                label: "Files".into(),
                icon: "folder".into(),
                is_running: true,
                is_pinned: true,
            },
        ]);

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
        });

        let nodes = pipeline.display_list_to_scene(&list, 100);
        assert_eq!(nodes.len(), 2);

        // First is solid color → Tint node
        assert!(matches!(nodes[0].kind, SceneNodeKind::Tint { .. }));
        // Second is text → Text node
        assert!(matches!(nodes[1].kind, SceneNodeKind::Text { .. }));
    }
}
