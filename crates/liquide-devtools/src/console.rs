//! Debug console — interactive console that can inspect DOM, styles,
//! layout, and execute commands against the engine API.
//!
//! Supports a command-line input with history, output log, and built-in
//! commands for querying nodes, styles, layout, and document state.

use std::collections::VecDeque;
use std::time::Instant;

use liquide_dom::{Document, NodeId};
use liquide_layout::tree::LayoutTree;
use liquide_style_engine::StyleMap;
use serde::{Deserialize, Serialize};

/// An action requested by a console command that must be handled by
/// the host (e.g. the desktop shell or session controller).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleAction {
    /// Reload all stylesheets and re-run the CSS pipeline.
    ReloadStyles,
    /// Full UI restart — tear down and recreate the shell/session.
    RestartUI,
    /// Select the given node in the Elements panel.
    InspectNode(NodeId),
}

/// A single entry in the console output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleEntry {
    /// The message text.
    pub text: String,
    /// The kind of entry.
    pub kind: ConsoleEntryKind,
    /// Timestamp (ms since console creation).
    pub timestamp_ms: u64,
}

/// What kind of console entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsoleEntryKind {
    /// User input command.
    Input,
    /// Successful output.
    Output,
    /// Warning message.
    Warning,
    /// Error message.
    Error,
    /// Informational / system message.
    Info,
}

/// The debug console state.
pub struct DebugConsole {
    /// Output entries (ring buffer).
    entries: VecDeque<ConsoleEntry>,
    /// Maximum entries to keep.
    capacity: usize,
    /// Current input line being typed.
    input_buffer: String,
    /// Cursor position within input_buffer.
    cursor_pos: usize,
    /// Command history.
    history: Vec<String>,
    /// Current history navigation index (None = new input).
    history_idx: Option<usize>,
    /// Start time for timestamps.
    start: Instant,
    /// Action requested by the last command — caller should drain this.
    pending_action: Option<ConsoleAction>,
    /// Currently selected node (set externally by the Elements panel).
    selected_node: Option<NodeId>,
}

impl DebugConsole {
    /// Create a new empty console.
    pub fn new() -> Self {
        let mut console = Self {
            entries: VecDeque::with_capacity(1024),
            capacity: 1024,
            input_buffer: String::new(),
            cursor_pos: 0,
            history: Vec::new(),
            history_idx: None,
            start: Instant::now(),
            pending_action: None,
            selected_node: None,
        };
        console.push_info("LiquiDE DevTools Console v0.1".to_string());
        console.push_info("Type 'help' for available commands.".to_string());
        console
    }

    /// Push a new entry.
    fn push_entry(&mut self, text: String, kind: ConsoleEntryKind) {
        let timestamp_ms = self.start.elapsed().as_millis() as u64;
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(ConsoleEntry {
            text,
            kind,
            timestamp_ms,
        });
    }

    /// Push an info message.
    pub fn push_info(&mut self, text: String) {
        self.push_entry(text, ConsoleEntryKind::Info);
    }

    /// Push a warning message.
    pub fn push_warning(&mut self, text: String) {
        self.push_entry(text, ConsoleEntryKind::Warning);
    }

    /// Push an error message.
    pub fn push_error(&mut self, text: String) {
        self.push_entry(text, ConsoleEntryKind::Error);
    }

    /// Take any pending action requested by the last command.
    pub fn take_pending_action(&mut self) -> Option<ConsoleAction> {
        self.pending_action.take()
    }

    /// Set the currently selected node (called by Elements panel).
    pub fn set_selected_node(&mut self, node: Option<NodeId>) {
        self.selected_node = node;
    }

    /// Push output text.
    pub fn push_output(&mut self, text: String) {
        self.push_entry(text, ConsoleEntryKind::Output);
    }

    /// Get all entries.
    pub fn entries(&self) -> &VecDeque<ConsoleEntry> {
        &self.entries
    }

    /// Get the current input buffer.
    pub fn input_buffer(&self) -> &str {
        &self.input_buffer
    }

    /// Get cursor position.
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Insert a character at cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.input_buffer.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    /// Delete character before cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input_buffer[..self.cursor_pos]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            let start = self.cursor_pos - prev;
            self.input_buffer.drain(start..self.cursor_pos);
            self.cursor_pos = start;
        }
    }

    /// Delete character at cursor (delete key).
    pub fn delete(&mut self) {
        if self.cursor_pos < self.input_buffer.len() {
            let next = self.input_buffer[self.cursor_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.input_buffer.drain(self.cursor_pos..self.cursor_pos + next);
        }
    }

    /// Move cursor left.
    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input_buffer[..self.cursor_pos]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos -= prev;
        }
    }

    /// Move cursor right.
    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.input_buffer.len() {
            let next = self.input_buffer[self.cursor_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos += next;
        }
    }

    /// Move cursor to start.
    pub fn cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to end.
    pub fn cursor_end(&mut self) {
        self.cursor_pos = self.input_buffer.len();
    }

    /// Navigate history up.
    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_idx {
            None => self.history.len() - 1,
            Some(0) => return,
            Some(i) => i - 1,
        };
        self.history_idx = Some(idx);
        self.input_buffer = self.history[idx].clone();
        self.cursor_pos = self.input_buffer.len();
    }

    /// Navigate history down.
    pub fn history_down(&mut self) {
        match self.history_idx {
            None => {}
            Some(i) if i + 1 >= self.history.len() => {
                self.history_idx = None;
                self.input_buffer.clear();
                self.cursor_pos = 0;
            }
            Some(i) => {
                self.history_idx = Some(i + 1);
                self.input_buffer = self.history[i + 1].clone();
                self.cursor_pos = self.input_buffer.len();
            }
        }
    }

    /// Submit the current input, execute the command, return true if handled.
    pub fn submit(
        &mut self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> bool {
        let input = self.input_buffer.trim().to_string();
        if input.is_empty() {
            return false;
        }

        // Echo the input.
        self.push_entry(format!("> {}", input), ConsoleEntryKind::Input);

        // Save to history.
        self.history.push(input.clone());
        self.history_idx = None;

        // Clear input.
        self.input_buffer.clear();
        self.cursor_pos = 0;

        // Execute command.
        self.pending_action = None;
        self.execute(&input, doc, layout, styles);
        true
    }

    /// Execute a console command.
    fn execute(
        &mut self,
        cmd: &str,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) {
        // Expand $0 to the currently selected node ID.
        let expanded = if cmd.contains("$0") {
            if let Some(sel) = self.selected_node {
                cmd.replace("$0", &sel.to_string())
            } else {
                self.push_error("$0: no element selected (select one in Elements panel)".into());
                return;
            }
        } else {
            cmd.to_string()
        };

        let parts: Vec<&str> = expanded.split_whitespace().collect();
        match parts.first().copied() {
            // ── Help ──
            Some("help") => {
                self.push_output("─── DOM ───".into());
                self.push_output("  dom.stats             — Document statistics".into());
                self.push_output("  dom.query <sel>       — Query nodes by tag/#id/.class".into());
                self.push_output("  dom.node <id>         — Inspect a node".into());
                self.push_output("  dom.children <id>     — List children".into());
                self.push_output("  dom.parent <id>       — Show ancestor chain".into());
                self.push_output("  dom.text <id>         — Get text content".into());
                self.push_output("  dom.tree [id] [depth] — Print subtree (default root, depth 3)".into());
                self.push_output("  dom.attrs <id>        — Show all attributes".into());
                self.push_output("  dom.classes <id>      — Show CSS classes".into());
                self.push_output("  dom.find <text>       — Search text content across all nodes".into());
                self.push_output("─── Layout ───".into());
                self.push_output("  layout.box <id>       — Show layout box".into());
                self.push_output("  layout.stats          — Layout tree statistics".into());
                self.push_output("  layout.overflow       — List nodes with overflow".into());
                self.push_output("─── Style ───".into());
                self.push_output("  style.get <id>        — Show computed styles".into());
                self.push_output("  style.prop <id> <p>   — Single property value".into());
                self.push_output("  style.search <value>  — Find nodes with a property value".into());
                self.push_output("─── Actions ───".into());
                self.push_output("  inspect <id>          — Select node in Elements panel".into());
                self.push_output("  reload                — Reload stylesheets & re-render".into());
                self.push_output("  restart               — Full UI restart".into());
                self.push_output("─── Misc ───".into());
                self.push_output("  clear / cls           — Clear console".into());
                self.push_output("  version               — Engine version info".into());
                self.push_output("  uptime                — Console session uptime".into());
                self.push_output("  history               — Show command history".into());
                self.push_output("  $0                    — Alias for selected element ID".into());
            }

            // ── Clear ──
            Some("clear") | Some("cls") => {
                self.entries.clear();
                self.push_info("Console cleared.".into());
            }

            // ── DOM commands ──
            Some("dom.stats") => self.cmd_dom_stats(doc),

            Some("dom.query") if parts.len() >= 2 => {
                let selector = parts[1..].join(" ");
                self.cmd_dom_query(doc, &selector);
            }

            Some("dom.node") if parts.len() >= 2 => {
                if let Ok(id) = parts[1].parse::<NodeId>() {
                    self.cmd_dom_node(doc, id);
                } else {
                    self.push_error(format!("Invalid node ID: {}", parts[1]));
                }
            }

            Some("dom.children") if parts.len() >= 2 => {
                if let Ok(id) = parts[1].parse::<NodeId>() {
                    self.cmd_dom_children(doc, id);
                } else {
                    self.push_error(format!("Invalid node ID: {}", parts[1]));
                }
            }

            Some("dom.parent") if parts.len() >= 2 => {
                if let Ok(id) = parts[1].parse::<NodeId>() {
                    self.cmd_dom_parent(doc, id);
                } else {
                    self.push_error(format!("Invalid node ID: {}", parts[1]));
                }
            }

            Some("dom.text") if parts.len() >= 2 => {
                if let Ok(id) = parts[1].parse::<NodeId>() {
                    self.cmd_dom_text(doc, id);
                } else {
                    self.push_error(format!("Invalid node ID: {}", parts[1]));
                }
            }

            Some("dom.tree") => {
                let node_id = parts.get(1)
                    .and_then(|s| s.parse::<NodeId>().ok())
                    .unwrap_or_else(|| doc.root());
                let depth = parts.get(2)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(3);
                self.cmd_dom_tree(doc, node_id, depth);
            }

            Some("dom.attrs") if parts.len() >= 2 => {
                if let Ok(id) = parts[1].parse::<NodeId>() {
                    self.cmd_dom_attrs(doc, id);
                } else {
                    self.push_error(format!("Invalid node ID: {}", parts[1]));
                }
            }

            Some("dom.classes") if parts.len() >= 2 => {
                if let Ok(id) = parts[1].parse::<NodeId>() {
                    self.cmd_dom_classes(doc, id);
                } else {
                    self.push_error(format!("Invalid node ID: {}", parts[1]));
                }
            }

            Some("dom.find") if parts.len() >= 2 => {
                let needle = parts[1..].join(" ");
                self.cmd_dom_find(doc, &needle);
            }

            // ── Layout commands ──
            Some("layout.box") if parts.len() >= 2 => {
                if let Ok(id) = parts[1].parse::<NodeId>() {
                    self.cmd_layout_box(layout, id);
                } else {
                    self.push_error(format!("Invalid node ID: {}", parts[1]));
                }
            }

            Some("layout.stats") => self.cmd_layout_stats(layout),

            Some("layout.overflow") => self.cmd_layout_overflow(layout),

            // ── Style commands ──
            Some("style.get") if parts.len() >= 2 => {
                if let Ok(id) = parts[1].parse::<NodeId>() {
                    self.cmd_style_get(styles, id);
                } else {
                    self.push_error(format!("Invalid node ID: {}", parts[1]));
                }
            }

            Some("style.prop") if parts.len() >= 3 => {
                if let Ok(id) = parts[1].parse::<NodeId>() {
                    self.cmd_style_prop(styles, id, parts[2]);
                } else {
                    self.push_error(format!("Invalid node ID: {}", parts[1]));
                }
            }

            Some("style.search") if parts.len() >= 2 => {
                let value = parts[1..].join(" ");
                self.cmd_style_search(styles, doc, &value);
            }

            // ── Action commands ──
            Some("inspect") if parts.len() >= 2 => {
                if let Ok(id) = parts[1].parse::<NodeId>() {
                    if doc.get(id).is_some() {
                        self.pending_action = Some(ConsoleAction::InspectNode(id));
                        self.push_info(format!("Inspecting node #{}", id));
                    } else {
                        self.push_error(format!("Node #{} not found", id));
                    }
                } else {
                    self.push_error(format!("Invalid node ID: {}", parts[1]));
                }
            }

            Some("reload") => {
                self.pending_action = Some(ConsoleAction::ReloadStyles);
                self.push_info("Stylesheet reload requested.".into());
            }

            Some("restart") => {
                self.pending_action = Some(ConsoleAction::RestartUI);
                self.push_warning("Full UI restart requested.".into());
            }

            // ── Misc commands ──
            Some("version") => {
                self.push_output("LiquiDE Engine v0.1.0".into());
                self.push_output(format!("  DOM nodes: {}", doc.node_count()));
                self.push_output(format!("  Layout boxes: {}", layout.box_count()));
                self.push_output(format!("  Console entries: {}", self.entries.len()));
            }

            Some("uptime") => {
                let elapsed = self.start.elapsed();
                let secs = elapsed.as_secs();
                let mins = secs / 60;
                let hrs = mins / 60;
                if hrs > 0 {
                    self.push_output(format!("Uptime: {}h {}m {}s", hrs, mins % 60, secs % 60));
                } else if mins > 0 {
                    self.push_output(format!("Uptime: {}m {}s", mins, secs % 60));
                } else {
                    self.push_output(format!("Uptime: {}s", secs));
                }
            }

            Some("history") => {
                if self.history.is_empty() {
                    self.push_output("No command history.".into());
                } else {
                    let count = self.history.len();
                    let lines: Vec<String> = self.history.iter().enumerate()
                        .map(|(i, h)| format!("  [{}] {}", i, h))
                        .collect();
                    self.push_output(format!("History ({} entries):", count));
                    for line in lines {
                        self.push_output(line);
                    }
                }
            }

            Some(unknown) => {
                self.push_error(format!(
                    "Unknown command: '{}'. Type 'help' for available commands.",
                    unknown
                ));
            }
            None => {}
        }
    }

    // ── Command implementations ──

    fn cmd_dom_stats(&mut self, doc: &Document) {
        let total = doc.node_count();
        self.push_output(format!("Document: {} nodes total", total));
        let root = doc.root();
        self.push_output(format!("Root node: #{}", root));
    }

    fn cmd_dom_query(&mut self, doc: &Document, selector: &str) {
        // Simple query: search by tag name, #id, or .class.
        let mut matches = Vec::new();
        let root = doc.root();
        let all_ids = doc.descendants(root);
        for id in std::iter::once(root).chain(all_ids) {
            if let Some(node) = doc.get(id) {
                let tag = node.tag_name();
                let eid = node.element_id.as_deref().unwrap_or("");

                let matched = if selector.starts_with('#') {
                    eid == &selector[1..]
                } else if selector.starts_with('.') {
                    node.classes.iter().any(|c| c == &selector[1..])
                } else {
                    tag == selector
                };

                if matched {
                    matches.push(id);
                }
            }
        }

        if matches.is_empty() {
            self.push_output(format!("No nodes matching '{}'", selector));
        } else {
            self.push_output(format!("Found {} node(s):", matches.len()));
            for (i, id) in matches.iter().enumerate().take(20) {
                if let Some(node) = doc.get(*id) {
                    let tag = node.tag_name();
                    let eid = node.element_id.as_deref().unwrap_or("");
                    self.push_output(format!(
                        "  [{}] #{} <{}{}>",
                        i,
                        id,
                        tag,
                        if eid.is_empty() {
                            String::new()
                        } else {
                            format!(" id=\"{}\"", eid)
                        }
                    ));
                }
            }
            if matches.len() > 20 {
                self.push_output(format!("  ... and {} more", matches.len() - 20));
            }
        }
    }

    fn cmd_dom_node(&mut self, doc: &Document, node_id: NodeId) {
        if let Some(node) = doc.get(node_id) {
            self.push_output(format!("Node #{}:", node_id));
            self.push_output(format!("  tag: {}", node.tag_name()));
            if let Some(eid) = &node.element_id {
                self.push_output(format!("  id: {}", eid));
            }
            if !node.classes.is_empty() {
                let cls: Vec<&str> = node.classes.iter().collect();
                self.push_output(format!("  classes: {}", cls.join(", ")));
            }
            if !node.attrs.is_empty() {
                self.push_output("  attributes:".into());
                for (k, v) in node.attrs.iter() {
                    self.push_output(format!("    {}=\"{}\"", k, v));
                }
            }
            if let Some(parent) = doc.parent(node_id) {
                self.push_output(format!("  parent: #{}", parent));
            }
            let children = doc.children(node_id);
            self.push_output(format!("  children: {} node(s)", children.len()));
        } else {
            self.push_error(format!("Node #{} not found", node_id));
        }
    }

    fn cmd_dom_children(&mut self, doc: &Document, node_id: NodeId) {
        let children = doc.children(node_id);
        if children.is_empty() {
            self.push_output(format!("Node #{} has no children", node_id));
        } else {
            self.push_output(format!("Children of #{} ({}):", node_id, children.len()));
            for (i, child) in children.iter().enumerate().take(30) {
                if let Some(node) = doc.get(*child) {
                    let tag = node.tag_name();
                    self.push_output(format!("  [{}] #{} <{}>", i, child, tag));
                }
            }
        }
    }

    fn cmd_dom_text(&mut self, doc: &Document, node_id: NodeId) {
        if let Some(node) = doc.get(node_id) {
            if let Some(text) = node.text_content() {
                let display = if text.len() > 200 {
                    format!("{}...", &text[..200])
                } else {
                    text.to_string()
                };
                self.push_output(format!("Text: \"{}\"", display));
            } else {
                self.push_output("Node has no text content.".into());
            }
        } else {
            self.push_error(format!("Node #{} not found", node_id));
        }
    }

    fn cmd_layout_box(&mut self, layout: &LayoutTree, node_id: NodeId) {
        if let Some(b) = layout.find_by_node(node_id) {
            let box_id = b.id;
            let ac = layout.absolute_content_rect(box_id);
            let ap = layout.absolute_padding_rect(box_id);
            let ab = layout.absolute_border_rect(box_id);
            let am = layout.absolute_margin_rect(box_id);
            self.push_output(format!("Layout for node #{}:", node_id));
            self.push_output(format!(
                "  Content:  ({:.1}, {:.1}) {:.1} × {:.1}",
                ac.x, ac.y, ac.width, ac.height
            ));
            self.push_output(format!(
                "  Padding:  ({:.1}, {:.1}) {:.1} × {:.1}",
                ap.x, ap.y, ap.width, ap.height
            ));
            self.push_output(format!(
                "  Border:   ({:.1}, {:.1}) {:.1} × {:.1}",
                ab.x, ab.y, ab.width, ab.height
            ));
            self.push_output(format!(
                "  Margin:   ({:.1}, {:.1}) {:.1} × {:.1}",
                am.x, am.y, am.width, am.height
            ));
            self.push_output(format!("  Box type: {:?}", b.box_type));
            self.push_output(format!("  Children: {}", b.children.len()));
            if let Some(ref sz) = b.scroll_size {
                self.push_output(format!("  Scroll:   {:.1} × {:.1}", sz.width, sz.height));
            }
        } else {
            self.push_error(format!("No layout box for node #{}", node_id));
        }
    }

    fn cmd_layout_stats(&mut self, layout: &LayoutTree) {
        self.push_output(format!("Layout tree: {} boxes", layout.box_count()));
    }

    fn cmd_style_get(&mut self, styles: &StyleMap, node_id: NodeId) {
        use crate::style_panel::StyleInspector;
        let mut tmp = StyleInspector::new();
        tmp.inspect(node_id, styles);
        let props = tmp.visible_properties();
        if props.is_empty() {
            self.push_error(format!("No computed styles for node #{}", node_id));
        } else {
            self.push_output(format!("Computed styles for #{} ({} props):", node_id, props.len()));
            for p in props.iter().take(40) {
                let inh = if p.inherited { " (inherited)" } else { "" };
                self.push_output(format!("  {}: {}{}", p.name, p.value, inh));
            }
            if props.len() > 40 {
                self.push_output(format!("  ... and {} more", props.len() - 40));
            }
        }
    }

    fn cmd_style_prop(&mut self, styles: &StyleMap, node_id: NodeId, prop_name: &str) {
        use crate::style_panel::StyleInspector;
        let mut tmp = StyleInspector::new();
        tmp.inspect(node_id, styles);
        let props = tmp.visible_properties();
        if let Some(p) = props.iter().find(|p| p.name == prop_name) {
            self.push_output(format!("{}: {}", p.name, p.value));
        } else {
            self.push_error(format!("Property '{}' not found for node #{}", prop_name, node_id));
        }
    }

    // ── New command implementations ──

    fn cmd_dom_parent(&mut self, doc: &Document, node_id: NodeId) {
        if doc.get(node_id).is_none() {
            self.push_error(format!("Node #{} not found", node_id));
            return;
        }
        let ancestors = doc.ancestors(node_id);
        if ancestors.is_empty() {
            self.push_output(format!("Node #{} has no parent (root)", node_id));
        } else {
            self.push_output(format!("Ancestor chain for #{}:", node_id));
            for (depth, anc) in ancestors.iter().enumerate() {
                if let Some(node) = doc.get(*anc) {
                    let tag = node.tag_name();
                    let eid = node
                        .element_id
                        .as_deref()
                        .map(|e| format!(" id=\"{}\"", e))
                        .unwrap_or_default();
                    self.push_output(format!(
                        "  {}<{}{}> #{}",
                        "  ".repeat(depth),
                        tag,
                        eid,
                        anc
                    ));
                }
            }
        }
    }

    fn cmd_dom_tree(&mut self, doc: &Document, root: NodeId, max_depth: usize) {
        self.push_output(format!("DOM tree from #{} (depth {}):", root, max_depth));
        self.dom_tree_walk(doc, root, 0, max_depth);
    }

    fn dom_tree_walk(&mut self, doc: &Document, node_id: NodeId, depth: usize, max_depth: usize) {
        if depth > max_depth {
            return;
        }
        if let Some(node) = doc.get(node_id) {
            let tag = node.tag_name();
            let eid = node
                .element_id
                .as_deref()
                .map(|e| format!(" id=\"{}\"", e))
                .unwrap_or_default();
            let cls = if node.classes.is_empty() {
                String::new()
            } else {
                let names: Vec<&str> = node.classes.iter().collect();
                format!(" .{}", names.join("."))
            };
            let indent = "  ".repeat(depth + 1);
            self.push_output(format!("{}<{}{}{}> #{}", indent, tag, eid, cls, node_id));

            let children = doc.children(node_id);
            if children.len() > 50 && depth < max_depth {
                // Too many children — summarise
                for child in children.iter().take(10) {
                    self.dom_tree_walk(doc, *child, depth + 1, max_depth);
                }
                self.push_output(format!(
                    "{}... and {} more children",
                    "  ".repeat(depth + 2),
                    children.len() - 10
                ));
            } else {
                for child in children {
                    self.dom_tree_walk(doc, *child, depth + 1, max_depth);
                }
            }
        }
    }

    fn cmd_dom_attrs(&mut self, doc: &Document, node_id: NodeId) {
        if let Some(node) = doc.get(node_id) {
            if node.attrs.is_empty() {
                self.push_output(format!("Node #{} has no attributes", node_id));
            } else {
                self.push_output(format!("Attributes of #{} ({}):", node_id, node.attrs.len()));
                for (k, v) in node.attrs.iter() {
                    self.push_output(format!("  {}=\"{}\"", k, v));
                }
            }
        } else {
            self.push_error(format!("Node #{} not found", node_id));
        }
    }

    fn cmd_dom_classes(&mut self, doc: &Document, node_id: NodeId) {
        if let Some(node) = doc.get(node_id) {
            if node.classes.is_empty() {
                self.push_output(format!("Node #{} has no CSS classes", node_id));
            } else {
                let cls: Vec<&str> = node.classes.iter().collect();
                self.push_output(format!("Classes of #{}: {}", node_id, cls.join(", ")));
            }
        } else {
            self.push_error(format!("Node #{} not found", node_id));
        }
    }

    fn cmd_dom_find(&mut self, doc: &Document, needle: &str) {
        let lower = needle.to_lowercase();
        let root = doc.root();
        let all_ids = doc.descendants(root);
        let mut found = Vec::new();

        for id in std::iter::once(root).chain(all_ids) {
            if let Some(node) = doc.get(id) {
                if let Some(text) = node.text_content() {
                    if text.to_lowercase().contains(&lower) {
                        let preview = if text.len() > 60 {
                            format!("{}...", &text[..60])
                        } else {
                            text.to_string()
                        };
                        found.push((id, node.tag_name().to_string(), preview));
                    }
                }
            }
        }

        if found.is_empty() {
            self.push_output(format!("No text nodes containing '{}'", needle));
        } else {
            self.push_output(format!("Found {} node(s) containing '{}':", found.len(), needle));
            for (id, tag, preview) in found.iter().take(20) {
                self.push_output(format!("  #{} <{}> \"{}\"", id, tag, preview));
            }
            if found.len() > 20 {
                self.push_output(format!("  ... and {} more", found.len() - 20));
            }
        }
    }

    fn cmd_layout_overflow(&mut self, layout: &LayoutTree) {
        let mut overflow_nodes = Vec::new();
        for b in &layout.boxes {
            if let Some(ref sz) = b.scroll_size {
                let exceeds_w = sz.width > b.content_rect.width + 1.0;
                let exceeds_h = sz.height > b.content_rect.height + 1.0;
                if exceeds_w || exceeds_h {
                    overflow_nodes.push((
                        b.node,
                        b.content_rect.width,
                        b.content_rect.height,
                        sz.width,
                        sz.height,
                    ));
                }
            }
        }

        if overflow_nodes.is_empty() {
            self.push_output("No boxes with scroll overflow detected.".into());
        } else {
            self.push_output(format!("{} box(es) with overflow:", overflow_nodes.len()));
            for (nid, cw, ch, sw, sh) in overflow_nodes.iter().take(20) {
                self.push_output(format!(
                    "  node #{}: content {:.0}×{:.0}, scroll {:.0}×{:.0}",
                    nid, cw, ch, sw, sh
                ));
            }
        }
    }

    fn cmd_style_search(&mut self, styles: &StyleMap, doc: &Document, value: &str) {
        use crate::style_panel::StyleInspector;
        let lower = value.to_lowercase();
        let root = doc.root();
        let all_ids = doc.descendants(root);
        let mut found = Vec::new();

        for id in std::iter::once(root).chain(all_ids) {
            let mut inspector = StyleInspector::new();
            inspector.inspect(id, styles);
            for prop in inspector.visible_properties() {
                if prop.value.to_lowercase().contains(&lower)
                    || prop.name.to_lowercase().contains(&lower)
                {
                    found.push((id, prop.name.clone(), prop.value.clone()));
                }
            }
        }

        if found.is_empty() {
            self.push_output(format!("No properties matching '{}'", value));
        } else {
            self.push_output(format!("Found {} match(es) for '{}':", found.len(), value));
            for (id, name, val) in found.iter().take(30) {
                self.push_output(format!("  #{}: {} = {}", id, name, val));
            }
            if found.len() > 30 {
                self.push_output(format!("  ... and {} more", found.len() - 30));
            }
        }
    }

    /// Total entry count.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the console is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.push_info("Console cleared.".into());
    }
}

impl Default for DebugConsole {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_creation() {
        let console = DebugConsole::new();
        assert_eq!(console.entries().len(), 2); // welcome + help hint
        assert!(console.input_buffer().is_empty());
    }

    #[test]
    fn test_console_input() {
        let mut console = DebugConsole::new();
        console.insert_char('h');
        console.insert_char('e');
        console.insert_char('l');
        console.insert_char('p');
        assert_eq!(console.input_buffer(), "help");
        assert_eq!(console.cursor_pos(), 4);
    }

    #[test]
    fn test_console_backspace() {
        let mut console = DebugConsole::new();
        console.insert_char('a');
        console.insert_char('b');
        console.backspace();
        assert_eq!(console.input_buffer(), "a");
    }

    #[test]
    fn test_console_submit() {
        let mut console = DebugConsole::new();
        console.insert_char('h');
        console.insert_char('e');
        console.insert_char('l');
        console.insert_char('p');
        let doc = Document::new();
        let layout = LayoutTree::new();
        let styles = StyleMap::new();
        assert!(console.submit(&doc, &layout, &styles));
        // Should have: welcome, hint, "> help", plus output lines
        assert!(console.entries().len() > 4);
    }

    #[test]
    fn test_console_history() {
        let mut console = DebugConsole::new();
        let doc = Document::new();
        let layout = LayoutTree::new();
        let styles = StyleMap::new();

        console.insert_char('a');
        console.submit(&doc, &layout, &styles);
        console.insert_char('b');
        console.submit(&doc, &layout, &styles);

        console.history_up();
        assert_eq!(console.input_buffer(), "b");
        console.history_up();
        assert_eq!(console.input_buffer(), "a");
        console.history_down();
        assert_eq!(console.input_buffer(), "b");
    }
}
