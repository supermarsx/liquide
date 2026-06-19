//! DevTools panel — the top-level container that composes all sub-panels
//! into a docked/floating developer tools window.
//!
//! The panel is designed to be rendered as an overlay on top of the
//! compositor scene. It handles tab switching, keyboard shortcuts,
//! and coordinates the inspector, style panel, layout overlay, element
//! picker, mutation log, and DOM serializer.

mod keyboard;
mod mouse;
mod rendering;
mod scene;
mod side_panels;
#[cfg(test)]
mod tests;
mod types;

pub use types::{DevToolsConfig, DevToolsTab, DockPosition, FrameSnapshot, SideTab};

use std::collections::VecDeque;
use std::time::Instant;

use liquide_components::TemplateNode;
use liquide_compositor::geometry::Rect;
use liquide_dom::NodeId;

use crate::console::DebugConsole;
use crate::context_menu::ContextMenu;
use crate::dom_serializer::DomSerializer;
use crate::element_picker::ElementPicker;
use crate::inspector::ElementTreeInspector;
use crate::layout_overlay::LayoutOverlay;
use crate::mutation_log::MutationLog;
use crate::scene_graph::SceneGraphDebugger;
use crate::style_editor::StyleEditor;
use crate::style_panel::StyleInspector;

/// The top-level DevTools panel.
///
/// Composes all sub-modules and manages panel visibility, tab state,
/// and the coordinate system for the devtools overlay scene nodes.
pub struct DevToolsPanel {
    /// Whether the panel is visible.
    pub(crate) visible: bool,
    /// Active tab.
    pub(crate) active_tab: DevToolsTab,
    /// Active sub-tab in the Elements side panel.
    pub(crate) side_tab: SideTab,
    /// Configuration.
    pub(crate) config: DevToolsConfig,
    /// Element tree inspector.
    pub inspector: ElementTreeInspector,
    /// Style property viewer.
    pub style_inspector: StyleInspector,
    /// Layout box overlay.
    pub layout_overlay: LayoutOverlay,
    /// Element picker.
    pub element_picker: ElementPicker,
    /// DOM mutation log.
    pub mutation_log: MutationLog,
    /// DOM serializer.
    pub dom_serializer: DomSerializer,
    /// Debug console.
    pub console: DebugConsole,
    /// Scene graph debugger.
    pub scene_debugger: SceneGraphDebugger,
    /// Live style editor.
    pub style_editor: StyleEditor,
    /// Queued style edits waiting to be applied to the document.
    pub(crate) style_edit_queue: Vec<crate::style_editor::StyleEdit>,
    /// Context menu.
    pub context_menu: ContextMenu,
    /// Currently selected node (shared across panels).
    pub(crate) selected_node: Option<NodeId>,
    /// Screen dimensions for layout calculations.
    pub(crate) screen_width: f32,
    pub(crate) screen_height: f32,
    /// Vertical scroll offset (in pixels) for the active tab content.
    pub(crate) scroll_offset: f32,
    /// Whether the panel is requesting detach into a separate window.
    pub(crate) detach_requested: bool,
    /// Whether the panel is requesting that an attached separate devtools window
    /// be closed / re-docked into the in-DE overlay.
    pub(crate) close_window_requested: bool,
    /// Tab bar scroll offset (horizontal, for when many tabs exceed width).
    #[allow(dead_code)]
    pub(crate) tab_scroll: f32,
    /// Whether the console input is focused for keyboard capture.
    pub(crate) console_focused: bool,
    /// Epoch for cursor blink animation — reset on each keystroke so the
    /// caret stays solid for 500 ms after the last input.
    pub(crate) caret_blink_epoch: Instant,
    /// Latest frame snapshot from the pipeline (Debugger tab).
    pub(crate) frame_snapshot: Option<FrameSnapshot>,
    /// Recent frame times (ms) for sparkline display — last ~120 frames.
    pub(crate) frame_times: VecDeque<f64>,
}

impl DevToolsPanel {
    /// Create a new devtools panel with the given configuration.
    pub fn new(config: DevToolsConfig) -> Self {
        let visible = config.initially_visible;
        let show_overlay = config.show_layout_overlay;

        let mut overlay = LayoutOverlay::new();
        if show_overlay {
            overlay.set_enabled(true);
        }

        Self {
            visible,
            active_tab: DevToolsTab::Elements,
            side_tab: SideTab::Styles,
            config,
            inspector: ElementTreeInspector::new(),
            style_inspector: StyleInspector::new(),
            layout_overlay: overlay,
            element_picker: ElementPicker::new(),
            mutation_log: MutationLog::new(),
            dom_serializer: DomSerializer::new(),
            console: DebugConsole::new(),
            scene_debugger: SceneGraphDebugger::new(),
            style_editor: StyleEditor::new(),
            style_edit_queue: Vec::new(),
            context_menu: ContextMenu::new(),
            selected_node: None,
            screen_width: 1920.0,
            screen_height: 1080.0,
            scroll_offset: 0.0,
            detach_requested: false,
            close_window_requested: false,
            tab_scroll: 0.0,
            console_focused: false,
            caret_blink_epoch: Instant::now(),
            frame_snapshot: None,
            frame_times: VecDeque::with_capacity(128),
        }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(DevToolsConfig::default())
    }

    // ─── Visibility ───────────────────────────────────────────

    /// Toggle the devtools panel open/closed.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.element_picker.deactivate();
        }
    }

    /// Show the devtools panel.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the devtools panel.
    pub fn hide(&mut self) {
        self.visible = false;
        self.element_picker.deactivate();
    }

    /// Whether the panel is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Whether [`build_scene`](Self::build_scene) would emit any direct overlay
    /// scene nodes (element-picker highlight, layout overlay, or a hover/selected
    /// element highlight) on TOP of the page viewport.
    ///
    /// These overlays are added to the scene graph AFTER `build_scene`, so the
    /// shell's precomputed-damage fast-path hint cannot bound them. The render
    /// loop uses this to decide whether the fast path is still a true superset:
    /// when the devtools panel is merely visible (its panel is part of the CSS
    /// pipeline and IS bounded by precomputed damage) with NO active overlays,
    /// the fast path stays valid; when an overlay is live, the loop falls back to
    /// the conservative full diff. This is what keeps an idle devtools frame from
    /// forcing a full-frame repaint every frame (t130 jank).
    pub fn has_active_overlays(&self) -> bool {
        // Layout box overlay only paints when ENABLED *and* it has a target node
        // (it emits nothing for a null target — see `LayoutOverlay::build_overlay`).
        if self.layout_overlay.is_enabled() && self.layout_overlay.target().is_some() {
            return true;
        }
        // Element picker highlight — emitted whenever the picker is active.
        if self.element_picker.is_active() {
            return true;
        }
        // Hover / selected element highlights are only emitted while visible.
        if self.visible {
            if self.active_tab == DevToolsTab::Elements && self.inspector.hovered().is_some() {
                return true;
            }
            if self.selected_node.is_some() {
                return true;
            }
        }
        false
    }

    // ─── Pipeline Stats ───────────────────────────────────────

    /// Push a frame snapshot so the Debugger tab can display live numbers.
    pub fn push_frame_snapshot(&mut self, snap: FrameSnapshot) {
        let ft = snap.avg_frame_ms;
        self.frame_snapshot = Some(snap);
        self.frame_times.push_back(ft);
        if self.frame_times.len() > 120 {
            self.frame_times.pop_front();
        }
    }

    // ─── Tab management ───────────────────────────────────────

    /// Switch to a specific tab.
    pub fn set_tab(&mut self, tab: DevToolsTab) {
        if self.active_tab != tab {
            self.scroll_offset = 0.0;
        }
        self.active_tab = tab;
    }

    /// Get the active tab.
    pub fn active_tab(&self) -> DevToolsTab {
        self.active_tab
    }

    /// Cycle to the next tab.
    pub fn next_tab(&mut self) {
        let tabs = DevToolsTab::ALL;
        let cur = tabs.iter().position(|t| *t == self.active_tab).unwrap_or(0);
        self.active_tab = tabs[(cur + 1) % tabs.len()];
    }

    /// Cycle to the previous tab.
    pub fn prev_tab(&mut self) {
        let tabs = DevToolsTab::ALL;
        let cur = tabs.iter().position(|t| *t == self.active_tab).unwrap_or(0);
        self.active_tab = tabs[(cur + tabs.len() - 1) % tabs.len()];
    }

    /// Get the active side tab.
    pub fn side_tab(&self) -> SideTab {
        self.side_tab
    }

    /// Set the active side tab.
    pub fn set_side_tab(&mut self, tab: SideTab) {
        self.side_tab = tab;
    }

    // ─── Virtual scroll helpers ───────────────────────────────

    /// Compute the available content height (panel height minus toolbar and
    /// status bar) in pixels.
    fn content_height(&self) -> f32 {
        let bounds = self.panel_bounds();
        let toolbar_h = 30.0;
        let statusbar_h = 20.0;
        let borders = 2.0; // top + bottom border
        (bounds.height - toolbar_h - statusbar_h - borders).max(0.0)
    }

    /// Given a fixed `row_height`, return `(first_visible_index, count)` for
    /// virtual scrolling so that only visible rows are emitted.
    fn visible_row_range(&self, total_rows: usize, row_height: f32) -> (usize, usize) {
        let ch = self.content_height();
        if ch <= 0.0 || total_rows == 0 {
            return (0, 0);
        }
        let first = (self.scroll_offset / row_height).floor() as usize;
        let count = (ch / row_height).ceil() as usize + 1; // +1 for partial row
        let first = first.min(total_rows.saturating_sub(1));
        let count = count.min(total_rows - first);
        (first, count)
    }

    // ─── Element selection ────────────────────────────────────

    /// Select a DOM node by ID (updates all sub-panels).
    pub fn select_node(&mut self, node_id: NodeId, styles: &liquide_style_engine::StyleMap) {
        self.selected_node = Some(node_id);
        self.inspector.select(node_id);
        self.style_inspector.inspect(node_id, styles);
        self.layout_overlay.set_target(Some(node_id));
        self.console.set_selected_node(Some(node_id));
        self.style_editor.set_target(Some(node_id));
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selected_node = None;
        self.style_inspector.clear();
        self.layout_overlay.set_target(None);
        self.console.set_selected_node(None);
        self.style_editor.set_target(None);
    }

    /// Get the currently selected node.
    pub fn selected_node(&self) -> Option<NodeId> {
        self.selected_node
    }

    // ─── Element picker ───────────────────────────────────────

    /// Toggle the element picker mode (click-to-select).
    pub fn toggle_picker(&mut self) {
        if self.element_picker.is_active() {
            self.element_picker.deactivate();
        } else {
            self.element_picker.activate();
        }
    }

    // ─── Screen dimensions ────────────────────────────────────

    /// Update screen dimensions (called on resize).
    pub fn set_screen_size(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    // ─── Panel bounds ─────────────────────────────────────────

    /// Compute the panel bounds based on dock position.
    pub fn panel_bounds(&self) -> Rect {
        let size = self.config.panel_size;
        match self.config.dock_position {
            DockPosition::Bottom => {
                Rect::new(0.0, self.screen_height - size, self.screen_width, size)
            }
            DockPosition::Right => {
                Rect::new(self.screen_width - size, 0.0, size, self.screen_height)
            }
            DockPosition::Left => Rect::new(0.0, 0.0, size, self.screen_height),
            DockPosition::Detached => {
                // When detached the panel fills its OWN native window, whose
                // client size is mirrored into `screen_width`/`screen_height` by
                // the host. Bounds == the whole window so panel hit-testing /
                // scrolling cover the entire surface.
                Rect::new(0.0, 0.0, self.screen_width, self.screen_height)
            }
            DockPosition::Float => {
                // Floating overlay inside the DE: a centered box.
                let w = (self.screen_width * 0.6).min(800.0);
                let h = (self.screen_height * 0.5).min(500.0);
                Rect::new(
                    (self.screen_width - w) / 2.0,
                    (self.screen_height - h) / 2.0,
                    w,
                    h,
                )
            }
        }
    }

    // ─── Public APIs ──────────────────────────────────────────

    /// Apply all queued style edits to the document as inline styles.
    ///
    /// Returns the number of edits applied.  The host should call this
    /// once per frame after `handle_key()` / `on_panel_click()`.
    pub fn apply_pending_style_edits(&mut self, doc: &mut liquide_dom::Document) -> usize {
        let edits: Vec<crate::style_editor::StyleEdit> = self.style_edit_queue.drain(..).collect();
        let count = edits.len();
        for edit in &edits {
            doc.set_inline_style(edit.node_id, &edit.property, &edit.new_value);
            self.style_editor.mark_applied(edit.node_id, &edit.property);
        }
        count
    }

    /// Drain any console action (reload, restart, inspect) produced by
    /// the last console submit.
    pub fn take_console_action(&mut self) -> Option<crate::console::ConsoleAction> {
        self.console.take_pending_action()
    }

    /// Whether the panel is requesting to be detached into a separate window.
    pub fn detach_requested(&self) -> bool {
        self.detach_requested
    }

    /// Clear the detach request after the compositor handles it.
    pub fn clear_detach_request(&mut self) {
        self.detach_requested = false;
    }

    /// Whether the panel is requesting a previously-spawned separate devtools
    /// window be torn down (and the panel returned to the in-DE overlay).
    pub fn close_window_requested(&self) -> bool {
        self.close_window_requested
    }

    /// Clear the close-window request after the host tears the window down.
    pub fn clear_close_window_request(&mut self) {
        self.close_window_requested = false;
    }

    /// Request that any open separate devtools window be torn down (used when
    /// the panel is hidden while a window is open but the panel was not in the
    /// Detached dock position).
    pub fn request_close_window(&mut self) {
        self.close_window_requested = true;
    }

    /// Whether the panel is currently detached into a separate window.
    pub fn is_detached(&self) -> bool {
        self.config.dock_position == DockPosition::Detached
    }

    /// Toggle detached state.
    ///
    /// Going INTO detached state raises [`detach_requested`](Self::detach_requested)
    /// so the host spawns a separate native window. Coming OUT of it raises
    /// [`close_window_requested`](Self::close_window_requested) so the host tears
    /// that window down and the panel returns to the in-DE bottom dock.
    pub fn toggle_detach(&mut self) {
        if self.config.dock_position == DockPosition::Detached {
            self.config.dock_position = DockPosition::Bottom;
            self.close_window_requested = true;
            self.detach_requested = false;
        } else {
            self.config.dock_position = DockPosition::Detached;
            self.detach_requested = true;
            self.close_window_requested = false;
        }
    }

    /// Mark that the separate devtools window was closed by the OS / its own F12
    /// or close button — re-dock the panel into the in-DE overlay without
    /// re-raising a teardown request (the window is already gone).
    pub fn on_window_closed(&mut self) {
        if self.config.dock_position == DockPosition::Detached {
            self.config.dock_position = DockPosition::Bottom;
        }
        self.detach_requested = false;
        self.close_window_requested = false;
    }

    /// Whether the console input is focused.
    pub fn is_console_focused(&self) -> bool {
        self.console_focused
    }

    /// Update the scene graph debugger snapshot from a scene root.
    pub fn update_scene_snapshot(&mut self, root: &liquide_compositor::scene::SceneNode) {
        self.scene_debugger.snapshot(root);
    }

    /// Convenience: update the inspector snapshot from the document.
    pub fn refresh_inspector(&mut self, doc: &liquide_dom::Document) {
        self.inspector.build_snapshot(doc);
    }

    /// A cheap, allocation-free fingerprint of every piece of panel state that
    /// changes WHAT the devtools panel should re-serialize / re-render.
    ///
    /// The host throttles the (expensive) devtools refresh — the full DOM-tree
    /// snapshot, the scene-graph snapshot, and the panel `render_template` —
    /// to a low rate instead of every main frame. But a refresh must ALSO fire
    /// promptly on any explicit interaction (tab switch, expand/collapse, scroll,
    /// selection, picker toggle) and on real DOM churn (a new mutation observed),
    /// so the tools stay responsive between the periodic ticks. This signature is
    /// compared frame-to-frame: when it changes the host forces an immediate
    /// refresh; otherwise it waits for the next periodic tick. Computing it is
    /// O(1) (no DOM/scene walk), so it is safe to call every frame.
    pub fn refresh_signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.visible.hash(&mut h);
        (self.active_tab as u8).hash(&mut h);
        (self.side_tab as u8).hash(&mut h);
        (self.config.dock_position as u8).hash(&mut h);
        // Quantise the scroll offset so sub-pixel jitter does not force a refresh
        // but a real scroll (which reveals different virtual rows) does.
        (self.scroll_offset as i64).hash(&mut h);
        self.selected_node.hash(&mut h);
        self.inspector.hovered().hash(&mut h);
        // Tree expand/collapse: the materialised tree snapshot is rebuilt from the
        // inspector's expanded set, so a toggle must bump the signature or the
        // expand/collapse interaction stays frozen until the next periodic tick.
        self.inspector.expansion_fingerprint().hash(&mut h);
        self.element_picker.is_active().hash(&mut h);
        // DOM churn: the running total of observed mutations only advances when
        // the live document actually changed, so a bump means the inspector /
        // mutations tab content is stale and must be rebuilt.
        self.mutation_log.total_count().hash(&mut h);
        h.finish()
    }

    /// Get the dock position.
    pub fn dock_position(&self) -> DockPosition {
        self.config.dock_position
    }

    /// Change the dock position.
    pub fn set_dock_position(&mut self, pos: DockPosition) {
        self.config.dock_position = pos;
    }

    /// Get the panel size.
    pub fn panel_size(&self) -> f32 {
        self.config.panel_size
    }

    /// Resize the panel.
    pub fn set_panel_size(&mut self, size: f32) {
        self.config.panel_size = size.max(self.config.min_panel_size);
    }
}

impl Default for DevToolsPanel {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ── Free helper functions used across sub-modules ──

/// Build a generic key-value row template node.
pub(crate) fn row_kv(label: &str, value: &str, cls: &str) -> TemplateNode {
    TemplateNode::el("devtools-row")
        .child(TemplateNode::el("devtools-label").child(TemplateNode::text(label)))
        .child(
            TemplateNode::el("devtools-value")
                .class(cls)
                .child(TemplateNode::text(value)),
        )
}

/// Build a key-value row identical to `row_kv` — alias kept for call-site clarity
/// where the class indicates a status (ok/warn/error) rather than a colour.
pub(crate) fn row_kv_class(label: &str, value: &str, cls: &str) -> TemplateNode {
    row_kv(label, value, cls)
}

/// Build a key-value row whose VALUE cell is pinned to a fixed pixel `width` and
/// whose value text is marked **paint-only** (t136/t142).
///
/// Use this ONLY for per-frame live numerics whose magnitude is bounded so a
/// generous fixed-width cell can never clip them (e.g. the Performance-tab FPS
/// readout, `0.0`..`999.9`). The fixed `width` makes the value box's geometry
/// content-independent — a same-cell text swap (FPS bump) provably cannot reflow
/// the row or its siblings — so the text update is safely demoted from LAYOUT to
/// PAINT by [`TemplateNode::paint_only`]. The text is right-aligned so the digits
/// stay anchored to the label as the value changes width.
///
/// Do NOT use this for unbounded numerics (e.g. the frame counter, which grows
/// without limit and would eventually overflow any fixed cell); those stay on the
/// conservative LAYOUT path via [`row_kv`].
pub(crate) fn row_kv_fixed_paint_only(
    label: &str,
    value: &str,
    cls: &str,
    width_px: u32,
) -> TemplateNode {
    TemplateNode::el("devtools-row")
        .child(TemplateNode::el("devtools-label").child(TemplateNode::text(label)))
        .child(
            TemplateNode::el("devtools-value")
                .class(cls)
                .style("width", &format!("{}px", width_px))
                .style("text-align", "right")
                .style("overflow", "hidden")
                .child(TemplateNode::text(value).paint_only()),
        )
}

/// Format a `TimingFunction` as a human-readable string.
pub(crate) fn format_timing_function(
    tf: &liquide_style_engine::computed::TimingFunction,
) -> String {
    use liquide_style_engine::computed::TimingFunction;
    match tf {
        TimingFunction::Linear => "linear".to_string(),
        TimingFunction::Ease => "ease".to_string(),
        TimingFunction::EaseIn => "ease-in".to_string(),
        TimingFunction::EaseOut => "ease-out".to_string(),
        TimingFunction::EaseInOut => "ease-in-out".to_string(),
        TimingFunction::CubicBezier(x1, y1, x2, y2) => {
            format!("cubic-bezier({:.2},{:.2},{:.2},{:.2})", x1, y1, x2, y2)
        }
        TimingFunction::Steps(n, pos) => {
            format!("steps({}, {:?})", n, pos)
        }
    }
}

/// Format a mutation record as a single-line description.
pub(crate) fn format_mutation_record(record: &crate::mutation_log::MutationRecord) -> String {
    use crate::mutation_log::MutationKind;

    let ts = record.timestamp_ms;
    let desc = match &record.kind {
        MutationKind::ChildAdded { parent, child } => {
            format!("+child #{} \u{2192} parent #{}", child, parent)
        }
        MutationKind::ChildRemoved { parent, child } => {
            format!("-child #{} \u{2190} parent #{}", child, parent)
        }
        MutationKind::AttributeChanged {
            node,
            attribute,
            new_value,
            ..
        } => {
            let val = new_value.as_deref().unwrap_or("(removed)");
            format!("attr #{} {}=\"{}\"", node, attribute, val)
        }
        MutationKind::ClassChanged { node, classes } => {
            format!("class #{} \u{2192} [{}]", node, classes.join(" "))
        }
        MutationKind::TextChanged { node, text } => {
            let t = if text.len() > 40 { &text[..40] } else { text };
            format!("text #{} \"{}\"", node, t)
        }
        MutationKind::PseudoStateChanged {
            node, new_flags, ..
        } => {
            format!("pseudo #{} flags={:#x}", node, new_flags)
        }
        MutationKind::IdChanged { node, new_id, .. } => {
            let id = new_id.as_deref().unwrap_or("(none)");
            format!("id #{} \u{2192} \"{}\"", node, id)
        }
    };
    format!("[{:>6}ms] {}", ts, desc)
}

/// Pick a CSS class name for different mutation kinds.
pub(crate) fn mutation_class(kind: &crate::mutation_log::MutationKind) -> &'static str {
    use crate::mutation_log::MutationKind;
    match kind {
        MutationKind::ChildAdded { .. } => "ok",
        MutationKind::ChildRemoved { .. } => "error",
        MutationKind::AttributeChanged { .. } => "blue",
        MutationKind::ClassChanged { .. } => "warn",
        MutationKind::TextChanged { .. } => "dim",
        MutationKind::PseudoStateChanged { .. } => "teal",
        MutationKind::IdChanged { .. } => "purple",
    }
}
