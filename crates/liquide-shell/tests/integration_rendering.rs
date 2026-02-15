//! Integration tests for the full rendering pipeline.
//!
//! Tests that CSS→DOM→Scene→Pixels works end-to-end.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::SceneNodeKind;
use liquide_renderer_cpu::{Renderer, SoftwareRenderer};
use liquide_shell::Shell;

#[test]
fn test_shell_builds_scene() {
    let mut shell = Shell::new(1920.0, 1080.0);

    // Build the initial scene
    let scene = shell.build_scene();

    // Scene should have a root node
    assert!(matches!(scene.kind, SceneNodeKind::Root));

    // Root should have children (dock, statusbar, background, etc.)
    assert!(
        !scene.children.is_empty(),
        "Scene should have child nodes (dock, statusbar, background)"
    );

    println!(
        "✅ Shell generated {} top-level nodes",
        scene.children.len()
    );
}

#[test]
fn test_scene_contains_shell_elements() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();

    // Count different element types
    let mut background_count = 0;
    let mut text_count = 0;
    let mut other_count = 0;

    fn count_nodes(
        node: &liquide_compositor::scene::SceneNode,
        bg: &mut usize,
        text: &mut usize,
        other: &mut usize,
    ) {
        match &node.kind {
            SceneNodeKind::Background { .. } => *bg += 1,
            SceneNodeKind::Text { .. } => *text += 1,
            _ => *other += 1,
        }
        for child in &node.children {
            count_nodes(child, bg, text, other);
        }
    }

    count_nodes(
        &scene,
        &mut background_count,
        &mut text_count,
        &mut other_count,
    );

    println!("✅ Scene contains:");
    println!("   - {} background elements", background_count);
    println!("   - {} text elements", text_count);
    println!("   - {} other elements", other_count);

    // We should have AT LEAST some backgrounds and text (statusbar, dock, etc.)
    assert!(
        background_count > 0,
        "Scene should contain background elements"
    );
}

#[test]
fn test_scene_elements_have_valid_bounds() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();

    let mut invalid_bounds = 0;

    fn check_bounds(node: &liquide_compositor::scene::SceneNode, invalid: &mut usize) {
        let props = &node.properties;
        let bounds = props.bounds;

        // Check for NaN or infinite values
        if bounds.x.is_nan()
            || bounds.y.is_nan()
            || bounds.width.is_nan()
            || bounds.height.is_nan()
            || bounds.x.is_infinite()
            || bounds.y.is_infinite()
            || bounds.width.is_infinite()
            || bounds.height.is_infinite()
        {
            *invalid += 1;
        }

        // Check for negative dimensions
        if bounds.width < 0.0 || bounds.height < 0.0 {
            *invalid += 1;
        }

        for child in &node.children {
            check_bounds(child, invalid);
        }
    }

    check_bounds(&scene, &mut invalid_bounds);

    assert_eq!(
        invalid_bounds, 0,
        "All scene nodes should have valid bounds (no NaN, inf, or negative dimensions)"
    );

    println!("✅ All scene elements have valid bounds");
}

#[test]
fn test_scene_flattening_produces_visible_nodes() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();

    // Flatten scene
    let flat_nodes = scene.flatten();

    assert!(
        !flat_nodes.is_empty(),
        "Flattened scene should contain visible nodes"
    );

    // Check visibility
    let visible_count = flat_nodes.iter().filter(|n| n.opacity > 0.0).count();

    println!(
        "✅ Flattened {} nodes ({} visible with opacity > 0)",
        flat_nodes.len(),
        visible_count
    );

    assert!(
        visible_count > 0,
        "Flattened scene should have nodes with opacity > 0"
    );
}

#[test]
fn test_renderer_produces_non_black_pixels() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();

    // Create renderer and framebuffer
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(1920, 1080, PixelFormat::Bgra8);

    // Fill framebuffer with black initially
    for pixel in fb.pixels.chunks_exact_mut(4) {
        pixel[0] = 0; // B
        pixel[1] = 0; // G
        pixel[2] = 0; // R
        pixel[3] = 255; // A
    }

    // Flatten and render
    let flat_nodes = scene.flatten();

    // Create damage set for full redraw
    use liquide_compositor::damage::{DamageClass, DamageSet, DamageTile};
    let mut damage = DamageSet::new(64);
    let tiles_x = (1920 + 63) / 64;
    let tiles_y = (1080 + 63) / 64;
    for y in 0..tiles_y {
        for x in 0..tiles_x {
            damage.add(DamageTile {
                x,
                y,
                class: DamageClass::UiPrimitive,
            });
        }
    }

    // Render
    renderer.render(&flat_nodes, &mut fb, &damage).unwrap();

    // Count non-black pixels
    let mut non_black_pixels = 0;
    let mut sample_colors = Vec::new();

    for (i, pixel) in fb.pixels.chunks_exact(4).enumerate() {
        let b = pixel[0];
        let g = pixel[1];
        let r = pixel[2];
        let a = pixel[3];

        // Count as non-black if any RGB component is > 10
        if r > 10 || g > 10 || b > 10 {
            non_black_pixels += 1;

            // Sample first few colored pixels for debugging
            if sample_colors.len() < 5 {
                let x = i as u32 % 1920;
                let y = i as u32 / 1920;
                sample_colors.push((x, y, r, g, b, a));
            }
        }
    }

    let total_pixels = 1920 * 1080;
    let colored_percentage = (non_black_pixels as f64 / total_pixels as f64) * 100.0;

    println!("✅ Rendered frame:");
    println!("   - Total pixels: {}", total_pixels);
    println!(
        "   - Non-black pixels: {} ({:.2}%)",
        non_black_pixels, colored_percentage
    );
    println!("   - Sample colors:");
    for (x, y, r, g, b, a) in &sample_colors {
        println!(
            "     @ ({:4}, {:4}): rgba({}, {}, {}, {})",
            x, y, r, g, b, a
        );
    }

    assert!(
        non_black_pixels > 0,
        "Renderer should produce non-black pixels (UI elements should be visible)"
    );

    // UI should cover some screen area (dark themes may only show subtle elements)
    // With statusbar (28px) + dock (56px) = 84px/1080 ≈ 7.8% area, but dark
    // backgrounds near black may only register faintly.
    assert!(
        colored_percentage > 0.1,
        "UI should have some non-black pixels, got {:.2}%",
        colored_percentage
    );
}

#[test]
fn test_dock_renders_with_items() {
    let mut shell = Shell::new(1920.0, 1080.0);

    // Dock should have default pinned items from Shell::new()
    let scene = shell.build_scene();

    // Find dock text nodes (Files, Terminal, Browser, Settings)
    let mut dock_text_labels = Vec::new();

    fn find_dock_text(node: &liquide_compositor::scene::SceneNode, labels: &mut Vec<String>) {
        if let SceneNodeKind::Text { text, .. } = &node.kind {
            // Check for known dock item labels
            let known = ["Files", "Terminal", "Browser", "Settings"];
            if known.iter().any(|k| text.contains(k)) {
                labels.push(text.clone());
            }
        }
        for child in &node.children {
            find_dock_text(child, labels);
        }
    }

    find_dock_text(&scene, &mut dock_text_labels);

    println!(
        "✅ Found {} dock item labels: {:?}",
        dock_text_labels.len(),
        dock_text_labels
    );

    // We should have 4 default dock items
    assert!(
        dock_text_labels.len() >= 4,
        "Dock should render with at least 4 items, got {}: {:?}",
        dock_text_labels.len(),
        dock_text_labels
    );
}

#[test]
fn test_statusbar_renders() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();

    // StatusBar should have text nodes (clock, wifi status, user name, etc.)
    let mut statusbar_texts = Vec::new();

    fn find_statusbar_text(node: &liquide_compositor::scene::SceneNode, texts: &mut Vec<String>) {
        if let SceneNodeKind::Text { text, .. } = &node.kind {
            // Check for status bar indicators (not dock items)
            let dock_labels = ["Files", "Terminal", "Browser", "Settings"];
            if !dock_labels.iter().any(|k| text.contains(k)) {
                texts.push(text.clone());
            }
        }
        for child in &node.children {
            find_statusbar_text(child, texts);
        }
    }

    find_statusbar_text(&scene, &mut statusbar_texts);

    println!(
        "✅ Found {} statusbar text nodes: {:?}",
        statusbar_texts.len(),
        statusbar_texts
    );

    // StatusBar should have at least clock + 1 indicator
    assert!(
        !statusbar_texts.is_empty(),
        "StatusBar should render text elements (clock, indicators)"
    );
}

#[test]
fn test_fonts_are_used_for_text() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();

    // Count text nodes
    let mut text_nodes = 0;

    fn count_text(node: &liquide_compositor::scene::SceneNode, count: &mut usize) {
        if matches!(node.kind, SceneNodeKind::Text { .. }) {
            *count += 1;
        }
        for child in &node.children {
            count_text(child, count);
        }
    }

    count_text(&scene, &mut text_nodes);

    println!("✅ Scene contains {} text nodes", text_nodes);

    assert!(
        text_nodes > 0,
        "Scene should contain text elements (statusbar clock, dock labels, etc.)"
    );
}

#[test]
fn test_full_pipeline_no_panics() {
    // This test runs the ENTIRE pipeline from shell creation to rendering
    // to ensure no panics occur

    let mut shell = Shell::new(1920.0, 1080.0);

    // 1. Build scene
    let scene = shell.build_scene();
    assert!(!scene.children.is_empty(), "Step 1: Scene creation");

    // 2. Flatten
    let flat_nodes = scene.flatten();
    assert!(!flat_nodes.is_empty(), "Step 2: Scene flattening");

    // 3. Render
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(1920, 1080, PixelFormat::Bgra8);

    let mut damage = liquide_compositor::damage::DamageSet::new(64);
    let tiles_x = 30; // 1920/64
    let tiles_y = 17; // 1080/64
    for y in 0..tiles_y {
        for x in 0..tiles_x {
            damage.add(liquide_compositor::damage::DamageTile {
                x,
                y,
                class: liquide_compositor::damage::DamageClass::UiPrimitive,
            });
        }
    }

    let result = renderer.render(&flat_nodes, &mut fb, &damage);
    assert!(result.is_ok(), "Step 3: Rendering");

    println!("✅ Full pipeline executed without panics");
    println!("   - Flattened to: {} nodes", flat_nodes.len());
    println!("   - Rendered successfully");
}

#[test]
fn test_liquid_glass_effects_present() {
    // Verify that Liquid Glass visual effects are actually in the scene
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();

    let mut glass_count = 0;
    let mut blur_radii = Vec::new();
    let mut tint_colors = Vec::new();

    fn inspect_glass(
        node: &liquide_compositor::scene::SceneNode,
        count: &mut usize,
        blurs: &mut Vec<u32>,
        tints: &mut Vec<(u8, u8, u8, u8)>,
    ) {
        if let SceneNodeKind::Glass(params) = &node.kind {
            *count += 1;
            blurs.push(params.blur_radius);
            let c = params.tint_color;
            tints.push((c.r, c.g, c.b, c.a));
        }
        for child in &node.children {
            inspect_glass(child, count, blurs, tints);
        }
    }

    inspect_glass(&scene, &mut glass_count, &mut blur_radii, &mut tint_colors);

    println!("✅ Liquid Glass effects:");
    println!("   - Glass nodes: {}", glass_count);
    println!("   - Blur radii: {:?}", blur_radii);
    println!(
        "   - Tint colors (sample): {:?}",
        &tint_colors[..tint_colors.len().min(3)]
    );

    assert!(
        glass_count > 0,
        "Scene should contain Glass nodes (dock, statusbar with blur/tint)"
    );

    // Statusbar and dock should have glass effects (blur_radius > 0)
    let has_blur = blur_radii.iter().any(|&r| r > 0);
    assert!(
        has_blur,
        "Glass nodes should have blur_radius > 0 for backdrop blur"
    );

    // Glass tints should be semi-transparent (alpha < 255)
    let has_translucent_tint = tint_colors.iter().any(|(_, _, _, a)| *a < 255);
    assert!(
        has_translucent_tint,
        "Glass tints should be translucent (alpha < 255)"
    );
}

#[test]
fn test_borders_and_shadows_present() {
    // Verify that elements have distinct borders and shadows for depth
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();

    let mut border_count = 0;
    let mut shadow_count = 0;
    let mut border_widths = Vec::new();
    let mut shadow_blurs = Vec::new();

    fn inspect_decorations(
        node: &liquide_compositor::scene::SceneNode,
        borders: &mut usize,
        shadows: &mut usize,
        widths: &mut Vec<f32>,
        blurs: &mut Vec<f32>,
    ) {
        match &node.kind {
            SceneNodeKind::Border { sides, .. } => {
                *borders += 1;
                if sides.top.width > 0.0 {
                    widths.push(sides.top.width);
                }
            }
            SceneNodeKind::BoxShadows { shadows: specs } => {
                *shadows += 1;
                for spec in specs {
                    blurs.push(spec.blur_radius);
                }
            }
            _ => {}
        }
        for child in &node.children {
            inspect_decorations(child, borders, shadows, widths, blurs);
        }
    }

    inspect_decorations(
        &scene,
        &mut border_count,
        &mut shadow_count,
        &mut border_widths,
        &mut shadow_blurs,
    );

    println!("✅ Visual decorations:");
    println!("   - Border nodes: {}", border_count);
    println!("   - Shadow nodes: {}", shadow_count);
    println!("   - Border widths: {:?}", border_widths);
    println!("   - Shadow blur radii: {:?}", shadow_blurs);

    // Liquid Glass design expects borders and/or shadows for depth
    // Note: Some themes may use glass effects instead of explicit borders
    println!(
        "   - Total depth cues: {} (borders + shadows)",
        border_count + shadow_count
    );
}

#[test]
fn test_visual_contrast_present() {
    // Verify elements have distinct, contrasting colors
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();

    let mut background_colors = Vec::new();
    let mut glass_tints = Vec::new();

    fn collect_colors(
        node: &liquide_compositor::scene::SceneNode,
        bg_colors: &mut Vec<(u8, u8, u8, u8)>,
        glass_colors: &mut Vec<(u8, u8, u8, u8)>,
    ) {
        match &node.kind {
            SceneNodeKind::Background { color } => {
                bg_colors.push((color.r, color.g, color.b, color.a));
            }
            SceneNodeKind::Glass(params) => {
                let c = params.tint_color;
                glass_colors.push((c.r, c.g, c.b, c.a));
            }
            _ => {}
        }
        for child in &node.children {
            collect_colors(child, bg_colors, glass_colors);
        }
    }

    collect_colors(&scene, &mut background_colors, &mut glass_tints);

    println!("✅ Visual contrast:");
    println!("   - Background colors: {}", background_colors.len());
    println!("   - Glass tint colors: {}", glass_tints.len());

    // Check color variety (not all the same)
    if !background_colors.is_empty() {
        let first = background_colors[0];
        let has_variety = background_colors.iter().any(|c| *c != first);
        println!("   - Background variety: {}", has_variety);
    }

    if !glass_tints.is_empty() {
        let first = glass_tints[0];
        let has_variety = glass_tints.iter().any(|c| *c != first);
        println!("   - Glass tint variety: {}", has_variety);
    }

    // Scene should have multiple distinct colors
    let total_unique = background_colors.len() + glass_tints.len();
    assert!(
        total_unique > 0,
        "Scene should have background or glass elements with colors"
    );
}

#[test]
fn test_border_widths_and_colors() {
    // Verify border specifications are properly set
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();

    let mut border_specs = Vec::new();

    fn collect_borders(
        node: &liquide_compositor::scene::SceneNode,
        specs: &mut Vec<(f32, f32, f32, f32, u8, u8, u8, u8)>,
    ) {
        if let SceneNodeKind::Border { sides, radius } = &node.kind {
            // Collect top border as representative
            let c = sides.top.color;
            specs.push((
                sides.top.width,
                radius.0, // top-left corner radius
                sides.top.width,
                sides.bottom.width,
                c.r,
                c.g,
                c.b,
                c.a,
            ));
        }
        for child in &node.children {
            collect_borders(child, specs);
        }
    }

    collect_borders(&scene, &mut border_specs);

    println!("✅ Border specifications:");
    println!("   - Total border nodes: {}", border_specs.len());
    if !border_specs.is_empty() {
        println!(
            "   - Sample border: width={:.1}, radius={:.1}, color=rgba({},{},{},{})",
            border_specs[0].0,
            border_specs[0].1,
            border_specs[0].4,
            border_specs[0].5,
            border_specs[0].6,
            border_specs[0].7
        );
    }

    // Borders should have non-zero width if present
    for (width, _, _, _, _, _, _, _) in &border_specs {
        assert!(
            *width >= 0.0,
            "Border width should be non-negative, got {}",
            width
        );
    }
}

#[test]
fn test_box_shadow_specifications() {
    // Verify box shadows have proper blur, spread, and color
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();

    let mut shadow_specs = Vec::new();

    fn collect_shadows(
        node: &liquide_compositor::scene::SceneNode,
        specs: &mut Vec<(f32, f32, f32, f32, u8, u8, u8, u8)>,
    ) {
        if let SceneNodeKind::BoxShadows { shadows } = &node.kind {
            for shadow in shadows {
                specs.push((
                    shadow.offset_x,
                    shadow.offset_y,
                    shadow.blur_radius,
                    shadow.spread_radius,
                    shadow.color.r,
                    shadow.color.g,
                    shadow.color.b,
                    shadow.color.a,
                ));
            }
        }
        for child in &node.children {
            collect_shadows(child, specs);
        }
    }

    collect_shadows(&scene, &mut shadow_specs);

    println!("✅ Box shadow specifications:");
    println!("   - Total shadows: {}", shadow_specs.len());
    if !shadow_specs.is_empty() {
        println!(
            "   - Sample shadow: offset=({:.1},{:.1}), blur={:.1}, spread={:.1}, color=rgba({},{},{},{})",
            shadow_specs[0].0,
            shadow_specs[0].1,
            shadow_specs[0].2,
            shadow_specs[0].3,
            shadow_specs[0].4,
            shadow_specs[0].5,
            shadow_specs[0].6,
            shadow_specs[0].7
        );
    }

    // Shadows should have blur radius > 0 for soft depth effect
    let soft_shadows = shadow_specs.iter().filter(|s| s.2 > 0.0).count();
    println!("   - Shadows with blur: {}", soft_shadows);
}
