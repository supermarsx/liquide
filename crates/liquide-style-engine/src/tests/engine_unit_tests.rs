use super::*;
use crate::PseudoKind;
use liquide_dom::Document;

#[test]
fn empty_engine() {
    let engine = StyleEngine::default();
    let doc = Document::new();
    let style = engine.compute_style(&doc, doc.root());
    assert_eq!(style.display, Display::Block);
}

#[test]
fn basic_style_computation() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            div {
                display: flex;
                width: 100px;
                color: red;
            }
            "#,
    );

    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);

    let style = engine.compute_style(&doc, div);
    assert_eq!(style.display, Display::Flex);
}

#[test]
fn restyle_all() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            statusbar {
                display: flex;
                position: fixed;
                height: 28px;
            }
            dock {
                display: flex;
                gap: 4px;
            }
            "#,
    );

    let mut doc = Document::new();
    let root = doc.root();
    let bar = doc.create_element("statusbar");
    let dock = doc.create_element("dock");
    doc.append_child(root, bar);
    doc.append_child(root, dock);

    let map = engine.restyle_all(&doc);
    let bar_style = map.get(bar).unwrap();
    assert_eq!(bar_style.display, Display::Flex);
    assert_eq!(bar_style.position, Position::Fixed);

    let dock_style = map.get(dock).unwrap();
    assert_eq!(dock_style.display, Display::Flex);
}

#[test]
fn prefers_color_scheme_media_query_follows_setting() {
    let mut engine = StyleEngine::default();
    assert!(engine.evaluate_media_condition("(prefers-color-scheme: light)"));
    assert!(!engine.evaluate_media_condition("(prefers-color-scheme: dark)"));

    engine.set_preferred_color_scheme("dark");
    assert!(engine.evaluate_media_condition("(prefers-color-scheme: dark)"));
    assert!(!engine.evaluate_media_condition("(prefers-color-scheme: light)"));
}

#[test]
fn supports_rules_are_respected() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            button { color: #ff0000; }
            @supports (display: grid) {
                button { color: #00ff00; }
            }
            @supports (nonexistent-prop: foo) {
                button { color: #0000ff; }
            }
            "#,
    );

    let mut doc = Document::new();
    let root = doc.root();
    let button = doc.create_element("button");
    doc.append_child(root, button);

    let style = engine.compute_style(&doc, button);
    assert_eq!(style.color.g, 255);
    assert_eq!(style.color.r, 0);
}

#[test]
fn scope_rules_match_dom_descendants() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            @scope (.panel) {
                button { color: #00ff00; }
            }
            button { color: #ff0000; }
            "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let panel = doc.create_element("div");
    doc.add_class(panel, "panel");
    doc.append_child(root, panel);

    let scoped_button = doc.create_element("button");
    doc.append_child(panel, scoped_button);

    let unscoped_button = doc.create_element("button");
    doc.append_child(root, unscoped_button);

    let scoped_style = engine.compute_style(&doc, scoped_button);
    assert_eq!(scoped_style.color.g, 255);

    let plain_style = engine.compute_style(&doc, unscoped_button);
    assert_eq!(plain_style.color.r, 255);
}

#[test]
fn current_color_on_color_property_inherits_parent_value() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .parent { color: red; }
            .child  { color: currentColor; }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let parent = doc.create_element("div");
    doc.add_class(parent, "parent");
    doc.append_child(root, parent);

    let child = doc.create_element("span");
    doc.add_class(child, "child");
    doc.append_child(parent, child);

    let styles = engine.restyle_all(&doc);
    let child_style = styles.get(child).unwrap();
    // `color: currentColor` on the `color` property must resolve to the
    // inherited (parent) value per CSS Color Level 4.
    assert_eq!(child_style.color.r, 255);
    assert_eq!(child_style.color.g, 0);
    assert_eq!(child_style.color.b, 0);
}

#[test]
fn invalidate_preserves_inherited_custom_property_scope() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .red-scope { --accent: #ff0000; }
            .blue-scope { --accent: #0000ff; }
            .target { color: var(--accent); }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let red_scope = doc.create_element("div");
    doc.add_class(red_scope, "red-scope");
    doc.append_child(root, red_scope);

    let target = doc.create_element("span");
    doc.add_class(target, "target");
    doc.append_child(red_scope, target);

    let blue_scope = doc.create_element("div");
    doc.add_class(blue_scope, "blue-scope");
    doc.append_child(root, blue_scope);

    let mut styles = engine.restyle_all(&doc);
    let initial = styles.get(target).unwrap();
    assert_eq!(
        (initial.color.r, initial.color.g, initial.color.b),
        (255, 0, 0)
    );

    engine.invalidate(&doc, &[target], &mut styles);

    let updated = styles.get(target).unwrap();
    assert_eq!(
        (updated.color.r, updated.color.g, updated.color.b),
        (255, 0, 0)
    );
}

#[test]
fn restyle_dirty_rebuilds_ancestor_custom_property_scope() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .red-scope { --accent: #ff0000; }
            .blue-scope { --accent: #0000ff; }
            .target { color: var(--accent); }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let red_scope = doc.create_element("div");
    doc.add_class(red_scope, "red-scope");
    doc.append_child(root, red_scope);

    let target = doc.create_element("span");
    doc.add_class(target, "target");
    doc.append_child(red_scope, target);

    let blue_scope = doc.create_element("div");
    doc.add_class(blue_scope, "blue-scope");
    doc.append_child(root, blue_scope);

    let mut styles = engine.restyle_all(&doc);
    let mut dirty = liquide_dom::dirty::DirtySet::new();
    dirty.mark_style(target);
    engine.restyle_dirty(&doc, &dirty, &mut styles);

    let updated = styles.get(target).unwrap();
    assert_eq!(
        (updated.color.r, updated.color.g, updated.color.b),
        (255, 0, 0)
    );
}

#[test]
fn shadow_root_custom_property_scope_stays_isolated() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .host { --accent: #ff0000; }
            .other { --accent: #0000ff; }
            .inner { color: var(--accent, #00ff00); }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let host = doc.create_element("div");
    doc.add_class(host, "host");
    doc.append_child(root, host);

    let shadow_root = doc.create_shadow_root();
    doc.append_child(host, shadow_root);

    let inner = doc.create_element("span");
    doc.add_class(inner, "inner");
    doc.append_child(shadow_root, inner);

    let other = doc.create_element("div");
    doc.add_class(other, "other");
    doc.append_child(root, other);

    let mut styles = engine.restyle_all(&doc);
    let full = styles.get(inner).unwrap();
    assert_eq!((full.color.r, full.color.g, full.color.b), (0, 255, 0));

    engine.invalidate(&doc, &[inner], &mut styles);
    let incremental = styles.get(inner).unwrap();
    assert_eq!(
        (
            incremental.color.r,
            incremental.color.g,
            incremental.color.b
        ),
        (0, 255, 0)
    );
}

#[ignore = "public pseudo-rule ingestion still depends on selector/stylesheet routing outside this validation path"]
#[test]
fn pseudo_elements_use_local_custom_properties() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            div::first-line {
                --accent: #ff0000;
                color: var(--accent);
            }
            .other { --accent: #0000ff; }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let host = doc.create_element("div");
    doc.append_child(root, host);

    let other = doc.create_element("div");
    doc.add_class(other, "other");
    doc.append_child(root, other);

    let styles = engine.restyle_all(&doc);
    let first_line = styles.get_pseudo(host, PseudoKind::FirstLine).unwrap();
    assert_eq!(
        (first_line.color.r, first_line.color.g, first_line.color.b),
        (255, 0, 0)
    );
}

#[test]
fn all_initial_resets_inherited_properties_to_initial_values() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .parent { color: #ff0000; }
            .reset { all: initial; }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let parent = doc.create_element("div");
    doc.add_class(parent, "parent");
    doc.append_child(root, parent);

    let child = doc.create_element("span");
    doc.add_class(child, "reset");
    doc.append_child(parent, child);

    let styles = engine.restyle_all(&doc);
    let child_style = styles.get(child).unwrap();
    assert_eq!(
        (
            child_style.color.r,
            child_style.color.g,
            child_style.color.b
        ),
        (0, 0, 0)
    );
}

#[test]
fn all_revert_restores_parent_inherited_values() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .parent { color: #ff0000; }
            .reset { color: #0000ff; }
            .reset { all: revert; }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let parent = doc.create_element("div");
    doc.add_class(parent, "parent");
    doc.append_child(root, parent);

    let child = doc.create_element("span");
    doc.add_class(child, "reset");
    doc.append_child(parent, child);

    let styles = engine.restyle_all(&doc);
    let child_style = styles.get(child).unwrap();
    assert_eq!(
        (
            child_style.color.r,
            child_style.color.g,
            child_style.color.b
        ),
        (255, 0, 0)
    );
}

// ── t50-e13 regressions (t49-e2-F1 / F3 / F4) ───────────────────────────────

/// t49-e2-F1: a normal unlayered rule must beat a normal layered rule even when
/// the layered rule has higher specificity — unlayered author styles act as the
/// last implicit layer for normal declarations (CSS Cascade 5 §6.4.2).
#[test]
fn unlayered_normal_rule_beats_layered_rule() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            @layer base {
                div#main { color: #ff0000; }
            }
            div { color: #00ff00; }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.set_id(div, "main");
    doc.append_child(root, div);

    let style = engine.compute_style(&doc, div);
    // Unlayered green wins over the higher-specificity layered red.
    assert_eq!((style.color.r, style.color.g, style.color.b), (0, 255, 0));
}

/// t49-e2-F1: among declared layers, the later-declared layer wins for normal
/// declarations.
#[test]
fn later_declared_layer_wins_for_normal_declarations() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            @layer first, second;
            @layer first  { div { color: #ff0000; } }
            @layer second { div { color: #00ff00; } }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);

    let style = engine.compute_style(&doc, div);
    // `second` is declared after `first`, so it wins.
    assert_eq!((style.color.r, style.color.g, style.color.b), (0, 255, 0));
}

/// t49-e2-F1: `!important` reverses layer ordering — a layered `!important` rule
/// beats an unlayered `!important` rule (unlayered loses for important).
#[test]
fn important_reverses_layer_ordering_unlayered_loses() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            @layer base {
                div { color: #ff0000 !important; }
            }
            div { color: #00ff00 !important; }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);

    let style = engine.compute_style(&doc, div);
    // For !important, layer order reverses: the layered red beats unlayered green.
    assert_eq!((style.color.r, style.color.g, style.color.b), (255, 0, 0));
}

/// t49-e2-F3: a value that merely *contains* a CSS-wide keyword as a substring
/// (e.g. `fade-initial`, `"Inherit Sans"`) must NOT be treated as the keyword and
/// dropped/inherited, while a bare CSS-wide keyword still is honored.
#[test]
fn css_wide_keyword_requires_whole_value_match() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            #anim   { animation-name: fade-initial; }
            #fonted { font-family: "Inherit Sans"; }
            #bare   { animation-name: inherit; }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    // `fade-initial` contains "initial" but is NOT the keyword — must be kept.
    let anim = doc.create_element("div");
    doc.set_id(anim, "anim");
    doc.append_child(root, anim);
    let anim_style = engine.compute_style(&doc, anim);
    assert_eq!(anim_style.animation_name.as_deref(), Some("fade-initial"));

    // `"Inherit Sans"` contains "inherit" but is NOT the keyword — must be set.
    let fonted = doc.create_element("div");
    doc.set_id(fonted, "fonted");
    doc.append_child(root, fonted);
    let fonted_style = engine.compute_style(&doc, fonted);
    assert!(
        fonted_style.font_family.iter().any(|f| f == "Inherit Sans"),
        "font_family was {:?}",
        fonted_style.font_family
    );

    // A bare `inherit` is still a CSS-wide keyword: animation-name inherits the
    // root default (None) rather than becoming Some("inherit").
    let bare = doc.create_element("div");
    doc.set_id(bare, "bare");
    doc.append_child(root, bare);
    let bare_style = engine.compute_style(&doc, bare);
    assert_ne!(bare_style.animation_name.as_deref(), Some("inherit"));
}

/// t49-e2-F4: default `to bottom` (180deg) gradient must run top→bottom in the
/// renderer's y-down space, and explicit angles must map to the right direction.
#[test]
fn linear_gradient_angle_maps_to_ydown_space() {
    use liquide_compositor::scene::{BackgroundImage, GradientSpec};

    fn endpoints(css: &str) -> (f32, f32, f32, f32) {
        let mut engine = StyleEngine::default();
        engine.add_stylesheet(&format!("div {{ background: {css}; }}"));
        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);
        let styles = engine.restyle_all(&doc);
        let style = styles.get(div).unwrap();
        match style.background[0].image.as_ref().unwrap() {
            BackgroundImage::Gradient(GradientSpec::Linear {
                start_x,
                start_y,
                end_x,
                end_y,
                ..
            }) => (*start_x, *start_y, *end_x, *end_y),
            other => panic!("expected linear gradient, got {other:?}"),
        }
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    // Default `to bottom` (180deg): start at top (y=0), end at bottom (y=1).
    let (sx, sy, ex, ey) = endpoints("linear-gradient(red, blue)");
    assert!(approx(sx, 0.5) && approx(sy, 0.0), "start {sx},{sy}");
    assert!(approx(ex, 0.5) && approx(ey, 1.0), "end {ex},{ey}");

    // `to right` (90deg): start at left (x=0), end at right (x=1).
    let (sx, sy, ex, ey) = endpoints("linear-gradient(90deg, red, blue)");
    assert!(approx(sx, 0.0) && approx(sy, 0.5), "start {sx},{sy}");
    assert!(approx(ex, 1.0) && approx(ey, 0.5), "end {ex},{ey}");

    // `to top` (0deg): start at bottom (y=1), end at top (y=0).
    let (sx, sy, ex, ey) = endpoints("linear-gradient(0deg, red, blue)");
    assert!(approx(sx, 0.5) && approx(sy, 1.0), "start {sx},{sy}");
    assert!(approx(ex, 0.5) && approx(ey, 0.0), "end {ex},{ey}");
}

#[test]
fn transition_duration_defaults_transition_definitions_to_all() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet("div { transition-duration: 120ms; }");

    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);

    let styles = engine.restyle_all(&doc);
    let style = styles.get(div).unwrap();
    assert_eq!(style.transition.len(), 1);
    assert_eq!(style.transition[0].property, "all");
    assert!((style.transition[0].duration_ms - 120.0).abs() < 0.01);
}

// ── Regression: cascaded physical padding/margin must survive the
// restyle_node / restyle_all path (the path the real layout pipeline uses).
//
// Previously `resolve_logical_properties` clobbered freshly-cascaded physical
// padding/margin back to zero, because the logical longhands defaulted to
// `Dimension::Zero` (treated as "set") instead of `Auto` ("unset"). This fired
// only on restyle_*, NOT on compute_style — which is why earlier
// compute_style-only parity tests masked the bug. These tests drive the
// restyle path explicitly. See `.orchestration/logs/t62-logical.md`.

#[test]
fn restyle_path_preserves_physical_padding_left() {
    use crate::Dimension;

    let mut engine = StyleEngine::default();
    engine.add_stylesheet("menu-item { padding-left: 12px; }");

    let mut doc = Document::new();
    let root = doc.root();
    let item = doc.create_element("menu-item");
    doc.append_child(root, item);

    let map = engine.restyle_all(&doc);
    let style = map.get(item).unwrap();
    assert_eq!(
        style.padding.left,
        Dimension::Px(12.0),
        "padding-left:12px must reach computed padding.left on the restyle path"
    );
}

#[test]
fn restyle_path_preserves_physical_margin_right() {
    use crate::Dimension;

    let mut engine = StyleEngine::default();
    engine.add_stylesheet("menu-item { margin-right: 8px; }");

    let mut doc = Document::new();
    let root = doc.root();
    let item = doc.create_element("menu-item");
    doc.append_child(root, item);

    let map = engine.restyle_all(&doc);
    let style = map.get(item).unwrap();
    assert_eq!(
        style.margin.right,
        Dimension::Px(8.0),
        "margin-right:8px must reach computed margin.right on the restyle path"
    );
}

#[test]
fn restyle_path_preserves_padding_and_margin_shorthands() {
    use crate::Dimension;

    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        "menu-item { padding: 5px 10px 15px 20px; margin: 3px 6px; }",
    );

    let mut doc = Document::new();
    let root = doc.root();
    let item = doc.create_element("menu-item");
    doc.append_child(root, item);

    let map = engine.restyle_all(&doc);
    let style = map.get(item).unwrap();

    // padding: top right bottom left
    assert_eq!(style.padding.top, Dimension::Px(5.0), "padding.top");
    assert_eq!(style.padding.right, Dimension::Px(10.0), "padding.right");
    assert_eq!(style.padding.bottom, Dimension::Px(15.0), "padding.bottom");
    assert_eq!(style.padding.left, Dimension::Px(20.0), "padding.left");

    // margin: TB=3 LR=6
    assert_eq!(style.margin.top, Dimension::Px(3.0), "margin.top");
    assert_eq!(style.margin.bottom, Dimension::Px(3.0), "margin.bottom");
    assert_eq!(style.margin.left, Dimension::Px(6.0), "margin.left");
    assert_eq!(style.margin.right, Dimension::Px(6.0), "margin.right");
}

#[test]
fn restyle_path_unset_logical_does_not_clobber_physical() {
    use crate::Dimension;

    // Element sets ONLY physical longhands; the logical longhands stay at their
    // (now `Auto`) default. They must NOT overwrite the physical values.
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        "menu-item { padding-left: 12px; padding-top: 7px; margin-left: 4px; }",
    );

    let mut doc = Document::new();
    let root = doc.root();
    let item = doc.create_element("menu-item");
    doc.append_child(root, item);

    let map = engine.restyle_all(&doc);
    let style = map.get(item).unwrap();
    assert_eq!(style.padding.left, Dimension::Px(12.0), "padding.left survives");
    assert_eq!(style.padding.top, Dimension::Px(7.0), "padding.top survives");
    assert_eq!(style.margin.left, Dimension::Px(4.0), "margin.left survives");
}

#[test]
fn restyle_path_real_logical_property_still_maps() {
    use crate::Dimension;

    // A genuinely-set logical property must still map to the physical side on
    // the restyle path (horizontal-tb LTR: inline-start -> left).
    let mut engine = StyleEngine::default();
    engine.add_stylesheet("menu-item { padding-inline-start: 9px; }");

    let mut doc = Document::new();
    let root = doc.root();
    let item = doc.create_element("menu-item");
    doc.append_child(root, item);

    let map = engine.restyle_all(&doc);
    let style = map.get(item).unwrap();
    assert_eq!(
        style.padding.left,
        Dimension::Px(9.0),
        "padding-inline-start must map to physical padding.left (LTR)"
    );
}
