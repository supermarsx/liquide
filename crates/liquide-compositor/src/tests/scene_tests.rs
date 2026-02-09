use crate::scene::*;
use crate::geometry::Rect;
use crate::pixel::Color;

#[test]
fn scene_node_flatten_basic() {
    let root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );
    let flat = root.flatten();
    // Root alone is not visual (Root kind is excluded)
    assert!(flat.is_empty());
}

#[test]
fn scene_node_flatten_with_children() {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );

    let bg = SceneNode::new(
        1,
        SceneNodeKind::Background {
            color: Color::new(30, 30, 40, 255),
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );

    let surface = SceneNode::new(
        2,
        SceneNodeKind::Surface {
            surface_id: 42,
            buffer: None,
        },
        NodeProperties::new(Rect::new(100.0, 100.0, 800.0, 600.0)),
    );

    root.add_child(bg);
    root.add_child(surface);

    let flat = root.flatten();
    assert_eq!(flat.len(), 2);
    assert_eq!(flat[0].id, 1); // Background
    assert_eq!(flat[1].id, 2); // Surface
    assert!((flat[1].absolute_bounds.x - 100.0).abs() < f32::EPSILON);
}

#[test]
fn scene_node_invisible_skipped() {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );

    let hidden = SceneNode::new(
        1,
        SceneNodeKind::Background {
            color: Color::BLACK,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)).with_visible(false),
    );

    root.add_child(hidden);
    let flat = root.flatten();
    assert!(flat.is_empty());
}
