use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use liquide_dom::Document;
use liquide_style_engine::StyleEngine;

fn temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("liquide-{label}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn cleanup_dir(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

#[test]
fn load_stylesheet_file_honors_import_qualifiers() {
    let dir = temp_dir("t13-e4-imports");
    let imported = dir.join("imported.css");
    let root_true = dir.join("root-true.css");
    let root_false = dir.join("root-false.css");

    fs::write(&imported, "button { background-color: #0000ff; }").unwrap();
    fs::write(
        &root_true,
        "@import \"imported.css\" supports(display: grid) screen; button { color: #ff0000; }",
    )
    .unwrap();
    fs::write(
        &root_false,
        "@import \"imported.css\" supports(display: definitely-not-real) screen; button { color: #ff0000; }",
    )
    .unwrap();

    let mut doc = Document::new();
    let root = doc.root();
    let button = doc.create_element("button");
    doc.append_child(root, button);

    let mut true_engine = StyleEngine::default();
    true_engine.load_stylesheet_file(&root_true).unwrap();
    let true_style = true_engine.compute_style(&doc, button);
    assert_eq!(true_style.color.r, 255);
    assert_eq!(true_style.background_color.b, 255);

    let mut false_engine = StyleEngine::default();
    false_engine.load_stylesheet_file(&root_false).unwrap();
    let false_style = false_engine.compute_style(&doc, button);
    assert_eq!(false_style.color.r, 255);
    assert_eq!(false_style.background_color.a, 0);

    cleanup_dir(&dir);
}

#[test]
fn scope_end_bounds_are_enforced() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            @scope (.panel) to (.limit) {
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

    assert_eq!((allowed_style.color.r, allowed_style.color.g), (0, 255));
    assert_eq!((blocked_style.color.r, blocked_style.color.g), (255, 0));
}

#[test]
fn nested_container_contents_are_compiled() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .panel { container-type: inline-size; }
            button { color: #ff0000; }
            @container (min-width: 100px) {
                @supports (display: flex) {
                    button { color: #00ff00; }
                }
            }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let panel = doc.create_element("div");
    doc.add_class(panel, "panel");
    doc.append_child(root, panel);

    let inside = doc.create_element("button");
    doc.append_child(panel, inside);

    let outside = doc.create_element("button");
    doc.append_child(root, outside);

    let styles = engine.restyle_all(&doc);
    let inside_style = styles.get(inside).unwrap();
    let outside_style = styles.get(outside).unwrap();

    assert_eq!((inside_style.color.r, inside_style.color.g), (0, 255));
    assert_eq!((outside_style.color.r, outside_style.color.g), (255, 0));
}

#[test]
fn unsupported_supports_and_media_fail_closed() {
    let engine = StyleEngine::default();

    assert!(!engine.evaluate_supports_condition("selector(:has(*))"));
    assert!(!engine.evaluate_supports_condition("(display: definitely-not-real)"));
    assert!(engine.evaluate_media_condition("(hover: hover) or (pointer: coarse)"));
    assert!(engine.evaluate_media_condition("(400px < width < 2400px)"));
    assert!(!engine.evaluate_media_condition("(400px < width < 1200px)"));
    assert!(!engine.evaluate_media_condition("(totally-unknown: 1)"));
}