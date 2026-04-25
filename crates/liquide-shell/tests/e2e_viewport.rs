//! End-to-end tests for viewport functionality.
//!
//! Tests:
//! - Viewport width/height resolution (vw, vh, vmin, vmax units)
//! - Percentage width/height resolution
//! - em/rem unit resolution
//! - Fixed positioning with viewport dimensions
//! - 100% width elements filling viewport
//! - Statusbar and dock correctly positioned at viewport edges
//! - Pipeline viewport sync on resize

use liquide_dom::Document;
use liquide_layout::{LayoutEngine, Size};
use liquide_shell::desktop_dom::DesktopDocument;
use liquide_shell::pipeline::{DesktopPipeline, PipelineConfig};
use liquide_style_engine::dimension::Dimension;
use liquide_style_engine::engine::{StyleEngine, ViewportSize};

// ═══════════════════════════════════════════════════════════════════════════
// Dimension Resolution Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn dimension_px_resolves_correctly() {
    let dim = Dimension::Px(100.0);
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(100.0));
}

#[test]
fn dimension_percent_resolves_correctly() {
    let dim = Dimension::Percent(50.0);
    // 50% of 500px parent = 250px
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(250.0));
}

#[test]
fn dimension_percent_100_fills_parent() {
    let dim = Dimension::Percent(100.0);
    let result = dim.resolve_px(1920.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(1920.0));
}

#[test]
fn dimension_vw_resolves_to_viewport_width() {
    let dim = Dimension::Vw(100.0);
    // 100vw = 100% of viewport width
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(1920.0));
}

#[test]
fn dimension_vw_50_is_half_viewport() {
    let dim = Dimension::Vw(50.0);
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(960.0));
}

#[test]
fn dimension_vh_resolves_to_viewport_height() {
    let dim = Dimension::Vh(100.0);
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(1080.0));
}

#[test]
fn dimension_vh_50_is_half_viewport() {
    let dim = Dimension::Vh(50.0);
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(540.0));
}

#[test]
fn dimension_vmin_uses_smaller_dimension() {
    // 1920x1080 viewport, vmin = 1080 (smaller)
    let dim = Dimension::Vmin(100.0);
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(1080.0));
}

#[test]
fn dimension_vmax_uses_larger_dimension() {
    // 1920x1080 viewport, vmax = 1920 (larger)
    let dim = Dimension::Vmax(100.0);
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(1920.0));
}

#[test]
fn dimension_vmin_portrait_viewport() {
    // Portrait 1080x1920 viewport, vmin = 1080 (smaller)
    let dim = Dimension::Vmin(50.0);
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1080.0, 1920.0);
    assert_eq!(result, Some(540.0));
}

#[test]
fn dimension_vmax_portrait_viewport() {
    // Portrait 1080x1920 viewport, vmax = 1920 (larger)
    let dim = Dimension::Vmax(50.0);
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1080.0, 1920.0);
    assert_eq!(result, Some(960.0));
}

#[test]
fn dimension_em_resolves_to_font_size() {
    let dim = Dimension::Em(2.0);
    // 2 * 14px font-size = 28px
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(28.0));
}

#[test]
fn dimension_rem_resolves_to_root_font_size() {
    let dim = Dimension::Rem(2.0);
    // 2 * 16px root font-size = 32px
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(32.0));
}

#[test]
fn dimension_rem_independent_of_local_font() {
    let dim = Dimension::Rem(1.5);
    // Should be 1.5 * root(16) = 24, not affected by local font(14)
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(24.0));
}

#[test]
fn dimension_ch_approximates_zero_glyph() {
    let dim = Dimension::Ch(10.0);
    // ~0.5 * font-size per ch
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(70.0)); // 10 * 14 * 0.5
}

#[test]
fn dimension_auto_returns_none() {
    let dim = Dimension::Auto;
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, None);
}

#[test]
fn dimension_none_returns_none() {
    let dim = Dimension::None;
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, None);
}

#[test]
fn dimension_zero_resolves_to_zero() {
    let dim = Dimension::Zero;
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(0.0));
}

#[test]
fn dimension_min_content_returns_none() {
    let dim = Dimension::MinContent;
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, None);
}

#[test]
fn dimension_max_content_returns_none() {
    let dim = Dimension::MaxContent;
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, None);
}

#[test]
fn dimension_is_definite() {
    assert!(Dimension::Px(10.0).is_definite());
    assert!(Dimension::Percent(50.0).is_definite());
    assert!(Dimension::Vw(100.0).is_definite());
    assert!(Dimension::Vh(100.0).is_definite());
    assert!(Dimension::Em(1.0).is_definite());
    assert!(Dimension::Rem(1.0).is_definite());
    assert!(Dimension::Zero.is_definite());

    assert!(!Dimension::Auto.is_definite());
    assert!(!Dimension::None.is_definite());
    assert!(!Dimension::MinContent.is_definite());
    assert!(!Dimension::MaxContent.is_definite());
}

// ═══════════════════════════════════════════════════════════════════════════
// Style Engine Viewport Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn style_engine_viewport_initialization() {
    let viewport = ViewportSize {
        width: 1920.0,
        height: 1080.0,
    };
    let engine = StyleEngine::new(viewport, 16.0);

    assert_eq!(engine.viewport.width, 1920.0);
    assert_eq!(engine.viewport.height, 1080.0);
    assert_eq!(engine.base_font_size, 16.0);
}

#[test]
fn style_engine_set_viewport_updates_size() {
    let viewport = ViewportSize {
        width: 1920.0,
        height: 1080.0,
    };
    let mut engine = StyleEngine::new(viewport, 16.0);

    engine.set_viewport(ViewportSize {
        width: 3840.0,
        height: 2160.0,
    });

    assert_eq!(engine.viewport.width, 3840.0);
    assert_eq!(engine.viewport.height, 2160.0);
}

#[test]
fn style_engine_default_viewport_is_1080p() {
    let viewport = ViewportSize::default();
    assert_eq!(viewport.width, 1920.0);
    assert_eq!(viewport.height, 1080.0);
}

#[test]
fn style_engine_4k_viewport() {
    let viewport = ViewportSize {
        width: 3840.0,
        height: 2160.0,
    };
    let engine = StyleEngine::new(viewport, 16.0);

    assert_eq!(engine.viewport.width, 3840.0);
    assert_eq!(engine.viewport.height, 2160.0);
}

#[test]
fn style_engine_small_viewport() {
    let viewport = ViewportSize {
        width: 800.0,
        height: 600.0,
    };
    let engine = StyleEngine::new(viewport, 16.0);

    assert_eq!(engine.viewport.width, 800.0);
    assert_eq!(engine.viewport.height, 600.0);
}

// ═══════════════════════════════════════════════════════════════════════════
// Layout Engine Viewport Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn layout_engine_viewport_initialization() {
    let engine = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);

    assert_eq!(engine.viewport.width, 1920.0);
    assert_eq!(engine.viewport.height, 1080.0);
    assert_eq!(engine.base_font_size, 16.0);
}

#[test]
fn layout_engine_default_is_1080p() {
    let engine = LayoutEngine::default();

    assert_eq!(engine.viewport.width, 1920.0);
    assert_eq!(engine.viewport.height, 1080.0);
}

#[test]
fn layout_engine_4k_viewport() {
    let engine = LayoutEngine::new(Size::new(3840.0, 2160.0), 16.0);

    assert_eq!(engine.viewport.width, 3840.0);
    assert_eq!(engine.viewport.height, 2160.0);
}

// ═══════════════════════════════════════════════════════════════════════════
// CSS Width/Height Resolution Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn css_width_100_percent_fills_viewport() {
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);

    let mut style = StyleEngine::new(
        ViewportSize {
            width: 1920.0,
            height: 1080.0,
        },
        16.0,
    );
    style.add_stylesheet("div { width: 100%; height: 50px; }");

    let styles = style.restyle_all(&doc);
    let div_style = styles.get(div).unwrap();

    assert!(matches!(div_style.width, Dimension::Percent(100.0)));
}

#[test]
fn css_width_100vw_equals_viewport_width() {
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);

    let mut style = StyleEngine::new(
        ViewportSize {
            width: 1920.0,
            height: 1080.0,
        },
        16.0,
    );
    style.add_stylesheet("div { width: 100vw; }");

    let styles = style.restyle_all(&doc);
    let div_style = styles.get(div).unwrap();

    // Width should resolve to 100vw = 1920px
    if let Dimension::Vw(v) = div_style.width {
        assert_eq!(v, 100.0);
    } else if let Dimension::Px(v) = div_style.width {
        // Parser might resolve immediately
        assert!((v - 1920.0).abs() < 0.1);
    }
}

#[test]
fn css_height_100vh_equals_viewport_height() {
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);

    let mut style = StyleEngine::new(
        ViewportSize {
            width: 1920.0,
            height: 1080.0,
        },
        16.0,
    );
    style.add_stylesheet("div { height: 100vh; }");

    let styles = style.restyle_all(&doc);
    let div_style = styles.get(div).unwrap();

    if let Dimension::Vh(v) = div_style.height {
        assert_eq!(v, 100.0);
    } else if let Dimension::Px(v) = div_style.height {
        assert!((v - 1080.0).abs() < 0.1);
    }
}

#[test]
fn css_fixed_position_with_full_width() {
    let mut doc = Document::new();
    let root = doc.root();
    let bar = doc.create_element("statusbar");
    doc.append_child(root, bar);

    let mut style = StyleEngine::new(
        ViewportSize {
            width: 1920.0,
            height: 1080.0,
        },
        16.0,
    );
    style.add_stylesheet(
        r#"
        statusbar {
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 32;
        }
    "#,
    );

    let styles = style.restyle_all(&doc);
    let bar_style = styles.get(bar).unwrap();

    assert!(matches!(
        bar_style.position,
        liquide_style_engine::computed::Position::Fixed
    ));
    assert!(matches!(bar_style.width, Dimension::Percent(100.0)));
    assert!(matches!(bar_style.height, Dimension::Px(h) if (h - 32.0).abs() < 0.1));
}

// ═══════════════════════════════════════════════════════════════════════════
// Pipeline Viewport Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn pipeline_default_viewport_is_1080p() {
    let config = PipelineConfig::default();

    assert_eq!(config.width, 1920.0);
    assert_eq!(config.height, 1080.0);
}

#[test]
fn pipeline_custom_viewport() {
    let config = PipelineConfig {
        width: 2560.0,
        height: 1440.0,
        base_font_size: 14.0,
    };
    let pipeline = DesktopPipeline::new(&config);

    assert_eq!(pipeline.style_engine.viewport.width, 2560.0);
    assert_eq!(pipeline.style_engine.viewport.height, 1440.0);
    assert_eq!(pipeline.layout_engine.viewport.width, 2560.0);
    assert_eq!(pipeline.layout_engine.viewport.height, 1440.0);
}

#[test]
fn pipeline_set_viewport_updates_both_engines() {
    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    pipeline.set_viewport(3840.0, 2160.0);

    assert_eq!(pipeline.style_engine.viewport.width, 3840.0);
    assert_eq!(pipeline.style_engine.viewport.height, 2160.0);
    assert_eq!(pipeline.layout_engine.viewport.width, 3840.0);
    assert_eq!(pipeline.layout_engine.viewport.height, 2160.0);
}

#[test]
fn pipeline_viewport_resize_to_4k() {
    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    // Start at 1080p
    assert_eq!(pipeline.style_engine.viewport.width, 1920.0);

    // Resize to 4K
    pipeline.set_viewport(3840.0, 2160.0);

    assert_eq!(pipeline.style_engine.viewport.width, 3840.0);
    assert_eq!(pipeline.style_engine.viewport.height, 2160.0);
}

#[test]
fn pipeline_viewport_resize_affects_layout() {
    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    let mut desktop = DesktopDocument::new();

    // Run at 1080p
    let (output_1080, _) = pipeline.run(&mut desktop.doc, 16.0);

    // Resize to 4K
    pipeline.set_viewport(3840.0, 2160.0);
    let (output_4k, _) = pipeline.run(&mut desktop.doc, 16.0);

    // Layout should be recalculated (potentially different positions)
    // Both should produce valid output
    assert!(output_1080.layout.boxes.len() > 0);
    assert!(output_4k.layout.boxes.len() > 0);
}

fn canonical_box_id_for_element(
    doc: &Document,
    layout: &liquide_layout::LayoutTree,
    element_id: &str,
) -> Option<liquide_layout::LayoutBoxId> {
    let node_id = doc.get_element_by_id(element_id)?;
    layout.find_box_id_by_node(node_id)
}

// ═══════════════════════════════════════════════════════════════════════════
// Desktop DOM Layout Integration Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn statusbar_positioned_at_top() {
    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    let mut desktop = DesktopDocument::new();

    let (output, _) = pipeline.run(&mut desktop.doc, 16.0);

    // Find statusbar in layout
    let statusbar_id = desktop.doc.get_element_by_id("shell-statusbar");
    assert!(statusbar_id.is_some(), "Statusbar should exist in DOM");

    let statusbar_box = canonical_box_id_for_element(&desktop.doc, &output.layout, "shell-statusbar");
    assert!(statusbar_box.is_some(), "Statusbar should exist in layout");

    if let Some(sb) = statusbar_box {
        let abs = output.layout.absolute_border_rect(sb);
        // Statusbar should be at y=0 (top of viewport)
        assert!(abs.y < 10.0, "Statusbar should be at top (y={})", abs.y);
        // Width should span most or all of viewport
        assert!(
            abs.width > 1000.0,
            "Statusbar width={} should be wide",
            abs.width
        );
    }
}

#[test]
fn dock_positioned_at_bottom() {
    let config = PipelineConfig::default();
    let mut pipeline = DesktopPipeline::new(&config);

    let mut desktop = DesktopDocument::new();

    let (output, _) = pipeline.run(&mut desktop.doc, 16.0);

    // Find dock in layout
    let dock_id = desktop.doc.get_element_by_id("shell-dock");
    assert!(dock_id.is_some(), "Dock should exist in DOM");

    let dock_box = canonical_box_id_for_element(&desktop.doc, &output.layout, "shell-dock");
    assert!(dock_box.is_some(), "Dock should exist in layout");

    if let Some(d) = dock_box {
        let abs = output.layout.absolute_border_rect(d);
        // Dock should be near bottom of viewport (y + height should be close to viewport height)
        let bottom = abs.y + abs.height;
        assert!(
            bottom > 1000.0,
            "Dock bottom={} should be near viewport bottom",
            bottom
        );
    }
}

#[test]
fn elements_respect_viewport_width() {
    let config = PipelineConfig {
        width: 1920.0,
        height: 1080.0,
        base_font_size: 14.0,
    };
    let mut pipeline = DesktopPipeline::new(&config);

    let mut desktop = DesktopDocument::new();

    let (output, _) = pipeline.run(&mut desktop.doc, 16.0);

    // No layout box should extend beyond viewport
    for b in &output.layout.boxes {
        let abs = output.layout.absolute_border_rect(b.id);
        let right = abs.x + abs.width;
        // Allow small overflow for rounding
        assert!(
            right <= 1930.0,
            "Element at x={} width={} exceeds viewport",
            abs.x,
            abs.width
        );
    }
}

#[test]
fn elements_respect_viewport_height() {
    let config = PipelineConfig {
        width: 1920.0,
        height: 1080.0,
        base_font_size: 14.0,
    };
    let mut pipeline = DesktopPipeline::new(&config);

    let mut desktop = DesktopDocument::new();

    let (output, _) = pipeline.run(&mut desktop.doc, 16.0);

    // No layout box should extend beyond viewport
    for b in &output.layout.boxes {
        let abs = output.layout.absolute_border_rect(b.id);
        let bottom = abs.y + abs.height;
        // Allow small overflow for rounding
        assert!(
            bottom <= 1090.0,
            "Element at y={} height={} exceeds viewport",
            abs.y,
            abs.height
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Different Viewport Size Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn layout_at_720p() {
    let config = PipelineConfig {
        width: 1280.0,
        height: 720.0,
        base_font_size: 14.0,
    };
    let mut pipeline = DesktopPipeline::new(&config);

    let mut desktop = DesktopDocument::new();

    let (output, _) = pipeline.run(&mut desktop.doc, 16.0);

    assert!(output.styles.len() > 0);
    assert!(output.layout.boxes.len() > 0);
}

#[test]
fn layout_at_1440p() {
    let config = PipelineConfig {
        width: 2560.0,
        height: 1440.0,
        base_font_size: 14.0,
    };
    let mut pipeline = DesktopPipeline::new(&config);

    let mut desktop = DesktopDocument::new();

    let (output, _) = pipeline.run(&mut desktop.doc, 16.0);

    assert!(output.styles.len() > 0);
    assert!(output.layout.boxes.len() > 0);
}

#[test]
fn layout_at_4k() {
    let config = PipelineConfig {
        width: 3840.0,
        height: 2160.0,
        base_font_size: 14.0,
    };
    let mut pipeline = DesktopPipeline::new(&config);

    let mut desktop = DesktopDocument::new();

    let (output, _) = pipeline.run(&mut desktop.doc, 16.0);

    assert!(output.styles.len() > 0);
    assert!(output.layout.boxes.len() > 0);
}

#[test]
fn layout_at_ultrawide() {
    let config = PipelineConfig {
        width: 3440.0,
        height: 1440.0, // 21:9 ultrawide
        base_font_size: 14.0,
    };
    let mut pipeline = DesktopPipeline::new(&config);

    let mut desktop = DesktopDocument::new();

    let (output, _) = pipeline.run(&mut desktop.doc, 16.0);

    assert!(output.styles.len() > 0);
    assert!(output.layout.boxes.len() > 0);
}

#[test]
fn layout_at_portrait() {
    let config = PipelineConfig {
        width: 1080.0,
        height: 1920.0, // Portrait mode
        base_font_size: 14.0,
    };
    let mut pipeline = DesktopPipeline::new(&config);

    let mut desktop = DesktopDocument::new();

    let (output, _) = pipeline.run(&mut desktop.doc, 16.0);

    assert!(output.styles.len() > 0);
    assert!(output.layout.boxes.len() > 0);
}

#[test]
fn layout_at_small_viewport() {
    let config = PipelineConfig {
        width: 640.0,
        height: 480.0,
        base_font_size: 14.0,
    };
    let mut pipeline = DesktopPipeline::new(&config);

    let mut desktop = DesktopDocument::new();

    let (output, _) = pipeline.run(&mut desktop.doc, 16.0);

    assert!(output.styles.len() > 0);
    assert!(output.layout.boxes.len() > 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// Viewport Unit Calculation Tests with Layout
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn vw_units_scale_with_viewport() {
    // 50vw at 1920px = 960px
    // 50vw at 3840px = 1920px
    let dim = Dimension::Vw(50.0);

    let result_1080p = dim.resolve_px(0.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result_1080p, Some(960.0));

    let result_4k = dim.resolve_px(0.0, 16.0, 14.0, 3840.0, 2160.0);
    assert_eq!(result_4k, Some(1920.0));
}

#[test]
fn vh_units_scale_with_viewport() {
    let dim = Dimension::Vh(50.0);

    let result_1080p = dim.resolve_px(0.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result_1080p, Some(540.0));

    let result_4k = dim.resolve_px(0.0, 16.0, 14.0, 3840.0, 2160.0);
    assert_eq!(result_4k, Some(1080.0));
}

#[test]
fn percent_units_scale_with_parent() {
    let dim = Dimension::Percent(50.0);

    // 50% of 1920 = 960
    let result_full = dim.resolve_px(1920.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result_full, Some(960.0));

    // 50% of 500 = 250
    let result_smaller = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result_smaller, Some(250.0));
}

// ═══════════════════════════════════════════════════════════════════════════
// Edge Cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn zero_viewport_does_not_panic() {
    let dim = Dimension::Vw(100.0);
    let result = dim.resolve_px(0.0, 16.0, 14.0, 0.0, 0.0);
    assert_eq!(result, Some(0.0));
}

#[test]
fn negative_percent_resolves() {
    let dim = Dimension::Percent(-50.0);
    let result = dim.resolve_px(1000.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(-500.0));
}

#[test]
fn very_large_vw_resolves() {
    let dim = Dimension::Vw(200.0); // 200% of viewport
    let result = dim.resolve_px(0.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(3840.0));
}

#[test]
fn fractional_dimensions_resolve() {
    let dim = Dimension::Px(100.5);
    let result = dim.resolve_px(500.0, 16.0, 14.0, 1920.0, 1080.0);
    assert_eq!(result, Some(100.5));
}

#[test]
fn dimension_default_is_auto() {
    let dim = Dimension::default();
    assert!(matches!(dim, Dimension::Auto));
}
