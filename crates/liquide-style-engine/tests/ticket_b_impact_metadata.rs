use liquide_common::PipelineImpact;
use liquide_style_engine::{StyleChangeImpact, StyleDiffSummary, classify_style_property};

fn impact_for(property: &str) -> StyleChangeImpact {
    StyleChangeImpact::from_property(property)
}

#[test]
fn paint_only_properties_do_not_require_layout_or_compositor() {
    for property in [
        "color",
        "background-color",
        "border-color",
        "border-top-color",
    ] {
        let impact = impact_for(property);
        assert!(impact.affects_paint(), "{property} should affect paint");
        assert!(
            !impact.affects_layout(),
            "{property} should not affect layout"
        );
        assert!(
            !impact.affects_compositor(),
            "{property} should not affect compositor metadata"
        );
        assert!(
            !impact.affects_accessibility(),
            "{property} should not affect accessibility"
        );
    }

    assert!(impact_for("color").affects_inherited_style());
    assert!(!impact_for("background-color").affects_inherited_style());
}

#[test]
fn layout_and_intrinsic_properties_are_geometry_affecting() {
    for property in ["width", "height", "margin", "padding", "display"] {
        let impact = impact_for(property);
        assert!(impact.affects_layout(), "{property} should affect layout");
        assert!(
            impact.affects_paint(),
            "{property} should affect paint output"
        );
    }

    for property in ["font-size", "line-height"] {
        let impact = impact_for(property);
        assert!(impact.affects_layout(), "{property} should affect layout");
        assert!(
            impact.affects_intrinsic_measure(),
            "{property} should affect intrinsic measure"
        );
        assert!(
            impact.affects_inherited_style(),
            "{property} should propagate inherited style"
        );
    }
}

#[test]
fn transform_and_effect_properties_are_compositor_affecting() {
    let transform = impact_for("transform");
    assert!(transform.contains(PipelineImpact::TRANSFORM_ONLY));
    assert!(transform.affects_compositor());
    assert!(!transform.affects_layout());

    let opacity = impact_for("opacity");
    assert!(opacity.contains(PipelineImpact::OPACITY_ONLY));
    assert!(opacity.affects_compositor());
    assert!(!opacity.affects_layout());

    for property in ["filter", "clip-path"] {
        let impact = impact_for(property);
        assert!(
            impact.affects_compositor(),
            "{property} should affect compositor"
        );
        assert!(
            !impact.affects_layout(),
            "{property} should not require layout"
        );
    }
}

#[test]
fn inherited_and_resource_properties_are_marked() {
    let font_family = impact_for("font-family");
    assert!(font_family.affects_inherited_style());
    assert!(font_family.affects_intrinsic_measure());
    assert!(font_family.affects_layout());
    assert!(font_family.affects_resources());

    let background_image = impact_for("background-image");
    assert!(background_image.affects_paint());
    assert!(background_image.affects_resources());
    assert!(!background_image.affects_layout());

    let cursor = impact_for("cursor");
    assert!(cursor.affects_inherited_style());
    assert!(cursor.affects_resources());
}

#[test]
fn visibility_and_generated_content_affect_accessibility() {
    for property in ["display", "visibility", "content"] {
        let impact = impact_for(property);
        assert!(
            impact.affects_accessibility(),
            "{property} should affect accessibility"
        );
    }

    assert!(impact_for("visibility").affects_inherited_style());
    assert!(impact_for("content").affects_layout());
}

#[test]
fn custom_and_unknown_properties_are_conservative() {
    for property in ["--accent", "future-rendering-mode"] {
        let impact = impact_for(property);
        assert!(
            impact.affects_inherited_style(),
            "{property} should be inherited-safe"
        );
        assert!(
            impact.affects_layout(),
            "{property} should not under-invalidate layout"
        );
        assert!(
            impact.affects_paint(),
            "{property} should not under-invalidate paint"
        );
        assert!(
            impact.affects_compositor(),
            "{property} should not under-invalidate compositor"
        );
        assert!(
            impact.affects_accessibility(),
            "{property} should not under-invalidate accessibility"
        );
        assert!(
            impact.affects_resources(),
            "{property} should not under-invalidate resources"
        );
    }
}

#[test]
fn style_diff_summary_preserves_changes_and_combines_impacts() {
    let summary = StyleDiffSummary::from_properties(["color", "transform", "font-size"]);

    assert_eq!(summary.len(), 3);
    assert_eq!(summary.changes()[0].property, "color");
    assert!(summary.affects_inherited_style());
    assert!(summary.affects_layout());
    assert!(summary.affects_paint());
    assert!(summary.affects_compositor());
    assert!(!summary.affects_accessibility());

    let raw = classify_style_property("opacity");
    assert!(raw.contains(PipelineImpact::OPACITY_ONLY));
}
