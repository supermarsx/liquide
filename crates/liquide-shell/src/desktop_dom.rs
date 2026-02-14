//! Desktop DOM — builds the element tree for all shell surfaces.
//!
//! Every shell element (statusbar, dock, dock items, context menus,
//! notifications, windows) is represented as a DOM node. The
//! [`DesktopDocument`] wraps a [`Document`] and exposes well-typed helpers
//! for the shell to manipulate the tree without raw node-id juggling.
//!
//! ## Tree structure
//!
//! ```text
//! <root>
//! ├── <desktop-background>
//! ├── <statusbar>
//! │   ├── <statusbar-slot class="left">
//! │   │   └── (items …)
//! │   ├── <statusbar-slot class="center">
//! │   │   └── <statusbar-item id="clock">
//! │   └── <statusbar-slot class="right">
//! │       ├── <statusbar-item id="notifications">
//! │       ├── <statusbar-item id="connection">
//! │       ├── <statusbar-item id="tray">
//! │       └── <statusbar-item id="session">
//! ├── <workspace-container>
//! │   └── <window> … </window>
//! ├── <dock>
//! │   └── <dock-item class="active"> … </dock-item>
//! └── <notification> … </notification>
//! ```

use liquide_dom::{Document, NodeId, PseudoStateFlags};
use tracing::debug;

/// Well-known element IDs for shell surfaces.
pub mod element_ids {
    pub const DESKTOP_BG: &str = "desktop-bg";
    pub const STATUSBAR: &str = "shell-statusbar";
    pub const STATUSBAR_SLOT_LEFT: &str = "statusbar-slot-left";
    pub const STATUSBAR_SLOT_CENTER: &str = "statusbar-slot-center";
    pub const STATUSBAR_SLOT_RIGHT: &str = "statusbar-slot-right";
    pub const WORKSPACE: &str = "workspace-container";
    pub const DOCK: &str = "shell-dock";
}

/// Desktop document wrapping a DOM tree with shell-element accessors.
pub struct DesktopDocument {
    /// The underlying DOM tree.
    pub doc: Document,
    /// The `<desktop-background>` node.
    pub desktop_bg: NodeId,
    /// The `<statusbar>` node.
    pub statusbar: NodeId,
    /// The left / center / right statusbar flex containers.
    pub statusbar_slot_left: NodeId,
    pub statusbar_slot_center: NodeId,
    pub statusbar_slot_right: NodeId,
    /// The `<workspace-container>` node.
    pub workspace: NodeId,
    /// The `<dock>` node.
    pub dock: NodeId,
}

impl DesktopDocument {
    /// Build the initial desktop DOM with empty containers.
    pub fn new() -> Self {
        let mut doc = Document::new();
        let root = doc.root();

        // <desktop-background>
        let desktop_bg = doc.create_element("desktop-background");
        doc.set_id(desktop_bg, element_ids::DESKTOP_BG);
        doc.append_child(root, desktop_bg);

        // <statusbar> with three flex slots
        let statusbar = doc.create_element("statusbar");
        doc.set_id(statusbar, element_ids::STATUSBAR);
        doc.append_child(root, statusbar);

        let slot_left = doc.create_element("statusbar-slot");
        doc.set_id(slot_left, element_ids::STATUSBAR_SLOT_LEFT);
        doc.add_class(slot_left, "left");
        doc.append_child(statusbar, slot_left);

        let slot_center = doc.create_element("statusbar-slot");
        doc.set_id(slot_center, element_ids::STATUSBAR_SLOT_CENTER);
        doc.add_class(slot_center, "center");
        doc.append_child(statusbar, slot_center);

        let slot_right = doc.create_element("statusbar-slot");
        doc.set_id(slot_right, element_ids::STATUSBAR_SLOT_RIGHT);
        doc.add_class(slot_right, "right");
        doc.append_child(statusbar, slot_right);

        // <workspace-container>
        let workspace = doc.create_element("workspace-container");
        doc.set_id(workspace, element_ids::WORKSPACE);
        doc.append_child(root, workspace);

        // <dock>
        let dock = doc.create_element("dock");
        doc.set_id(dock, element_ids::DOCK);
        doc.append_child(root, dock);

        debug!(
            nodes = doc.node_count(),
            "DesktopDocument: initial tree built"
        );

        Self {
            doc,
            desktop_bg,
            statusbar,
            statusbar_slot_left: slot_left,
            statusbar_slot_center: slot_center,
            statusbar_slot_right: slot_right,
            workspace,
            dock,
        }
    }

    // ── Dock helpers ──────────────────────────────────────────────

    /// Clear and rebuild the dock items subtree from the current dock data.
    ///
    /// Each item becomes a `<dock-item>` with an optional `.active` class
    /// and `data-app-id`, `data-label` attributes.
    pub fn sync_dock_items(&mut self, items: &[DockItemInfo]) {
        // Remove stale children
        let old_children: Vec<NodeId> = self.doc.children(self.dock).to_vec();
        for child in old_children {
            self.doc.remove_child(self.dock, child);
            self.doc.destroy_node(child);
        }

        // Add fresh items
        for (i, item) in items.iter().enumerate() {
            let el = self.doc.create_element("dock-item");
            if item.is_running {
                self.doc.add_class(el, "active");
            }
            if item.is_pinned {
                self.doc.add_class(el, "pinned");
            }
            self.doc.set_attribute(el, "data-app-id", &item.app_id);
            self.doc.set_attribute(el, "data-label", &item.label);
            self.doc.set_attribute(el, "data-icon", &item.icon);
            self.doc
                .set_attribute(el, "data-index", &i.to_string());

            // Text child with the label (for accessibility / CSS content)
            let txt = self.doc.create_text(&item.label);
            self.doc.append_child(el, txt);

            self.doc.append_child(self.dock, el);
        }
    }

    /// Set the hover pseudo-state on a dock item by index.
    pub fn set_dock_hover(&mut self, index: Option<usize>) {
        let children: Vec<NodeId> = self.doc.children(self.dock).to_vec();
        for (i, &child) in children.iter().enumerate() {
            let hover = index == Some(i);
            self.doc
                .set_pseudo_state(child, PseudoStateFlags::HOVER, hover);
        }
    }

    // ── Status-bar helpers ───────────────────────────────────────

    /// Set / update a status-bar item in the given slot.
    ///
    /// If an item with the given `id` already exists it is updated in place;
    /// otherwise a new `<statusbar-item>` is appended to the slot.
    pub fn set_statusbar_item(
        &mut self,
        slot: StatusBarSlotKind,
        item_id: &str,
        text: &str,
        classes: &[&str],
    ) {
        let parent = match slot {
            StatusBarSlotKind::Left => self.statusbar_slot_left,
            StatusBarSlotKind::Center => self.statusbar_slot_center,
            StatusBarSlotKind::Right => self.statusbar_slot_right,
        };

        // Try to find existing item
        if let Some(existing) = self.doc.get_element_by_id(item_id) {
            // Update text of first child
            let kids: Vec<NodeId> = self.doc.children(existing).to_vec();
            if let Some(&txt_node) = kids.first() {
                self.doc.set_text_content(txt_node, text);
            }
            return;
        }

        // Create new
        let el = self.doc.create_element("statusbar-item");
        self.doc.set_id(el, item_id);
        for cls in classes {
            self.doc.add_class(el, cls);
        }
        let txt = self.doc.create_text(text);
        self.doc.append_child(el, txt);
        self.doc.append_child(parent, el);
    }

    /// Populate default status-bar items matching `ShellStatusBar` defaults.
    pub fn populate_default_statusbar(&mut self) {
        self.set_statusbar_item(StatusBarSlotKind::Center, "clock", "00:00", &[]);
        self.set_statusbar_item(StatusBarSlotKind::Right, "notifications", "", &[]);
        self.set_statusbar_item(StatusBarSlotKind::Right, "connection", "", &[]);
        self.set_statusbar_item(StatusBarSlotKind::Right, "tray", "", &[]);
        self.set_statusbar_item(StatusBarSlotKind::Right, "session", "", &[]);
    }

    // ── Window helpers ───────────────────────────────────────────

    /// Add a window to the workspace container.
    ///
    /// Returns the `<window>` node id. The caller can further manipulate
    /// children (titlebar, content) through this id.
    pub fn add_window(&mut self, window_id: &str, title: &str, focused: bool) -> NodeId {
        let el = self.doc.create_element("window");
        self.doc.set_id(el, window_id);
        if focused {
            self.doc.add_class(el, "focused");
            self.doc
                .set_pseudo_state(el, PseudoStateFlags::FOCUS, true);
        }

        // <window-titlebar>
        let titlebar = self.doc.create_element("window-titlebar");
        let title_txt = self.doc.create_text(title);
        self.doc.append_child(titlebar, title_txt);
        self.doc.append_child(el, titlebar);

        // <window-content>
        let content = self.doc.create_element("window-content");
        self.doc.append_child(el, content);

        self.doc.append_child(self.workspace, el);
        el
    }

    /// Remove a window by its element id.
    pub fn remove_window(&mut self, window_id: &str) {
        if let Some(node_id) = self.doc.get_element_by_id(window_id) {
            self.doc.remove_child(self.workspace, node_id);
            self.doc.destroy_node(node_id);
        }
    }

    /// Update focus state on all windows.
    pub fn set_focused_window(&mut self, focused_id: Option<&str>) {
        let children: Vec<NodeId> = self.doc.children(self.workspace).to_vec();
        for child in children {
            let is_focused = self
                .doc
                .get(child)
                .and_then(|n| n.element_id.as_deref())
                .map_or(false, |eid| Some(eid) == focused_id);
            if is_focused {
                self.doc.add_class(child, "focused");
                self.doc
                    .set_pseudo_state(child, PseudoStateFlags::FOCUS, true);
            } else {
                self.doc.remove_class(child, "focused");
                self.doc
                    .set_pseudo_state(child, PseudoStateFlags::FOCUS, false);
            }
        }
    }

    // ── Context menu helpers ─────────────────────────────────────

    /// Add a context menu overlay to the DOM.
    pub fn add_context_menu(
        &mut self,
        menu_id: &str,
        items: &[ContextMenuItemInfo],
    ) -> NodeId {
        let el = self.doc.create_element("context-menu");
        self.doc.set_id(el, menu_id);
        let root = self.doc.root();

        for (i, item) in items.iter().enumerate() {
            match item {
                ContextMenuItemInfo::Action { label, disabled } => {
                    let mi = self.doc.create_element("menu-item");
                    self.doc
                        .set_attribute(mi, "data-index", &i.to_string());
                    if *disabled {
                        self.doc.add_class(mi, "disabled");
                        self.doc.set_pseudo_state(
                            mi,
                            PseudoStateFlags::DISABLED,
                            true,
                        );
                    }
                    let txt = self.doc.create_text(label);
                    self.doc.append_child(mi, txt);
                    self.doc.append_child(el, mi);
                }
                ContextMenuItemInfo::Separator => {
                    let sep = self.doc.create_element("menu-separator");
                    self.doc.append_child(el, sep);
                }
            }
        }

        self.doc.append_child(root, el);
        el
    }

    /// Remove a context menu by id.
    pub fn remove_context_menu(&mut self, menu_id: &str) {
        if let Some(node_id) = self.doc.get_element_by_id(menu_id) {
            let root = self.doc.root();
            self.doc.remove_child(root, node_id);
            self.doc.destroy_node(node_id);
        }
    }

    // ── Notification helpers ─────────────────────────────────────

    /// Add a notification toast to the DOM.
    pub fn add_notification(&mut self, notif_id: &str, title: &str, body: &str) -> NodeId {
        let el = self.doc.create_element("notification");
        self.doc.set_id(el, notif_id);

        let title_el = self.doc.create_element("notification-title");
        let title_txt = self.doc.create_text(title);
        self.doc.append_child(title_el, title_txt);
        self.doc.append_child(el, title_el);

        let body_el = self.doc.create_element("notification-body");
        let body_txt = self.doc.create_text(body);
        self.doc.append_child(body_el, body_txt);
        self.doc.append_child(el, body_el);

        let root = self.doc.root();
        self.doc.append_child(root, el);
        el
    }

    /// Remove a notification by id.
    pub fn remove_notification(&mut self, notif_id: &str) {
        if let Some(node_id) = self.doc.get_element_by_id(notif_id) {
            let root = self.doc.root();
            self.doc.remove_child(root, node_id);
            self.doc.destroy_node(node_id);
        }
    }
}

impl Default for DesktopDocument {
    fn default() -> Self {
        Self::new()
    }
}

// ── Lightweight info structs (no dependency on dock/statusbar crates) ──

/// Minimal dock item info for DOM sync.
#[derive(Debug, Clone)]
pub struct DockItemInfo {
    pub app_id: String,
    pub label: String,
    pub icon: String,
    pub is_running: bool,
    pub is_pinned: bool,
}

/// Slot kind for status bar items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarSlotKind {
    Left,
    Center,
    Right,
}

/// Minimal context menu item info for DOM construction.
#[derive(Debug, Clone)]
pub enum ContextMenuItemInfo {
    Action { label: String, disabled: bool },
    Separator,
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_tree_structure() {
        let desktop = DesktopDocument::new();
        let doc = &desktop.doc;

        // Root has 4 children: desktop-bg, statusbar, workspace, dock
        assert_eq!(doc.children(doc.root()).len(), 4);

        // Statusbar has 3 slots
        assert_eq!(doc.children(desktop.statusbar).len(), 3);
    }

    #[test]
    fn sync_dock_items() {
        let mut desktop = DesktopDocument::new();
        let items = vec![
            DockItemInfo {
                app_id: "files".into(),
                label: "Files".into(),
                icon: "folder".into(),
                is_running: true,
                is_pinned: true,
            },
            DockItemInfo {
                app_id: "terminal".into(),
                label: "Terminal".into(),
                icon: "terminal".into(),
                is_running: false,
                is_pinned: true,
            },
        ];

        desktop.sync_dock_items(&items);
        assert_eq!(desktop.doc.children(desktop.dock).len(), 2);

        // First item should have .active class
        let first = desktop.doc.children(desktop.dock)[0];
        assert!(desktop.doc.get(first).unwrap().has_class("active"));
        assert!(desktop.doc.get(first).unwrap().has_class("pinned"));

        // Second item should NOT have .active
        let second = desktop.doc.children(desktop.dock)[1];
        assert!(!desktop.doc.get(second).unwrap().has_class("active"));
    }

    #[test]
    fn statusbar_items() {
        let mut desktop = DesktopDocument::new();
        desktop.populate_default_statusbar();

        // Center slot should have "clock"
        let center_kids = desktop.doc.children(desktop.statusbar_slot_center);
        assert_eq!(center_kids.len(), 1);
        assert_eq!(
            desktop.doc.get(center_kids[0]).unwrap().element_id.as_deref(),
            Some("clock")
        );

        // Right slot should have 4 items
        let right_kids = desktop.doc.children(desktop.statusbar_slot_right);
        assert_eq!(right_kids.len(), 4);
    }

    #[test]
    fn window_management() {
        let mut desktop = DesktopDocument::new();
        let win = desktop.add_window("win-1", "Test App", true);
        assert!(desktop.doc.get(win).unwrap().has_class("focused"));

        // Window has titlebar + content children
        let kids = desktop.doc.children(win);
        assert_eq!(kids.len(), 2);

        desktop.remove_window("win-1");
        assert!(desktop.doc.get_element_by_id("win-1").is_none());
    }

    #[test]
    fn context_menu() {
        let mut desktop = DesktopDocument::new();
        let items = vec![
            ContextMenuItemInfo::Action {
                label: "Copy".into(),
                disabled: false,
            },
            ContextMenuItemInfo::Separator,
            ContextMenuItemInfo::Action {
                label: "Paste".into(),
                disabled: true,
            },
        ];

        let menu = desktop.add_context_menu("ctx-1", &items);
        let kids = desktop.doc.children(menu);
        assert_eq!(kids.len(), 3);

        desktop.remove_context_menu("ctx-1");
        assert!(desktop.doc.get_element_by_id("ctx-1").is_none());
    }

    #[test]
    fn notification() {
        let mut desktop = DesktopDocument::new();
        let n = desktop.add_notification("notif-1", "Alert", "Something happened");
        assert_eq!(desktop.doc.children(n).len(), 2);

        desktop.remove_notification("notif-1");
        assert!(desktop.doc.get_element_by_id("notif-1").is_none());
    }

    #[test]
    fn dock_hover_state() {
        let mut desktop = DesktopDocument::new();
        let items = vec![
            DockItemInfo {
                app_id: "a".into(),
                label: "A".into(),
                icon: "a".into(),
                is_running: false,
                is_pinned: false,
            },
            DockItemInfo {
                app_id: "b".into(),
                label: "B".into(),
                icon: "b".into(),
                is_running: false,
                is_pinned: false,
            },
        ];
        desktop.sync_dock_items(&items);
        desktop.set_dock_hover(Some(1));

        let kids: Vec<NodeId> = desktop.doc.children(desktop.dock).to_vec();
        assert!(!desktop.doc.get(kids[0]).unwrap().has_pseudo_state(PseudoStateFlags::HOVER));
        assert!(desktop.doc.get(kids[1]).unwrap().has_pseudo_state(PseudoStateFlags::HOVER));
    }
}
