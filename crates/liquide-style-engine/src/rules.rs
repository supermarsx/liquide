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

use std::collections::HashMap;

use liquide_dom::{Document, NodeId};

use crate::computed::{
    AlignItems, ComputedStyle, Display, FlexDirection, JustifyContent, Position,
};
use crate::style_map::StyleMap;

// ═══════════════════════════════════════════════════════════════════════════
// Rule Types
// ═══════════════════════════════════════════════════════════════════════════

/// Severity level for rule violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Will cause visual glitches but may still render.
    Warning,
    /// Element will not function correctly.
    Error,
    /// Element will be completely broken/invisible.
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Warning => write!(f, "WARN"),
            Severity::Error => write!(f, "ERROR"),
            Severity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// A violation found during rule validation.
#[derive(Debug, Clone)]
pub struct Violation {
    /// The element tag or ID that has the violation.
    pub element: String,
    /// Node ID in the document.
    pub node_id: NodeId,
    /// The rule that was violated.
    pub rule_name: String,
    /// Human-readable description of the problem.
    pub message: String,
    /// How severe is this violation.
    pub severity: Severity,
    /// Suggested fix.
    pub suggestion: Option<String>,
}

/// A property requirement that an element must satisfy.
#[derive(Debug, Clone)]
pub enum PropertyRequirement {
    /// Display must be one of these values.
    DisplayOneOf(Vec<Display>),
    /// Position must be one of these values.
    PositionOneOf(Vec<Position>),
    /// Flex direction must be one of these values.
    FlexDirectionOneOf(Vec<FlexDirection>),
    /// Justify content must be one of these values.
    JustifyContentOneOf(Vec<JustifyContent>),
    /// Align items must be one of these values.
    AlignItemsOneOf(Vec<AlignItems>),
    /// Width must be set (not Auto).
    WidthDefined,
    /// Height must be set (not Auto).
    HeightDefined,
    /// Min width must be set.
    MinWidthDefined,
    /// Max width must be set.
    MaxWidthDefined,
    /// z-index must be set.
    ZIndexDefined,
    /// Border radius must be > 0.
    BorderRadiusPositive,
    /// Background color alpha must be > 0.
    BackgroundVisible,
    /// Blur radius must be > 0 (for glass effects).
    BlurRadiusPositive,
    /// Gap must be > 0 (for flex/grid containers).
    GapPositive,
    /// Padding must be set on at least one side.
    PaddingDefined,
    /// Font size must be in range.
    FontSizeInRange { min: f32, max: f32 },
    /// Color must be visible (alpha > 0).
    ColorVisible,
    /// Custom predicate with message.
    Custom {
        name: String,
        check: fn(&ComputedStyle) -> bool,
        message: String,
    },
}

/// A structural requirement for the DOM hierarchy.
#[derive(Debug, Clone)]
pub enum StructureRequirement {
    /// Element must be a direct child of elements with these tags.
    ParentTagOneOf(Vec<String>),
    /// Element must contain at least one child with this tag.
    MustContainChild(String),
    /// Element must have specific children in order.
    RequiredChildren(Vec<String>),
    /// Element must have an ID attribute.
    MustHaveId,
    /// Element must have specific attribute.
    MustHaveAttribute(String),
    /// Element must have specific class.
    MustHaveClass(String),
}

/// A complete rule for an element type.
#[derive(Debug, Clone)]
pub struct ElementRule {
    /// Tag name this rule applies to (e.g., "dock", "statusbar").
    pub tag: String,
    /// Human-readable description.
    pub description: String,
    /// CSS property requirements.
    pub property_requirements: Vec<(PropertyRequirement, Severity)>,
    /// DOM structure requirements.
    pub structure_requirements: Vec<(StructureRequirement, Severity)>,
}

impl ElementRule {
    /// Create a new rule for a tag.
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            description: String::new(),
            property_requirements: Vec::new(),
            structure_requirements: Vec::new(),
        }
    }

    /// Add a description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Require display to be one of these values.
    pub fn display_one_of(mut self, values: &[Display], severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::DisplayOneOf(values.to_vec()), severity));
        self
    }

    /// Require position to be one of these values.
    pub fn position_one_of(mut self, values: &[Position], severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::PositionOneOf(values.to_vec()), severity));
        self
    }

    /// Require flex direction to be one of these values.
    pub fn flex_direction_one_of(mut self, values: &[FlexDirection], severity: Severity) -> Self {
        self.property_requirements.push((
            PropertyRequirement::FlexDirectionOneOf(values.to_vec()),
            severity,
        ));
        self
    }

    /// Require justify content to be one of these values.
    pub fn justify_content_one_of(
        mut self,
        values: &[JustifyContent],
        severity: Severity,
    ) -> Self {
        self.property_requirements.push((
            PropertyRequirement::JustifyContentOneOf(values.to_vec()),
            severity,
        ));
        self
    }

    /// Require align items to be one of these values.
    pub fn align_items_one_of(mut self, values: &[AlignItems], severity: Severity) -> Self {
        self.property_requirements.push((
            PropertyRequirement::AlignItemsOneOf(values.to_vec()),
            severity,
        ));
        self
    }

    /// Require width to be defined.
    pub fn width_defined(mut self, severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::WidthDefined, severity));
        self
    }

    /// Require height to be defined.
    pub fn height_defined(mut self, severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::HeightDefined, severity));
        self
    }

    /// Require z-index to be defined.
    pub fn z_index_defined(mut self, severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::ZIndexDefined, severity));
        self
    }

    /// Require background to be visible.
    pub fn background_visible(mut self, severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::BackgroundVisible, severity));
        self
    }

    /// Require blur radius to be positive.
    pub fn blur_radius_positive(mut self, severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::BlurRadiusPositive, severity));
        self
    }

    /// Require padding to be defined.
    pub fn padding_defined(mut self, severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::PaddingDefined, severity));
        self
    }

    /// Require gap to be positive.
    pub fn gap_positive(mut self, severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::GapPositive, severity));
        self
    }

    /// Require font size in range.
    pub fn font_size_in_range(mut self, min: f32, max: f32, severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::FontSizeInRange { min, max }, severity));
        self
    }

    /// Require color to be visible.
    pub fn color_visible(mut self, severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::ColorVisible, severity));
        self
    }

    /// Require min-width to be defined.
    pub fn min_width_defined(mut self, severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::MinWidthDefined, severity));
        self
    }

    /// Require max-width to be defined.
    pub fn max_width_defined(mut self, severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::MaxWidthDefined, severity));
        self
    }

    /// Require border radius to be positive.
    pub fn border_radius_positive(mut self, severity: Severity) -> Self {
        self.property_requirements
            .push((PropertyRequirement::BorderRadiusPositive, severity));
        self
    }

    /// Require parent to be one of these tags.
    pub fn parent_one_of(mut self, tags: &[&str], severity: Severity) -> Self {
        self.structure_requirements.push((
            StructureRequirement::ParentTagOneOf(tags.iter().map(|s| s.to_string()).collect()),
            severity,
        ));
        self
    }

    /// Require element to contain a child with this tag.
    pub fn must_contain_child(mut self, tag: &str, severity: Severity) -> Self {
        self.structure_requirements
            .push((StructureRequirement::MustContainChild(tag.into()), severity));
        self
    }

    /// Require element to have an ID.
    pub fn must_have_id(mut self, severity: Severity) -> Self {
        self.structure_requirements
            .push((StructureRequirement::MustHaveId, severity));
        self
    }

    /// Require element to have specific attribute.
    pub fn must_have_attribute(mut self, attr: &str, severity: Severity) -> Self {
        self.structure_requirements.push((
            StructureRequirement::MustHaveAttribute(attr.into()),
            severity,
        ));
        self
    }

    /// Require element to have specific class.
    pub fn must_have_class(mut self, class: &str, severity: Severity) -> Self {
        self.structure_requirements
            .push((StructureRequirement::MustHaveClass(class.into()), severity));
        self
    }
}

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

    /// Create a rule engine with all builtin rules for shell elements.
    pub fn with_builtin_rules() -> Self {
        let mut engine = Self::new();

        // ── Desktop Background ──
        engine.add_rule(
            ElementRule::new("desktop-background")
                .description("Root background element covering the entire screen")
                .position_one_of(&[Position::Fixed, Position::Absolute], Severity::Critical)
                .width_defined(Severity::Error)
                .height_defined(Severity::Error)
                .background_visible(Severity::Warning),
        );

        // ── Status Bar ──
        engine.add_rule(
            ElementRule::new("statusbar")
                .description("Top status bar with system indicators")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .height_defined(Severity::Error)
                .width_defined(Severity::Error)
                .z_index_defined(Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .background_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("statusbar-slot")
                .description("Status bar region (left/center/right)")
                .display_one_of(&[Display::Flex], Severity::Error)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .parent_one_of(&["statusbar"], Severity::Error),
        );

        engine.add_rule(
            ElementRule::new("statusbar-logo")
                .description("Brand logo in status bar")
                .display_one_of(&[Display::Flex], Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("statusbar-item")
                .description("Individual status bar item")
                .display_one_of(&[Display::Flex, Display::InlineFlex], Severity::Warning)
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("status-indicator")
                .description("Status indicator (connection, etc.)")
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("notification-indicator")
                .description("Notification count indicator")
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("session-button")
                .description("Session/power button in status bar")
                .color_visible(Severity::Warning),
        );

        // ── Dock ──
        engine.add_rule(
            ElementRule::new("dock")
                .description("Application dock at bottom of screen")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .height_defined(Severity::Error)
                .width_defined(Severity::Error)
                .justify_content_one_of(&[JustifyContent::Center], Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .gap_positive(Severity::Warning)
                .background_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("dock-item")
                .description("Individual dock icon slot")
                .display_one_of(&[Display::Flex], Severity::Error)
                .width_defined(Severity::Error)
                .height_defined(Severity::Error)
                .border_radius_positive(Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .justify_content_one_of(&[JustifyContent::Center], Severity::Warning)
                .parent_one_of(&["dock"], Severity::Error)
                .must_have_attribute("data-app-id", Severity::Warning),
        );

        // ── Context Menu ──
        engine.add_rule(
            ElementRule::new("context-menu")
                .description("Right-click context menu")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .z_index_defined(Severity::Error)
                .min_width_defined(Severity::Warning)
                .max_width_defined(Severity::Warning)
                .border_radius_positive(Severity::Warning)
                .background_visible(Severity::Error)
                .blur_radius_positive(Severity::Warning)
                .padding_defined(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("session-menu")
                .description("Session/power menu")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .z_index_defined(Severity::Error)
                .background_visible(Severity::Error)
                .blur_radius_positive(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("app-menu")
                .description("Application menu")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .z_index_defined(Severity::Error)
                .background_visible(Severity::Error),
        );

        engine.add_rule(
            ElementRule::new("menu-item")
                .description("Individual menu item")
                .display_one_of(&[Display::Flex], Severity::Error)
                .height_defined(Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .color_visible(Severity::Warning)
                .padding_defined(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("menu-separator")
                .description("Menu separator line")
                .height_defined(Severity::Error)
                .background_visible(Severity::Warning),
        );

        // ── Launcher ──
        engine.add_rule(
            ElementRule::new("launcher-overlay")
                .description("Launcher overlay backdrop")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .width_defined(Severity::Error)
                .height_defined(Severity::Error)
                .z_index_defined(Severity::Error)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .justify_content_one_of(&[JustifyContent::Center], Severity::Warning)
                .background_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("launcher")
                .description("Launcher search panel")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .width_defined(Severity::Error)
                .border_radius_positive(Severity::Warning)
                .background_visible(Severity::Error)
                .blur_radius_positive(Severity::Warning)
                .padding_defined(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("launcher-search")
                .description("Launcher search input")
                .height_defined(Severity::Error)
                .padding_defined(Severity::Warning)
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("launcher-results")
                .description("Launcher results container")
                .display_one_of(&[Display::Flex], Severity::Error)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error),
        );

        engine.add_rule(
            ElementRule::new("launcher-item")
                .description("Individual launcher result item")
                .display_one_of(&[Display::Flex], Severity::Error)
                .height_defined(Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .color_visible(Severity::Warning),
        );

        // ── Notifications ──
        engine.add_rule(
            ElementRule::new("notification-area")
                .description("Notification container area")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .z_index_defined(Severity::Error)
                .gap_positive(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("notification")
                .description("Individual notification toast")
                .display_one_of(&[Display::Flex], Severity::Error)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .width_defined(Severity::Warning)
                .border_radius_positive(Severity::Warning)
                .background_visible(Severity::Error)
                .blur_radius_positive(Severity::Warning)
                .padding_defined(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("notification-title")
                .description("Notification title text")
                .color_visible(Severity::Warning)
                .font_size_in_range(10.0, 20.0, Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("notification-body")
                .description("Notification body text")
                .color_visible(Severity::Warning),
        );

        // ── Windows ──
        engine.add_rule(
            ElementRule::new("window")
                .description("Application window frame")
                .position_one_of(&[Position::Absolute, Position::Fixed], Severity::Critical)
                .display_one_of(&[Display::Flex], Severity::Error)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .border_radius_positive(Severity::Warning)
                .background_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("window-titlebar")
                .description("Window title bar")
                .display_one_of(&[Display::Flex], Severity::Error)
                .height_defined(Severity::Error)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("window-title")
                .description("Window title text")
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("window-content")
                .description("Window content area")
                .background_visible(Severity::Warning),
        );

        // ── Title bar buttons ──
        for button in &["close-button", "maximize-button", "minimize-button"] {
            engine.add_rule(
                ElementRule::new(*button)
                    .description("Window decoration button")
                    .display_one_of(&[Display::Flex], Severity::Error)
                    .width_defined(Severity::Error)
                    .height_defined(Severity::Error)
                    .border_radius_positive(Severity::Warning)
                    .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                    .justify_content_one_of(&[JustifyContent::Center], Severity::Warning),
            );
        }

        // ── Loading ──
        engine.add_rule(
            ElementRule::new("loading-overlay")
                .description("Loading screen overlay")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .width_defined(Severity::Error)
                .height_defined(Severity::Error)
                .z_index_defined(Severity::Error)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .justify_content_one_of(&[JustifyContent::Center], Severity::Warning)
                .background_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("loading-panel")
                .description("Loading panel content")
                .display_one_of(&[Display::Flex], Severity::Error)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .border_radius_positive(Severity::Warning)
                .background_visible(Severity::Warning)
                .color_visible(Severity::Warning),
        );

        engine
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

/// Summary report of validation results.
#[derive(Debug)]
pub struct ValidationReport {
    /// All violations found.
    pub violations: Vec<Violation>,
    /// Number of critical violations.
    pub critical_count: usize,
    /// Number of error violations.
    pub error_count: usize,
    /// Number of warning violations.
    pub warning_count: usize,
}

impl ValidationReport {
    /// Returns true if there are no critical or error violations.
    pub fn is_valid(&self) -> bool {
        self.critical_count == 0 && self.error_count == 0
    }

    /// Returns true if there are no violations at all.
    pub fn is_perfect(&self) -> bool {
        self.violations.is_empty()
    }

    /// Format report as human-readable string.
    pub fn to_string_report(&self) -> String {
        let mut report = String::new();
        report.push_str(&format!(
            "Validation Report: {} critical, {} errors, {} warnings\n",
            self.critical_count, self.error_count, self.warning_count
        ));
        report.push_str("═".repeat(60).as_str());
        report.push('\n');

        for v in &self.violations {
            report.push_str(&format!("[{}] <{}> {}\n", v.severity, v.element, v.message));
            if let Some(suggestion) = &v.suggestion {
                report.push_str(&format!("    ↳ Fix: {}\n", suggestion));
            }
        }

        if self.violations.is_empty() {
            report.push_str("✓ All elements pass validation\n");
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computed::ComputedStyle;
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

    fn create_test_styles(doc: &Document) -> StyleMap {
        let mut styles = StyleMap::new();

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
