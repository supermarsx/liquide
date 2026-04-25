//! Side panel sub-tab content for the Elements tab.
//!
//! Each method renders one of the sub-tabs: Styles, Layout, Computed, Fonts,
//! or Animations.

use liquide_components::TemplateNode;
use liquide_layout::tree::LayoutTree;
use liquide_style_engine::StyleMap;

use super::{DevToolsPanel, format_timing_function};

impl DevToolsPanel {
    /// Side: Styles — computed CSS properties grouped by category.
    pub(super) fn side_styles(&self, styles: &StyleMap) -> Vec<TemplateNode> {
        let Some(id) = self.selected_node else {
            return vec![
                TemplateNode::el("devtools-row").child(
                    TemplateNode::el("devtools-value")
                        .class("dim")
                        .child(TemplateNode::text("Select an element")),
                ),
            ];
        };
        if styles.get(id).is_none() {
            return vec![
                TemplateNode::el("devtools-row").child(
                    TemplateNode::el("devtools-value")
                        .class("dim")
                        .child(TemplateNode::text("No styles")),
                ),
            ];
        }

        let mut sections = Vec::new();
        for (cat, props) in self.style_inspector.grouped_properties() {
            let cat_label = format!("{:?}", cat);
            let mut section = TemplateNode::el("devtools-style-section")
                .key(&cat_label)
                .child(
                    TemplateNode::el("devtools-section-header")
                        .child(TemplateNode::text(&format!("\u{25BC} {}", cat_label))),
                );
            for prop in &props {
                let is_editing = self.style_editor.editing_property() == Some(prop.name.as_str());
                section = section.child(
                    TemplateNode::el("devtools-prop")
                        .key(&prop.name)
                        .class_if("inherited", prop.inherited)
                        .class_if("editing", is_editing)
                        .child(
                            TemplateNode::el("devtools-prop-name")
                                .child(TemplateNode::text(&format!("{}:", prop.name))),
                        )
                        .child(
                            TemplateNode::el("devtools-prop-value")
                                .class("editable")
                                .child(TemplateNode::text(if is_editing {
                                    self.style_editor.editing_value()
                                } else {
                                    &prop.value
                                })),
                        ),
                );
            }
            sections.push(section);
        }
        sections
    }

    /// Side: Layout — box model + layout properties.
    pub(super) fn side_layout(&self, layout: &LayoutTree, styles: &StyleMap) -> Vec<TemplateNode> {
        let Some(id) = self.selected_node else {
            return vec![
                TemplateNode::el("devtools-row").child(
                    TemplateNode::el("devtools-value")
                        .class("dim")
                        .child(TemplateNode::text("Select an element")),
                ),
            ];
        };
        let layout_box = match layout.find_by_node(id) {
            Some(b) => b,
            None => {
                return vec![
                    TemplateNode::el("devtools-row").child(
                        TemplateNode::el("devtools-value")
                            .class("dim")
                            .child(TemplateNode::text("No layout box")),
                    ),
                ];
            }
        };

        let mut nodes = Vec::new();

        // Box model.
        let mr = &layout_box.margin_rect;
        let br = &layout_box.border_rect;
        let pr = &layout_box.padding_rect;
        let cr = &layout_box.content_rect;

        let margin_t = br.y - mr.y;
        let margin_r_val = (mr.x + mr.width) - (br.x + br.width);
        let margin_b = (mr.y + mr.height) - (br.y + br.height);
        let margin_l = br.x - mr.x;

        let border_t = pr.y - br.y;
        let border_r_val = (br.x + br.width) - (pr.x + pr.width);
        let border_b = (br.y + br.height) - (pr.y + pr.height);
        let border_l = pr.x - br.x;

        let padding_t = cr.y - pr.y;
        let padding_r_val = (pr.x + pr.width) - (cr.x + cr.width);
        let padding_b = (pr.y + pr.height) - (cr.y + cr.height);
        let padding_l = cr.x - pr.x;

        nodes.push(
            TemplateNode::el("devtools-box-model").child(
                TemplateNode::el("devtools-box-margin")
                    .attr(
                        "data-label",
                        &format!(
                            "m: {:.0} {:.0} {:.0} {:.0}",
                            margin_t, margin_r_val, margin_b, margin_l
                        ),
                    )
                    .child(
                        TemplateNode::el("devtools-box-border")
                            .attr(
                                "data-label",
                                &format!(
                                    "b: {:.0} {:.0} {:.0} {:.0}",
                                    border_t, border_r_val, border_b, border_l
                                ),
                            )
                            .child(
                                TemplateNode::el("devtools-box-padding")
                                    .attr(
                                        "data-label",
                                        &format!(
                                            "p: {:.0} {:.0} {:.0} {:.0}",
                                            padding_t, padding_r_val, padding_b, padding_l
                                        ),
                                    )
                                    .child(TemplateNode::el("devtools-box-content").child(
                                        TemplateNode::text(&format!(
                                            "{:.0}\u{00D7}{:.0}",
                                            cr.width, cr.height
                                        )),
                                    )),
                            ),
                    ),
            ),
        );

        // Layout properties.
        if let Some(computed) = styles.get(id) {
            let prop_list = [
                ("position", format!("{:?}", computed.position)),
                ("display", format!("{:?}", computed.display)),
                ("box-sizing", format!("{:?}", computed.box_sizing)),
                ("overflow-x", format!("{:?}", computed.overflow_x)),
                ("overflow-y", format!("{:?}", computed.overflow_y)),
                ("float", format!("{:?}", computed.float)),
                ("clear", format!("{:?}", computed.clear)),
            ];
            for (name, value) in &prop_list {
                nodes.push(
                    TemplateNode::el("devtools-row")
                        .key(name)
                        .child(
                            TemplateNode::el("devtools-label")
                                .child(TemplateNode::text(&format!("{}:", name))),
                        )
                        .child(TemplateNode::el("devtools-value").child(TemplateNode::text(value))),
                );
            }

            if format!("{:?}", computed.display).contains("Flex") {
                nodes.push(
                    TemplateNode::el("devtools-heading").child(TemplateNode::text("Flexbox")),
                );
                let flex_props = [
                    ("flex-direction", format!("{:?}", computed.flex_direction)),
                    ("flex-wrap", format!("{:?}", computed.flex_wrap)),
                    ("justify-content", format!("{:?}", computed.justify_content)),
                    ("align-items", format!("{:?}", computed.align_items)),
                    ("align-content", format!("{:?}", computed.align_content)),
                    ("gap", format!("{:?}", computed.gap)),
                ];
                for (name, value) in &flex_props {
                    nodes.push(
                        TemplateNode::el("devtools-row")
                            .key(name)
                            .child(
                                TemplateNode::el("devtools-label")
                                    .child(TemplateNode::text(&format!("{}:", name))),
                            )
                            .child(
                                TemplateNode::el("devtools-value").child(TemplateNode::text(value)),
                            ),
                    );
                }
            }
        }
        nodes
    }

    /// Side: Computed — all visible (filtered) properties as a flat list.
    pub(super) fn side_computed(&self, styles: &StyleMap) -> Vec<TemplateNode> {
        let Some(id) = self.selected_node else {
            return vec![
                TemplateNode::el("devtools-row").child(
                    TemplateNode::el("devtools-value")
                        .class("dim")
                        .child(TemplateNode::text("Select an element")),
                ),
            ];
        };
        if styles.get(id).is_none() {
            return vec![
                TemplateNode::el("devtools-row").child(
                    TemplateNode::el("devtools-value")
                        .class("dim")
                        .child(TemplateNode::text("No styles")),
                ),
            ];
        }

        let props = self.style_inspector.visible_properties();
        props
            .iter()
            .map(|prop| {
                TemplateNode::el("devtools-prop")
                    .key(&prop.name)
                    .class_if("inherited", prop.inherited)
                    .child(
                        TemplateNode::el("devtools-prop-name")
                            .child(TemplateNode::text(&format!("{}:", prop.name))),
                    )
                    .child(
                        TemplateNode::el("devtools-prop-value")
                            .child(TemplateNode::text(&prop.value)),
                    )
            })
            .collect()
    }

    /// Side: Fonts — font properties from computed style.
    pub(super) fn side_fonts(&self, styles: &StyleMap) -> Vec<TemplateNode> {
        let Some(id) = self.selected_node else {
            return vec![
                TemplateNode::el("devtools-row").child(
                    TemplateNode::el("devtools-value")
                        .class("dim")
                        .child(TemplateNode::text("Select an element")),
                ),
            ];
        };
        let computed = match styles.get(id) {
            Some(c) => c,
            None => {
                return vec![
                    TemplateNode::el("devtools-row").child(
                        TemplateNode::el("devtools-value")
                            .class("dim")
                            .child(TemplateNode::text("No styles")),
                    ),
                ];
            }
        };

        let mut nodes = Vec::new();
        let families: Vec<String> = computed
            .font_family
            .iter()
            .map(|f| format!("\"{}\"", f))
            .collect();
        let primary_props = [
            ("font-family", families.join(", ")),
            ("font-size", format!("{:.1}px", computed.font_size)),
            ("font-weight", format!("{}", computed.font_weight)),
            ("font-style", format!("{:?}", computed.font_style)),
            ("line-height", format!("{:?}", computed.line_height)),
            ("letter-spacing", format!("{:?}", computed.letter_spacing)),
            ("word-spacing", format!("{:?}", computed.word_spacing)),
        ];
        for (name, value) in &primary_props {
            nodes.push(
                TemplateNode::el("devtools-row")
                    .key(name)
                    .child(
                        TemplateNode::el("devtools-label")
                            .child(TemplateNode::text(&format!("{}:", name))),
                    )
                    .child(TemplateNode::el("devtools-value").child(TemplateNode::text(value))),
            );
        }

        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Typography")));
        let typo_props = [
            ("text-align", format!("{:?}", computed.text_align)),
            ("text-transform", format!("{:?}", computed.text_transform)),
            ("white-space", format!("{:?}", computed.white_space)),
            ("word-break", format!("{:?}", computed.word_break)),
        ];
        for (name, value) in &typo_props {
            nodes.push(
                TemplateNode::el("devtools-row")
                    .key(&format!("f-{}", name))
                    .child(
                        TemplateNode::el("devtools-label")
                            .child(TemplateNode::text(&format!("{}:", name))),
                    )
                    .child(TemplateNode::el("devtools-value").child(TemplateNode::text(value))),
            );
        }

        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Rendered Font")));
        for (i, family) in computed.font_family.iter().enumerate() {
            let marker = if i == 0 {
                "\u{25B6}"
            } else {
                "\u{2003}\u{25B7}"
            };
            nodes.push(
                TemplateNode::el("devtools-row")
                    .key(&format!("rf-{}", i))
                    .child(
                        TemplateNode::el("devtools-value")
                            .class_if("teal", i == 0)
                            .class_if("dim", i > 0)
                            .child(TemplateNode::text(&format!(
                                "{} \"{}\" \u{2014} {:.0}px, wt {}",
                                marker, family, computed.font_size, computed.font_weight
                            ))),
                    ),
            );
        }
        nodes
    }

    /// Side: Animations — transitions and CSS animations.
    pub(super) fn side_animations(&self, styles: &StyleMap) -> Vec<TemplateNode> {
        let Some(id) = self.selected_node else {
            return vec![
                TemplateNode::el("devtools-row").child(
                    TemplateNode::el("devtools-value")
                        .class("dim")
                        .child(TemplateNode::text("Select an element")),
                ),
            ];
        };
        let computed = match styles.get(id) {
            Some(c) => c,
            None => {
                return vec![
                    TemplateNode::el("devtools-row").child(
                        TemplateNode::el("devtools-value")
                            .class("dim")
                            .child(TemplateNode::text("No styles")),
                    ),
                ];
            }
        };

        let mut nodes = Vec::new();
        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Transitions")));
        if computed.transition.is_empty() {
            nodes.push(
                TemplateNode::el("devtools-row").child(
                    TemplateNode::el("devtools-value")
                        .class("dim")
                        .child(TemplateNode::text("none")),
                ),
            );
        } else {
            for (i, tr) in computed.transition.iter().enumerate() {
                let timing_str = format_timing_function(&tr.timing_function);
                nodes.push(
                    TemplateNode::el("devtools-row")
                        .key(&format!("tr-{}", i))
                        .child(
                            TemplateNode::el("devtools-label")
                                .child(TemplateNode::text(&tr.property)),
                        )
                        .child(TemplateNode::el("devtools-value").child(TemplateNode::text(
                            &format!(
                                "{}ms {} delay {}ms",
                                tr.duration_ms, timing_str, tr.delay_ms
                            ),
                        ))),
                );
            }
        }

        nodes
            .push(TemplateNode::el("devtools-heading").child(TemplateNode::text("CSS Animations")));
        if computed.animation.is_empty() {
            nodes.push(
                TemplateNode::el("devtools-row").child(
                    TemplateNode::el("devtools-value")
                        .class("dim")
                        .child(TemplateNode::text("none")),
                ),
            );
        } else {
            for (i, anim) in computed.animation.iter().enumerate() {
                let timing_str = format_timing_function(&anim.timing_function);
                nodes.push(
                    TemplateNode::el("devtools-row")
                        .key(&format!("an-{}", i))
                        .child(
                            TemplateNode::el("devtools-label")
                                .child(TemplateNode::text(&anim.name)),
                        )
                        .child(TemplateNode::el("devtools-value").child(TemplateNode::text(
                            &format!(
                                "{}ms {} x{:?} {:?} {:?}",
                                anim.duration_ms,
                                timing_str,
                                anim.iteration_count,
                                anim.direction,
                                anim.fill_mode
                            ),
                        ))),
                );
            }
        }

        if !computed.transform.is_empty() || computed.opacity < 1.0 {
            nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Related")));
            if !computed.transform.is_empty() {
                nodes.push(
                    TemplateNode::el("devtools-row")
                        .key("rp-transform")
                        .child(
                            TemplateNode::el("devtools-label")
                                .child(TemplateNode::text("transform:")),
                        )
                        .child(
                            TemplateNode::el("devtools-value")
                                .child(TemplateNode::text(&format!("{:?}", computed.transform))),
                        ),
                );
            }
            if computed.opacity < 1.0 {
                nodes.push(
                    TemplateNode::el("devtools-row")
                        .key("rp-opacity")
                        .child(
                            TemplateNode::el("devtools-label")
                                .child(TemplateNode::text("opacity:")),
                        )
                        .child(
                            TemplateNode::el("devtools-value")
                                .child(TemplateNode::text(&format!("{:.2}", computed.opacity))),
                        ),
                );
            }
        }
        nodes
    }
}
