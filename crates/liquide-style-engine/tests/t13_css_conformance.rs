use liquide_dom::Document;
use liquide_dom::dirty::DirtySet;
use liquide_style_engine::StyleEngine;

#[path = "../../liquide-conformance/src/css.rs"]
mod shared_css;

use shared_css::{STYLE_ENGINE_FIXTURES, StyleEngineScenario};

fn assert_rgb(actual: (u8, u8, u8), expected: (u8, u8, u8), label: &str) {
    assert_eq!(actual, expected, "{label}");
}

#[test]
fn css_conformance_style_engine_fixtures() {
    for fixture in STYLE_ENGINE_FIXTURES {
        match fixture.scenario {
            StyleEngineScenario::RelativeHasChildSelector => {
                let mut engine = StyleEngine::default();
                engine.add_stylesheet(fixture.css);

                let mut doc = Document::new();
                let root = doc.root();

                let section_with_child = doc.create_element("section");
                let direct_img = doc.create_element("img");
                doc.append_child(root, section_with_child);
                doc.append_child(section_with_child, direct_img);

                let section_with_nested = doc.create_element("section");
                let wrapper = doc.create_element("div");
                let nested_img = doc.create_element("img");
                doc.append_child(root, section_with_nested);
                doc.append_child(section_with_nested, wrapper);
                doc.append_child(wrapper, nested_img);

                let styles = engine.restyle_all(&doc);
                let direct = styles.get(section_with_child).unwrap();
                let nested = styles.get(section_with_nested).unwrap();

                assert_rgb(
                    (direct.color.r, direct.color.g, direct.color.b),
                    (0, 255, 0),
                    &format!("{} {}", fixture.meta.id, fixture.meta.title),
                );
                assert_rgb(
                    (nested.color.r, nested.color.g, nested.color.b),
                    (255, 0, 0),
                    &format!("{} {}", fixture.meta.id, fixture.meta.title),
                );
            }
            StyleEngineScenario::UnsupportedPseudoFailsClosed => {
                let mut engine = StyleEngine::default();
                engine.add_stylesheet(fixture.css);

                let mut doc = Document::new();
                let root = doc.root();
                let div = doc.create_element("div");
                doc.append_child(root, div);

                let style = engine.compute_style(&doc, div);
                assert_rgb(
                    (style.color.r, style.color.g, style.color.b),
                    (255, 0, 0),
                    &format!("{} {}", fixture.meta.id, fixture.meta.title),
                );
            }
            StyleEngineScenario::QuotedAttributeSelector => {
                let mut engine = StyleEngine::default();
                engine.add_stylesheet(fixture.css);

                let mut doc = Document::new();
                let root = doc.root();
                let matching = doc.create_element("a");
                doc.set_attribute(matching, "href", "https://example.com?q=.foo123");
                doc.append_child(root, matching);

                let plain = doc.create_element("a");
                doc.set_attribute(plain, "href", "https://example.net?q=.foo");
                doc.append_child(root, plain);

                let styles = engine.restyle_all(&doc);
                let matching_style = styles.get(matching).unwrap();
                let plain_style = styles.get(plain).unwrap();

                assert_rgb(
                    (
                        matching_style.color.r,
                        matching_style.color.g,
                        matching_style.color.b,
                    ),
                    (0, 255, 0),
                    &format!("{} {}", fixture.meta.id, fixture.meta.title),
                );
                assert_rgb(
                    (
                        plain_style.color.r,
                        plain_style.color.g,
                        plain_style.color.b,
                    ),
                    (255, 0, 0),
                    &format!("{} {}", fixture.meta.id, fixture.meta.title),
                );
            }
            StyleEngineScenario::LangAndDirInheritance => {
                let mut engine = StyleEngine::default();
                engine.add_stylesheet(fixture.css);

                let mut doc = Document::new();
                let root = doc.root();
                let parent = doc.create_element("div");
                let child = doc.create_element("span");
                doc.append_child(root, parent);
                doc.append_child(parent, child);
                doc.set_attribute(parent, "lang", "en-US");
                doc.set_attribute(parent, "dir", "rtl");

                let style = engine.compute_style(&doc, child);
                assert_rgb(
                    (style.color.r, style.color.g, style.color.b),
                    (0, 255, 0),
                    &format!("{} {}", fixture.meta.id, fixture.meta.title),
                );
                assert_rgb(
                    (
                        style.background_color.r,
                        style.background_color.g,
                        style.background_color.b,
                    ),
                    (0, 0, 255),
                    &format!("{} {}", fixture.meta.id, fixture.meta.title),
                );
            }
            StyleEngineScenario::SupportsAndMediaFailClosed => {
                let engine = StyleEngine::default();
                let label = format!("{} {}", fixture.meta.id, fixture.meta.title);

                assert!(
                    !engine.evaluate_supports_condition("selector(:has(*))"),
                    "{label}"
                );
                assert!(
                    !engine.evaluate_supports_condition("(display: definitely-not-real)"),
                    "{label}"
                );
                assert!(
                    engine.evaluate_media_condition("(hover: hover) or (pointer: coarse)"),
                    "{label}"
                );
                assert!(
                    engine.evaluate_media_condition("(400px < width < 2400px)"),
                    "{label}"
                );
                assert!(
                    !engine.evaluate_media_condition("(400px < width < 1200px)"),
                    "{label}"
                );
                assert!(
                    !engine.evaluate_media_condition("(totally-unknown: 1)"),
                    "{label}"
                );
            }
            StyleEngineScenario::ScopeEndBounds => {
                let mut engine = StyleEngine::default();
                engine.add_stylesheet(fixture.css);

                let mut doc = Document::new();
                let root = doc.root();

                let panel = doc.create_element("div");
                doc.add_class(panel, "panel");
                doc.append_child(root, panel);

                let allowed = doc.create_element("button");
                doc.append_child(panel, allowed);

                let limit = doc.create_element("div");
                doc.add_class(limit, "limit");
                doc.append_child(panel, limit);

                let blocked = doc.create_element("button");
                doc.append_child(limit, blocked);

                let styles = engine.restyle_all(&doc);
                let allowed_style = styles.get(allowed).unwrap();
                let blocked_style = styles.get(blocked).unwrap();

                assert_rgb(
                    (
                        allowed_style.color.r,
                        allowed_style.color.g,
                        allowed_style.color.b,
                    ),
                    (0, 255, 0),
                    &format!("{} {}", fixture.meta.id, fixture.meta.title),
                );
                assert_rgb(
                    (
                        blocked_style.color.r,
                        blocked_style.color.g,
                        blocked_style.color.b,
                    ),
                    (255, 0, 0),
                    &format!("{} {}", fixture.meta.id, fixture.meta.title),
                );
            }
            StyleEngineScenario::IncrementalCustomPropertyScope => {
                let mut engine = StyleEngine::default();
                engine.add_stylesheet(fixture.css);

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
                assert_rgb(
                    (initial.color.r, initial.color.g, initial.color.b),
                    (255, 0, 0),
                    &format!("{} {} initial", fixture.meta.id, fixture.meta.title),
                );

                engine.invalidate(&doc, &[target], &mut styles);
                let invalidated = styles.get(target).unwrap();
                assert_rgb(
                    (
                        invalidated.color.r,
                        invalidated.color.g,
                        invalidated.color.b,
                    ),
                    (255, 0, 0),
                    &format!("{} {} invalidate", fixture.meta.id, fixture.meta.title),
                );

                let mut dirty = DirtySet::new();
                dirty.mark_style(target);
                engine.restyle_dirty(&doc, &dirty, &mut styles);
                let dirty_style = styles.get(target).unwrap();
                assert_rgb(
                    (
                        dirty_style.color.r,
                        dirty_style.color.g,
                        dirty_style.color.b,
                    ),
                    (255, 0, 0),
                    &format!("{} {} dirty", fixture.meta.id, fixture.meta.title),
                );
            }
            StyleEngineScenario::ShadowBoundaryIsolation => {
                let mut engine = StyleEngine::default();
                engine.add_stylesheet(fixture.css);

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
                assert_rgb(
                    (full.color.r, full.color.g, full.color.b),
                    (0, 255, 0),
                    &format!("{} {} full", fixture.meta.id, fixture.meta.title),
                );

                engine.invalidate(&doc, &[inner], &mut styles);
                let incremental = styles.get(inner).unwrap();
                assert_rgb(
                    (
                        incremental.color.r,
                        incremental.color.g,
                        incremental.color.b,
                    ),
                    (0, 255, 0),
                    &format!("{} {} incremental", fixture.meta.id, fixture.meta.title),
                );
            }
        }
    }
}
