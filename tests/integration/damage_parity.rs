//! CPU/Wgpu damage classification parity tests.
//!
//! These tests verify that both the CPU and wgpu renderers produce
//! identical damage tile classifications for the same scene and damage
//! input. Session tile encoding relies on this classification parity.

use liquide_compositor::damage::{DamageClass, DamageSet, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{FlatNode, SceneNodeKind};
use liquide_compositor::Renderer;
use liquide_renderer_cpu::SoftwareRenderer;

#[cfg(not(target_os = "windows"))]
use liquide_renderer_wgpu::WgpuRenderer;

const TILE_SIZE: u32 = 64;
const WIDTH: u32 = 256;
const HEIGHT: u32 = 192;

fn make_fb() -> FrameBuffer {
    FrameBuffer::new(WIDTH, HEIGHT)
}

fn make_full_damage() -> DamageSet {
    DamageSet::full(
        TILE_SIZE,
        WIDTH / TILE_SIZE,
        HEIGHT / TILE_SIZE,
        DamageClass::UiPrimitive,
    )
}

fn make_partial_damage(tiles: Vec<(u32, u32)>) -> DamageSet {
    let damage_tiles = tiles
        .into_iter()
        .map(|(x, y)| DamageTile {
            x,
            y,
            class: DamageClass::UiPrimitive,
        })
        .collect();
    DamageSet::from_tiles(TILE_SIZE, damage_tiles)
}

fn classify_as_set(tiles: &[DamageTile]) -> std::collections::HashMap<(u32, u32), DamageClass> {
    tiles.iter().map(|t| ((t.x, t.y), t.class)).collect()
}

#[test]
fn text_node_classified_as_text_glyph() {
    let nodes = vec![FlatNode {
        kind: SceneNodeKind::Text {
            text: "Hello".to_string(),
            font_family: "sans".to_string(),
            font_size: 14.0,
            font_weight: 400,
            color: Color::rgb(0, 0, 0),
            letter_spacing: 0.0,
            word_spacing: 0.0,
            font_features: Vec::new(),
        },
        absolute_bounds: Rect::new(10.0, 10.0, 80.0, 20.0),
        clip: None,
        opacity: 1.0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
    }];

    let mut fb = make_fb();
    let damage = make_full_damage();

    let mut cpu = SoftwareRenderer::new();
    let cpu_damage = cpu.render(&nodes, &mut fb, &damage).unwrap();
    let cpu_classes = classify_as_set(&cpu_damage);

    // Text node spans tile (0, 0) — should be TextGlyph
    assert_eq!(
        cpu_classes.get(&(0, 0)),
        Some(&DamageClass::TextGlyph),
        "CPU should classify text tile as TextGlyph"
    );

    #[cfg(not(target_os = "windows"))]
    {
        let mut wgpu = WgpuRenderer::new(WIDTH, HEIGHT).expect("wgpu init");
        let wgpu_damage = wgpu.render(&nodes, &mut fb, &damage).unwrap();
        let wgpu_classes = classify_as_set(&wgpu_damage);

        assert_eq!(
            wgpu_classes.get(&(0, 0)),
            Some(&DamageClass::TextGlyph),
            "Wgpu should classify text tile as TextGlyph"
        );
        assert_eq!(
            cpu_classes.get(&(0, 0)),
            wgpu_classes.get(&(0, 0)),
            "CPU and wgpu must agree on text classification"
        );
    }
}

#[test]
fn cursor_node_classified_as_cursor_only() {
    let nodes = vec![FlatNode {
        kind: SceneNodeKind::Cursor {
            bitmap_width: 24,
            bitmap_height: 24,
            hotspot_x: 0,
            hotspot_y: 0,
        },
        absolute_bounds: Rect::new(100.0, 100.0, 24.0, 24.0),
        clip: None,
        opacity: 1.0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
    }];

    let mut fb = make_fb();
    let damage = make_full_damage();

    let mut cpu = SoftwareRenderer::new();
    let cpu_damage = cpu.render(&nodes, &mut fb, &damage).unwrap();
    let cpu_classes = classify_as_set(&cpu_damage);

    // Cursor is in tile (1, 1) — should be CursorOnly
    assert_eq!(
        cpu_classes.get(&(1, 1)),
        Some(&DamageClass::CursorOnly),
        "CPU should classify cursor tile as CursorOnly"
    );

    #[cfg(not(target_os = "windows"))]
    {
        let mut wgpu = WgpuRenderer::new(WIDTH, HEIGHT).expect("wgpu init");
        let wgpu_damage = wgpu.render(&nodes, &mut fb, &damage).unwrap();
        let wgpu_classes = classify_as_set(&wgpu_damage);

        assert_eq!(
            wgpu_classes.get(&(1, 1)),
            Some(&DamageClass::CursorOnly),
            "Wgpu should classify cursor tile as CursorOnly"
        );
        assert_eq!(
            cpu_classes.get(&(1, 1)),
            wgpu_classes.get(&(1, 1)),
            "CPU and wgpu must agree on cursor classification"
        );
    }
}

#[test]
fn surface_node_classified_as_bitmap_region() {
    let nodes = vec![FlatNode {
        kind: SceneNodeKind::Surface { scale: 1.0 },
        absolute_bounds: Rect::new(0.0, 0.0, 128.0, 128.0),
        clip: None,
        opacity: 1.0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
    }];

    let mut fb = make_fb();
    let damage = make_full_damage();

    let mut cpu = SoftwareRenderer::new();
    let cpu_damage = cpu.render(&nodes, &mut fb, &damage).unwrap();
    let cpu_classes = classify_as_set(&cpu_damage);

    // Surface covers tiles (0,0), (1,0), (0,1), (1,1) — should be BitmapRegion
    assert_eq!(
        cpu_classes.get(&(0, 0)),
        Some(&DamageClass::BitmapRegion),
        "CPU should classify surface tile as BitmapRegion"
    );

    #[cfg(not(target_os = "windows"))]
    {
        let mut wgpu = WgpuRenderer::new(WIDTH, HEIGHT).expect("wgpu init");
        let wgpu_damage = wgpu.render(&nodes, &mut fb, &damage).unwrap();
        let wgpu_classes = classify_as_set(&wgpu_damage);

        assert_eq!(
            wgpu_classes.get(&(0, 0)),
            Some(&DamageClass::BitmapRegion),
            "Wgpu should classify surface tile as BitmapRegion"
        );
        assert_eq!(
            cpu_classes.get(&(0, 0)),
            wgpu_classes.get(&(0, 0)),
            "CPU and wgpu must agree on surface classification"
        );
    }
}

#[test]
fn background_node_classified_as_ui_primitive() {
    let nodes = vec![FlatNode {
        kind: SceneNodeKind::Background {
            color: Color::rgb(128, 128, 128),
        },
        absolute_bounds: Rect::new(50.0, 50.0, 80.0, 40.0),
        clip: None,
        opacity: 1.0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
    }];

    let mut fb = make_fb();
    let damage = make_full_damage();

    let mut cpu = SoftwareRenderer::new();
    let cpu_damage = cpu.render(&nodes, &mut fb, &damage).unwrap();
    let cpu_classes = classify_as_set(&cpu_damage);

    // Background spans tile (0, 0) — should be UiPrimitive
    assert_eq!(
        cpu_classes.get(&(0, 0)),
        Some(&DamageClass::UiPrimitive),
        "CPU should classify background tile as UiPrimitive"
    );

    #[cfg(not(target_os = "windows"))]
    {
        let mut wgpu = WgpuRenderer::new(WIDTH, HEIGHT).expect("wgpu init");
        let wgpu_damage = wgpu.render(&nodes, &mut fb, &damage).unwrap();
        let wgpu_classes = classify_as_set(&wgpu_damage);

        assert_eq!(
            wgpu_classes.get(&(0, 0)),
            Some(&DamageClass::UiPrimitive),
            "Wgpu should classify background tile as UiPrimitive"
        );
        assert_eq!(
            cpu_classes.get(&(0, 0)),
            wgpu_classes.get(&(0, 0)),
            "CPU and wgpu must agree on background classification"
        );
    }
}

#[test]
fn overlapping_nodes_use_highest_priority_class() {
    let nodes = vec![
        FlatNode {
            kind: SceneNodeKind::Background {
                color: Color::rgb(200, 200, 200),
            },
            absolute_bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            clip: None,
            opacity: 1.0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
        },
        FlatNode {
            kind: SceneNodeKind::Text {
                text: "Overlay".to_string(),
                font_family: "sans".to_string(),
                font_size: 16.0,
                font_weight: 400,
                color: Color::rgb(0, 0, 0),
                letter_spacing: 0.0,
                word_spacing: 0.0,
                font_features: Vec::new(),
            },
            absolute_bounds: Rect::new(10.0, 10.0, 80.0, 20.0),
            clip: None,
            opacity: 1.0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
        },
    ];

    let mut fb = make_fb();
    let damage = make_full_damage();

    let mut cpu = SoftwareRenderer::new();
    let cpu_damage = cpu.render(&nodes, &mut fb, &damage).unwrap();
    let cpu_classes = classify_as_set(&cpu_damage);

    // Tile (0, 0) has both Background (UiPrimitive) and Text (TextGlyph)
    // TextGlyph has priority 0, so it wins
    assert_eq!(
        cpu_classes.get(&(0, 0)),
        Some(&DamageClass::TextGlyph),
        "CPU should use highest priority (TextGlyph) for overlapping content"
    );

    #[cfg(not(target_os = "windows"))]
    {
        let mut wgpu = WgpuRenderer::new(WIDTH, HEIGHT).expect("wgpu init");
        let wgpu_damage = wgpu.render(&nodes, &mut fb, &damage).unwrap();
        let wgpu_classes = classify_as_set(&wgpu_damage);

        assert_eq!(
            wgpu_classes.get(&(0, 0)),
            Some(&DamageClass::TextGlyph),
            "Wgpu should use highest priority (TextGlyph) for overlapping content"
        );
        assert_eq!(
            cpu_classes.get(&(0, 0)),
            wgpu_classes.get(&(0, 0)),
            "CPU and wgpu must agree on priority resolution"
        );
    }
}

#[test]
fn partial_damage_classified_correctly() {
    let nodes = vec![
        FlatNode {
            kind: SceneNodeKind::Background {
                color: Color::rgb(255, 255, 255),
            },
            absolute_bounds: Rect::new(0.0, 0.0, 64.0, 64.0),
            clip: None,
            opacity: 1.0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
        },
        FlatNode {
            kind: SceneNodeKind::Text {
                text: "Partial".to_string(),
                font_family: "sans".to_string(),
                font_size: 14.0,
                font_weight: 400,
                color: Color::rgb(0, 0, 0),
                letter_spacing: 0.0,
                word_spacing: 0.0,
                font_features: Vec::new(),
            },
            absolute_bounds: Rect::new(128.0, 0.0, 64.0, 32.0),
            clip: None,
            opacity: 1.0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
        },
    ];

    let mut fb = make_fb();
    // Only damage tiles (0, 0) and (2, 0)
    let damage = make_partial_damage(vec![(0, 0), (2, 0)]);

    let mut cpu = SoftwareRenderer::new();
    let cpu_damage = cpu.render(&nodes, &mut fb, &damage).unwrap();
    let cpu_classes = classify_as_set(&cpu_damage);

    // For CPU renderer with partial damage, we expect classification of just
    // the damaged tiles: (0,0) -> UiPrimitive, (2,0) -> TextGlyph
    assert_eq!(cpu_classes.get(&(0, 0)), Some(&DamageClass::UiPrimitive));
    assert_eq!(cpu_classes.get(&(2, 0)), Some(&DamageClass::TextGlyph));

    #[cfg(not(target_os = "windows"))]
    {
        let mut wgpu = WgpuRenderer::new(WIDTH, HEIGHT).expect("wgpu init");
        let wgpu_damage = wgpu.render(&nodes, &mut fb, &damage).unwrap();
        let wgpu_classes = classify_as_set(&wgpu_damage);

        // Wgpu promotes partial damage to full for CPU readback, but should still
        // classify tiles according to content. Both (0,0) and (2,0) should be
        // classified identically to CPU.
        assert_eq!(
            cpu_classes.get(&(0, 0)),
            wgpu_classes.get(&(0, 0)),
            "CPU and wgpu must agree on tile (0,0) classification"
        );
        assert_eq!(
            cpu_classes.get(&(2, 0)),
            wgpu_classes.get(&(2, 0)),
            "CPU and wgpu must agree on tile (2,0) classification"
        );
    }
}

#[test]
fn empty_damage_returns_empty_classification() {
    let nodes = vec![FlatNode {
        kind: SceneNodeKind::Background {
            color: Color::rgb(100, 100, 100),
        },
        absolute_bounds: Rect::new(0.0, 0.0, 256.0, 192.0),
        clip: None,
        opacity: 1.0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
    }];

    let mut fb = make_fb();
    let damage = DamageSet::new(TILE_SIZE);

    let mut cpu = SoftwareRenderer::new();
    let cpu_damage = cpu.render(&nodes, &mut fb, &damage).unwrap();
    assert!(cpu_damage.is_empty(), "CPU should return empty for empty damage");

    #[cfg(not(target_os = "windows"))]
    {
        let mut wgpu = WgpuRenderer::new(WIDTH, HEIGHT).expect("wgpu init");
        let wgpu_damage = wgpu.render(&nodes, &mut fb, &damage).unwrap();
        assert!(wgpu_damage.is_empty(), "Wgpu should return empty for empty damage");
    }
}
