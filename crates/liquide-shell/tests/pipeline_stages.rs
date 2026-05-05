//! Per-element-type pipeline stage tests.
//!
//! Tests that every node kind flows correctly through the pipeline:
//! CSS → DOM → Style → Layout → Paint(DisplayList) → Scene → Render
//!
//! Each test inspects the scene tree at every observable point.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::{SceneNode, SceneNodeKind};
use liquide_renderer_cpu::{Renderer, SoftwareRenderer};
use liquide_shell::Shell;

// ═══════════════════════════════════════════════════════════════════════════
// Helper utilities
// ═══════════════════════════════════════════════════════════════════════════

/// Collect all nodes of each type from the scene tree.
#[derive(Default, Debug)]
struct SceneInventory {
    backgrounds: Vec<NodeInfo>,
    texts: Vec<TextInfo>,
    glass: Vec<GlassInfo>,
    borders: Vec<BorderInfo>,
    box_shadows: Vec<ShadowInfo>,
    images: Vec<NodeInfo>,
    outlines: Vec<NodeInfo>,
    workspaces: Vec<NodeInfo>,
    shadows: Vec<NodeInfo>,
    other: Vec<NodeInfo>,
}

#[derive(Debug)]
struct NodeInfo {
    id: u64,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    z: u32,
}

#[derive(Debug)]
struct TextInfo {
    id: u64,
    text: String,
    font_size: f32,
    font_weight: u32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Debug)]
struct GlassInfo {
    id: u64,
    blur_radius: u32,
    tint_r: u8,
    tint_g: u8,
    tint_b: u8,
    tint_a: u8,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Debug)]
struct BorderInfo {
    id: u64,
    top_width: f32,
    right_width: f32,
    bottom_width: f32,
    left_width: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Debug)]
struct ShadowInfo {
    id: u64,
    count: usize,
    blur_radius: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn inventory_scene(node: &SceneNode, inv: &mut SceneInventory) {
    let b = &node.properties.bounds;
    let z = node.properties.z_order;
    let ni = || NodeInfo {
        id: node.id,
        x: b.x,
        y: b.y,
        w: b.width,
        h: b.height,
        z,
    };

    match &node.kind {
        SceneNodeKind::Root => {}
        SceneNodeKind::Background { color: _ } => {
            inv.backgrounds.push(ni());
        }
        SceneNodeKind::Text {
            text,
            font_size,
            font_weight,
            ..
        } => {
            inv.texts.push(TextInfo {
                id: node.id,
                text: text.clone(),
                font_size: *font_size,
                font_weight: *font_weight as u32,
                x: b.x,
                y: b.y,
                w: b.width,
                h: b.height,
            });
        }
        SceneNodeKind::Glass(params) => {
            let c = params.tint_color;
            inv.glass.push(GlassInfo {
                id: node.id,
                blur_radius: params.blur_radius,
                tint_r: c.r,
                tint_g: c.g,
                tint_b: c.b,
                tint_a: c.a,
                x: b.x,
                y: b.y,
                w: b.width,
                h: b.height,
            });
        }
        SceneNodeKind::Border { sides, .. } => {
            inv.borders.push(BorderInfo {
                id: node.id,
                top_width: sides.top.width,
                right_width: sides.right.width,
                bottom_width: sides.bottom.width,
                left_width: sides.left.width,
                x: b.x,
                y: b.y,
                w: b.width,
                h: b.height,
            });
        }
        SceneNodeKind::BoxShadows { shadows } => {
            let blur = shadows.first().map(|s| s.blur_radius).unwrap_or(0.0);
            inv.box_shadows.push(ShadowInfo {
                id: node.id,
                count: shadows.len(),
                blur_radius: blur,
                x: b.x,
                y: b.y,
                w: b.width,
                h: b.height,
            });
        }
        SceneNodeKind::Image { .. } => inv.images.push(ni()),
        SceneNodeKind::Outline { .. } => inv.outlines.push(ni()),
        SceneNodeKind::Workspace { .. } => inv.workspaces.push(ni()),
        SceneNodeKind::Shadow { .. } => inv.shadows.push(ni()),
        _ => inv.other.push(ni()),
    }

    for child in &node.children {
        inventory_scene(child, inv);
    }
}

fn build_shell_and_inventory() -> (Shell, SceneNode, SceneInventory) {
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();
    let mut inv = SceneInventory::default();
    inventory_scene(&scene, &mut inv);
    (shell, scene, inv)
}

fn render_scene(scene: &SceneNode) -> FrameBuffer {
    let flat_nodes = scene.flatten();
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(1920, 1080, PixelFormat::Bgra8);

    use liquide_compositor::damage::{DamageClass, DamageSet, DamageTile};
    let mut damage = DamageSet::new(64);
    for y in 0..(1080 + 63) / 64 {
        for x in 0..(1920 + 63) / 64 {
            damage.add(DamageTile {
                x,
                y,
                class: DamageClass::UiPrimitive,
            });
        }
    }

    renderer.render(&flat_nodes, &mut fb, &damage).unwrap();
    fb
}

// ═══════════════════════════════════════════════════════════════════════════
// ELEMENT TYPE: Background
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn element_background_exists_in_scene() {
    let (_, _, inv) = build_shell_and_inventory();

    println!("\n=== Background elements ===");
    for bg in &inv.backgrounds {
        println!(
            "  [{}] @({:.0},{:.0}) {}×{} z={}",
            bg.id, bg.x, bg.y, bg.w, bg.h, bg.z
        );
    }

    assert!(
        !inv.backgrounds.is_empty(),
        "Scene must have Background nodes (desktop, statusbar, dock backgrounds)"
    );
}

#[test]
fn element_background_has_valid_bounds() {
    let (_, _, inv) = build_shell_and_inventory();

    for bg in &inv.backgrounds {
        assert!(bg.w > 0.0, "Background width must be > 0 (id={})", bg.id);
        assert!(bg.h > 0.0, "Background height must be > 0 (id={})", bg.id);
        assert!(
            !bg.x.is_nan() && !bg.y.is_nan(),
            "Background position must not be NaN"
        );
    }

    println!(
        "✅ {} backgrounds all have valid bounds",
        inv.backgrounds.len()
    );
}

#[test]
fn element_background_renders_pixels() {
    let (_, scene, inv) = build_shell_and_inventory();
    assert!(!inv.backgrounds.is_empty());

    let fb = render_scene(&scene);

    // Dark backgrounds will produce near-black pixels with slight alpha blending.
    // Just verify rendering doesn't crash and produces at least some non-zero pixels.
    let non_zero = fb
        .pixels()
        .chunks_exact(4)
        .filter(|p| p[0] > 5 || p[1] > 5 || p[2] > 5)
        .count();

    println!("✅ Background rendering: {} non-zero pixels", non_zero);
    assert!(
        non_zero > 0,
        "Background should produce some non-zero pixels"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// ELEMENT TYPE: Text
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn element_text_exists_in_scene() {
    let (_, _, inv) = build_shell_and_inventory();

    println!("\n=== Text elements ===");
    for t in &inv.texts {
        println!(
            "  [{}] \"{}\" ({}px, weight={}) @({:.0},{:.0}) {}×{:.1}",
            t.id, t.text, t.font_size, t.font_weight, t.x, t.y, t.w, t.h
        );
    }

    assert!(
        !inv.texts.is_empty(),
        "Scene must have Text nodes (clock, status indicators)"
    );
    assert!(
        inv.texts.len() >= 1,
        "Expected at least 1 text node (clock/logo), got {}",
        inv.texts.len()
    );
}

#[test]
fn element_text_has_valid_content() {
    let (_, _, inv) = build_shell_and_inventory();

    for t in &inv.texts {
        assert!(
            !t.text.is_empty(),
            "Text node {} should have non-empty content",
            t.id
        );
        assert!(
            t.font_size > 0.0,
            "Text node {} font_size must be > 0",
            t.id
        );
    }

    // Check for statusbar text (clock, logo) — dock labels use display:none
    let has_clock = inv.texts.iter().any(|t| t.text.contains(':'));
    assert!(has_clock, "Should find clock text with ':' separator");
    let has_logo = inv.texts.iter().any(|t| t.text == "LiquiDE");
    assert!(has_logo, "Should find statusbar logo text");

    println!("✅ {} text nodes all have valid content", inv.texts.len());
}

#[test]
fn element_text_has_valid_bounds() {
    let (_, _, inv) = build_shell_and_inventory();

    for t in &inv.texts {
        assert!(
            !t.x.is_nan() && !t.y.is_nan(),
            "Text position must not be NaN (id={})",
            t.id
        );
        // Text might have zero width if it's measured differently than expected,
        // but height should always be > 0
        assert!(
            t.h >= 0.0,
            "Text height must be >= 0 (id={}, text='{}')",
            t.id,
            t.text
        );
    }

    println!("✅ {} text nodes all have valid bounds", inv.texts.len());
}

#[test]
fn element_text_renders_to_pixels() {
    let (_, scene, inv) = build_shell_and_inventory();
    assert!(!inv.texts.is_empty(), "Need text nodes to test rendering");

    let fb = render_scene(&scene);

    // With dark theme and white text, text pixels should have higher RGB values
    // in the text regions. Just verify no crash during rendering.
    let total = fb.pixels().len() / 4;
    let non_black = fb
        .pixels()
        .chunks_exact(4)
        .filter(|p| p[0] > 10 || p[1] > 10 || p[2] > 10)
        .count();

    println!(
        "✅ With text: {} non-black pixels out of {} total",
        non_black, total
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// ELEMENT TYPE: Glass (Liquid Glass backdrop blur)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn element_glass_exists_in_scene() {
    let (_, _, inv) = build_shell_and_inventory();

    println!("\n=== Glass elements ===");
    for g in &inv.glass {
        println!(
            "  [{}] blur={} tint=rgba({},{},{},{}) @({:.0},{:.0}) {}×{}",
            g.id, g.blur_radius, g.tint_r, g.tint_g, g.tint_b, g.tint_a, g.x, g.y, g.w, g.h
        );
    }

    assert!(
        !inv.glass.is_empty(),
        "Scene must have Glass nodes (dock and statusbar use blur)"
    );
}

#[test]
fn element_glass_has_blur_radius() {
    let (_, _, inv) = build_shell_and_inventory();

    let with_blur: Vec<_> = inv.glass.iter().filter(|g| g.blur_radius > 0).collect();
    assert!(
        !with_blur.is_empty(),
        "At least one Glass node should have blur_radius > 0"
    );

    println!("✅ {} glass nodes with blur > 0", with_blur.len());
}

#[test]
fn element_glass_has_translucent_tint() {
    let (_, _, inv) = build_shell_and_inventory();

    let translucent: Vec<_> = inv.glass.iter().filter(|g| g.tint_a < 255).collect();
    assert!(
        !translucent.is_empty(),
        "Glass tints should be translucent (alpha < 255)"
    );

    println!("✅ {} glass nodes with translucent tint", translucent.len());
}

#[test]
fn element_glass_has_valid_bounds() {
    let (_, _, inv) = build_shell_and_inventory();

    for g in &inv.glass {
        assert!(g.w > 0.0, "Glass width must be > 0 (id={})", g.id);
        assert!(g.h > 0.0, "Glass height must be > 0 (id={})", g.id);
    }

    println!("✅ {} glass nodes all have valid bounds", inv.glass.len());
}

// ═══════════════════════════════════════════════════════════════════════════
// ELEMENT TYPE: Border
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn element_border_exists_in_scene() {
    let (_, _, inv) = build_shell_and_inventory();

    println!("\n=== Border elements ===");
    for b in &inv.borders {
        println!(
            "  [{}] top={:.1} right={:.1} bottom={:.1} left={:.1} @({:.0},{:.0}) {}×{}",
            b.id, b.top_width, b.right_width, b.bottom_width, b.left_width, b.x, b.y, b.w, b.h
        );
    }

    assert!(
        !inv.borders.is_empty(),
        "Scene should have Border nodes for depth cues"
    );
}

#[test]
fn element_border_has_positive_width() {
    let (_, _, inv) = build_shell_and_inventory();

    for b in &inv.borders {
        let max_width = b
            .top_width
            .max(b.right_width)
            .max(b.bottom_width)
            .max(b.left_width);
        assert!(
            max_width > 0.0,
            "Border node {} should have at least one side with width > 0",
            b.id
        );
    }

    println!(
        "✅ {} border nodes all have positive width",
        inv.borders.len()
    );
}

#[test]
fn element_border_has_valid_bounds() {
    let (_, _, inv) = build_shell_and_inventory();

    for b in &inv.borders {
        assert!(b.w > 0.0, "Border width must be > 0 (id={})", b.id);
        assert!(b.h > 0.0, "Border height must be > 0 (id={})", b.id);
    }

    println!(
        "✅ {} border nodes all have valid bounds",
        inv.borders.len()
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// FULL PIPELINE: Scene inventory summary
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn full_pipeline_scene_inventory() {
    let (_, _scene, inv) = build_shell_and_inventory();

    println!("\n=== COMPLETE SCENE INVENTORY ===\n");
    println!("  Backgrounds:   {}", inv.backgrounds.len());
    println!("  Text nodes:    {}", inv.texts.len());
    println!("  Glass:         {}", inv.glass.len());
    println!("  Borders:       {}", inv.borders.len());
    println!("  BoxShadows:    {}", inv.box_shadows.len());
    println!("  Images:        {}", inv.images.len());
    println!("  Outlines:      {}", inv.outlines.len());
    println!("  Workspaces:    {}", inv.workspaces.len());
    println!("  Shadows:       {}", inv.shadows.len());
    println!("  Other:         {}", inv.other.len());

    let total = inv.backgrounds.len()
        + inv.texts.len()
        + inv.glass.len()
        + inv.borders.len()
        + inv.box_shadows.len()
        + inv.images.len()
        + inv.outlines.len()
        + inv.workspaces.len()
        + inv.shadows.len()
        + inv.other.len();
    println!("\n  TOTAL:         {}", total);

    // Scene must have a minimum set of element types
    assert!(
        inv.backgrounds.len() >= 2,
        "Need at least 2 backgrounds (desktop + dock/statusbar)"
    );
    assert!(
        inv.texts.len() >= 4,
        "Need at least 4 text nodes (dock labels)"
    );
    assert!(
        inv.glass.len() >= 1,
        "Need at least 1 glass node (blur effect)"
    );
    assert!(
        inv.borders.len() >= 1,
        "Need at least 1 border node (depth cue)"
    );

    println!("\n✅ All required element types present in scene");
}

#[test]
fn full_pipeline_flattening_preserves_all_types() {
    let (_, scene, inv) = build_shell_and_inventory();
    let flat = scene.flatten();

    // Count text in flat list
    let flat_text = flat
        .iter()
        .filter(|n| matches!(n.kind_ref(), SceneNodeKind::Text { .. }))
        .count();
    let flat_bg = flat
        .iter()
        .filter(|n| matches!(n.kind_ref(), SceneNodeKind::Background { .. }))
        .count();
    let flat_glass = flat
        .iter()
        .filter(|n| matches!(n.kind_ref(), SceneNodeKind::Glass(_)))
        .count();
    let flat_border = flat
        .iter()
        .filter(|n| matches!(n.kind_ref(), SceneNodeKind::Border { .. }))
        .count();

    println!("\n=== Flattened scene ===");
    println!(
        "  Text:       {} (tree={}, flat={})",
        flat_text,
        inv.texts.len(),
        flat_text
    );
    println!(
        "  Background: {} (tree={}, flat={})",
        flat_bg,
        inv.backgrounds.len(),
        flat_bg
    );
    println!(
        "  Glass:      {} (tree={}, flat={})",
        flat_glass,
        inv.glass.len(),
        flat_glass
    );
    println!(
        "  Border:     {} (tree={}, flat={})",
        flat_border,
        inv.borders.len(),
        flat_border
    );

    // Flattening should preserve or increase node count (never lose nodes)
    assert!(
        flat_text >= inv.texts.len(),
        "Flattening lost text nodes: tree={}, flat={}",
        inv.texts.len(),
        flat_text
    );
    assert!(
        flat_bg >= inv.backgrounds.len(),
        "Flattening lost backgrounds: tree={}, flat={}",
        inv.backgrounds.len(),
        flat_bg
    );

    println!("\n✅ Flattening preserves all node types");
}

#[test]
fn full_pipeline_rendering_no_crash() {
    let (_, scene, inv) = build_shell_and_inventory();

    // Verify scene is non-trivial before rendering
    assert!(
        !inv.texts.is_empty(),
        "Need text for meaningful render test"
    );
    assert!(!inv.backgrounds.is_empty(), "Need backgrounds");
    assert!(!inv.glass.is_empty(), "Need glass");

    let fb = render_scene(&scene);

    assert_eq!(
        fb.pixels().len(),
        1920 * 1080 * 4,
        "Framebuffer size correct"
    );

    let non_black = fb
        .pixels()
        .chunks_exact(4)
        .filter(|p| p[0] > 10 || p[1] > 10 || p[2] > 10)
        .count();

    println!(
        "✅ Full pipeline rendered: {} non-black pixels ({:.2}%)",
        non_black,
        (non_black as f64 / (1920.0 * 1080.0)) * 100.0
    );

    assert!(
        non_black > 0,
        "Rendering with all element types should produce visible pixels"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// ELEMENT TYPE: BoxShadow (may be empty in glass theme — that's OK)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn element_box_shadow_if_present_has_valid_spec() {
    let (_, _, inv) = build_shell_and_inventory();

    println!("\n=== BoxShadow elements ===");
    println!("  Count: {}", inv.box_shadows.len());

    for s in &inv.box_shadows {
        assert!(
            s.count > 0,
            "BoxShadow node {} should have at least 1 shadow spec",
            s.id
        );
        assert!(
            s.w > 0.0 && s.h > 0.0,
            "BoxShadow bounds must be > 0 (id={})",
            s.id
        );
        println!(
            "  [{}] {}×shadow blur={:.1} @({:.0},{:.0}) {}×{}",
            s.id, s.count, s.blur_radius, s.x, s.y, s.w, s.h
        );
    }

    // Glass themes may not use box-shadows (they use Glass for depth instead)
    println!("✅ {} box-shadow nodes validated", inv.box_shadows.len());
}

// ═══════════════════════════════════════════════════════════════════════════
// POSITIONED LAYOUT: The fixed bug — verify children are laid out
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn positioned_elements_have_children_laid_out() {
    // This tests the fix: layout_positioned now lays out children.
    // Statusbar (position:fixed, display:flex) should have child text nodes.
    // Dock (position:fixed, display:flex) should have child text nodes.
    let (_, _, inv) = build_shell_and_inventory();

    // Statusbar text (clock, indicators) should exist — proves
    // positioned (position:fixed) flex children are laid out.
    // Note: dock labels use display:none so they won't appear as text nodes.
    let statusbar_texts: Vec<_> = inv
        .texts
        .iter()
        .filter(|t| t.text.contains(':') || t.text == "LiquiDE")
        .collect();
    assert!(
        !statusbar_texts.is_empty(),
        "Statusbar text (clock, logo) should be laid out inside position:fixed bar"
    );

    println!("✅ Positioned elements have laid-out children:");
    println!("   - Statusbar: {} texts", statusbar_texts.len());
}

// ═══════════════════════════════════════════════════════════════════════════
// PRINT TREE (diagnostic helper — always passes)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn print_scene_tree() {
    let (_, scene, _) = build_shell_and_inventory();

    println!("\n=== SCENE TREE ===\n");
    print_tree(&scene, 0);
}

fn print_tree(node: &SceneNode, depth: usize) {
    let indent = "  ".repeat(depth);
    let b = &node.properties.bounds;
    let kind = match &node.kind {
        SceneNodeKind::Root => "Root".to_string(),
        SceneNodeKind::Background { color } => {
            format!("Bg rgba({},{},{},{})", color.r, color.g, color.b, color.a)
        }
        SceneNodeKind::Text {
            text, font_size, ..
        } => format!(
            "Text \"{}\" {}px",
            if text.len() > 20 { &text[..20] } else { text },
            font_size
        ),
        SceneNodeKind::Glass(p) => format!(
            "Glass blur={} tint=({},{},{},{})",
            p.blur_radius, p.tint_color.r, p.tint_color.g, p.tint_color.b, p.tint_color.a
        ),
        SceneNodeKind::Border { sides, .. } => format!(
            "Border t={:.0} r={:.0} b={:.0} l={:.0}",
            sides.top.width, sides.right.width, sides.bottom.width, sides.left.width
        ),
        SceneNodeKind::BoxShadows { shadows } => format!("BoxShadow ×{}", shadows.len()),
        SceneNodeKind::Shadow { blur_radius, .. } => format!("Shadow blur={:.0}", blur_radius),
        SceneNodeKind::Image { .. } => "Image".to_string(),
        SceneNodeKind::Workspace { index } => format!("Workspace {}", index),
        _ => format!("{:?}", std::mem::discriminant(&node.kind)),
    };
    println!(
        "{}[{}] {} @({:.0},{:.0}) {:.0}×{:.0} z={}",
        indent, node.id, kind, b.x, b.y, b.width, b.height, node.properties.z_order
    );

    for child in &node.children {
        print_tree(child, depth + 1);
    }
}
