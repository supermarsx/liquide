//! Stage-by-stage pipeline diagnosis through scene inspection.
//!
//! Since pipeline internals are private, we diagnose failures by examining
//! the final scene output in detail, looking for evidence of each stage.

use liquide_shell::Shell;
use liquide_compositor::scene::{SceneNode, SceneNodeKind};

// ═══════════════════════════════════════════════════════════════████═══════════
// COMPREHENSIVE SCENE INSPECTION
// ══════════════════════════════════════════════════════════════════════════

#[test]
fn inspect_scene_text_nodes() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();
    
    println!("\n=== DETAILED SCENE TEXT NODE INSPECTION ===\n");
    
    let mut text_nodes = Vec::new();
    collect_text_nodes(&scene, &mut text_nodes, "");
    
    println!("Total text nodes in scene: {}\n", text_nodes.len());
    
    if text_nodes.is_empty() {
        println!("❌ NO TEXT NODES FOUND IN SCENE!");
        println!("\nPossible causes:");
        println!("  1. Components don't create DOM text nodes (Stage 1 fail)");
        println!("  2. CSS hides all text (display:none, visibility:hidden)");
        println!("  3. Layout doesn't position text (width/height = 0)");
        println!("  4. Paint doesn't emit DisplayItem::Text (Stage 4 fail)");
        println!("  5. Scene builder doesn't convert to SceneNode::Text (Stage 5 fail)");
    } else {
        println!("✅ Text nodes present:");
        for (i, (path, text, size, bounds)) in text_nodes.iter().enumerate() {
            println!(
                "[{}] \"{text}\" ({}px) at [{path}]",
                i, size
            );
            println!("     bounds: ({:.0},{:.0}) {}×{}",
                bounds.0, bounds.1, bounds.2, bounds.3);
        }
    }
    
    // This test documents what we find, doesn't assert
}

fn collect_text_nodes(
    node: &SceneNode,
    result: &mut Vec<(String, String, f32, (f32, f32, f32, f32))>,
    path: &str,
) {
    let node_path = format!("{}/node_{}", path, node.id);
    
    if let SceneNodeKind::Text { text, font_size, .. } = &node.kind {
        let bounds = node.properties.bounds;
        result.push((
            node_path.clone(),
            text.clone(),
            *font_size,
            (bounds.x, bounds.y, bounds.width, bounds.height),
        ));
    }
    
    for (i, child) in node.children.iter().enumerate() {
        collect_text_nodes(child, result, &format!("{}/{}", node_path, i));
    }
}

#[test]
fn inspect_scene_structure_detailed() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();
    
    println!("\n=== DETAILED SCENE STRUCTURE ===\n");
    
    let mut counts = NodeCounts::default();
    count_node_types(&scene, &mut counts);
    
    println!("Node type distribution:");
    println!("  Root:         {}", counts.root);
    println!("  Background:   {} (Stage 1 ✅ if > 0)", counts.background);
    println!("  Text:         {} (Stage 1-5, FAILS if 0)", counts.text);
    println!("  Glass:        {}", counts.glass);
    println!("  Border:       {}", counts.border);
    println!("  BoxShadows:   {}", counts.box_shadows);
    println!("  Image:        {}", counts.image);
    println!("  Outline:      {}", counts.outline);
    println!("  Other:        {}", counts.other);
    println!("\nTotal nodes: {}", counts.total());
    
    println!("\n=== NODE TREE ===\n");
    print_tree(&scene, 0);
    
    println!("\n===DIAGNOSIS ===");
    if counts.background > 0 {
        println!("✅ Stage 1-4: Backgrounds rendered (CSS→Layout→Paint working)");
    }
    if counts.glass > 0 || counts.border > 0 {
        println!("✅ Visual properties: Glass effects and borders working");
    }
    if counts.text == 0 {
        println!("❌ TEXT PIPELINE BROKEN:");
        println!("   Either DOM has no text, or text doesn't survive CSS/Layout/Paint");
    } else {
        println!("✅ Text pipeline working ({} text nodes)", counts.text);
    }
}

#[derive(Default)]
struct NodeCounts {
    root: usize,
    background: usize,
    text: usize,
    glass: usize,
    border: usize,
    box_shadows: usize,
    image: usize,
    outline: usize,
    other: usize,
}

impl NodeCounts {
    fn total(&self) -> usize {
        self.root + self.background + self.text + self.glass + 
        self.border + self.box_shadows + self.image + self.outline + self.other
    }
}

fn count_node_types(node: &SceneNode, counts: &mut NodeCounts) {
    match &node.kind {
        SceneNodeKind::Root => counts.root += 1,
        SceneNodeKind::Background { .. } => counts.background += 1,
        SceneNodeKind::Text { .. } => counts.text += 1,
        SceneNodeKind::Glass(_) => counts.glass += 1,
        SceneNodeKind::Border { .. } => counts.border += 1,
        SceneNodeKind::BoxShadows { .. } => counts.box_shadows += 1,
        SceneNodeKind::Image { .. } => counts.image += 1,
        SceneNodeKind::Outline { .. } => counts.outline += 1,
        _ => counts.other += 1,
    }
    
    for child in &node.children {
        count_node_types(child, counts);
    }
}

fn print_tree(node: &SceneNode, depth: usize) {
    let indent = "  ".repeat(depth);
    let kind_str = match &node.kind {
        SceneNodeKind::Root => "Root".to_string(),
        SceneNodeKind::Background { color, .. } => {
            format!("Background rgba({},{},{},{})", 
                color.r, color.g, color.b, color.a)
        }
        SceneNodeKind::Text { text, font_size, .. } => {
            format!("Text \"{}\" ({}px)", 
                if text.len() > 30 { &text[..30] } else { text },
                font_size)
        }
        SceneNodeKind::Glass(params) => {
            format!("Glass blur={} tint=rgba({},{},{},{})",
                params.blur_radius,
                params.tint_color.r,
                params.tint_color.g,
                params.tint_color.b,
                params.tint_color.a)
        }
        SceneNodeKind::Border { sides, .. } => {
            format!("Border (top={}, right={}, bottom={}, left={})",
                sides.top.width, sides.right.width, sides.bottom.width, sides.left.width)
        }
        _ => format!("{:?}", node.kind),
    };
    
    let bounds = &node.properties.bounds;
    println!(
        "{}[{}] {} @({:.0},{:.0}) {}×{} z={}",
        indent,
        node.id,
        kind_str,
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        node.properties.z_order
    );
    
    for child in &node.children {
        print_tree(child, depth + 1);
    }
}

#[test]
fn inspect_shell_components() {
    // Test what components are supposed to generate
    let mut shell = Shell::new(1920.0, 1080.0);
    let scene = shell.build_scene();
    
    println!("\n=== EXPECTED COMPONENTS ===\n");
    println!("StatusBar should have:");
    println!("  - Clock text (e.g., '12:30 PM')");
    println!("  - WiFi, Battery, Volume status");
    println!();
    println!("Dock should have:");
    println!("  - App labels (Finder, Safari, etc.)");
    println!("  - At least 3-4 dock items");
    println!();
    println!("Background should have:");
    println!("  - Liquid glass panels");
    println!("  - Colored backgrounds");
    
    let mut text_count = 0;
    fn count_text(node: &SceneNode, count: &mut usize) {
        if matches!(node.kind, SceneNodeKind::Text { .. }) {
            *count += 1;
        }
        for child in &node.children {
            count_text(child, count);
        }
    }
    count_text(&scene, &mut text_count);
    
    println!("\n=== ACTUAL SCENE CONTENT ===\n");
    println!("Text nodes found: {}", text_count);
    println!("Expected: 5+ (clock + dock labels)");
    
    if text_count == 0 {
        println!("\n❌ CRITICAL: Components are NOT generating visible text!");
        println!("   Root cause is in component template generation.");
    } else {
        println!("\n✅ Components generating text successfully");
    }
}
