//! Debug test to understand CSS query behavior

#[cfg(test)]
mod tests {
    use liquide_theme_css::{ThemeEngine, ThemeParser};
    use liquide_renderer_css::StyleResolver;
    use std::sync::Arc;

    #[test]
    fn debug_css_query() {
        let css = r#"
            dock {
                background: rgba(46, 52, 64, 225);
                border-color: rgb(76, 86, 106);
            }
        "#;

        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(css).unwrap();
        
        // Check what rules were parsed
        eprintln!("Stylesheet rules: {}", stylesheet.rule_count());
        
        let engine = ThemeEngine::new(stylesheet);
        
        // Try direct query
        let props = engine.query("dock", &[], &[]).unwrap();
        let prop_keys: Vec<_> = props.keys();
        eprintln!("Properties found: {}", prop_keys.len());
        eprintln!("Property names: {:?}", prop_keys);
        
        // Try to get specific properties
        if let Some(bg) = props.get("background") {
            eprintln!("background property found: {:?}", bg);
        } else {
            eprintln!("background property NOT found");
        }
        
        if let Some(bg) = props.get("background-color") {
            eprintln!("background-color property found: {:?}", bg);
        } else {
            eprintln!("background-color property NOT found");
        }
        
        // Now try StyleResolver
        let resolver = StyleResolver::from_arc(Arc::new(engine));
        let style = resolver.resolve("dock", &[], &[], None).unwrap();
        
        eprintln!("RenderStyle background_color: {:?}", style.background_color);
        eprintln!("RenderStyle border.color: {:?}", style.border.color);
    }
}
