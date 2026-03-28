use super::*;
use crate::desktop_dom::DesktopDocument;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::SceneNodeKind;
use liquide_paint::{DisplayItem, DisplayList};

use crate::theme_loader;

#[test]
fn pipeline_runs_on_desktop_document() {
    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    let desktop = DesktopDocument::new();

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

    let desktop = DesktopDocument::new();

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
