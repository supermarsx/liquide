//! Rule engine: validation of DOM elements against CSS property and structure rules.

use std::collections::HashMap;

use liquide_dom::{Document, NodeId};

use crate::computed::{ComputedStyle, Display};
use crate::style_map::StyleMap;

use super::element_rule::ElementRule;
use super::report::ValidationReport;
use super::types::{PropertyRequirement, Severity, StructureRequirement, Violation};

// ═══════════════════════════════════════════════════════════════════════════
// Rule Engine
// ═══════════════════════════════════════════════════════════════════════════

/// The rule validation engine.
pub struct RuleEngine {
    /// Rules indexed by tag name.
    rules: HashMap<String, ElementRule>,
}

impl RuleEngine {
    /// Create an empty rule engine.
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }

    /// Add a rule to the engine.
    pub fn add_rule(&mut self, rule: ElementRule) {
        self.rules.insert(rule.tag.clone(), rule);
    }

    /// Get a rule by tag name.
    pub fn get_rule(&self, tag: &str) -> Option<&ElementRule> {
        self.rules.get(tag)
    }

    /// Validate a document against all rules.
    ///
    /// Returns a list of violations found.
    pub fn validate(&self, doc: &Document, styles: &StyleMap) -> Vec<Violation> {
        let mut violations = Vec::new();

        // Collect all node IDs (root + descendants)
        let root = doc.root();
        let mut all_nodes = vec![root];
        all_nodes.extend(doc.descendants(root));

        // Iterate through all nodes in the document
        for node_id in all_nodes {
            if let Some(node) = doc.get(node_id) {
                if !node.is_element() {
                    continue;
                }

                let tag = node.tag_name();

                // Check if we have a rule for this tag
                if let Some(rule) = self.rules.get(&tag) {
                    // Get computed style for this node
                    let style = styles.get(node_id);

                    // Check property requirements against style
                    if let Some(style) = style {
                        self.check_property_requirements(
                            node_id,
                            &tag,
                            rule,
                            style,
                            &mut violations,
                        );
                    } else {
                        // No style computed at all - critical issue
                        violations.push(Violation {
                            element: tag.to_string(),
                            node_id,
                            rule_name: "style-computed".to_string(),
                            message: format!("Element '{}' has no computed style", tag),
                            severity: Severity::Critical,
                            suggestion: Some(
                                "Ensure element is included in style computation".to_string(),
                            ),
                        });
                    }

                    // Check structure requirements against DOM
                    self.check_structure_requirements(doc, node_id, &tag, rule, &mut violations);
                }
            }
        }

        // Sort by severity (critical first)
        violations.sort_by(|a, b| b.severity.cmp(&a.severity));
        violations
    }

    /// Check property requirements for an element.
    fn check_property_requirements(
        &self,
        node_id: NodeId,
        tag: &str,
        rule: &ElementRule,
        style: &ComputedStyle,
        violations: &mut Vec<Violation>,
    ) {
        use crate::dimension::Dimension;

        for (req, severity) in &rule.property_requirements {
            let violated = match req {
                PropertyRequirement::DisplayOneOf(allowed) => {
                    if !allowed.contains(&style.display) {
                        Some((
                            "display".to_string(),
                            format!(
                                "display is {:?}, expected one of {:?}",
                                style.display, allowed
                            ),
                            Some(format!("Set display to {:?}", allowed[0])),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::PositionOneOf(allowed) => {
                    if !allowed.contains(&style.position) {
                        Some((
                            "position".to_string(),
                            format!(
                                "position is {:?}, expected one of {:?}",
                                style.position, allowed
                            ),
                            Some(format!("Set position to {:?}", allowed[0])),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::FlexDirectionOneOf(allowed) => {
                    if style.display == Display::Flex || style.display == Display::InlineFlex {
                        if !allowed.contains(&style.flex_direction) {
                            Some((
                                "flex-direction".to_string(),
                                format!(
                                    "flex-direction is {:?}, expected one of {:?}",
                                    style.flex_direction, allowed
                                ),
                                Some(format!("Set flex-direction to {:?}", allowed[0])),
                            ))
                        } else {
                            None
                        }
                    } else {
                        None // Only applies to flex containers
                    }
                }

                PropertyRequirement::JustifyContentOneOf(allowed) => {
                    if style.display == Display::Flex || style.display == Display::InlineFlex {
                        if !allowed.contains(&style.justify_content) {
                            Some((
                                "justify-content".to_string(),
                                format!(
                                    "justify-content is {:?}, expected one of {:?}",
                                    style.justify_content, allowed
                                ),
                                Some(format!("Set justify-content to {:?}", allowed[0])),
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }

                PropertyRequirement::AlignItemsOneOf(allowed) => {
                    if style.display == Display::Flex || style.display == Display::InlineFlex {
                        if !allowed.contains(&style.align_items) {
                            Some((
                                "align-items".to_string(),
                                format!(
                                    "align-items is {:?}, expected one of {:?}",
                                    style.align_items, allowed
                                ),
                                Some(format!("Set align-items to {:?}", allowed[0])),
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }

                PropertyRequirement::WidthDefined => {
                    if matches!(style.width, Dimension::Auto) {
                        Some((
                            "width".to_string(),
                            "width is auto, should be explicitly defined".to_string(),
                            Some("Set width to a specific value or 100%".to_string()),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::HeightDefined => {
                    if matches!(style.height, Dimension::Auto) {
                        Some((
                            "height".to_string(),
                            "height is auto, should be explicitly defined".to_string(),
                            Some("Set height to a specific value".to_string()),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::MinWidthDefined => {
                    if matches!(style.min_width, Dimension::Auto | Dimension::Zero) {
                        Some((
                            "min-width".to_string(),
                            "min-width should be defined".to_string(),
                            Some("Set min-width to ensure minimum size".to_string()),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::MaxWidthDefined => {
                    if matches!(style.max_width, Dimension::None) {
                        Some((
                            "max-width".to_string(),
                            "max-width should be defined".to_string(),
                            Some("Set max-width to constrain size".to_string()),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::ZIndexDefined => {
                    if style.z_index.is_none() {
                        Some((
                            "z-index".to_string(),
                            "z-index should be defined for layered elements".to_string(),
                            Some("Set z-index to control stacking order".to_string()),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::BorderRadiusPositive => {
                    let has_radius = style.border_radius.top_left > 0.0
                        || style.border_radius.top_right > 0.0
                        || style.border_radius.bottom_left > 0.0
                        || style.border_radius.bottom_right > 0.0;
                    if !has_radius {
                        Some((
                            "border-radius".to_string(),
                            "border-radius should be > 0 for rounded corners".to_string(),
                            Some("Set border-radius for visual polish".to_string()),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::BackgroundVisible => {
                    if style.background_color.a == 0 && style.background.is_none() {
                        Some((
                            "background".to_string(),
                            "background is fully transparent".to_string(),
                            Some(
                                "Set background-color with alpha > 0 or add background".to_string(),
                            ),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::BlurRadiusPositive => {
                    if style.x_blur_radius <= 0.0 {
                        Some((
                            "blur-radius".to_string(),
                            "blur-radius should be > 0 for glass effects".to_string(),
                            Some("Set blur-radius for frosted glass effect".to_string()),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::GapPositive => {
                    let gap_x = match style.gap.width {
                        Dimension::Px(v) => v,
                        _ => 0.0,
                    };
                    let gap_y = match style.gap.height {
                        Dimension::Px(v) => v,
                        _ => 0.0,
                    };
                    if gap_x <= 0.0 && gap_y <= 0.0 {
                        Some((
                            "gap".to_string(),
                            "gap should be > 0 for flex/grid spacing".to_string(),
                            Some("Set gap to add spacing between children".to_string()),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::PaddingDefined => {
                    let has_padding = !matches!(style.padding.top, Dimension::Zero)
                        || !matches!(style.padding.right, Dimension::Zero)
                        || !matches!(style.padding.bottom, Dimension::Zero)
                        || !matches!(style.padding.left, Dimension::Zero);
                    if !has_padding {
                        Some((
                            "padding".to_string(),
                            "padding should be defined for content spacing".to_string(),
                            Some("Set padding for internal spacing".to_string()),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::FontSizeInRange { min, max } => {
                    if style.font_size < *min || style.font_size > *max {
                        Some((
                            "font-size".to_string(),
                            format!(
                                "font-size {} is outside range {}–{}",
                                style.font_size, min, max
                            ),
                            Some(format!("Set font-size between {} and {}", min, max)),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::ColorVisible => {
                    if style.color.a == 0 {
                        Some((
                            "color".to_string(),
                            "text color is fully transparent".to_string(),
                            Some("Set color with alpha > 0".to_string()),
                        ))
                    } else {
                        None
                    }
                }

                PropertyRequirement::Custom {
                    name,
                    check,
                    message,
                } => {
                    if !check(style) {
                        Some((name.clone(), message.clone(), None))
                    } else {
                        None
                    }
                }
            };

            if let Some((prop, msg, suggestion)) = violated {
                violations.push(Violation {
                    element: tag.to_string(),
                    node_id,
                    rule_name: prop,
                    message: msg,
                    severity: *severity,
                    suggestion,
                });
            }
        }
    }

    /// Check structure requirements for an element.
    fn check_structure_requirements(
        &self,
        doc: &Document,
        node_id: NodeId,
        tag: &str,
        rule: &ElementRule,
        violations: &mut Vec<Violation>,
    ) {
        let node = match doc.get(node_id) {
            Some(n) => n,
            None => return,
        };

        for (req, severity) in &rule.structure_requirements {
            let violated = match req {
                StructureRequirement::ParentTagOneOf(allowed_parents) => {
                    if let Some(parent_id) = doc.parent(node_id) {
                        if let Some(parent) = doc.get(parent_id) {
                            let parent_tag = parent.tag_name();
                            if !allowed_parents.iter().any(|t| t == &parent_tag) {
                                Some((
                                    "parent-tag".to_string(),
                                    format!(
                                        "parent tag is '{}', expected one of {:?}",
                                        parent_tag, allowed_parents
                                    ),
                                    Some(format!(
                                        "Move element inside {}",
                                        allowed_parents.join(" or ")
                                    )),
                                ))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        // No parent (root element) - may be OK for some elements
                        None
                    }
                }

                StructureRequirement::MustContainChild(child_tag) => {
                    let has_child = doc
                        .children(node_id)
                        .iter()
                        .filter_map(|&cid| doc.get(cid))
                        .any(|c| c.tag_name() == *child_tag);
                    if !has_child {
                        Some((
                            "required-child".to_string(),
                            format!("element should contain a <{}> child", child_tag),
                            Some(format!("Add <{}> as child element", child_tag)),
                        ))
                    } else {
                        None
                    }
                }

                StructureRequirement::RequiredChildren(children) => {
                    let child_tags: Vec<_> = doc
                        .children(node_id)
                        .iter()
                        .filter_map(|&cid| doc.get(cid))
                        .map(|c| c.tag_name())
                        .collect();

                    let missing: Vec<_> = children
                        .iter()
                        .filter(|c| !child_tags.contains(c))
                        .collect();

                    if !missing.is_empty() {
                        Some((
                            "required-children".to_string(),
                            format!("missing required children: {:?}", missing),
                            Some(format!("Add missing children: {:?}", missing)),
                        ))
                    } else {
                        None
                    }
                }

                StructureRequirement::MustHaveId => {
                    if node.element_id.is_none() || node.element_id.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                        Some((
                            "id-required".to_string(),
                            "element should have an id attribute".to_string(),
                            Some("Add id=\"unique-id\" attribute".to_string()),
                        ))
                    } else {
                        None
                    }
                }

                StructureRequirement::MustHaveAttribute(attr) => {
                    if node.attrs.get(attr).is_none() {
                        Some((
                            format!("attr-{}", attr),
                            format!("element should have '{}' attribute", attr),
                            Some(format!("Add {}=\"value\" attribute", attr)),
                        ))
                    } else {
                        None
                    }
                }

                StructureRequirement::MustHaveClass(class) => {
                    if !node.has_class(class) {
                        Some((
                            format!("class-{}", class),
                            format!("element should have '{}' class", class),
                            Some(format!("Add class=\"{}\"", class)),
                        ))
                    } else {
                        None
                    }
                }
            };

            if let Some((rule_name, msg, suggestion)) = violated {
                violations.push(Violation {
                    element: tag.to_string(),
                    node_id,
                    rule_name,
                    message: msg,
                    severity: *severity,
                    suggestion,
                });
            }
        }
    }

    /// Run validation and return a summary report.
    pub fn validate_with_report(&self, doc: &Document, styles: &StyleMap) -> ValidationReport {
        let violations = self.validate(doc, styles);

        let critical_count = violations.iter().filter(|v| v.severity == Severity::Critical).count();
        let error_count = violations.iter().filter(|v| v.severity == Severity::Error).count();
        let warning_count = violations.iter().filter(|v| v.severity == Severity::Warning).count();

        ValidationReport {
            violations,
            critical_count,
            error_count,
            warning_count,
        }
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}
