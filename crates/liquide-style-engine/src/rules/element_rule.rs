//! Element rule definition and builder methods.

use crate::computed::{
    AlignItems, Display, FlexDirection, JustifyContent, Position,
};

use super::types::{PropertyRequirement, Severity, StructureRequirement};

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
