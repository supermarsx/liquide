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

    let mut desktop = DesktopDocument::new();

    let (output, _animations_active) = pipeline.run(&mut desktop.doc, 16.0);

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

    let (nodes, _animations_active) = pipeline.render_to_scene(&mut desktop.doc, 0, 16.0);
    // The pipeline should produce at least some nodes from styled elements
    // (background colors, borders, text, etc.)
    // Note: exact count depends on which elements have visible styles
    assert!(!nodes.is_empty());
}

#[test]
fn pipeline_converts_gradient_background_to_scene_node() {
    let config = PipelineConfig {
        width: 200.0,
        height: 120.0,
        ..PipelineConfig::default()
    };
    let mut pipeline = DesktopPipeline::new(&config);
    pipeline.set_theme(
        r#"
        desktop-background {
            position: absolute;
            left: 0;
            top: 0;
            width: 200px;
            height: 120px;
            background: linear-gradient(180deg, rgb(18, 22, 48), rgb(12, 16, 38));
        }
    "#,
    );

    let mut desktop = DesktopDocument::from_html(r#"<desktop-background id="desktop-bg" />"#);
    let (nodes, _animations_active) = pipeline.render_to_scene(&mut desktop.doc, 0, 16.0);

    assert!(
        nodes
            .iter()
            .any(|node| matches!(node.kind, SceneNodeKind::GradientFill { .. })),
        "gradient backgrounds should become native gradient scene nodes"
    );
    assert!(
        pipeline.pending_images().is_empty(),
        "gradient backgrounds must not be treated as image URLs"
    );
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
fn theme_switch_after_render_invalidates_cached_scene_output() {
    let config = PipelineConfig {
        width: 200.0,
        height: 100.0,
        ..PipelineConfig::default()
    };
    let mut pipeline = DesktopPipeline::new(&config);
    let desktop = DesktopDocument::from_html(r#"<desktop-background id="desktop-bg" />"#);

    pipeline.set_theme(
        r#"
        desktop-background {
            position: absolute;
            left: 0;
            top: 0;
            width: 200;
            height: 100;
            background: rgb(1, 2, 3);
        }
        "#,
    );
    let before = pipeline.render_to_scene(&desktop.doc, 0, 16.0).0;

    pipeline.set_theme(
        r#"
        desktop-background {
            position: absolute;
            left: 0;
            top: 0;
            width: 200;
            height: 100;
            background: rgb(4, 5, 6);
        }
        "#,
    );
    let after = pipeline.render_to_scene(&desktop.doc, 0, 16.0).0;

    assert_eq!(
        first_background_color(&before),
        Some(Color::new(1, 2, 3, 255))
    );
    assert_eq!(
        first_background_color(&after),
        Some(Color::new(4, 5, 6, 255))
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// TODO 11 — container-query forced SECOND PASS.
//
// The first style pass evaluates `@container` with NO measured container size
// (it falls back to the viewport). After layout records the real host size, the
// pipeline must force a bounded second style+layout pass so `@container` rules
// re-evaluate against the REAL container dimensions, not the viewport.
// ═══════════════════════════════════════════════════════════════════════════

/// A container host narrower than the `@container` threshold must NOT match the
/// query, even though the (much larger) VIEWPORT would. Before the second pass
/// this evaluated against the viewport and wrongly matched.
#[test]
fn container_query_uses_real_host_size_not_viewport() {
    // Large viewport (1000px) so a viewport-fallback `@container (min-width:
    // 400px)` would WRONGLY match; the real host is only 120px wide.
    let config = PipelineConfig {
        width: 1000.0,
        height: 600.0,
        ..PipelineConfig::default()
    };
    let mut pipeline = DesktopPipeline::new(&config);
    pipeline.set_theme(
        r#"
        host {
            display: block;
            container-type: inline-size;
            width: 120;
            height: 80;
        }
        child { display: block; width: 40; height: 10; color: rgb(10, 10, 10); }
        @container (min-width: 400px) {
            child { color: rgb(200, 0, 0); }
        }
        "#,
    );
    let mut desktop =
        DesktopDocument::from_html(r#"<host id="host"><child id="child" /></host>"#);

    let (output, _a) = pipeline.run(&mut desktop.doc, 16.0);

    let child_id = desktop.doc.get_element_by_id("child").expect("child node");
    let child_style = output.styles.get(child_id).expect("child style");

    // Host content width is 120px < 400px, so the `@container` rule must NOT
    // apply: the child keeps its default color. If the pipeline evaluated the
    // query against the 1000px viewport (the pre-TODO-11 bug) the color would be
    // the red override.
    assert_eq!(
        (child_style.color.r, child_style.color.g, child_style.color.b),
        (10, 10, 10),
        "@container (min-width:400px) must NOT match a 120px-wide container — it \
         must evaluate against the REAL host size, not the {}px viewport",
        1000
    );

    // The host's measured container size was recorded for the cascade to read.
    assert_eq!(
        output.styles.container_size(host_id(&desktop)),
        Some((120.0, 80.0)),
        "the measured container size must be recorded after layout"
    );
}

/// The complement: a host WIDER than the threshold matches the query. Proves the
/// test above is not vacuously green (the rule does apply when the real host is
/// large enough).
#[test]
fn container_query_matches_when_real_host_is_large_enough() {
    let config = PipelineConfig {
        width: 1000.0,
        height: 600.0,
        ..PipelineConfig::default()
    };
    let mut pipeline = DesktopPipeline::new(&config);
    pipeline.set_theme(
        r#"
        host {
            display: block;
            container-type: inline-size;
            width: 500;
            height: 80;
        }
        child { display: block; width: 40; height: 10; color: rgb(10, 10, 10); }
        @container (min-width: 400px) {
            child { color: rgb(200, 0, 0); }
        }
        "#,
    );
    let mut desktop =
        DesktopDocument::from_html(r#"<host id="host"><child id="child" /></host>"#);

    let (output, _a) = pipeline.run(&mut desktop.doc, 16.0);

    let child_id = desktop.doc.get_element_by_id("child").expect("child node");
    let child_style = output.styles.get(child_id).expect("child style");

    assert_eq!(
        (child_style.color.r, child_style.color.g, child_style.color.b),
        (200, 0, 0),
        "@container (min-width:400px) must match a 500px-wide host (after the \
         second pass re-evaluates against the measured size)"
    );
}

fn host_id(desktop: &DesktopDocument) -> liquide_dom::NodeId {
    desktop.doc.get_element_by_id("host").expect("host node")
}

#[test]
fn added_stylesheet_after_render_invalidates_cached_scene_output() {
    let config = PipelineConfig {
        width: 200.0,
        height: 100.0,
        ..PipelineConfig::default()
    };
    let mut pipeline = DesktopPipeline::new(&config);
    let desktop = DesktopDocument::from_html(r#"<desktop-background id="desktop-bg" />"#);

    pipeline.set_theme(
        r#"
        desktop-background {
            position: absolute;
            left: 0;
            top: 0;
            width: 200;
            height: 100;
            background: rgb(10, 20, 30);
        }
        "#,
    );
    let before = pipeline.render_to_scene(&desktop.doc, 0, 16.0).0;

    pipeline.add_stylesheet(
        r#"
        desktop-background {
            background: rgb(40, 50, 60);
        }
        "#,
    );
    let after = pipeline.render_to_scene(&desktop.doc, 0, 16.0).0;

    assert_eq!(
        first_background_color(&before),
        Some(Color::new(10, 20, 30, 255))
    );
    assert_eq!(
        first_background_color(&after),
        Some(Color::new(40, 50, 60, 255))
    );
}

#[test]
fn viewport_change_after_render_invalidates_cached_scene_output() {
    let config = PipelineConfig {
        width: 200.0,
        height: 100.0,
        ..PipelineConfig::default()
    };
    let mut pipeline = DesktopPipeline::new(&config);
    let desktop = DesktopDocument::from_html(r#"<desktop-background id="desktop-bg" />"#);

    pipeline.set_theme(
        r#"
        desktop-background {
            position: absolute;
            left: 0;
            top: 0;
            width: 100%;
            height: 100%;
            background: rgb(1, 2, 3);
        }
        "#,
    );
    let before = pipeline.render_to_scene(&desktop.doc, 0, 16.0).0;

    pipeline.set_viewport(320.0, 180.0);
    let after = pipeline.render_to_scene(&desktop.doc, 0, 16.0).0;

    assert_eq!(
        first_background_bounds(&before).map(|b| b.width),
        Some(200.0)
    );
    assert_eq!(
        first_background_bounds(&after).map(|b| b.width),
        Some(320.0)
    );
    assert_eq!(
        first_background_bounds(&after).map(|b| b.height),
        Some(180.0)
    );
}

fn first_background_color(nodes: &[liquide_compositor::scene::SceneNode]) -> Option<Color> {
    nodes.iter().find_map(|node| match node.kind {
        SceneNodeKind::Background { color } => Some(color),
        _ => None,
    })
}

fn first_background_bounds(
    nodes: &[liquide_compositor::scene::SceneNode],
) -> Option<liquide_compositor::geometry::Rect> {
    nodes.iter().find_map(|node| match node.kind {
        SceneNodeKind::Background { .. } => Some(node.properties.bounds),
        _ => None,
    })
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
        radius: liquide_style_engine::dimension::Corners::all(0.0_f32.into()),
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
        font_family: std::sync::Arc::new(vec!["Inter".into()]),
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
        text_emphasis: None,
        caret_color: None,
    });

    let nodes = pipeline.display_list_to_scene(&list, 100);
    assert_eq!(nodes.len(), 2);

    // First is solid color → Background node
    assert!(matches!(nodes[0].kind, SceneNodeKind::Background { .. }));
    // Second is text → Text node
    assert!(matches!(nodes[1].kind, SceneNodeKind::Text { .. }));
}

/// t64-f10/f11: word-break and text-emphasis on a display-list Text item must
/// reach the compositor Text scene node (no longer dropped at the bridge).
#[test]
fn text_word_break_and_emphasis_reach_scene_node() {
    use liquide_compositor::scene::{TextEmphasisPosition, WordBreak};
    use liquide_paint::display_list::{
        EmphasisFill, EmphasisPosition, EmphasisShape, TextEmphasis,
    };

    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    let mut list = DisplayList::new();
    list.push(DisplayItem::Text {
        rect: liquide_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 80.0,
            height: 20.0,
        },
        text: "wrap".into(),
        color: Color::new(255, 255, 255, 255),
        font_size: 14.0,
        font_family: std::sync::Arc::new(vec!["Inter".into()]),
        font_weight: 400,
        font_style: liquide_style_engine::computed::FontStyle::Normal,
        letter_spacing: 0.0,
        word_spacing: 0.0,
        line_height: liquide_style_engine::computed::LineHeight::Normal,
        text_align: liquide_style_engine::computed::TextAlign::Start,
        text_transform: liquide_style_engine::computed::TextTransform::None,
        text_overflow: liquide_style_engine::computed::TextOverflow::Clip,
        white_space: liquide_style_engine::computed::WhiteSpace::Normal,
        word_break: liquide_style_engine::computed::WordBreak::BreakAll,
        text_indent: 0.0,
        text_decoration: None,
        text_shadows: Vec::new(),
        text_emphasis: Some(TextEmphasis {
            fill: EmphasisFill::Filled,
            shape: EmphasisShape::Circle,
            color: Color::new(255, 0, 0, 255),
            position: EmphasisPosition::Over,
        }),
        caret_color: None,
    });

    let nodes = pipeline.display_list_to_scene(&list, 0);
    let text = nodes
        .iter()
        .find_map(|n| match &n.kind {
            SceneNodeKind::Text {
                word_break,
                text_emphasis,
                ..
            } => Some((*word_break, text_emphasis.clone())),
            _ => None,
        })
        .expect("a Text scene node");

    // word-break: break-all must reach the node (was dropped before t64-f10).
    assert_eq!(text.0, WordBreak::BreakAll, "word-break must reach scene");

    // text-emphasis: filled circle → ● mark, red color, over position.
    let em = text.1.expect("text-emphasis must reach scene (t64-f11)");
    assert_eq!(em.mark, "\u{25CF}", "filled circle resolves to ● mark");
    assert_eq!(em.color, Some(Color::new(255, 0, 0, 255)));
    assert_eq!(em.position, TextEmphasisPosition::Over);
}

/// t64-f12: a Text item carrying caret-color emits a TextCaret scene node with
/// the computed colour.
#[test]
fn caret_color_emits_text_caret_node() {
    use liquide_compositor::scene::SceneNodeKind as K;

    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    let caret_col = Color::new(0, 200, 255, 255);
    let mut list = DisplayList::new();
    list.push(DisplayItem::Text {
        rect: liquide_layout::Rect {
            x: 5.0,
            y: 7.0,
            width: 60.0,
            height: 18.0,
        },
        text: "edit me".into(),
        color: Color::new(255, 255, 255, 255),
        font_size: 14.0,
        font_family: std::sync::Arc::new(vec!["Inter".into()]),
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
        text_emphasis: None,
        caret_color: Some(caret_col),
    });

    let nodes = pipeline.display_list_to_scene(&list, 0);
    let caret = nodes
        .iter()
        .find_map(|n| match &n.kind {
            K::TextCaret { color, width } => Some((*color, *width, n.properties.bounds)),
            _ => None,
        })
        .expect("a TextCaret node for an editable with caret-color (t64-f12)");

    assert_eq!(
        caret.0, caret_col,
        "caret colour must match CSS caret-color"
    );
    assert!(caret.1 > 0.0, "caret has a positive width");
    // Caret sits at the text box's leading edge.
    assert!((caret.2.x - 5.0).abs() < 0.01);
    assert!((caret.2.y - 7.0).abs() < 0.01);
}

/// t64-f13: a background Image item maps to ImageFit::Sized matching the
/// painter-computed background-size box (no longer the hardcoded Cover).
#[test]
fn background_image_uses_sized_fit_from_background_size() {
    use liquide_compositor::scene::{ImageFit, SceneNodeKind as K};

    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    let mut list = DisplayList::new();
    list.push(DisplayItem::Image {
        rect: liquide_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 120.0,
            height: 40.0,
        },
        src: "wallpaper.png".into(),
        radius: liquide_style_engine::dimension::Corners::all(0.0_f32.into()),
    });

    let nodes = pipeline.display_list_to_scene(&list, 0);
    let fit = nodes
        .iter()
        .find_map(|n| match &n.kind {
            K::Image { fit, .. } => Some(*fit),
            _ => None,
        })
        .expect("an Image scene node");

    match fit {
        ImageFit::Sized { width, height } => {
            assert!((width - 120.0).abs() < 0.01, "sized width = bg-size box w");
            assert!((height - 40.0).abs() < 0.01, "sized height = bg-size box h");
        }
        other => panic!("background image must use Sized fit, got {other:?}"),
    }
}
