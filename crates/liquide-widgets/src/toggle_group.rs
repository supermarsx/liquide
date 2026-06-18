//! `<lq-toggle-group>` — a group of toggle-buttons (COMP-6).
//!
//! A row of toggle-buttons in one of two modes:
//!
//! - **Single-select** (radio-like): exactly one button is active; clicking
//!   another moves the selection. (Optionally allow zero with
//!   [`ToggleGroup::allow_deselect`].)
//! - **Multi-select**: each button toggles independently; any subset can be
//!   active.
//!
//! Behavior:
//!
//! - **Click** a button's LAID-OUT box (`data-part="opt-<i>"`) toggles/selects
//!   it (hit-tested against the real box, never `index * width`).
//! - **Left/Right (and Up/Down) arrows** move a roving focus cursor across
//!   buttons (wrapping); **Space/Enter** toggles the focused button; **Home/End**
//!   jump the cursor to the first/last.
//! - Active buttons carry `:checked`/`.active`; the focus cursor carries `:focus`.
//! - Emits `Changed`(selection) — the comma-joined active values.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the selection changes (payload: comma-joined active values).
pub const CHANGED_ACTION: &str = "changed";

/// Selection mode of a toggle group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleMode {
    /// At most one button active (radio-like).
    Single,
    /// Any subset of buttons active.
    Multi,
}

/// A group of toggle-buttons.
#[derive(Debug, Clone)]
pub struct ToggleGroup {
    /// (value, label) options in order.
    options: Vec<(String, String)>,
    /// Active flags per option (parallel to `options`).
    active: Vec<bool>,
    /// Selection mode.
    mode: ToggleMode,
    /// In Single mode, whether clicking the active button deselects it.
    allow_deselect: bool,
    /// Roving keyboard focus cursor index.
    cursor: usize,
    /// Hovered option (mouse).
    hovered: Option<usize>,
    disabled: bool,
}

impl ToggleGroup {
    /// A single-select group over `(value, label)` options (first selected).
    pub fn single(options: impl IntoIterator<Item = (String, String)>) -> Self {
        let options: Vec<_> = options.into_iter().collect();
        let mut active = vec![false; options.len()];
        if !active.is_empty() {
            active[0] = true;
        }
        Self {
            options,
            active,
            mode: ToggleMode::Single,
            allow_deselect: false,
            cursor: 0,
            hovered: None,
            disabled: false,
        }
    }

    /// A multi-select group over `(value, label)` options (none selected).
    pub fn multi(options: impl IntoIterator<Item = (String, String)>) -> Self {
        let options: Vec<_> = options.into_iter().collect();
        let active = vec![false; options.len()];
        Self {
            options,
            active,
            mode: ToggleMode::Multi,
            allow_deselect: true,
            cursor: 0,
            hovered: None,
            disabled: false,
        }
    }

    /// In Single mode, allow clicking the active button to deselect (zero active).
    pub fn allow_deselect(mut self, a: bool) -> Self {
        self.allow_deselect = a;
        self
    }

    /// Pre-select an option by index.
    pub fn select(mut self, idx: usize) -> Self {
        if idx < self.active.len() {
            if self.mode == ToggleMode::Single {
                self.active.iter_mut().for_each(|a| *a = false);
            }
            self.active[idx] = true;
            self.cursor = idx;
        }
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The selection mode.
    pub fn mode(&self) -> ToggleMode {
        self.mode
    }

    /// Whether option `idx` is active.
    pub fn is_active(&self, idx: usize) -> bool {
        self.active.get(idx).copied().unwrap_or(false)
    }

    /// The active option values, in order.
    pub fn selection(&self) -> Vec<String> {
        self.options
            .iter()
            .zip(&self.active)
            .filter(|&(_, &a)| a)
            .map(|((v, _), _)| v.clone())
            .collect()
    }

    /// The roving focus cursor index.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Number of options.
    pub fn len(&self) -> usize {
        self.options.len()
    }

    /// Whether there are no options.
    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    fn part_name(i: usize) -> String {
        format!("opt-{i}")
    }

    fn opt_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.options.len() {
            if let Some(r) = layout.box_of_part(root, &Self::part_name(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn changed(&self) -> WidgetOutcome {
        WidgetOutcome::action_with(CHANGED_ACTION, self.selection().join(","))
    }

    /// Toggle/select option `idx` per the mode. Returns whether the selection
    /// state actually changed.
    fn toggle(&mut self, idx: usize) -> WidgetOutcome {
        if idx >= self.options.len() {
            return WidgetOutcome::Ignored;
        }
        match self.mode {
            ToggleMode::Multi => {
                self.active[idx] = !self.active[idx];
                self.cursor = idx;
                self.changed()
            }
            ToggleMode::Single => {
                if self.active[idx] {
                    if self.allow_deselect {
                        self.active[idx] = false;
                        self.cursor = idx;
                        self.changed()
                    } else {
                        // Already selected, no deselect allowed: just move cursor.
                        if self.cursor != idx {
                            self.cursor = idx;
                            return WidgetOutcome::Changed;
                        }
                        WidgetOutcome::Ignored
                    }
                } else {
                    self.active.iter_mut().for_each(|a| *a = false);
                    self.active[idx] = true;
                    self.cursor = idx;
                    self.changed()
                }
            }
        }
    }

    fn move_cursor(&mut self, next: usize) -> WidgetOutcome {
        if next == self.cursor {
            return WidgetOutcome::Ignored;
        }
        self.cursor = next;
        WidgetOutcome::Changed
    }
}

impl WidgetBehavior for ToggleGroup {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Toggle
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseMove { x: 0.0, y: 0.0 },
            DomEventKind::MouseLeave,
            DomEventKind::Click {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
        ]
    }

    fn on_dom_event(
        &mut self,
        root: NodeId,
        event: &DomEvent,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        match &event.kind {
            DomEventKind::MouseLeave => {
                if self.hovered.is_none() {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = None;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseMove { x, y } => {
                let hit = self.opt_at(root, Point::new(*x, *y), layout);
                if hit == self.hovered {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = hit;
                WidgetOutcome::Changed
            }
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => match self.opt_at(root, Point::new(*x, *y), layout) {
                Some(i) => self.toggle(i),
                None => WidgetOutcome::Ignored,
            },
            _ => WidgetOutcome::Ignored,
        }
    }

    fn on_keyboard(
        &mut self,
        _root: NodeId,
        key: KeyInput,
        _layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled || self.options.is_empty() {
            return WidgetOutcome::Ignored;
        }
        let n = self.options.len();
        match key.key {
            keys::ARROW_RIGHT | keys::ARROW_DOWN => {
                let next = (self.cursor + 1) % n;
                self.move_cursor(next)
            }
            keys::ARROW_LEFT | keys::ARROW_UP => {
                let next = (self.cursor + n - 1) % n;
                self.move_cursor(next)
            }
            keys::HOME => self.move_cursor(0),
            keys::END => self.move_cursor(n - 1),
            keys::SPACE | keys::ENTER => self.toggle(self.cursor),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let single = self.mode == ToggleMode::Single;
        let mut group = TemplateNode::el("lq-toggle-group")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", if single { "radiogroup" } else { "group" })
            .class_if("single", single)
            .class_if("multi", !single)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        for (i, (value, label)) in self.options.iter().enumerate() {
            let active = self.active[i];
            let focused = i == self.cursor;
            let opt = TemplateNode::el("lq-toggle-opt")
                .key(value)
                .attr("data-part", &Self::part_name(i))
                .attr("data-value", value)
                .attr("role", if single { "radio" } else { "button" })
                .attr("aria-pressed", if active { "true" } else { "false" })
                .class_if("active", active)
                .pseudo_if(PseudoStateFlags::CHECKED, active)
                .pseudo_if(PseudoStateFlags::FOCUS, focused && !self.disabled)
                .pseudo_if(
                    PseudoStateFlags::HOVER,
                    self.hovered == Some(i) && !self.disabled,
                )
                .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
                .child(TemplateNode::text(label));
            group = group.child(opt);
        }
        if self.disabled {
            group = group.attr("disabled", "true");
        }
        group
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
