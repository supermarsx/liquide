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
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        match parts.first().copied() {
            Some("help") => {
                self.push_output("Available commands:".into());
                self.push_output("  help              — Show this help".into());
                self.push_output("  clear             — Clear console".into());
                self.push_output("  dom.stats         — Document statistics".into());
                self.push_output("  dom.query <sel>   — Query nodes by tag/id/class".into());
                self.push_output("  dom.node <id>     — Inspect a specific node".into());
                self.push_output("  dom.children <id> — List children of a node".into());
                self.push_output("  dom.text <id>     — Get text content of a node".into());
                self.push_output("  layout.box <id>   — Show layout box for node".into());
                self.push_output("  layout.stats      — Layout tree statistics".into());
                self.push_output("  style.get <id>    — Show computed styles".into());
                self.push_output("  style.prop <id> <name> — Get a specific property".into());
            }
            Some("clear") => {
                self.entries.clear();
                self.push_info("Console cleared.".into());
            }
            Some("dom.stats") => {
                self.cmd_dom_stats(doc);
            }
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
            Some("dom.text") if parts.len() >= 2 => {
                if let Ok(id) = parts[1].parse::<NodeId>() {
                    self.cmd_dom_text(doc, id);
                } else {
                    self.push_error(format!("Invalid node ID: {}", parts[1]));
                }
            }
            Some("layout.box") if parts.len() >= 2 => {
                if let Ok(id) = parts[1].parse::<NodeId>() {
                    self.cmd_layout_box(layout, id);
                } else {
                    self.push_error(format!("Invalid node ID: {}", parts[1]));
                }
            }
            Some("layout.stats") => {
                self.cmd_layout_stats(layout);
            }
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
            Some(unknown) => {
                self.push_error(format!("Unknown command: '{}'. Type 'help' for available commands.", unknown));
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
            self.push_output(format!("Layout for node #{}:", node_id));
            self.push_output(format!(
                "  Content:  ({:.1}, {:.1}) {:.1} × {:.1}",
                b.content_rect.x, b.content_rect.y, b.content_rect.width, b.content_rect.height
            ));
            self.push_output(format!(
                "  Padding:  ({:.1}, {:.1}) {:.1} × {:.1}",
                b.padding_rect.x, b.padding_rect.y, b.padding_rect.width, b.padding_rect.height
            ));
            self.push_output(format!(
                "  Border:   ({:.1}, {:.1}) {:.1} × {:.1}",
                b.border_rect.x, b.border_rect.y, b.border_rect.width, b.border_rect.height
            ));
            self.push_output(format!(
                "  Margin:   ({:.1}, {:.1}) {:.1} × {:.1}",
                b.margin_rect.x, b.margin_rect.y, b.margin_rect.width, b.margin_rect.height
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
