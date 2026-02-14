//! Debug integration test to inspect DOM contents and pipeline processing.

use liquide_shell::Shell;

#[test]
fn debug_dom_contents() {
    let mut shell = Shell::new(1920.0, 1080.0);
    
    // Access the desktop DOM
    let doc = &shell.desktop_dom().doc;
    
    // Check if DOM has elements
    let root = doc.root();
    
    println!("=== DOM Structure ===");
    print_element(&doc, root, 0);
    
    fn print_element(
        doc: &liquide_dom::Document,
        element_id: liquide_dom::ElementId,
        indent: usize,
    ) {
        let indent_str = "  ".repeat(indent);
        let element = doc.get_element(element_id).unwrap();
        let tag = doc.get_tag_name(element_id).unwrap_or("unknown");
        let id_attr = doc.get_attribute(element_id, "id");
        let class_attr = doc.get_attribute(element_id, "class");
        
        println!(
            "{}<<{}> id={} class={}>",
            indent_str,
            tag,
            id_attr.unwrap_or(""),
            class_attr.unwrap_or("")
        );
        
        // Check text content
        if let Some(text) = doc.get_text_content(element_id) {
            if !text.is_empty() {
                println!("{}  text: \"{}\"", indent_str, text.trim());
            }
        }
        
        // Print children
        for child_id in element.children() {
            print_element(doc, *child_id, indent + 1);
        }
    }
}

#[test]
fn debug_pipeline_output() {
    let mut shell = Shell::new(1920.0, 1080.0);
    
    // Trigger DOM sync
    let scene = shell.build_scene();
    
    println!("=== Scene Structure ===");
    print_scene(&scene, 0);
    
    fn print_scene(node: &liquide_compositor::scene::SceneNode, indent: usize) {
        let indent_str = "  ".repeat(indent);
        
        println!(
            "{}[{:?}] id={} z={} bounds=({:.0},{:.0} {}x{})",
            indent_str,
            node.kind,
            node.id,
            node.properties.z_order,
            node.properties.bounds.x,
            node.properties.bounds.y,
            node.properties.bounds.width,
            node.properties.bounds.height,
        );
        
        for child in &node.children {
            print_scene(child, indent + 1);
        }
    }
}

#[test]
fn debug_css_pipeline() {
    let mut shell = Shell::new(1920.0, 1080.0);
    
    // Get the pipeline
    let doc = &shell.desktop_dom().doc;
    
    println!("=== CSS Pipeline Test ===");
    println!("Document has {} elements", doc.root().0);
    
    // Check if elements have styles
    let root = doc.root();
    check_styles(doc, root, 0);
    
    fn check_styles(
        doc: &liquide_dom::Document,
        element_id: liquide_dom::ElementId,
        indent: usize,
    ) {
        let indent_str = "  ".repeat(indent);
        let tag = doc.get_tag_name(element_id).unwrap_or("unknown");
        let id_attr = doc.get_attribute(element_id, "id").unwrap_or("");
        
        println!("{}<<{}> id=\"{}\">", indent_str, tag, id_attr);
        
        // Check if element has text content that should be rendered
        if let Some(text) = doc.get_text_content(element_id) {
            if !text.trim().is_empty() {
                println!("{}  📝 Has text content: \"{}\"", indent_str, text.trim());
            }
        }
        
        let element = doc.get_element(element_id).unwrap();
        for child_id in element.children() {
            check_styles(doc, *child_id, indent + 1);
        }
    }
}
