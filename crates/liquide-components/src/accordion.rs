//! Accordion / collapsible component — renders expandable/collapsible sections
//! via the template engine.
//!
//! ## DOM structure
//!
//! ```text
//! <accordion id="accordion-{id}">
//!   <accordion-item data-key="{key}" class="expanded">
//!     <accordion-header>
//!       <accordion-icon />
//!       <accordion-title>{title}</accordion-title>
//!     </accordion-header>
//!     <accordion-content>
//!       {children…}
//!     </accordion-content>
//!   </accordion-item>
//!   …
//! </accordion>
//! ```
//!
//! ## CSS targeting
//!
//! - `accordion` — the overall container
//! - `accordion-item` — each collapsible section
//! - `accordion-item.expanded` — currently open section
//! - `accordion-item.disabled` — non-interactive section
//! - `accordion-header` — clickable header area
//! - `accordion-header:hover` — hovered header
//! - `accordion-icon` — the expand/collapse arrow
//! - `accordion-title` — the section title text
//! - `accordion-content` — the collapsible body area

use liquide_dom::PseudoStateFlags;

use crate::template::{Component, TemplateNode};

/// Describes a single accordion section.
#[derive(Debug, Clone)]
pub struct AccordionSection {
    /// Unique key for reconciliation.
    pub key: String,
    /// Display title for the section header.
    pub title: String,
    /// Whether this section is currently expanded.
    pub expanded: bool,
    /// Whether this section is interactive (can be toggled).
    pub enabled: bool,
    /// Child template nodes for the content area.
    /// If empty, the content area is still rendered but empty.
    pub children: Vec<TemplateNode>,
}

impl AccordionSection {
    /// Create a new accordion section.
    pub fn new(key: &str, title: &str) -> Self {
        Self {
            key: key.to_string(),
            title: title.to_string(),
            expanded: false,
            enabled: true,
            children: Vec::new(),
        }
    }

    /// Set whether the section is expanded.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Set whether the section is enabled.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set the content children.
    pub fn children(mut self, children: Vec<TemplateNode>) -> Self {
        self.children = children;
        self
    }

    /// Add a single child to the content.
    pub fn child(mut self, child: TemplateNode) -> Self {
        self.children.push(child);
        self
    }
}

/// Whether the accordion allows multiple sections open simultaneously.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccordionMode {
    /// Only one section can be open at a time (clicking one closes others).
    Single,
    /// Multiple sections can be open simultaneously.
    Multiple,
}

impl Default for AccordionMode {
    fn default() -> Self {
        AccordionMode::Multiple
    }
}

/// Accordion component that renders expandable/collapsible sections.
pub struct AccordionComponent<'a> {
    /// Unique identifier for this accordion instance.
    pub id: &'a str,
    /// The sections to render.
    pub sections: &'a [AccordionSection],
    /// Which section header is currently hovered (index).
    pub hover_index: Option<usize>,
    /// Accordion behavior mode.
    pub mode: AccordionMode,
}

impl<'a> AccordionComponent<'a> {
    /// Create a new accordion component.
    pub fn new(id: &'a str, sections: &'a [AccordionSection]) -> Self {
        Self {
            id,
            sections,
            hover_index: None,
            mode: AccordionMode::default(),
        }
    }

    /// Set the hover index.
    pub fn hover(mut self, index: Option<usize>) -> Self {
        self.hover_index = index;
        self
    }

    /// Set the accordion mode.
    pub fn mode(mut self, mode: AccordionMode) -> Self {
        self.mode = mode;
        self
    }
}

impl Component for AccordionComponent<'_> {
    fn render(&self) -> TemplateNode {
        let mode_class = match self.mode {
            AccordionMode::Single => "single",
            AccordionMode::Multiple => "multiple",
        };

        TemplateNode::el("accordion")
            .id(&format!("accordion-{}", self.id))
            .class(mode_class)
            .children(self.sections.iter().enumerate().map(|(i, section)| {
                let mut item = TemplateNode::el("accordion-item")
                    .key(&section.key)
                    .class_if("expanded", section.expanded)
                    .class_if("collapsed", !section.expanded)
                    .class_if("disabled", !section.enabled)
                    .attr("data-key", &section.key)
                    .attr("data-index", &i.to_string())
                    .pseudo_if(PseudoStateFlags::DISABLED, !section.enabled)
                    // Header (always visible)
                    .child(
                        TemplateNode::el("accordion-header")
                            .pseudo_if(PseudoStateFlags::HOVER, self.hover_index == Some(i))
                            .child(
                                TemplateNode::el("accordion-icon")
                                    .class_if("rotated", section.expanded),
                            )
                            .child(
                                TemplateNode::el("accordion-title")
                                    .child(TemplateNode::text(&section.title)),
                            ),
                    );

                // Content area
                let mut content = TemplateNode::el("accordion-content")
                    .style("display", if section.expanded { "block" } else { "none" });

                if section.expanded {
                    content = content.children(section.children.clone());
                }

                item = item.child(content);
                item
            }))
    }

    fn mount_point(&self) -> &str {
        // This leaks from the format! — in practice the caller should
        // ensure the mount point element exists with the correct id.
        // For the component trait, we return a static base.
        "accordion"
    }
}

/// Toggle a section in an accordion, respecting the mode.
///
/// Returns the updated expanded state for each section.
pub fn toggle_section(sections: &mut [AccordionSection], index: usize, mode: AccordionMode) {
    if index >= sections.len() || !sections[index].enabled {
        return;
    }

    match mode {
        AccordionMode::Single => {
            let was_expanded = sections[index].expanded;
            // Close all
            for section in sections.iter_mut() {
                section.expanded = false;
            }
            // Toggle the target
            sections[index].expanded = !was_expanded;
        }
        AccordionMode::Multiple => {
            sections[index].expanded = !sections[index].expanded;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accordion_section_builder() {
        let section = AccordionSection::new("general", "General Settings")
            .expanded(true)
            .enabled(true)
            .child(TemplateNode::text("Content here"));

        assert_eq!(section.key, "general");
        assert_eq!(section.title, "General Settings");
        assert!(section.expanded);
        assert!(section.enabled);
        assert_eq!(section.children.len(), 1);
    }

    #[test]
    fn accordion_render_basic() {
        let sections = vec![
            AccordionSection::new("s1", "Section 1").expanded(true),
            AccordionSection::new("s2", "Section 2").expanded(false),
            AccordionSection::new("s3", "Section 3").expanded(false),
        ];

        let component = AccordionComponent::new("test", &sections);
        let tree = component.render();

        assert_eq!(tree.tag, "accordion");
        assert_eq!(tree.children.len(), 3);

        // First section should have "expanded" class
        assert!(tree.children[0].classes.contains(&"expanded".to_string()));
        assert!(!tree.children[0].classes.contains(&"collapsed".to_string()));

        // Second section should have "collapsed" class
        assert!(tree.children[1].classes.contains(&"collapsed".to_string()));
        assert!(!tree.children[1].classes.contains(&"expanded".to_string()));
    }

    #[test]
    fn accordion_render_disabled_section() {
        let sections = vec![
            AccordionSection::new("s1", "Enabled").enabled(true),
            AccordionSection::new("s2", "Disabled").enabled(false),
        ];

        let component = AccordionComponent::new("test", &sections);
        let tree = component.render();

        assert!(!tree.children[0].classes.contains(&"disabled".to_string()));
        assert!(tree.children[1].classes.contains(&"disabled".to_string()));
        assert!(tree.children[1]
            .pseudo_states
            .contains(PseudoStateFlags::DISABLED));
    }

    #[test]
    fn accordion_render_expanded_has_content() {
        let sections = vec![AccordionSection::new("s1", "Section 1")
            .expanded(true)
            .child(TemplateNode::text("Content A"))
            .child(TemplateNode::text("Content B"))];

        let component = AccordionComponent::new("test", &sections);
        let tree = component.render();

        // accordion > accordion-item > [accordion-header, accordion-content]
        let item = &tree.children[0];
        assert_eq!(item.children.len(), 2); // header + content

        let content = &item.children[1]; // accordion-content
        assert_eq!(content.tag, "accordion-content");
        assert_eq!(content.children.len(), 2); // "Content A" + "Content B"

        // Check display style
        let display = content.inline_styles.iter().find(|(k, _)| k == "display");
        assert_eq!(display.map(|(_, v)| v.as_str()), Some("block"));
    }

    #[test]
    fn accordion_render_collapsed_hides_content() {
        let sections = vec![AccordionSection::new("s1", "Section 1")
            .expanded(false)
            .child(TemplateNode::text("Hidden"))];

        let component = AccordionComponent::new("test", &sections);
        let tree = component.render();

        let content = &tree.children[0].children[1];
        let display = content.inline_styles.iter().find(|(k, _)| k == "display");
        assert_eq!(display.map(|(_, v)| v.as_str()), Some("none"));
        // Collapsed sections don't include children in the content
        assert!(content.children.is_empty());
    }

    #[test]
    fn accordion_header_structure() {
        let sections = vec![AccordionSection::new("s1", "My Title")];
        let component = AccordionComponent::new("test", &sections);
        let tree = component.render();

        let item = &tree.children[0];
        let header = &item.children[0];
        assert_eq!(header.tag, "accordion-header");
        assert_eq!(header.children.len(), 2); // icon + title

        let icon = &header.children[0];
        assert_eq!(icon.tag, "accordion-icon");

        let title = &header.children[1];
        assert_eq!(title.tag, "accordion-title");
        assert_eq!(title.children[0].text.as_deref(), Some("My Title"));
    }

    #[test]
    fn accordion_hover_sets_pseudo_state() {
        let sections = vec![
            AccordionSection::new("s1", "A"),
            AccordionSection::new("s2", "B"),
        ];

        let component = AccordionComponent::new("test", &sections).hover(Some(1));
        let tree = component.render();

        // First section's header should NOT be hovered
        let header0 = &tree.children[0].children[0];
        assert!(!header0.pseudo_states.contains(PseudoStateFlags::HOVER));

        // Second section's header should be hovered
        let header1 = &tree.children[1].children[0];
        assert!(header1.pseudo_states.contains(PseudoStateFlags::HOVER));
    }

    #[test]
    fn toggle_section_single_mode() {
        let mut sections = vec![
            AccordionSection::new("s1", "A").expanded(true),
            AccordionSection::new("s2", "B").expanded(false),
            AccordionSection::new("s3", "C").expanded(false),
        ];

        // Toggle s2 in single mode → s1 closes, s2 opens
        toggle_section(&mut sections, 1, AccordionMode::Single);
        assert!(!sections[0].expanded);
        assert!(sections[1].expanded);
        assert!(!sections[2].expanded);

        // Toggle s2 again → all close
        toggle_section(&mut sections, 1, AccordionMode::Single);
        assert!(!sections[0].expanded);
        assert!(!sections[1].expanded);
        assert!(!sections[2].expanded);
    }

    #[test]
    fn toggle_section_multiple_mode() {
        let mut sections = vec![
            AccordionSection::new("s1", "A").expanded(true),
            AccordionSection::new("s2", "B").expanded(false),
        ];

        // Toggle s2 → s1 stays open, s2 opens
        toggle_section(&mut sections, 1, AccordionMode::Multiple);
        assert!(sections[0].expanded);
        assert!(sections[1].expanded);

        // Toggle s1 → s1 closes, s2 stays open
        toggle_section(&mut sections, 0, AccordionMode::Multiple);
        assert!(!sections[0].expanded);
        assert!(sections[1].expanded);
    }

    #[test]
    fn toggle_disabled_section_is_noop() {
        let mut sections = vec![AccordionSection::new("s1", "A")
            .expanded(false)
            .enabled(false)];

        toggle_section(&mut sections, 0, AccordionMode::Multiple);
        assert!(!sections[0].expanded, "disabled section should not toggle");
    }

    #[test]
    fn toggle_out_of_bounds_is_noop() {
        let mut sections = vec![AccordionSection::new("s1", "A")];
        toggle_section(&mut sections, 5, AccordionMode::Multiple);
        // Should not panic
    }

    #[test]
    fn accordion_mode_class() {
        let sections = vec![AccordionSection::new("s1", "A")];

        let single = AccordionComponent::new("test", &sections)
            .mode(AccordionMode::Single)
            .render();
        assert!(single.classes.contains(&"single".to_string()));

        let multi = AccordionComponent::new("test", &sections)
            .mode(AccordionMode::Multiple)
            .render();
        assert!(multi.classes.contains(&"multiple".to_string()));
    }
}
