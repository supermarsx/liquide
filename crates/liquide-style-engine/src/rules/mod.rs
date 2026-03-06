//! Element Structure Rule Engine
//!
//! Validates that DOM elements have the required CSS properties and container
//! hierarchy to render correctly. This catches missing styles early in
//! development rather than at runtime.
//!
//! # Example
//!
//! ```ignore
//! use liquide_style_engine::rules::{RuleEngine, BuiltinRules};
//!
//! let engine = RuleEngine::with_builtin_rules();
//! let violations = engine.validate(&document, &style_map);
//!
//! for v in &violations {
//!     eprintln!("[{}] {}: {}", v.severity, v.element, v.message);
//! }
//! ```

mod builtin;
mod element_rule;
mod engine;
mod report;
mod types;

pub use element_rule::ElementRule;
pub use engine::RuleEngine;
pub use report::ValidationReport;
pub use types::{PropertyRequirement, Severity, StructureRequirement, Violation};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computed::{
        AlignItems, ComputedStyle, Display, JustifyContent, Position,
    };
    use crate::dimension::Dimension;
    use liquide_dom::Document;

    fn create_test_doc() -> Document {
        let mut doc = Document::new();
        let root = doc.root();

        // Create a dock element
        let dock = doc.create_element("dock");
        doc.set_attribute(dock, "id", "shell-dock");
        doc.append_child(root, dock);

        // Create dock items
        let item1 = doc.create_element("dock-item");
        doc.set_attribute(item1, "data-app-id", "files");
        doc.append_child(dock, item1);

        let item2 = doc.create_element("dock-item");
        // Missing data-app-id attribute intentionally
        doc.append_child(dock, item2);

        // Create a context menu
        let menu = doc.create_element("context-menu");
        doc.append_child(root, menu);

        doc
    }

    fn create_test_styles(doc: &Document) -> crate::style_map::StyleMap {
        let mut styles = crate::style_map::StyleMap::new();

        // Collect all node IDs
        let root = doc.root();
        let mut all_nodes = vec![root];
        all_nodes.extend(doc.descendants(root));

        for node_id in all_nodes {
            if let Some(node) = doc.get(node_id) {
                if !node.is_element() {
                    continue;
                }

                let tag = node.tag_name();
                let mut style = ComputedStyle::default();

                match tag.as_str() {
                    "dock" => {
                        style.display = Display::Flex;
                        style.position = Position::Fixed;
                        style.width = Dimension::Percent(100.0);
                        style.height = Dimension::Px(56.0);
                        style.justify_content = JustifyContent::Center;
                        style.align_items = AlignItems::Center;
                        style.gap.width = Dimension::Px(4.0);
                        style.background_color.a = 180;
                    }
                    "dock-item" => {
                        style.display = Display::Flex;
                        style.width = Dimension::Px(44.0);
                        style.height = Dimension::Px(44.0);
                        style.border_radius.top_left = 12.0;
                        style.align_items = AlignItems::Center;
                        style.justify_content = JustifyContent::Center;
                    }
                    "context-menu" => {
                        // Missing required properties to trigger violations
                        style.display = Display::Block; // Should be Flex
                        style.position = Position::Static; // Should be Fixed
                    }
                    _ => {}
                }

                styles.insert(node_id, style);
            }
        }

        styles
    }

    #[test]
    fn test_rule_engine_creation() {
        let engine = RuleEngine::with_builtin_rules();
        assert!(engine.get_rule("dock").is_some());
        assert!(engine.get_rule("statusbar").is_some());
        assert!(engine.get_rule("context-menu").is_some());
        assert!(engine.get_rule("nonexistent").is_none());
    }

    #[test]
    fn test_dock_validation_passes() {
        let engine = RuleEngine::with_builtin_rules();
        let doc = create_test_doc();
        let styles = create_test_styles(&doc);

        let report = engine.validate_with_report(&doc, &styles);

        // Dock should pass, context-menu should fail
        let dock_violations: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.element == "dock")
            .collect();

        // Dock should have no critical/error violations for display/position
        let dock_critical: Vec<_> = dock_violations
            .iter()
            .filter(|v| v.severity == Severity::Critical)
            .collect();
        assert!(dock_critical.is_empty(), "Dock should have no critical violations");
    }

    #[test]
    fn test_context_menu_violations() {
        let engine = RuleEngine::with_builtin_rules();
        let doc = create_test_doc();
        let styles = create_test_styles(&doc);

        let report = engine.validate_with_report(&doc, &styles);

        // Context menu should have violations
        let menu_violations: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.element == "context-menu")
            .collect();

        // Should have display violation (Block instead of Flex)
        assert!(
            menu_violations.iter().any(|v| v.rule_name == "display"),
            "Should detect display violation"
        );

        // Should have position violation (Static instead of Fixed)
        assert!(
            menu_violations.iter().any(|v| v.rule_name == "position"),
            "Should detect position violation"
        );
    }

    #[test]
    fn test_missing_attribute_violation() {
        let engine = RuleEngine::with_builtin_rules();
        let doc = create_test_doc();
        let styles = create_test_styles(&doc);

        let report = engine.validate_with_report(&doc, &styles);

        // One dock-item is missing data-app-id
        let attr_violations: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.element == "dock-item" && v.rule_name.contains("attr-data-app-id"))
            .collect();

        assert_eq!(attr_violations.len(), 1, "Should detect one missing data-app-id");
    }

    #[test]
    fn test_validation_report() {
        let engine = RuleEngine::with_builtin_rules();
        let doc = create_test_doc();
        let styles = create_test_styles(&doc);

        let report = engine.validate_with_report(&doc, &styles);

        // Should not be valid (context-menu has critical violations)
        assert!(!report.is_valid(), "Report should not be valid");
        assert!(report.critical_count > 0, "Should have critical violations");

        // Report should be printable
        let report_str = report.to_string_report();
        assert!(report_str.contains("Validation Report"));
        assert!(report_str.contains("context-menu"));
    }

    #[test]
    fn test_custom_rule() {
        let mut engine = RuleEngine::new();

        engine.add_rule(
            ElementRule::new("custom-element")
                .description("Custom test element")
                .display_one_of(&[Display::Flex], Severity::Error),
        );

        assert!(engine.get_rule("custom-element").is_some());
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::Error);
        assert!(Severity::Error > Severity::Warning);
    }
}
