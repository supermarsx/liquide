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

/// Find the Image scene node (if any) and return (image_id, width, height, fit).
fn find_image_node(
    nodes: &[liquide_compositor::scene::SceneNode],
) -> Option<(
    u64,
    u32,
    u32,
    liquide_compositor::scene::ImageFit,
    liquide_compositor::geometry::Rect,
)> {
    nodes.iter().find_map(|n| match &n.kind {
        SceneNodeKind::Image {
            image_id,
            width,
            height,
            fit,
        } => Some((*image_id, *width, *height, *fit, n.properties.bounds)),
        _ => None,
    })
}

#[test]
fn img_element_emits_image_scene_node_sized_by_css_box_with_object_fit() {
    // An <img src=...> element must emit a SceneNodeKind::Image whose bounds are
    // the element's laid-out CSS box and whose fit reflects `object-fit`. This is
    // the t144 wiring: an HTML-parsed <img> becomes NodeData::Image -> painter
    // ImageRect -> scene Image. Its src must be queued in pending_images for the
    // host loader to decode/register.
    let config = PipelineConfig {
        width: 400.0,
        height: 300.0,
        ..PipelineConfig::default()
    };
    let mut pipeline = DesktopPipeline::new(&config);
    pipeline.set_theme(
        r#"
        img#hero {
            position: absolute;
            left: 10px;
            top: 20px;
            width: 120px;
            height: 80px;
            object-fit: contain;
        }
    "#,
    );

    let mut desktop = DesktopDocument::from_html(r#"<img id="hero" src="hero.png">"#);
    let (nodes, _) = pipeline.render_to_scene(&mut desktop.doc, 0, 16.0);

    let (image_id, w, h, fit, bounds) =
        find_image_node(&nodes).expect("an <img> must emit a SceneNodeKind::Image node");

    // Sized + positioned by the CSS box (content box = 120x80 at 10,20).
    assert_eq!(bounds.width, 120.0, "image node width = CSS box width");
    assert_eq!(bounds.height, 80.0, "image node height = CSS box height");
    assert_eq!(bounds.x, 10.0);
    assert_eq!(bounds.y, 20.0);
    assert_eq!(w, 120);
    assert_eq!(h, 80);

    // object-fit: contain must reach the scene as ImageFit::Contain.
    assert_eq!(
        fit,
        liquide_compositor::scene::ImageFit::Contain,
        "object-fit: contain must map to ImageFit::Contain"
    );

    // The src must be queued for the host image loader, keyed by the same hashed
    // id as the scene node (so register_image lands on this node's texture key).
    let pending = pipeline.pending_images();
    assert!(
        pending.iter().any(|(id, url)| *id == image_id && url == "hero.png"),
        "img src must be queued in pending_images keyed by the node image_id; got {pending:?}"
    );
}

#[test]
fn img_object_fit_cover_maps_to_cover_and_fill_maps_to_fill() {
    // object-fit: cover and fill must reach the scene as the matching ImageFit so
    // the renderer's cover/contain/fill src-rect/dst-rect math is selected
    // correctly.
    for (css_fit, expected) in [
        ("cover", liquide_compositor::scene::ImageFit::Cover),
        ("fill", liquide_compositor::scene::ImageFit::Fill),
    ] {
        let config = PipelineConfig {
            width: 400.0,
            height: 300.0,
            ..PipelineConfig::default()
        };
        let mut pipeline = DesktopPipeline::new(&config);
        pipeline.set_theme(&format!(
            "img#p {{ position: absolute; left:0; top:0; width:100px; height:50px; object-fit: {css_fit}; }}"
        ));
        let mut desktop = DesktopDocument::from_html(r#"<img id="p" src="p.png">"#);
        let (nodes, _) = pipeline.render_to_scene(&mut desktop.doc, 0, 16.0);
        let (_, _, _, fit, _) =
            find_image_node(&nodes).expect("img emits an Image node");
        assert_eq!(fit, expected, "object-fit: {css_fit}");
    }
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
fn active_animation_does_not_force_full_tree_restyle() {
    // t68-perf cause #3b: an active transition/animation used to disable the
    // pipeline fast path for the WHOLE tree, re-styling and re-laying-out every
    // static node every frame. After the scoped-invalidation fix, a frame with
    // an active transition but no DOM mutation must NOT restyle non-animating
    // nodes — their cached `ComputedStyle` Arc must survive (pointer-identical).
    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);
    let mut desktop = DesktopDocument::new();

    // Frame 0: full pipeline populates the caches.
    let (out0, _) = pipeline.run(&mut desktop.doc, 16.0);
    assert!(out0.styles.len() > 0);

    // Pick an animating node and a DIFFERENT static node.
    let mut node_ids: Vec<liquide_dom::NodeId> = out0.styles.iter().map(|(id, _)| *id).collect();
    node_ids.sort();
    assert!(
        node_ids.len() >= 2,
        "need at least two styled nodes for the scoping test"
    );
    let animating = node_ids[0];
    let static_node = node_ids[1];

    // Capture the static node's cached style Arc (pointer identity).
    let static_style_before = std::sync::Arc::clone(out0.styles.get(static_node).unwrap());

    // Clear DOM dirty flags so the NEXT frame has no DOM-driven work — the only
    // reason to do any pipeline work is the active transition.
    desktop.doc.dirty.clear_all();

    // Start a real transition on `animating` (opacity 1 → 0 over 1s). This makes
    // the transition engine active without dirtying the DOM.
    pipeline
        .transition_engine
        .start(
            animating,
            "opacity",
            1.0,
            0.0,
            1000.0,
            0.0,
            liquide_animation::EasingFunction::Linear,
        );
    assert!(pipeline.transition_engine.active_count() > 0);

    // Frame 1: animation active, DOM clean → scoped-animation path.
    let (out1, animations_active) = pipeline.run(&mut desktop.doc, 16.0);
    assert!(
        animations_active,
        "the active transition should report animations_active"
    );

    // The static (non-animating) node's computed style must be the SAME Arc —
    // proving it was NOT restyled (the whole-tree restyle would allocate fresh
    // Arcs for every node).
    let static_style_after = out1.styles.get(static_node).unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&static_style_before, static_style_after),
        "a non-animating node must keep its cached style across an animation frame"
    );
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

/// FIRST-FRAME teeth (t77): a `@container`-dependent property must resolve to the
/// CONTAINER-derived value on the very FIRST rendered frame — never the viewport
/// fallback that "snaps" to the right value only on frame 2.
///
/// Construction that makes the tooth load-bearing:
///   * Viewport is 1000px wide.
///   * Host (container) is 300px wide.
///   * Two thresholds straddle the host width:
///       - `@container (min-width: 200px)` MATCHES the 300px host  → blue.
///       - `@container (min-width: 600px)` does NOT match the host but
///         WOULD match the 1000px viewport → red (the wrong value).
/// So the *correct* (container) answer and the *viewport-fallback* answer are
/// DIFFERENT explicit colors. Exactly ONE `pipeline.run()` is issued (frame 1).
/// If the corrective container pass ran on a later frame (or not at all), frame 1
/// would carry the red viewport-fallback value and this assertion would fail.
#[test]
fn container_query_resolves_to_container_value_on_first_frame_not_viewport() {
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
            width: 300;
            height: 80;
        }
        child { display: block; width: 40; height: 10; color: rgb(10, 10, 10); }
        /* Matches the 300px host (and also the viewport) → desired value. */
        @container (min-width: 200px) {
            child { color: rgb(0, 0, 200); }
        }
        /* Does NOT match the 300px host, but WOULD match the 1000px viewport.
           Listed last so source order would make it win IF the cascade saw a
           (wrong) viewport-fallback match. */
        @container (min-width: 600px) {
            child { color: rgb(200, 0, 0); }
        }
        "#,
    );
    let mut desktop =
        DesktopDocument::from_html(r#"<host id="host"><child id="child" /></host>"#);

    // EXACTLY ONE frame — the first rendered frame.
    let (output, _a) = pipeline.run(&mut desktop.doc, 16.0);

    let child_id = desktop.doc.get_element_by_id("child").expect("child node");
    let child_style = output.styles.get(child_id).expect("child style");

    assert_eq!(
        (child_style.color.r, child_style.color.g, child_style.color.b),
        (0, 0, 200),
        "on the FIRST frame the child must take the CONTAINER-derived value \
         (min-width:200px matches the 300px host → blue). Red (200,0,0) means the \
         cascade fell back to the 1000px VIEWPORT and matched the 600px query — \
         the first-frame container-query bug."
    );

    // The host's real measured size must be recorded for the cascade to read on
    // this same frame (not deferred to frame 2).
    assert_eq!(
        output.styles.container_size(host_id(&desktop)),
        Some((300.0, 80.0)),
        "the measured container size must be recorded on the first frame"
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

/// The on-disk production `liquid_glass.css` theme, used so the test exercises
/// the REAL wallpaper CSS shipped in `assets/`, not an inline stand-in. If the
/// theme's `desktop-background` rule regresses (e.g. the wallpaper declaration
/// gets swallowed by a malformed comment, or the element stops covering the
/// viewport at the origin), this test fails.
const LIQUID_GLASS_CSS: &str = include_str!("../../../../assets/themes/liquid_glass.css");

/// Find the bounds of the first full-viewport desktop-background scene node,
/// accepting any of the background-bearing scene kinds the pipeline can emit for
/// a `desktop-background` element: a solid `Background`, a `GradientFill`, a
/// `BackgroundFill`, or a wallpaper `Image`. Only nodes that cover (almost) the
/// whole viewport qualify — small images/icons are ignored.
fn first_fullviewport_background_bounds(
    nodes: &[liquide_compositor::scene::SceneNode],
    viewport_w: f32,
    viewport_h: f32,
) -> Option<liquide_compositor::geometry::Rect> {
    let min_area = viewport_w * viewport_h * 0.9;
    nodes.iter().find_map(|node| {
        let is_bg_kind = matches!(
            node.kind,
            SceneNodeKind::Background { .. }
                | SceneNodeKind::GradientFill { .. }
                | SceneNodeKind::BackgroundFill { .. }
                | SceneNodeKind::Image { .. }
        );
        let b = node.properties.bounds;
        if is_bg_kind && b.width * b.height >= min_area {
            Some(b)
        } else {
            None
        }
    })
}

/// P1 (t86 full-CSS migration): the desktop backdrop must be a CSS-driven
/// background that covers the FULL viewport anchored at the origin (0,0) — no
/// imperative backdrop fill, and no left/top strip from a percentage-vs-pixel
/// position offset (the recently-fixed wallpaper-position bug).
///
/// This drives the REAL shipped `assets/themes/liquid_glass.css` through the
/// CSS pipeline over a bare `<desktop-background>` element and asserts the
/// emitted background scene node spans the entire viewport starting at (0,0).
/// No-fake-green: the assertions have teeth — a non-zero origin (offset strip),
/// a sub-viewport box, or a missing background node all fail.
#[test]
fn desktop_background_is_css_driven_fullviewport_at_origin() {
    const W: f32 = 320.0;
    const H: f32 = 200.0;

    let config = PipelineConfig {
        width: W,
        height: H,
        ..PipelineConfig::default()
    };
    let mut pipeline = DesktopPipeline::new(&config);
    // Use the production theme asset verbatim — this is what ships.
    pipeline.set_theme(LIQUID_GLASS_CSS);

    let desktop = DesktopDocument::from_html(r#"<desktop-background id="desktop-bg" />"#);
    let (nodes, _animations_active) = pipeline.render_to_scene(&desktop.doc, 0, 16.0);

    let bounds = first_fullviewport_background_bounds(&nodes, W, H).expect(
        "the desktop-background must emit a CSS-driven full-viewport background \
         scene node (solid/gradient/wallpaper) — none found, so the desktop \
         backdrop is not flowing through the CSS pipeline",
    );

    // Anchored at the origin: no left/top strip. The cover-wallpaper bug parked
    // the backdrop a few pixels in from (0,0); guard against any regression.
    assert!(
        bounds.x.abs() < 0.5,
        "desktop background must start at x=0 (no left strip); got x={}",
        bounds.x
    );
    assert!(
        bounds.y.abs() < 0.5,
        "desktop background must start at y=0 (no top strip); got y={}",
        bounds.y
    );

    // Covers the full viewport.
    assert!(
        (bounds.width - W).abs() < 0.5,
        "desktop background must span the full viewport width {W}; got {}",
        bounds.width
    );
    assert!(
        (bounds.height - H).abs() < 0.5,
        "desktop background must span the full viewport height {H}; got {}",
        bounds.height
    );
}

/// Companion to the above: the production `liquid_glass.css` must keep its
/// fallback `linear-gradient` declaration intact on `desktop-background`. A
/// regression that swallows it (e.g. a malformed `\*` comment consuming the
/// declaration up to the next `;`) would leave only the `url()` wallpaper, with
/// no backdrop at all when the image is missing/unsupported.
#[test]
fn liquid_glass_desktop_background_keeps_fallback_gradient() {
    // The desktop-background rule must declare a gradient backdrop. We assert on
    // the source so the fallback can't be silently dropped by a comment bug even
    // when the wallpaper PNG happens to load on top of it.
    let rule_start = LIQUID_GLASS_CSS
        .find("desktop-background")
        .expect("liquid_glass.css must define desktop-background");
    let rule_end = LIQUID_GLASS_CSS[rule_start..]
        .find('}')
        .map(|i| rule_start + i)
        .expect("desktop-background rule must be closed");
    let rule = &LIQUID_GLASS_CSS[rule_start..rule_end];

    assert!(
        rule.contains("linear-gradient"),
        "desktop-background must keep its fallback linear-gradient backdrop"
    );
    // The malformed-comment regression marker: a backslash-star sequence is not
    // a valid CSS comment opener and silently eats the next declaration.
    assert!(
        !LIQUID_GLASS_CSS.contains("\\*"),
        "liquid_glass.css contains a malformed `\\*` comment that swallows the \
         following declaration — use `/* */`"
    );
}

// ── t87-crisp: pixel-snapping of box/hairline geometry ─────────────────────
//
// Root cause (t83-crisp #5/#6): layout emits fractional box origins; the CPU
// rasterizer's `fill_rect` floors the origin and ceils the extent, so a 1px
// line at y=10.5,h=1.0 lights up rows 10 AND 11 (doubled/blurred). The bridge
// must snap box geometry to the device-pixel grid so hairlines land on a single
// row/col. These tests are written to FAIL if snapping is removed (they assert
// integer edges and single-pixel hairlines at deliberately fractional origins).

fn first_bounds_of<F>(nodes: &[liquide_compositor::scene::SceneNode], pred: F) -> CRect
where
    F: Fn(&liquide_compositor::scene::SceneNodeKind) -> bool,
{
    nodes
        .iter()
        .find(|n| pred(&n.kind))
        .map(|n| n.properties.bounds)
        .expect("expected a matching scene node")
}

use liquide_compositor::geometry::Rect as CRect;

/// A 1px divider drawn as a `Line` at a FRACTIONAL origin must land on a single
/// device row (height == 1, top == whole pixel) — not straddle two rows.
#[test]
fn hairline_divider_snaps_to_single_row() {
    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    let mut list = DisplayList::new();
    // Horizontal 1px separator at y = 10.5 (the worst case: half-pixel phase).
    list.push(DisplayItem::Line {
        x1: 0.0,
        y1: 10.5,
        x2: 200.0,
        y2: 10.5,
        color: Color::new(80, 80, 80, 255),
        width: 1.0,
    });

    let nodes = pipeline.display_list_to_scene(&list, 0);
    let b = first_bounds_of(&nodes, |k| {
        matches!(k, SceneNodeKind::Background { .. })
    });

    // Top edge must be a whole pixel.
    assert_eq!(
        b.y,
        b.y.round(),
        "hairline top must be integer-aligned, got y={}",
        b.y
    );
    // The line must be exactly one device row tall so floor/ceil in the
    // rasterizer covers a single row, not two.
    assert!(
        (b.height - 1.0).abs() < 1e-6,
        "1px divider must stay 1px tall after snap, got h={}",
        b.height
    );
    // Its bottom edge is therefore also integer (covers exactly row b.y).
    assert_eq!(b.bottom(), b.bottom().round(), "hairline bottom must be integer");
}

/// A border box at a fractional origin must have integer edges so the renderer
/// draws crisp 1px sides instead of doubling them.
#[test]
fn border_box_edges_snap_to_pixel_grid() {
    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    let edge = liquide_paint::display_list::BorderEdge {
        width: 1.0,
        style: liquide_style_engine::computed::BorderLineStyle::Solid,
        color: Color::new(0, 0, 0, 255),
    };
    let mut list = DisplayList::new();
    list.push(DisplayItem::Border {
        rect: liquide_layout::Rect {
            x: 12.4,
            y: 8.6,
            width: 100.3,
            height: 40.7,
        },
        top: edge.clone(),
        right: edge.clone(),
        bottom: edge.clone(),
        left: edge,
        radius: liquide_style_engine::dimension::Corners::all(0.0_f32.into()),
    });

    let nodes = pipeline.display_list_to_scene(&list, 0);
    let b = first_bounds_of(&nodes, |k| matches!(k, SceneNodeKind::Border { .. }));

    assert_eq!(b.x, b.x.round(), "left edge must snap to grid, got {}", b.x);
    assert_eq!(b.y, b.y.round(), "top edge must snap to grid, got {}", b.y);
    assert_eq!(
        b.right(),
        b.right().round(),
        "right edge must snap to grid, got {}",
        b.right()
    );
    assert_eq!(
        b.bottom(),
        b.bottom().round(),
        "bottom edge must snap to grid, got {}",
        b.bottom()
    );
    // Snapping rounds each edge to nearest: x 12.4→12, right 112.7→113 (w=101).
    assert!((b.x - 12.0).abs() < 1e-6);
    assert!((b.y - 9.0).abs() < 1e-6);
    assert!((b.width - 101.0).abs() < 1e-6);
    assert!((b.height - 40.0).abs() < 1e-6);
}

/// Two abutting siblings (right edge of A == left edge of B at a fractional
/// coordinate) must snap to the SAME integer so there is no seam/gap and no
/// rounding drift between siblings.
#[test]
fn abutting_siblings_share_snapped_edge_no_drift() {
    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    let radius = liquide_style_engine::dimension::Corners::all(0.0_f32.into());
    let mut list = DisplayList::new();
    // A: x=0..50.5 ; B: x=50.5..100.5 — shared edge at the half pixel.
    list.push(DisplayItem::SolidColor {
        rect: liquide_layout::Rect {
            x: 0.0,
            y: 0.0,
            width: 50.5,
            height: 20.0,
        },
        color: Color::new(255, 0, 0, 255),
        radius: radius.clone(),
    });
    list.push(DisplayItem::SolidColor {
        rect: liquide_layout::Rect {
            x: 50.5,
            y: 0.0,
            width: 50.0,
            height: 20.0,
        },
        color: Color::new(0, 255, 0, 255),
        radius,
    });

    let nodes = pipeline.display_list_to_scene(&list, 0);
    let backgrounds: Vec<CRect> = nodes
        .iter()
        .filter(|n| matches!(n.kind, SceneNodeKind::Background { .. }))
        .map(|n| n.properties.bounds)
        .collect();
    assert_eq!(backgrounds.len(), 2);

    // A's right edge and B's left edge must be the SAME integer — no gap, no
    // overlap (that's what prevents seams between adjacent chrome boxes).
    assert_eq!(
        backgrounds[0].right(),
        backgrounds[1].x,
        "shared edge must coincide after snapping (no seam/drift)"
    );
    assert_eq!(backgrounds[1].x, backgrounds[1].x.round());
}

/// Text bounds must NOT be snapped — the glyph rasterizer owns text sub-pixel
/// positioning / baseline placement. This guards the boundary with the peer
/// (t87-crisp-render) so a future "snap everything" change can't silently
/// pixel-snap text and regress baseline placement.
#[test]
fn text_bounds_remain_subpixel() {
    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    let mut list = DisplayList::new();
    list.push(DisplayItem::Text {
        rect: liquide_layout::Rect {
            x: 10.4,
            y: 10.6,
            width: 80.3,
            height: 20.2,
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

    let nodes = pipeline.display_list_to_scene(&list, 0);
    let b = first_bounds_of(&nodes, |k| matches!(k, SceneNodeKind::Text { .. }));
    assert!(
        (b.x - 10.4).abs() < 1e-6,
        "text x must stay sub-pixel (not snapped), got {}",
        b.x
    );
    assert!(
        (b.y - 10.6).abs() < 1e-6,
        "text y must stay sub-pixel (not snapped), got {}",
        b.y
    );
}

/// t149: a clip-path scope must emit a PAIRED begin/apply `ClipPath` marker that
/// BRACKETS the clipped element's own draws. The begin marker sorts (by z) before
/// the element's content; the apply marker after it. The renderer relies on this
/// pairing to snapshot-and-restore so the clip does not eat earlier siblings.
#[test]
fn t149_clip_path_emits_paired_begin_and_apply_markers() {
    use liquide_compositor::scene::SceneNode;
    use liquide_paint::display_list::ClipPath as PaintClipPath;
    use liquide_style_engine::dimension::{Corners, EllipticalRadius};

    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    let zero: Corners<EllipticalRadius> = Corners::all(0.0_f32.into());
    let rect = liquide_layout::Rect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
    };

    // Painter emission order for a clip-path element with an overflow clip:
    //   PushClipPath, PushClip, <content>, PopClip (overflow), PopClip (clip-path)
    let mut list = DisplayList::new();
    list.push(DisplayItem::PushClipPath {
        path: PaintClipPath::Polygon(vec![(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)]),
    });
    list.push(DisplayItem::PushClip {
        rect,
        radius: zero.clone(),
    });
    // The clipped element's OWN content.
    list.push(DisplayItem::SolidColor {
        rect,
        color: Color::new(10, 20, 30, 255),
        radius: zero,
    });
    list.push(DisplayItem::PopClip); // overflow
    list.push(DisplayItem::PopClip); // clip-path

    let nodes = pipeline.display_list_to_scene(&list, 0);

    // Exactly TWO ClipPath markers (begin + apply).
    let clip_markers: Vec<&SceneNode> = nodes
        .iter()
        .filter(|n| matches!(n.kind, SceneNodeKind::ClipPath { .. }))
        .collect();
    assert_eq!(
        clip_markers.len(),
        2,
        "a clip-path scope must emit a paired begin+apply marker, got {}",
        clip_markers.len()
    );

    let content = nodes
        .iter()
        .find(|n| matches!(n.kind, SceneNodeKind::Background { .. }))
        .expect("the clipped element's own Background content must be emitted");

    let z_begin = clip_markers
        .iter()
        .map(|n| n.properties.z_order)
        .min()
        .unwrap();
    let z_apply = clip_markers
        .iter()
        .map(|n| n.properties.z_order)
        .max()
        .unwrap();
    let z_content = content.properties.z_order;

    assert!(
        z_begin < z_content,
        "begin marker (z={z_begin}) must sort BEFORE the clipped content (z={z_content})"
    );
    assert!(
        z_content < z_apply,
        "apply marker (z={z_apply}) must sort AFTER the clipped content (z={z_content})"
    );

    // Both markers must carry the SAME bounds (so the renderer pairs them).
    assert_eq!(
        clip_markers[0].properties.bounds, clip_markers[1].properties.bounds,
        "begin/apply markers must share identical bounds for pairing"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// LEVER t91 — PAINT-ONLY DIRTY GRANULARITY.
//
// A change that affects ONLY paint properties (background-color, color,
// opacity, box-shadow, border-color, …) must mark PAINT (and STYLE, to
// recompute the value) but NOT LAYOUT, so the pipeline REUSES the cached
// layout tree and re-runs only paint. A geometry change (width/padding/
// font-size) must STILL run layout. These teeth fail if a paint-only change
// runs layout, or if a geometry change skips it.
//
// "Layout did/didn't run" is measured by the pipeline's `layout_runs`
// instrumentation counter (incremented in BOTH the full and incremental layout
// branches, nowhere else), corroborated by structural equality of the layout
// boxes (reuse → identical geometry).
// ═══════════════════════════════════════════════════════════════════════════

/// Snapshot every layout box as (node, content_rect) for structural comparison.
fn layout_box_signature(
    layout: &liquide_layout::LayoutTree,
) -> Vec<(liquide_dom::NodeId, [u32; 4])> {
    let mut v: Vec<_> = layout
        .boxes
        .iter()
        .map(|b| {
            (
                b.node,
                [
                    b.content_rect.x.to_bits(),
                    b.content_rect.y.to_bits(),
                    b.content_rect.width.to_bits(),
                    b.content_rect.height.to_bits(),
                ],
            )
        })
        .collect();
    v.sort_by_key(|(n, _)| *n);
    v
}

/// Extract a structural signature of every SolidColor/FillRect fill in a
/// display list: (rounded rect, rgba). Order-preserving so two display lists are
/// byte/structurally comparable for the recolour teeth.
fn fill_signature(list: &DisplayList) -> Vec<([u32; 4], [u8; 4])> {
    list.items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::SolidColor { rect, color, .. } | DisplayItem::FillRect { rect, color } => {
                Some((
                    [
                        rect.x.to_bits(),
                        rect.y.to_bits(),
                        rect.width.to_bits(),
                        rect.height.to_bits(),
                    ],
                    [color.r, color.g, color.b, color.a],
                ))
            }
            _ => None,
        })
        .collect()
}

/// A small two-child flex row with explicit backgrounds. A width change on one
/// child visibly relayouts the row; a recolour does not.
fn paint_lever_html() -> &'static str {
    r#"<box id="row"><box id="a" /><box id="b" /></box>"#
}

fn paint_lever_theme() -> &'static str {
    r#"
    box { display: block; }
    #row { display: flex; flex-direction: row; width: 200px; height: 40px; }
    #a { width: 80px; height: 40px; background-color: rgb(10, 20, 30); }
    #b { width: 80px; height: 40px; background-color: rgb(40, 50, 60); }
    "#
}

#[test]
fn paint_only_inline_recolor_reuses_layout_and_only_repaints() {
    let config = PipelineConfig {
        width: 400.0,
        height: 200.0,
        ..PipelineConfig::default()
    };
    let mut pipeline = DesktopPipeline::new(&config);
    pipeline.set_theme(paint_lever_theme());
    let mut desktop = DesktopDocument::from_html(paint_lever_html());

    // Frame 0: full pipeline populates caches.
    let (out0, _) = pipeline.run(&mut desktop.doc, 16.0);
    let layout_before = layout_box_signature(&out0.layout);
    let runs_layout_0 = pipeline.layout_runs;
    let runs_paint_0 = pipeline.paint_runs;
    assert_eq!(runs_layout_0, 1, "frame 0 must run a full layout");
    assert_eq!(runs_paint_0, 1, "frame 0 must paint");

    // Simulate frame boundary: clear DOM dirty (the real caller does this).
    desktop.doc.dirty.clear_all();

    // Paint-only change: recolour #a's background. The property name is known,
    // so the DOM classifies it paint-only → STYLE+PAINT, NOT LAYOUT.
    let a_id = desktop.doc.get_element_by_id("a").expect("node a");
    desktop
        .doc
        .set_inline_style(a_id, "background-color", "rgb(200, 0, 0)");
    assert!(
        !desktop.doc.dirty.layout.contains(&a_id),
        "the recolour must not have marked the layout dirty set"
    );

    // Frame 1: paint-only fast path.
    let (out1, _) = pipeline.run(&mut desktop.doc, 16.0);

    // TOOTH 1: layout did NOT run again; paint DID.
    assert_eq!(
        pipeline.layout_runs, runs_layout_0,
        "a paint-only recolour must NOT re-run layout (layout_runs incremented)"
    );
    assert_eq!(
        pipeline.paint_runs,
        runs_paint_0 + 1,
        "a paint-only recolour must re-run paint exactly once"
    );

    // TOOTH 2: the layout geometry is byte-identical (reused, not recomputed).
    let layout_after = layout_box_signature(&out1.layout);
    assert_eq!(
        layout_before, layout_after,
        "the cached layout must be reused verbatim across a paint-only change"
    );

    // TOOTH 3: paint output actually reflects the NEW colour (not a no-op).
    let fills = fill_signature(&out1.display_list);
    assert!(
        fills.iter().any(|(_, rgba)| *rgba == [200, 0, 0, 255]),
        "the recoloured node's new background must appear in the repaint; fills = {fills:?}"
    );
    assert!(
        !fills.iter().any(|(_, rgba)| *rgba == [10, 20, 30, 255]),
        "the OLD background colour must be gone after the recolour"
    );
}

#[test]
fn paint_only_incremental_recolor_matches_full_rebuild() {
    // TOOTH (c): the display list produced by the incremental paint-only path
    // must be structurally identical to one produced by a from-scratch pipeline
    // that renders the SAME recoloured DOM.
    let config = PipelineConfig {
        width: 400.0,
        height: 200.0,
        ..PipelineConfig::default()
    };

    // ── Incremental path: render base, then recolour on a second frame. ──
    let mut inc = DesktopPipeline::new(&config);
    inc.set_theme(paint_lever_theme());
    let mut inc_doc = DesktopDocument::from_html(paint_lever_html());
    let _ = inc.run(&mut inc_doc.doc, 16.0);
    inc_doc.doc.dirty.clear_all();
    let a_inc = inc_doc.doc.get_element_by_id("a").unwrap();
    inc_doc
        .doc
        .set_inline_style(a_inc, "background-color", "rgb(7, 8, 9)");
    let (inc_out, _) = inc.run(&mut inc_doc.doc, 16.0);
    assert_eq!(
        inc.layout_runs, 1,
        "the incremental recolour frame must not have re-run layout"
    );

    // ── Full rebuild: a fresh pipeline rendering the already-recoloured DOM. ──
    let mut full = DesktopPipeline::new(&config);
    full.set_theme(paint_lever_theme());
    let mut full_doc = DesktopDocument::from_html(paint_lever_html());
    let a_full = full_doc.doc.get_element_by_id("a").unwrap();
    full_doc
        .doc
        .set_inline_style(a_full, "background-color", "rgb(7, 8, 9)");
    let (full_out, _) = full.run(&mut full_doc.doc, 16.0);

    assert_eq!(
        fill_signature(&inc_out.display_list),
        fill_signature(&full_out.display_list),
        "incremental paint-only output must be structurally identical to a full rebuild"
    );
    assert_eq!(
        layout_box_signature(&inc_out.layout),
        layout_box_signature(&full_out.layout),
        "incremental-reused layout must match the from-scratch layout"
    );
}

#[test]
fn geometry_inline_change_still_runs_layout() {
    // TOOTH (b): a geometry property MUST run layout — no false fast-path.
    let config = PipelineConfig {
        width: 400.0,
        height: 200.0,
        ..PipelineConfig::default()
    };
    let mut pipeline = DesktopPipeline::new(&config);
    pipeline.set_theme(paint_lever_theme());
    let mut desktop = DesktopDocument::from_html(paint_lever_html());

    let (out0, _) = pipeline.run(&mut desktop.doc, 16.0);
    let layout_before = layout_box_signature(&out0.layout);
    let runs_layout_0 = pipeline.layout_runs;
    desktop.doc.dirty.clear_all();

    // Widen #a — a geometry change. Must mark LAYOUT and actually relayout.
    let a_id = desktop.doc.get_element_by_id("a").unwrap();
    desktop.doc.set_inline_style(a_id, "width", "160px");
    assert!(
        desktop.doc.dirty.layout.contains(&a_id),
        "a width change must mark the layout dirty set"
    );

    let (out1, _) = pipeline.run(&mut desktop.doc, 16.0);
    assert!(
        pipeline.layout_runs > runs_layout_0,
        "a geometry change MUST re-run layout (layout_runs unchanged = false fast-path)"
    );

    // And the geometry actually changed (so the layout run was meaningful).
    let layout_after = layout_box_signature(&out1.layout);
    let b_id = desktop.doc.get_element_by_id("b").unwrap();
    let before_b = layout_before.iter().find(|(n, _)| *n == b_id).copied();
    let after_b = layout_after.iter().find(|(n, _)| *n == b_id).copied();
    assert_ne!(
        before_b, after_b,
        "widening #a must shift sibling #b's position in the flex row"
    );
}

#[test]
fn pseudo_state_change_is_conservatively_layout_dirty() {
    // The DOM cannot know which properties a `:hover` rule changes, so a
    // pseudo-state flip is CONSERVATIVELY marked layout-dirty (err toward
    // LAYOUT). This guards against an over-eager fast path that would assume all
    // hovers are paint-only — a wrong assumption could leave stale layout if a
    // hover rule changed geometry.
    let mut desktop = DesktopDocument::from_html(r#"<box id="x" />"#);
    desktop.doc.dirty.clear_all();
    let x = desktop.doc.get_element_by_id("x").unwrap();
    desktop
        .doc
        .set_pseudo_state(x, liquide_dom::PseudoStateFlags::HOVER, true);
    assert!(
        desktop.doc.dirty.layout.contains(&x),
        "a pseudo-state change must be conservatively layout-dirty (unknown rule properties)"
    );
}
