//! `<lq-checkbox>` / `<lq-switch>` / `<lq-radio>` — two-state + exclusive toggles
//! (Group A: A4).
//!
//! Two behaviors cover the family:
//!
//! - [`Toggle`] drives a self-contained checkbox **or** switch: a click on the
//!   laid-out box (or **Space** when focused) flips a boolean `checked` state,
//!   toggling the `:checked` pseudo-state (CSS shows the check glyph / switch
//!   thumb) and emitting a `Changed`(`"true"`/`"false"`) Action. The element tag
//!   (`lq-checkbox` vs `lq-switch`) is chosen by [`ToggleStyle`] so one behavior
//!   styles two appearances.
//! - [`RadioGroup`] drives an EXCLUSIVE set: it owns N options under one mount
//!   (each an `lq-radio` with `data-part="option-<i>"`), so selecting one
//!   deselects the others by construction (single source of truth — no
//!   cross-widget coordination, no double-fire). Click an option's laid-out box,
//!   or **Up/Down/Left/Right** to move the selection when focused.
//!
//! All states carry `:hover`/`:focus`/`:disabled` too, styled in CSS.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action a toggle/radio emits on change.
pub const CHANGED_ACTION: &str = "changed";

/// Whether a [`Toggle`] renders as a checkbox or a switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleStyle {
    /// A box with a check glyph (`<lq-checkbox>`).
    Checkbox,
    /// A sliding switch (`<lq-switch>`).
    Switch,
}

impl ToggleStyle {
    fn tag(self) -> &'static str {
        match self {
            ToggleStyle::Checkbox => "lq-checkbox",
            ToggleStyle::Switch => "lq-switch",
        }
    }
}

/// A self-contained two-state toggle (checkbox or switch).
#[derive(Debug, Clone)]
pub struct Toggle {
    style: ToggleStyle,
    label: String,
    checked: bool,
    hovered: bool,
    disabled: bool,
}

impl Toggle {
    /// A checkbox labelled `label`.
    pub fn checkbox(label: impl Into<String>) -> Self {
        Self::new(ToggleStyle::Checkbox, label)
    }

    /// A switch labelled `label`.
    pub fn switch(label: impl Into<String>) -> Self {
        Self::new(ToggleStyle::Switch, label)
    }

    fn new(style: ToggleStyle, label: impl Into<String>) -> Self {
        Self {
            style,
            label: label.into(),
            checked: false,
            hovered: false,
            disabled: false,
        }
    }

    /// Start in the checked state.
    pub fn checked(mut self, c: bool) -> Self {
        self.checked = c;
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Whether currently checked.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Whether disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    fn toggle(&mut self) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        self.checked = !self.checked;
        WidgetOutcome::action_with(CHANGED_ACTION, if self.checked { "true" } else { "false" })
    }
}

impl WidgetBehavior for Toggle {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Toggle
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseEnter,
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
        match &event.kind {
            DomEventKind::MouseEnter => {
                if self.hovered {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = true;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseLeave => {
                if !self.hovered {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = false;
                WidgetOutcome::Changed
            }
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let inside = layout
                    .box_of(root)
                    .map(|r| r.contains(Point::new(*x, *y)))
                    .unwrap_or(false);
                if !inside {
                    return WidgetOutcome::Ignored;
                }
                self.toggle()
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    fn on_keyboard(
        &mut self,
        _root: NodeId,
        key: KeyInput,
        _layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if key.key == keys::SPACE {
            return self.toggle();
        }
        WidgetOutcome::Ignored
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut node = TemplateNode::el(self.style.tag())
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .pseudo_if(PseudoStateFlags::CHECKED, self.checked)
            .pseudo_if(PseudoStateFlags::HOVER, self.hovered && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(
                // The check-glyph / switch-thumb surface. The check glyph is a
                // CSS ::before on this element when :checked.
                TemplateNode::el("lq-indicator").attr("data-part", "indicator"),
            )
            .child(
                TemplateNode::el("lq-label")
                    .attr("data-part", "label")
                    .child(TemplateNode::text(&self.label)),
            );
        if self.disabled {
            node = node.attr("disabled", "true");
        }
        node
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// An exclusive radio group: one option selected at a time, enforced in the
/// behavior (single source of truth — no cross-widget coordination).
#[derive(Debug, Clone)]
pub struct RadioGroup {
    /// Group name (becomes the `name` attribute).
    name: String,
    /// Option (value, label) pairs in order.
    options: Vec<(String, String)>,
    /// Index of the selected option.
    selected: usize,
    /// Index of the hovered option, if any.
    hovered: Option<usize>,
    disabled: bool,
}

impl RadioGroup {
    /// A radio group named `name` over `(value, label)` options; the first is
    /// selected initially.
    pub fn new(
        name: impl Into<String>,
        options: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        Self {
            name: name.into(),
            options: options.into_iter().collect(),
            selected: 0,
            hovered: None,
            disabled: false,
        }
    }

    /// Select an initial option by index.
    pub fn select(mut self, idx: usize) -> Self {
        if idx < self.options.len() {
            self.selected = idx;
        }
        self
    }

    /// Mark the whole group disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The selected option index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// The selected option value.
    pub fn selected_value(&self) -> Option<&str> {
        self.options.get(self.selected).map(|(v, _)| v.as_str())
    }

    fn part_name(i: usize) -> String {
        format!("option-{i}")
    }

    /// Select `idx`; returns whether the selection changed (deselecting others
    /// is implicit — there is only one `selected` field).
    fn set_selected(&mut self, idx: usize) -> WidgetOutcome {
        if self.disabled || idx >= self.options.len() || idx == self.selected {
            return WidgetOutcome::Ignored;
        }
        self.selected = idx;
        WidgetOutcome::action_with(CHANGED_ACTION, self.options[idx].0.clone())
    }
}

impl WidgetBehavior for RadioGroup {
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
        match &event.kind {
            DomEventKind::MouseLeave => {
                if self.hovered.is_none() {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = None;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseMove { x, y } | DomEventKind::Click { x, y, .. } => {
                // Which option's LAID-OUT box contains the point? (geometry from
                // layout per-option, located by data-part — never an index math
                // over a constant row height).
                let p = Point::new(*x, *y);
                let mut hit = None;
                for i in 0..self.options.len() {
                    if let Some(r) = layout.box_of_part(root, &Self::part_name(i)) {
                        if r.contains(p) {
                            hit = Some(i);
                            break;
                        }
                    }
                }
                if matches!(event.kind, DomEventKind::Click { .. }) {
                    match hit {
                        Some(i) => self.set_selected(i),
                        None => WidgetOutcome::Ignored,
                    }
                } else {
                    // MouseMove -> hover tracking.
                    if hit == self.hovered {
                        return WidgetOutcome::Ignored;
                    }
                    self.hovered = hit;
                    WidgetOutcome::Changed
                }
            }
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
        let next = match key.key {
            keys::ARROW_DOWN | keys::ARROW_RIGHT => (self.selected + 1) % n,
            keys::ARROW_UP | keys::ARROW_LEFT => (self.selected + n - 1) % n,
            _ => return WidgetOutcome::Ignored,
        };
        self.set_selected(next)
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut group = TemplateNode::el("lq-radiogroup")
            .attr("name", &self.name)
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "radiogroup")
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        for (i, (value, label)) in self.options.iter().enumerate() {
            let opt = TemplateNode::el("lq-radio")
                .key(value)
                .attr("data-part", &Self::part_name(i))
                .attr("data-value", value)
                .pseudo_if(PseudoStateFlags::CHECKED, i == self.selected)
                .pseudo_if(
                    PseudoStateFlags::HOVER,
                    self.hovered == Some(i) && !self.disabled,
                )
                .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
                .child(TemplateNode::el("lq-indicator").attr("data-part", "indicator"))
                .child(
                    TemplateNode::el("lq-label")
                        .attr("data-part", "label")
                        .child(TemplateNode::text(label)),
                );
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
