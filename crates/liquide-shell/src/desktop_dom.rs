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

use std::path::Path;

use liquide_dom::html_parser::parse_html_into;
use liquide_dom::{Document, NodeId, PseudoStateFlags};
use tracing::{debug, warn};

// ── Embedded HTML templates ─────────────────────────────────────────

/// Default desktop layout template. Parsed by `from_html()` to build the
/// initial DOM tree. Users can override this by placing a `desktop.html`
/// file in `assets/` or `~/.config/liquide/`.
const DEFAULT_DESKTOP_HTML: &str = r#"
<desktop-background id="desktop-bg" />
<statusbar id="shell-statusbar">
  <statusbar-slot class="left" id="statusbar-slot-left" />
  <statusbar-slot class="center" id="statusbar-slot-center" />
  <statusbar-slot class="right" id="statusbar-slot-right" />
</statusbar>
<workspace-container id="workspace-container" />
<dock id="shell-dock" />
<notification-area id="notification-area" />
"#;

/// Default window template used by `add_window_from_html()`.
#[allow(dead_code)]
const DEFAULT_WINDOW_HTML: &str = r#"
<window>
  <window-titlebar>
    <window-title></window-title>
    <titlebar-buttons>
      <minimize-button />
      <maximize-button />
      <close-button />
    </titlebar-buttons>
  </window-titlebar>
  <window-content />
</window>
"#;

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
    /// Build the initial desktop DOM using the embedded default HTML template.
    pub fn new() -> Self {
        Self::from_html(DEFAULT_DESKTOP_HTML)
    }

    /// Build the desktop DOM from an HTML template string.
    ///
    /// The HTML is parsed and well-known element IDs are looked up from the
    /// resulting tree. Any missing required elements are created
    /// programmatically as a fallback so the shell always has a valid tree.
    pub fn from_html(html: &str) -> Self {
        let mut doc = Document::new();
        let root = doc.root();

        // Parse the HTML template into the document under root.
        parse_html_into(&mut doc, root, html);

        // Look up well-known elements by ID, creating missing ones as fallbacks.
        let desktop_bg = doc
            .get_element_by_id(element_ids::DESKTOP_BG)
            .unwrap_or_else(|| {
                warn!(
                    "HTML template missing #{}; creating fallback",
                    element_ids::DESKTOP_BG
                );
                let el = doc.create_element("desktop-background");
                doc.set_id(el, element_ids::DESKTOP_BG);
                doc.append_child(root, el);
                el
            });

        let statusbar = doc
            .get_element_by_id(element_ids::STATUSBAR)
            .unwrap_or_else(|| {
                warn!(
                    "HTML template missing #{}; creating fallback",
                    element_ids::STATUSBAR
                );
                let el = doc.create_element("statusbar");
                doc.set_id(el, element_ids::STATUSBAR);
                doc.append_child(root, el);
                el
            });

        let statusbar_slot_left = doc
            .get_element_by_id(element_ids::STATUSBAR_SLOT_LEFT)
            .unwrap_or_else(|| {
                warn!(
                    "HTML template missing #{}; creating fallback",
                    element_ids::STATUSBAR_SLOT_LEFT
                );
                let el = doc.create_element("statusbar-slot");
                doc.set_id(el, element_ids::STATUSBAR_SLOT_LEFT);
                doc.add_class(el, "left");
                doc.append_child(statusbar, el);
                el
            });

        let statusbar_slot_center = doc
            .get_element_by_id(element_ids::STATUSBAR_SLOT_CENTER)
            .unwrap_or_else(|| {
                warn!(
                    "HTML template missing #{}; creating fallback",
                    element_ids::STATUSBAR_SLOT_CENTER
                );
                let el = doc.create_element("statusbar-slot");
                doc.set_id(el, element_ids::STATUSBAR_SLOT_CENTER);
                doc.add_class(el, "center");
                doc.append_child(statusbar, el);
                el
            });

        let statusbar_slot_right = doc
            .get_element_by_id(element_ids::STATUSBAR_SLOT_RIGHT)
            .unwrap_or_else(|| {
                warn!(
                    "HTML template missing #{}; creating fallback",
                    element_ids::STATUSBAR_SLOT_RIGHT
                );
                let el = doc.create_element("statusbar-slot");
                doc.set_id(el, element_ids::STATUSBAR_SLOT_RIGHT);
                doc.add_class(el, "right");
                doc.append_child(statusbar, el);
                el
            });

        let workspace = doc
            .get_element_by_id(element_ids::WORKSPACE)
            .unwrap_or_else(|| {
                warn!(
                    "HTML template missing #{}; creating fallback",
                    element_ids::WORKSPACE
                );
                let el = doc.create_element("workspace-container");
                doc.set_id(el, element_ids::WORKSPACE);
                doc.append_child(root, el);
                el
            });

        let dock = doc.get_element_by_id(element_ids::DOCK).unwrap_or_else(|| {
            warn!(
                "HTML template missing #{}; creating fallback",
                element_ids::DOCK
            );
            let el = doc.create_element("dock");
            doc.set_id(el, element_ids::DOCK);
            doc.append_child(root, el);
            el
        });

        let notification_area = doc
            .get_element_by_id(element_ids::NOTIFICATION_AREA)
            .unwrap_or_else(|| {
                warn!(
                    "HTML template missing #{}; creating fallback",
                    element_ids::NOTIFICATION_AREA
                );
                let el = doc.create_element("notification-area");
                doc.set_id(el, element_ids::NOTIFICATION_AREA);
                doc.append_child(root, el);
                el
            });

        debug!(
            nodes = doc.node_count(),
            "DesktopDocument: tree built from HTML template"
        );

        Self {
            doc,
            desktop_bg,
            statusbar,
            statusbar_slot_left,
            statusbar_slot_center,
            statusbar_slot_right,
            workspace,
            dock,
            notification_area,
        }
    }

    /// Build the desktop DOM from an HTML file on disk.
    ///
    /// If the file cannot be read, falls back to the embedded default template.
    pub fn from_file(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(html) => {
                debug!(?path, "DesktopDocument: loaded HTML template from file");
                Self::from_html(&html)
            }
            Err(e) => {
                warn!(?path, %e, "DesktopDocument: failed to read HTML file, using embedded default");
                Self::new()
            }
        }
    }

    /// Load the desktop DOM from disk if available, otherwise use the
    /// embedded default template.
    ///
    /// Search order:
    /// 1. `assets/desktop.html` relative to the executable
    /// 2. `~/.config/liquide/desktop.html` for user customization
    /// 3. Embedded `DEFAULT_DESKTOP_HTML`
    pub fn load_or_default() -> Self {
        // Try relative to executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let candidate = exe_dir.join("assets").join("desktop.html");
                if candidate.exists() {
                    debug!(
                        ?candidate,
                        "DesktopDocument: found desktop.html next to executable"
                    );
                    return Self::from_file(&candidate);
                }
            }
        }

        // Try assets/ in the current working directory
        {
            let candidate = Path::new("assets").join("desktop.html");
            if candidate.exists() {
                debug!(?candidate, "DesktopDocument: found desktop.html in assets/");
                return Self::from_file(&candidate);
            }
        }

        // Try user config directory
        if let Some(home) = home_dir() {
            let candidate = home.join(".config").join("liquide").join("desktop.html");
            if candidate.exists() {
                debug!(?candidate, "DesktopDocument: found user desktop.html");
                return Self::from_file(&candidate);
            }
        }

        debug!("DesktopDocument: no external HTML template found, using embedded default");
        Self::new()
    }

    // ── HTML-based dynamic element helpers ────────────────────────

    /// Add a window to the workspace container using an HTML template.
    ///
    /// The `html` parameter should contain a `<window>` element tree.
    /// The window's `id` is set to `window_id`, the `<window-title>` text
    /// is replaced with `title`, and focus state is applied.
    ///
    /// Returns the `<window>` node id.
    pub fn add_window_from_html(
        &mut self,
        html: &str,
        window_id: &str,
        title: &str,
        focused: bool,
    ) -> NodeId {
        // Parse into a temporary container to get the top-level window element
        let fragment = self.doc.create_element("__fragment");
        parse_html_into(&mut self.doc, fragment, html);

        // Take the first child element as the window node
        let win_node = if let Some(first) = self.doc.children(fragment).first().copied() {
            first
        } else {
            // Fallback: use programmatic method
            let win = self.doc.create_element("window");
            self.doc.append_child(fragment, win);
            win
        };

        // Detach from fragment and set up
        self.doc.remove_child(fragment, win_node);
        self.doc.destroy_node(fragment);

        // Set the window ID
        self.doc.set_id(win_node, window_id);

        // Apply focus state
        if focused {
            self.doc.add_class(win_node, "focused");
            self.doc
                .set_pseudo_state(win_node, PseudoStateFlags::FOCUS, true);
        }

        // Find and set the title text
        Self::set_descendant_text(&mut self.doc, win_node, "window-title", title);

        // Append to workspace
        self.doc.append_child(self.workspace, win_node);
        win_node
    }

    /// Add a notification using an HTML template.
    ///
    /// The `html` parameter should contain a `<notification>` element tree.
    /// The notification's `id` is set to `notif_id`, and title/body text
    /// are filled in.
    ///
    /// Returns the `<notification>` node id.
    pub fn add_notification_from_html(
        &mut self,
        html: &str,
        notif_id: &str,
        title: &str,
        body: &str,
    ) -> NodeId {
        let fragment = self.doc.create_element("__fragment");
        parse_html_into(&mut self.doc, fragment, html);

        let notif_node = if let Some(first) = self.doc.children(fragment).first().copied() {
            first
        } else {
            let el = self.doc.create_element("notification");
            self.doc.append_child(fragment, el);
            el
        };

        self.doc.remove_child(fragment, notif_node);
        self.doc.destroy_node(fragment);

        self.doc.set_id(notif_node, notif_id);

        // Fill in title and body
        Self::set_descendant_text(&mut self.doc, notif_node, "notification-title", title);
        Self::set_descendant_text(&mut self.doc, notif_node, "notification-body", body);

        self.doc.append_child(self.notification_area, notif_node);
        notif_node
    }

    /// Find a descendant element by tag name and set its text content.
    ///
    /// If the element has a text child, that child's content is updated.
    /// If it has no children, a text node is created.
    fn set_descendant_text(doc: &mut Document, root: NodeId, tag: &str, text: &str) {
        if let Some(node_id) = Self::find_descendant_by_tag(doc, root, tag) {
            let children: Vec<NodeId> = doc.children(node_id).to_vec();
            if let Some(&first_child) = children.first() {
                // Update existing text child
                doc.set_text_content(first_child, text);
            } else {
                // Create a text node
                let txt = doc.create_text(text);
                doc.append_child(node_id, txt);
            }
        }
    }

    /// Find the first descendant element with the given tag name (depth-first).
    fn find_descendant_by_tag(doc: &Document, parent: NodeId, tag: &str) -> Option<NodeId> {
        let children: Vec<NodeId> = doc.children(parent).to_vec();
        for child in children {
            if let Some(node) = doc.get(child) {
                if node.tag_name() == tag {
                    return Some(child);
                }
                // Recurse
                if let Some(found) = Self::find_descendant_by_tag(doc, child, tag) {
                    return Some(found);
                }
            }
        }
        None
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
            self.doc.set_pseudo_state(el, PseudoStateFlags::FOCUS, true);
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

    /// Set the `:hover` pseudo-state on a dock item by index (t65-s3).
    ///
    /// The dock template injects a `.hovered` CLASS, but the theme styles the
    /// `dock-item:hover` PSEUDO-class, so the class alone never triggers the
    /// hover background/colour swap on the render path. This mirrors
    /// [`set_launcher_hover`] / [`set_menu_hover`]: it sets `PseudoStateFlags::HOVER`
    /// on the hovered dock item (and clears it on the others) so the themed
    /// `:hover` rule actually paints.
    pub fn set_dock_hover(&mut self, index: Option<usize>) {
        if let Some(dock) = self.doc.get_element_by_id(element_ids::DOCK) {
            let items: Vec<NodeId> = self.doc.children(dock).to_vec();
            for (i, &item) in items.iter().enumerate() {
                self.doc
                    .set_pseudo_state(item, PseudoStateFlags::HOVER, index == Some(i));
            }
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
                    self.doc
                        .set_pseudo_state(item, PseudoStateFlags::HOVER, index == Some(i));
                }
            }
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
    pub fn set_window_button_hover(&mut self, window_id: &str, button_tag: Option<&str>) {
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
                            .and_then(|tag| self.doc.get(btn).map(|n| n.tag_name() == tag))
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

/// Cross-platform home directory resolution (no external crate dependency).
fn home_dir() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(std::path::PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(std::path::PathBuf::from)
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
    fn notification() {
        let mut desktop = DesktopDocument::new();
        let n = desktop.add_notification("notif-1", "Alert", "Something happened");
        assert_eq!(desktop.doc.children(n).len(), 2);

        desktop.remove_notification("notif-1");
        assert!(desktop.doc.get_element_by_id("notif-1").is_none());
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
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::LAUNCHER_OVERLAY)
                .is_some()
        );
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::LAUNCHER)
                .is_some()
        );

        // Launcher has search + results
        let launcher = desktop
            .doc
            .get_element_by_id(element_ids::LAUNCHER)
            .unwrap();
        assert_eq!(desktop.doc.children(launcher).len(), 2);

        // Results has 2 items
        let results = desktop.doc.children(launcher)[1];
        assert_eq!(desktop.doc.children(results).len(), 2);

        // Verify overlay is child of root
        assert!(desktop.doc.children(desktop.doc.root()).contains(&overlay));

        desktop.hide_launcher();
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::LAUNCHER_OVERLAY)
                .is_none()
        );
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
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::APP_MENU)
                .is_some()
        );
        assert_eq!(desktop.doc.children(menu).len(), 1);

        desktop.hide_app_menu();
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::APP_MENU)
                .is_none()
        );
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

    // ── HTML template tests ────────────────────────────────────────

    #[test]
    fn from_html_matches_new() {
        let from_new = DesktopDocument::new();
        let from_html = DesktopDocument::from_html(DEFAULT_DESKTOP_HTML);

        // Both should have 5 root children
        assert_eq!(
            from_new.doc.children(from_new.doc.root()).len(),
            from_html.doc.children(from_html.doc.root()).len(),
        );

        // Both should resolve all well-known IDs
        assert!(
            from_html
                .doc
                .get_element_by_id(element_ids::DESKTOP_BG)
                .is_some()
        );
        assert!(
            from_html
                .doc
                .get_element_by_id(element_ids::STATUSBAR)
                .is_some()
        );
        assert!(
            from_html
                .doc
                .get_element_by_id(element_ids::STATUSBAR_SLOT_LEFT)
                .is_some()
        );
        assert!(
            from_html
                .doc
                .get_element_by_id(element_ids::STATUSBAR_SLOT_CENTER)
                .is_some()
        );
        assert!(
            from_html
                .doc
                .get_element_by_id(element_ids::STATUSBAR_SLOT_RIGHT)
                .is_some()
        );
        assert!(
            from_html
                .doc
                .get_element_by_id(element_ids::WORKSPACE)
                .is_some()
        );
        assert!(from_html.doc.get_element_by_id(element_ids::DOCK).is_some());
        assert!(
            from_html
                .doc
                .get_element_by_id(element_ids::NOTIFICATION_AREA)
                .is_some()
        );

        // Statusbar should have 3 slots
        assert_eq!(from_html.doc.children(from_html.statusbar).len(), 3);
    }

    #[test]
    fn from_html_finds_all_well_known_ids() {
        let desktop = DesktopDocument::from_html(DEFAULT_DESKTOP_HTML);

        // Verify the struct fields match the looked-up IDs
        assert_eq!(
            desktop.doc.get_element_by_id(element_ids::DESKTOP_BG),
            Some(desktop.desktop_bg)
        );
        assert_eq!(
            desktop.doc.get_element_by_id(element_ids::STATUSBAR),
            Some(desktop.statusbar)
        );
        assert_eq!(
            desktop
                .doc
                .get_element_by_id(element_ids::STATUSBAR_SLOT_LEFT),
            Some(desktop.statusbar_slot_left)
        );
        assert_eq!(
            desktop
                .doc
                .get_element_by_id(element_ids::STATUSBAR_SLOT_CENTER),
            Some(desktop.statusbar_slot_center)
        );
        assert_eq!(
            desktop
                .doc
                .get_element_by_id(element_ids::STATUSBAR_SLOT_RIGHT),
            Some(desktop.statusbar_slot_right)
        );
        assert_eq!(
            desktop.doc.get_element_by_id(element_ids::WORKSPACE),
            Some(desktop.workspace)
        );
        assert_eq!(
            desktop.doc.get_element_by_id(element_ids::DOCK),
            Some(desktop.dock)
        );
        assert_eq!(
            desktop
                .doc
                .get_element_by_id(element_ids::NOTIFICATION_AREA),
            Some(desktop.notification_area)
        );
    }

    #[test]
    fn from_html_fallback_for_missing_elements() {
        // HTML template with only desktop-bg — all other elements should be
        // created as fallbacks.
        let desktop = DesktopDocument::from_html(r#"<desktop-background id="desktop-bg" />"#);

        // All well-known fields should still be valid
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::DESKTOP_BG)
                .is_some()
        );
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::STATUSBAR)
                .is_some()
        );
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::WORKSPACE)
                .is_some()
        );
        assert!(desktop.doc.get_element_by_id(element_ids::DOCK).is_some());
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::NOTIFICATION_AREA)
                .is_some()
        );

        // Statusbar fallback should also create its child slots
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::STATUSBAR_SLOT_LEFT)
                .is_some()
        );
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::STATUSBAR_SLOT_CENTER)
                .is_some()
        );
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::STATUSBAR_SLOT_RIGHT)
                .is_some()
        );
    }

    #[test]
    fn load_or_default_uses_embedded_when_no_files() {
        // When no files exist on disk, load_or_default() should produce the
        // same structure as new().
        let desktop = DesktopDocument::load_or_default();
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::DESKTOP_BG)
                .is_some()
        );
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::STATUSBAR)
                .is_some()
        );
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::WORKSPACE)
                .is_some()
        );
        assert!(desktop.doc.get_element_by_id(element_ids::DOCK).is_some());
        assert!(
            desktop
                .doc
                .get_element_by_id(element_ids::NOTIFICATION_AREA)
                .is_some()
        );
    }

    #[test]
    fn add_window_from_html_template() {
        let mut desktop = DesktopDocument::new();
        let win =
            desktop.add_window_from_html(DEFAULT_WINDOW_HTML, "win-html-1", "HTML Window", true);

        // Window should be in workspace
        assert!(desktop.doc.children(desktop.workspace).contains(&win));

        // Should have the correct ID
        assert_eq!(
            desktop.doc.get(win).unwrap().element_id.as_deref(),
            Some("win-html-1")
        );

        // Should be focused
        assert!(desktop.doc.get(win).unwrap().has_class("focused"));

        // Should have titlebar + content children
        let kids = desktop.doc.children(win);
        assert_eq!(kids.len(), 2);

        // Titlebar should contain the title text
        let titlebar = kids[0];
        let title_el = desktop.doc.children(titlebar)[0];
        let title_text = desktop.doc.children(title_el)[0];
        assert_eq!(
            desktop.doc.get(title_text).unwrap().text_content(),
            Some("HTML Window")
        );
    }

    #[test]
    fn add_notification_from_html_template() {
        let mut desktop = DesktopDocument::new();
        let html = r#"
<notification>
  <notification-title></notification-title>
  <notification-body></notification-body>
</notification>
"#;
        let notif =
            desktop.add_notification_from_html(html, "notif-html-1", "Test Title", "Test Body");

        // Notification should be in notification-area
        assert!(
            desktop
                .doc
                .children(desktop.notification_area)
                .contains(&notif)
        );

        // Should have the correct ID
        assert_eq!(
            desktop.doc.get(notif).unwrap().element_id.as_deref(),
            Some("notif-html-1")
        );

        // Should have 2 children: title and body
        let kids = desktop.doc.children(notif);
        assert_eq!(kids.len(), 2);
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
        assert!(
            !desktop
                .doc
                .get(buttons[0])
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::HOVER)
        );
        assert!(
            !desktop
                .doc
                .get(buttons[1])
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::HOVER)
        );
        assert!(
            desktop
                .doc
                .get(buttons[2])
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::HOVER)
        );

        // Clear hover
        desktop.set_window_button_hover("win-btn", None);
        assert!(
            !desktop
                .doc
                .get(buttons[2])
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::HOVER)
        );
    }
}
