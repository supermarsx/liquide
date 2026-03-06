//! Template-based rendering for the DevTools panel UI.
//!
//! Produces declarative `TemplateNode` trees for the toolbar, content area,
//! status bar, and each main tab (Elements, Console, Sources, Performance,
//! Mutations, Scene).

use liquide_components::TemplateNode;
use liquide_dom::Document;
use liquide_layout::tree::LayoutTree;
use liquide_style_engine::StyleMap;

use super::{
    format_mutation_record, mutation_class, row_kv, row_kv_class,
    DevToolsPanel, DevToolsTab, DockPosition, SideTab,
};

impl DevToolsPanel {
    // ─── Template-based rendering ─────────────────────────────

    /// Produce a declarative `TemplateNode` tree for the entire devtools panel.
    ///
    /// The shell applies this to its document via `TemplateRenderer::apply_or_create`,
    /// and the CSS pipeline renders it as part of the normal scene.
    /// This replaces all the hand-built `SceneNode` vectors that were here before.
    ///
    /// Overlay highlights (picker, selection, hover) remain as direct scene nodes
    /// because they must render on top of the page viewport, not inside the panel.
    pub fn render_template(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> TemplateNode {
        let dock_class = match self.config.dock_position {
            DockPosition::Bottom => "dock-bottom",
            DockPosition::Right => "dock-right",
            DockPosition::Left => "dock-left",
            DockPosition::Float => "dock-float",
            DockPosition::Detached => "dock-detached",
        };

        let tab_id = self.tab_data_id();

        TemplateNode::el("devtools-panel")
            .id("devtools-panel")
            .class(dock_class)
            .child(self.render_toolbar_template())
            .child(self.render_content_template(doc, layout, styles, tab_id))
            .child(self.render_statusbar_template())
    }

    /// Map active tab to its data-tab string.
    fn tab_data_id(&self) -> &'static str {
        match self.active_tab {
            DevToolsTab::Elements => "elements",
            DevToolsTab::Console => "console",
            DevToolsTab::Sources => "sources",
            DevToolsTab::Performance => "perf",
            DevToolsTab::Mutations => "mutations",
            DevToolsTab::Scene => "scene",
        }
    }

    /// Toolbar: tab labels + action buttons.
    fn render_toolbar_template(&self) -> TemplateNode {
        let tabs = TemplateNode::el("devtools-tabs").children(
            DevToolsTab::ALL.iter().map(|tab| {
                let id = match tab {
                    DevToolsTab::Elements => "elements",
                    DevToolsTab::Console => "console",
                    DevToolsTab::Sources => "sources",
                    DevToolsTab::Performance => "perf",
                    DevToolsTab::Mutations => "mutations",
                    DevToolsTab::Scene => "scene",
                };
                TemplateNode::el("devtools-tab")
                    .key(&format!("tab-{}", id))
                    .attr("data-tab", id)
                    .class_if("active", *tab == self.active_tab)
                    .child(TemplateNode::text(tab.label()))
            }),
        );

        let actions = TemplateNode::el("devtools-actions")
            .child(
                TemplateNode::el("devtools-btn")
                    .key("btn-picker")
                    .attr("data-action", "picker")
                    .class_if("active", self.element_picker.is_active())
                    .child(TemplateNode::text("\u{2295}")), // ⊕
            )
            .child(
                TemplateNode::el("devtools-btn")
                    .key("btn-detach")
                    .attr("data-action", "detach")
                    .class_if("active", self.config.dock_position == DockPosition::Detached)
                    .child(TemplateNode::text("\u{25EB}")), // ◫ detach/window icon
            )
            .child(
                TemplateNode::el("devtools-btn")
                    .key("btn-dock-bottom")
                    .attr("data-action", "dock-bottom")
                    .class_if("active", self.config.dock_position == DockPosition::Bottom)
                    .child(TemplateNode::text("\u{22A5}")), // ⊥
            )
            .child(
                TemplateNode::el("devtools-btn")
                    .key("btn-dock-right")
                    .attr("data-action", "dock-right")
                    .class_if("active", self.config.dock_position == DockPosition::Right)
                    .child(TemplateNode::text("\u{22A2}")), // ⊢
            )
            .child(
                TemplateNode::el("devtools-btn")
                    .key("btn-close")
                    .attr("data-action", "close")
                    .child(TemplateNode::text("\u{00D7}")), // ×
            );

        TemplateNode::el("devtools-toolbar")
            .child(tabs)
            .child(actions)
    }

    /// Content area: active tab panel with its children.
    fn render_content_template(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
        active_tab: &str,
    ) -> TemplateNode {
        let tab_defs: &[(&str, fn(&Self, &Document, &LayoutTree, &StyleMap) -> Vec<TemplateNode>)] = &[
            ("elements", Self::template_elements),
            ("console", Self::template_console),
            ("sources", Self::template_sources),
            ("perf", Self::template_performance),
            ("mutations", Self::template_mutations),
            ("scene", Self::template_scene),
        ];

        TemplateNode::el("devtools-content")
            .children(tab_defs.iter().map(|(id, render_fn)| {
                let is_active = *id == active_tab;
                let mut panel = TemplateNode::el("devtools-tab-panel")
                    .key(&format!("panel-{}", id))
                    .attr("data-tab", id)
                    .class_if("active", is_active);
                if is_active {
                    panel = panel.children(render_fn(self, doc, layout, styles));
                }
                panel
            }))
    }

    /// Status bar at the bottom.
    fn render_statusbar_template(&self) -> TemplateNode {
        let text = if self.element_picker.is_active() {
            "Picker active \u{2014} click an element to inspect".to_string()
        } else {
            match self.selected_node {
                Some(id) => format!("Node #{} selected", id),
                None => "No element selected".to_string(),
            }
        };

        TemplateNode::el("devtools-statusbar")
            .child(
                TemplateNode::el("devtools-status-text")
                    .child(TemplateNode::text(&text)),
            )
    }

    // ─── Per-tab template renderers ─────────────────────────────

    /// Elements tab: split pane with DOM tree (left) and side panel (right).
    ///
    /// The side-panel has sub-tabs: Styles, Layout, Computed, Fonts, Anim.
    /// The DOM tree uses virtual scrolling — only visible rows are emitted.
    pub(super) fn template_elements(
        &self,
        _doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> Vec<TemplateNode> {
        // ── Left pane: DOM tree with virtual scrolling ──
        let main_pane = {
            let visible = self.inspector.visible_nodes();
            let row_h: f32 = 20.0;
            let (first, count) = self.visible_row_range(visible.len(), row_h);

            let selected = self.inspector.selected();
            let hovered = self.inspector.hovered();

            if visible.is_empty() {
                TemplateNode::el("devtools-main-pane")
                    .child(TemplateNode::el("devtools-tree")
                        .child(TemplateNode::el("devtools-row")
                            .child(TemplateNode::el("devtools-value").class("dim")
                                .child(TemplateNode::text("No elements \u{2014} run refresh_inspector(doc)")))))
            } else {
                let tree = TemplateNode::el("devtools-tree").children(
                    visible[first..first + count].iter().map(|node| {
                        let is_selected = selected == Some(node.id);
                        let is_hovered = hovered == Some(node.id);

                        let mut row = TemplateNode::el("devtools-tree-row")
                            .key(&format!("n-{}", node.id))
                            .attr("data-node", &node.id.to_string())
                            .style("padding-left", &format!("{}px", node.depth as u32 * 16 + 4))
                            .class_if("selected", is_selected)
                            .class_if("hovered", is_hovered);

                        if node.is_text {
                            let text = node.text.as_deref().unwrap_or("");
                            let truncated = if text.len() > 60 { &text[..60] } else { text };
                            row = row.child(
                                TemplateNode::el("devtools-tree-text")
                                    .child(TemplateNode::text(&format!("\"{}\"", truncated))),
                            );
                        } else {
                            let arrow = if node.child_count > 0 {
                                if node.children.is_empty() { "\u{25B6}" } else { "\u{25BC}" }
                            } else { "" };

                            if node.child_count > 0 {
                                row = row.child(
                                    TemplateNode::el("devtools-tree-arrow")
                                        .attr("data-tree-arrow", "")
                                        .attr("data-node", &node.id.to_string())
                                        .child(TemplateNode::text(arrow)),
                                );
                            }

                            row = row.child(
                                TemplateNode::el("devtools-tree-tag")
                                    .child(TemplateNode::text(&format!("<{}", node.tag))),
                            );

                            if let Some(ref eid) = node.element_id {
                                row = row.child(
                                    TemplateNode::el("devtools-tree-attr")
                                        .child(TemplateNode::text(&format!(" id=\"{}\"", eid))),
                                );
                            }
                            if !node.classes.is_empty() {
                                row = row.child(
                                    TemplateNode::el("devtools-tree-attr")
                                        .child(TemplateNode::text(&format!(" class=\"{}\"", node.classes.join(" ")))),
                                );
                            }
                            for (k, v) in &node.attributes {
                                row = row.child(
                                    TemplateNode::el("devtools-tree-attr")
                                        .child(TemplateNode::text(&format!(" {}=\"{}\"", k, v))),
                                );
                            }
                            row = row.child(
                                TemplateNode::el("devtools-tree-tag")
                                    .child(TemplateNode::text(">")),
                            );
                        }
                        row
                    }),
                );
                TemplateNode::el("devtools-main-pane").child(tree)
            }
        };

        // ── Right pane: side panel with sub-tabs ──
        let side_pane = {
            // Sub-tab bar.
            let side_tabs = TemplateNode::el("devtools-side-tabs").children(
                SideTab::ALL.iter().map(|st| {
                    let id = st.id();
                    TemplateNode::el("devtools-side-tab")
                        .key(&format!("st-{}", st.label()))
                        .attr("data-sidetab", id)
                        .class_if("active", *st == self.side_tab)
                        .child(TemplateNode::text(st.label()))
                }),
            );

            // Sub-tab content.
            let body_children = match self.side_tab {
                SideTab::Styles => self.side_styles(styles),
                SideTab::Layout => self.side_layout(layout, styles),
                SideTab::Computed => self.side_computed(styles),
                SideTab::Fonts => self.side_fonts(styles),
                SideTab::Animations => self.side_animations(styles),
            };

            TemplateNode::el("devtools-side-pane")
                .child(side_tabs)
                .child(TemplateNode::el("devtools-side-body").children(body_children))
        };

        let split = TemplateNode::el("devtools-split")
            .child(main_pane)
            .child(side_pane);

        vec![split]
    }

    /// Console tab: log entries + input line.
    pub(super) fn template_console(
        &self,
        _doc: &Document,
        _layout: &LayoutTree,
        _styles: &StyleMap,
    ) -> Vec<TemplateNode> {
        let entries = self.console.entries();

        // Virtual scroll the console log entries.
        let entry_h: f32 = 18.0;
        // Reserve 26px for the input bar.
        let avail = (self.content_height() - 26.0).max(0.0);
        let max_visible = (avail / entry_h).ceil() as usize + 1;
        let total = entries.len();
        let first = if total > max_visible { total - max_visible } else { 0 };

        let log = TemplateNode::el("devtools-console-log").children(
            entries.iter().enumerate().skip(first).map(|(i, entry)| {
                let kind_class = match entry.kind {
                    crate::console::ConsoleEntryKind::Input => "info",
                    crate::console::ConsoleEntryKind::Output => "log",
                    crate::console::ConsoleEntryKind::Warning => "warn",
                    crate::console::ConsoleEntryKind::Error => "error",
                    crate::console::ConsoleEntryKind::Info => "info",
                };
                TemplateNode::el("devtools-console-entry")
                    .key(&format!("ce-{}", i))
                    .class(kind_class)
                    .child(TemplateNode::text(&entry.text))
            }),
        );

        let input_text = self.console.input_buffer().to_string();
        let input = TemplateNode::el("devtools-console-input")
            .class_if("focused", self.console_focused)
            .child(TemplateNode::el("devtools-console-prompt").child(TemplateNode::text(">")))
            .child(TemplateNode::el("devtools-console-field").child(TemplateNode::text(&input_text)));

        vec![log, input]
    }

    /// Sources tab: document overview + DOM JSON.
    pub(super) fn template_sources(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> Vec<TemplateNode> {
        let mut nodes = Vec::new();

        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Document Overview")));

        let total_nodes = doc.node_count();
        let total_boxes = layout.box_count();
        let styled_count = styles.len();

        nodes.push(row_kv("Total DOM nodes", &total_nodes.to_string(), "teal"));
        nodes.push(row_kv("Layout boxes", &total_boxes.to_string(), "teal"));
        nodes.push(row_kv("Styled elements", &styled_count.to_string(), "teal"));

        let mut element_count = 0u32;
        let mut text_count = 0u32;
        for descendant in doc.descendants(doc.root()) {
            if let Some(node) = doc.get(descendant) {
                if node.is_text() {
                    text_count += 1;
                } else {
                    element_count += 1;
                }
            }
        }

        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Node Types")));
        nodes.push(row_kv("Elements", &element_count.to_string(), "blue"));
        nodes.push(row_kv("Text nodes", &text_count.to_string(), "blue"));

        // Tag distribution.
        let mut tag_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        for descendant in doc.descendants(doc.root()) {
            if let Some(node) = doc.get(descendant) {
                if !node.is_text() {
                    *tag_counts.entry(node.tag_name().to_string()).or_insert(0) += 1;
                }
            }
        }
        let mut sorted_tags: Vec<_> = tag_counts.into_iter().collect();
        sorted_tags.sort_by(|a, b| b.1.cmp(&a.1));

        if !sorted_tags.is_empty() {
            nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Tag Distribution")));
            for (tag, count) in sorted_tags.iter().take(15) {
                nodes.push(row_kv(&format!("<{}>", tag), &count.to_string(), "purple"));
            }
        }

        // DOM JSON (virtual scrolled).
        nodes.push(TemplateNode::el("devtools-separator"));
        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("DOM Tree (JSON)")));

        let json = self.dom_serializer.to_json(doc);
        let lines: Vec<&str> = json.lines().collect();
        let line_h: f32 = 16.0;
        let (first, count) = self.visible_row_range(lines.len(), line_h);

        let json_block = TemplateNode::el("devtools-dom-json").children(
            lines[first..first + count].iter().enumerate().map(|(i, line)| {
                TemplateNode::el("devtools-row")
                    .key(&format!("dj-{}", first + i))
                    .child(TemplateNode::el("devtools-value").class("teal").child(TemplateNode::text(line)))
            }),
        );
        nodes.push(json_block);

        nodes
    }

    /// Performance tab: pipeline metrics, frame timing, CSS engine stats.
    pub(super) fn template_performance(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        _styles: &StyleMap,
    ) -> Vec<TemplateNode> {
        let mut nodes = Vec::new();

        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Pipeline Metrics")));

        if let Some(ref snap) = self.frame_snapshot {
            let fps_class = if snap.fps >= 55.0 { "ok" } else if snap.fps >= 25.0 { "warn" } else { "error" };
            let frame_class = if snap.avg_frame_ms < 16.7 { "ok" } else if snap.avg_frame_ms < 33.3 { "warn" } else { "error" };

            nodes.push(row_kv_class("Frame", &snap.frame_number.to_string(), "teal"));
            nodes.push(row_kv_class("FPS", &format!("{:.1}", snap.fps), fps_class));
            nodes.push(row_kv_class("Avg frame time", &format!("{:.2}ms", snap.avg_frame_ms), frame_class));
            nodes.push(row_kv_class("Viewport", &format!("{:.0}\u{00D7}{:.0}", snap.viewport_w, snap.viewport_h), "dim"));
        } else {
            nodes.push(TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("Waiting for frame data..."))));
        }

        // Frame time sparkline.
        if self.frame_times.len() > 1 {
            nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Frame Times")));
            let chart = TemplateNode::el("devtools-bar-chart").children(
                self.frame_times.iter().enumerate().map(|(i, &ms)| {
                    let height_frac = (ms / 50.0).min(1.0);
                    let bar_class = if ms < 16.7 { "ok" } else if ms < 33.3 { "warn" } else { "error" };
                    TemplateNode::el("devtools-bar")
                        .key(&format!("ft-{}", i))
                        .class(bar_class)
                        .style("height", &format!("{:.0}%", height_frac * 100.0))
                }),
            );
            nodes.push(chart);
        }

        // CSS engine stats.
        if let Some(ref snap) = self.frame_snapshot {
            nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("CSS Engine")));
            nodes.push(row_kv("Stylesheets", &snap.stylesheet_count.to_string(), "teal"));
            nodes.push(row_kv("Rules", &snap.css_rule_count.to_string(), "teal"));
            nodes.push(row_kv("Variables", &snap.css_variable_count.to_string(), "teal"));
        }

        // DOM stats.
        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("DOM Statistics")));
        nodes.push(row_kv("Node count", &doc.node_count().to_string(), "blue"));
        nodes.push(row_kv("Layout boxes", &layout.box_count().to_string(), "blue"));

        nodes
    }

    /// Mutations tab: mutation log entries with virtual scrolling.
    pub(super) fn template_mutations(
        &self,
        _doc: &Document,
        _layout: &LayoutTree,
        _styles: &StyleMap,
    ) -> Vec<TemplateNode> {
        let records: Vec<_> = self.mutation_log.iter().collect();
        if records.is_empty() {
            return vec![TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("No mutations recorded")))];
        }

        let mut nodes = Vec::new();
        nodes.push(TemplateNode::el("devtools-heading")
            .child(TemplateNode::text(&format!("Mutations ({})", records.len()))));

        let row_h: f32 = 16.0;
        let (first, count) = self.visible_row_range(records.len(), row_h);

        let list = TemplateNode::el("devtools-mutations-list").children(
            records[first..first + count].iter().enumerate().map(|(i, record)| {
                let kind_class = mutation_class(&record.kind);
                TemplateNode::el("devtools-mutation-entry")
                    .key(&format!("mut-{}", first + i))
                    .class(kind_class)
                    .child(TemplateNode::text(&format_mutation_record(record)))
            }),
        );
        nodes.push(list);
        nodes
    }

    /// Scene tab: scene graph debugger + live style editor.
    pub(super) fn template_scene(
        &self,
        _doc: &Document,
        _layout: &LayoutTree,
        _styles: &StyleMap,
    ) -> Vec<TemplateNode> {
        let mut nodes = Vec::new();

        // Scene graph entries.
        let entries = self.scene_debugger.entries();
        if entries.is_empty() {
            nodes.push(TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("No scene graph captured"))));
        } else {
            let selected = self.scene_debugger.selected();
            let row_h: f32 = 16.0;
            let (first, count) = self.visible_row_range(entries.len(), row_h);

            nodes.push(TemplateNode::el("devtools-heading")
                .child(TemplateNode::text(&format!("Scene Graph ({} nodes)", entries.len()))));

            let list = TemplateNode::el("devtools-scene-list").children(
                entries[first..first + count].iter().enumerate().map(|(i, entry)| {
                    TemplateNode::el("devtools-scene-entry")
                        .key(&format!("sg-{}", first + i))
                        .class_if("selected", selected == Some(first + i))
                        .style("padding-left", &format!("{}px", entry.depth * 12 + 4))
                        .child(TemplateNode::text(&entry.kind))
                }),
            );
            nodes.push(list);
        }

        // Style editor section.
        nodes.push(TemplateNode::el("devtools-separator"));
        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Live Style Editor")));

        let edits = self.style_editor.pending_edits();
        if edits.is_empty() {
            nodes.push(TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("No style edits \u{2014} type property: value"))));
        } else {
            let list = TemplateNode::el("devtools-editor-list").children(
                edits.iter().enumerate().map(|(i, edit)| {
                    TemplateNode::el("devtools-editor-entry")
                        .key(&format!("ed-{}", i))
                        .class_if("applied", edit.applied)
                        .class_if("pending", !edit.applied)
                        .child(TemplateNode::text(&format!(
                            "{} {}: {}",
                            if edit.applied { "\u{2713}" } else { "\u{25CB}" },
                            edit.property, edit.new_value
                        )))
                }),
            );
            nodes.push(list);
        }

        let input_text = self.style_editor.editing_value().to_string();
        nodes.push(TemplateNode::el("devtools-editor-input")
            .child(TemplateNode::text(&format!("> {}", input_text))));

        nodes
    }
}
