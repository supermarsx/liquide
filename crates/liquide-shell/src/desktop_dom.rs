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
//! │   ├── <statusbar-slot class="left"> … </statusbar-slot>
//! │   ├── <statusbar-slot class="center"> … </statusbar-slot>
//! │   └── <statusbar-slot class="right"> … </statusbar-slot>
//! ├── <workspace-container>
//! │   └── <window>
//! │       ├── <window-titlebar>
//! │       │   ├── <window-title> … </window-title>
//! │       │   └── <titlebar-buttons>
//! │       │       ├── <minimize-button />
//! │       │       ├── <maximize-button />
//! │       │       └── <close-button />
//! │       └── <window-content />
//! ├── <dock>
//! │   └── <dock-item class="active"> … </dock-item>
//! ├── <notification-area>
//! │   └── <notification> … </notification>
//! ├── (on demand) <launcher-overlay> → <launcher> → …
//! ├── (on demand) <session-menu> → <menu-item> …
//! ├── (on demand) <app-menu> → <menu-item> …
//! └── (on demand) <context-menu> → <menu-item> / <menu-separator>
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
    pub const LAUNCHER_OVERLAY: &str = "launcher-overlay";
    pub const LAUNCHER: &str = "shell-launcher";
    pub const LAUNCHER_SEARCH: &str = "launcher-search";
    pub const SESSION_MENU: &str = "session-menu";
    pub const APP_MENU: &str = "app-menu";
    pub const NOTIFICATION_AREA: &str = "notification-area";
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
    /// The `<notification-area>` persistent container.
    pub notification_area: NodeId,
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

        // <notification-area> — persistent container for toast notifications
        let notification_area = doc.create_element("notification-area");
        doc.set_id(notification_area, element_ids::NOTIFICATION_AREA);
        doc.append_child(root, notification_area);

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
            notification_area,
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

        // Titlebar text
        let title_span = self.doc.create_element("window-title");
        let title_txt = self.doc.create_text(title);
        self.doc.append_child(title_span, title_txt);
        self.doc.append_child(titlebar, title_span);

        // Titlebar button group
        let btn_group = self.doc.create_element("titlebar-buttons");
        for btn_tag in &["minimize-button", "maximize-button", "close-button"] {
            let btn = self.doc.create_element(btn_tag);
            self.doc.append_child(btn_group, btn);
        }
        self.doc.append_child(titlebar, btn_group);

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

    /// Add a notification toast to the notification-area container.
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

        self.doc.append_child(self.notification_area, el);
        el
    }

    /// Remove a notification by id.
    pub fn remove_notification(&mut self, notif_id: &str) {
        if let Some(node_id) = self.doc.get_element_by_id(notif_id) {
            self.doc.remove_child(self.notification_area, node_id);
            self.doc.destroy_node(node_id);
        }
    }

    // ── Launcher helpers ─────────────────────────────────────────

    /// Show the launcher overlay with a search box and optional items.
    ///
    /// Creates:
    /// ```text
    /// <launcher-overlay>
    ///   <launcher>
    ///     <launcher-search />
    ///     <launcher-results>
    ///       <launcher-item data-app-id="…"> label </launcher-item>
    ///       …
    ///     </launcher-results>
    ///   </launcher>
    /// </launcher-overlay>
    /// ```
    pub fn show_launcher(&mut self, items: &[LauncherItemInfo]) -> NodeId {
        // Remove if already visible
        self.hide_launcher();

        let root = self.doc.root();

        let overlay = self.doc.create_element("launcher-overlay");
        self.doc.set_id(overlay, element_ids::LAUNCHER_OVERLAY);
        self.doc.append_child(root, overlay);

        let launcher = self.doc.create_element("launcher");
        self.doc.set_id(launcher, element_ids::LAUNCHER);
        self.doc.append_child(overlay, launcher);

        let search = self.doc.create_element("launcher-search");
        self.doc.set_id(search, element_ids::LAUNCHER_SEARCH);
        self.doc.append_child(launcher, search);

        let results = self.doc.create_element("launcher-results");
        for (i, item) in items.iter().enumerate() {
            let li = self.doc.create_element("launcher-item");
            self.doc.set_attribute(li, "data-app-id", &item.app_id);
            self.doc.set_attribute(li, "data-icon", &item.icon);
            self.doc.set_attribute(li, "data-index", &i.to_string());
            let txt = self.doc.create_text(&item.label);
            self.doc.append_child(li, txt);
            self.doc.append_child(results, li);
        }
        self.doc.append_child(launcher, results);

        overlay
    }

    /// Hide the launcher overlay.
    pub fn hide_launcher(&mut self) {
        if let Some(overlay) = self.doc.get_element_by_id(element_ids::LAUNCHER_OVERLAY) {
            let root = self.doc.root();
            self.doc.remove_child(root, overlay);
            self.doc.destroy_node(overlay);
        }
    }

    /// Set hover state on a launcher item by index.
    pub fn set_launcher_hover(&mut self, index: Option<usize>) {
        if let Some(launcher) = self.doc.get_element_by_id(element_ids::LAUNCHER) {
            let launcher_kids: Vec<NodeId> = self.doc.children(launcher).to_vec();
            // The results container is the second child (after search)
            if let Some(&results) = launcher_kids.get(1) {
                let items: Vec<NodeId> = self.doc.children(results).to_vec();
                for (i, &item) in items.iter().enumerate() {
                    self.doc.set_pseudo_state(
                        item,
                        PseudoStateFlags::HOVER,
                        index == Some(i),
                    );
                }
            }
        }
    }

    // ── Session-menu helpers ─────────────────────────────────────

    /// Show the session menu (lock, logout, restart, shutdown etc).
    pub fn show_session_menu(&mut self, items: &[MenuItemInfo]) -> NodeId {
        self.hide_session_menu();

        let root = self.doc.root();
        let menu = self.doc.create_element("session-menu");
        self.doc.set_id(menu, element_ids::SESSION_MENU);

        for (i, item) in items.iter().enumerate() {
            let mi = self.doc.create_element("menu-item");
            self.doc.set_attribute(mi, "data-action", &item.action);
            self.doc.set_attribute(mi, "data-index", &i.to_string());
            if !item.icon.is_empty() {
                self.doc.set_attribute(mi, "data-icon", &item.icon);
            }
            let txt = self.doc.create_text(&item.label);
            self.doc.append_child(mi, txt);
            self.doc.append_child(menu, mi);
        }

        self.doc.append_child(root, menu);
        menu
    }

    /// Hide the session menu.
    pub fn hide_session_menu(&mut self) {
        if let Some(node) = self.doc.get_element_by_id(element_ids::SESSION_MENU) {
            let root = self.doc.root();
            self.doc.remove_child(root, node);
            self.doc.destroy_node(node);
        }
    }

    // ── App-menu helpers ─────────────────────────────────────────

    /// Show an application menu (triggered from titlebar or statusbar).
    pub fn show_app_menu(&mut self, items: &[MenuItemInfo]) -> NodeId {
        self.hide_app_menu();

        let root = self.doc.root();
        let menu = self.doc.create_element("app-menu");
        self.doc.set_id(menu, element_ids::APP_MENU);

        for (i, item) in items.iter().enumerate() {
            let mi = self.doc.create_element("menu-item");
            self.doc.set_attribute(mi, "data-action", &item.action);
            self.doc.set_attribute(mi, "data-index", &i.to_string());
            if !item.icon.is_empty() {
                self.doc.set_attribute(mi, "data-icon", &item.icon);
            }
            let txt = self.doc.create_text(&item.label);
            self.doc.append_child(mi, txt);
            self.doc.append_child(menu, mi);
        }

        self.doc.append_child(root, menu);
        menu
    }

    /// Hide the app menu.
    pub fn hide_app_menu(&mut self) {
        if let Some(node) = self.doc.get_element_by_id(element_ids::APP_MENU) {
            let root = self.doc.root();
            self.doc.remove_child(root, node);
            self.doc.destroy_node(node);
        }
    }

    // ── Generic menu hover helper ────────────────────────────────

    /// Set hover on a menu item by index within a menu element.
    pub fn set_menu_hover(&mut self, menu_id: &str, index: Option<usize>) {
        if let Some(menu_node) = self.doc.get_element_by_id(menu_id) {
            let children: Vec<NodeId> = self.doc.children(menu_node).to_vec();
            let mut item_i = 0usize;
            for child in children {
                // Only count menu-item elements, skip separators
                if let Some(node) = self.doc.get(child) {
                    if node.tag_name() == "menu-item" {
                        self.doc.set_pseudo_state(
                            child,
                            PseudoStateFlags::HOVER,
                            index == Some(item_i),
                        );
                        item_i += 1;
                    }
                }
            }
        }
    }

    // ── Titlebar button hover ────────────────────────────────────

    /// Set hover on a specific titlebar button within a window.
    ///
    /// `button_tag` should be one of: `"close-button"`, `"maximize-button"`,
    /// `"minimize-button"`.
    pub fn set_window_button_hover(
        &mut self,
        window_id: &str,
        button_tag: Option<&str>,
    ) {
        if let Some(win_node) = self.doc.get_element_by_id(window_id) {
            let win_kids: Vec<NodeId> = self.doc.children(win_node).to_vec();
            // First child is window-titlebar
            if let Some(&titlebar) = win_kids.first() {
                let tb_kids: Vec<NodeId> = self.doc.children(titlebar).to_vec();
                // Second child of titlebar is titlebar-buttons
                if let Some(&btn_group) = tb_kids.get(1) {
                    let buttons: Vec<NodeId> = self.doc.children(btn_group).to_vec();
                    for btn in buttons {
                        let is_hovered = button_tag
                            .and_then(|tag| {
                                self.doc
                                    .get(btn)
                                    .map(|n| n.tag_name() == tag)
                            })
                            .unwrap_or(false);
                        self.doc
                            .set_pseudo_state(btn, PseudoStateFlags::HOVER, is_hovered);
                    }
                }
            }
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

/// Minimal launcher item info for DOM construction.
#[derive(Debug, Clone)]
pub struct LauncherItemInfo {
    pub app_id: String,
    pub label: String,
    pub icon: String,
}

/// Generic menu item info (session-menu, app-menu).
#[derive(Debug, Clone)]
pub struct MenuItemInfo {
    pub label: String,
    pub action: String,
    pub icon: String,
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_tree_structure() {
        let desktop = DesktopDocument::new();
        let doc = &desktop.doc;

        // Root has 5 children: desktop-bg, statusbar, workspace, dock, notification-area
        assert_eq!(doc.children(doc.root()).len(), 5);

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

        // Titlebar has window-title + titlebar-buttons
        let titlebar = kids[0];
        let tb_kids = desktop.doc.children(titlebar);
        assert_eq!(tb_kids.len(), 2);

        // Titlebar-buttons has 3 buttons: minimize, maximize, close
        let btn_group = tb_kids[1];
        assert_eq!(desktop.doc.children(btn_group).len(), 3);

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

    #[test]
    fn launcher_show_hide() {
        let mut desktop = DesktopDocument::new();
        let items = vec![
            LauncherItemInfo {
                app_id: "files".into(),
                label: "Files".into(),
                icon: "folder".into(),
            },
            LauncherItemInfo {
                app_id: "term".into(),
                label: "Terminal".into(),
                icon: "terminal".into(),
            },
        ];

        let overlay = desktop.show_launcher(&items);
        assert!(desktop.doc.get_element_by_id(element_ids::LAUNCHER_OVERLAY).is_some());
        assert!(desktop.doc.get_element_by_id(element_ids::LAUNCHER).is_some());

        // Launcher has search + results
        let launcher = desktop.doc.get_element_by_id(element_ids::LAUNCHER).unwrap();
        assert_eq!(desktop.doc.children(launcher).len(), 2);

        // Results has 2 items
        let results = desktop.doc.children(launcher)[1];
        assert_eq!(desktop.doc.children(results).len(), 2);

        // Verify overlay is child of root
        assert!(desktop.doc.children(desktop.doc.root()).contains(&overlay));

        desktop.hide_launcher();
        assert!(desktop.doc.get_element_by_id(element_ids::LAUNCHER_OVERLAY).is_none());
    }

    #[test]
    fn session_menu_show_hide() {
        let mut desktop = DesktopDocument::new();
        let items = vec![
            MenuItemInfo {
                label: "Lock".into(),
                action: "lock".into(),
                icon: "lock".into(),
            },
            MenuItemInfo {
                label: "Shutdown".into(),
                action: "shutdown".into(),
                icon: "power".into(),
            },
        ];

        let menu = desktop.show_session_menu(&items);
        assert!(desktop.doc.get_element_by_id(element_ids::SESSION_MENU).is_some());
        assert_eq!(desktop.doc.children(menu).len(), 2);

        desktop.hide_session_menu();
        assert!(desktop.doc.get_element_by_id(element_ids::SESSION_MENU).is_none());
    }

    #[test]
    fn app_menu_show_hide() {
        let mut desktop = DesktopDocument::new();
        let items = vec![MenuItemInfo {
            label: "About".into(),
            action: "about".into(),
            icon: "".into(),
        }];

        let menu = desktop.show_app_menu(&items);
        assert!(desktop.doc.get_element_by_id(element_ids::APP_MENU).is_some());
        assert_eq!(desktop.doc.children(menu).len(), 1);

        desktop.hide_app_menu();
        assert!(desktop.doc.get_element_by_id(element_ids::APP_MENU).is_none());
    }

    #[test]
    fn notification_in_area() {
        let mut desktop = DesktopDocument::new();
        let n = desktop.add_notification("notif-a", "Title", "Body");

        // Notification is inside the notification-area container
        let area_kids = desktop.doc.children(desktop.notification_area);
        assert_eq!(area_kids.len(), 1);
        assert_eq!(area_kids[0], n);

        desktop.remove_notification("notif-a");
        assert_eq!(desktop.doc.children(desktop.notification_area).len(), 0);
    }

    #[test]
    fn titlebar_button_hover() {
        let mut desktop = DesktopDocument::new();
        desktop.add_window("win-btn", "App", false);

        desktop.set_window_button_hover("win-btn", Some("close-button"));

        // Verify close-button has hover, others don't
        let win = desktop.doc.get_element_by_id("win-btn").unwrap();
        let titlebar = desktop.doc.children(win)[0];
        let btn_group = desktop.doc.children(titlebar)[1];
        let buttons: Vec<NodeId> = desktop.doc.children(btn_group).to_vec();

        // minimize=0, maximize=1, close=2
        assert!(!desktop.doc.get(buttons[0]).unwrap().has_pseudo_state(PseudoStateFlags::HOVER));
        assert!(!desktop.doc.get(buttons[1]).unwrap().has_pseudo_state(PseudoStateFlags::HOVER));
        assert!(desktop.doc.get(buttons[2]).unwrap().has_pseudo_state(PseudoStateFlags::HOVER));

        // Clear hover
        desktop.set_window_button_hover("win-btn", None);
        assert!(!desktop.doc.get(buttons[2]).unwrap().has_pseudo_state(PseudoStateFlags::HOVER));
    }
}
