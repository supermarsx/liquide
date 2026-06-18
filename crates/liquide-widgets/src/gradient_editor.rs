//! `<lq-gradient-editor>` — colour stops along a horizontal bar (CREATIVE/PRO).
//!
//! State: an ordered list of stops, each a `(position, rgb)` with `position` in
//! `0..=1` along the bar, plus the index of the selected stop and a small swatch
//! palette to recolour it. Behavior:
//! - **Click the bar** (`data-part="bar"`, away from an existing stop): ADDS a
//!   stop at that position — computed from `fraction_along_x` of the LAID-OUT BAR
//!   box, never a constant — selecting it.
//! - **Click a stop handle** (`data-part="stop-<i>"`): selects it (its laid-out
//!   handle box is the hit target).
//! - **Drag a stop**: moves it along the bar (position from the laid-out bar box);
//!   stops are kept ordered by position; the selected index follows the moved
//!   stop.
//! - **Click a palette swatch** (`data-part="swatch-<i>"`): recolours the selected
//!   stop (reusing the colour-picker swatch model).
//! - **Keyboard** (focused): Left/Right nudge the selected stop's position;
//!   Delete/Backspace removes it (keeping at least two stops).
//! - Each stop handle is positioned from its value as a percentage of the laid-out
//!   bar; the bar paints a CSS `linear-gradient` of the stops (the value IS the
//!   data); all box geometry is CSS.
//! - Emits `Changed(stops)` where stops = `"pos:#RRGGBB;..."`.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::color_picker::{Rgb, DEFAULT_PALETTE};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action emitted when the stops change (payload: `"pos:#RRGGBB;..."`).
pub const CHANGED_ACTION: &str = "changed";

/// How close (in fraction-of-bar) a click must be to an existing stop to select
/// it rather than add a new one.
const SELECT_TOLERANCE: f32 = 0.04;

/// A single gradient stop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stop {
    /// Position along the bar, 0..=1.
    pub pos: f32,
    /// The stop's colour.
    pub color: Rgb,
}

/// A gradient stop editor.
#[derive(Debug, Clone)]
pub struct GradientEditor {
    stops: Vec<Stop>,
    selected: usize,
    palette: Vec<Rgb>,
    dragging: bool,
    disabled: bool,
}

impl GradientEditor {
    /// A two-stop black→white gradient.
    pub fn new() -> Self {
        Self::with_stops(vec![
            Stop {
                pos: 0.0,
                color: (0, 0, 0),
            },
            Stop {
                pos: 1.0,
                color: (255, 255, 255),
            },
        ])
    }

    /// An editor over an explicit stop list (sorted by position; at least two
    /// stops are kept — a degenerate list is padded).
    pub fn with_stops(stops: impl IntoIterator<Item = Stop>) -> Self {
        let mut stops: Vec<Stop> = stops.into_iter().collect();
        if stops.is_empty() {
            stops.push(Stop {
                pos: 0.0,
                color: (0, 0, 0),
            });
        }
        if stops.len() == 1 {
            stops.push(Stop {
                pos: 1.0,
                color: (255, 255, 255),
            });
        }
        stops.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
        Self {
            stops,
            selected: 0,
            palette: DEFAULT_PALETTE.to_vec(),
            dragging: false,
            disabled: false,
        }
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The current stops (ordered by position).
    pub fn stops(&self) -> &[Stop] {
        &self.stops
    }

    /// The number of stops.
    pub fn stop_count(&self) -> usize {
        self.stops.len()
    }

    /// The selected stop index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// The selected stop, if any.
    pub fn selected_stop(&self) -> Option<Stop> {
        self.stops.get(self.selected).copied()
    }

    /// Whether a stop is being dragged.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    fn hex(c: Rgb) -> String {
        format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2)
    }

    fn css_rgb(c: Rgb) -> String {
        format!("rgb({}, {}, {})", c.0, c.1, c.2)
    }

    fn stop_part(i: usize) -> String {
        format!("stop-{i}")
    }
    fn swatch_part(i: usize) -> String {
        format!("swatch-{i}")
    }

    /// Serialize the stops as the Changed payload.
    fn payload(&self) -> String {
        self.stops
            .iter()
            .map(|s| format!("{:.3}:{}", s.pos, Self::hex(s.color)))
            .collect::<Vec<_>>()
            .join(";")
    }

    /// The CSS `linear-gradient(...)` string painting the current stops.
    fn gradient_css(&self) -> String {
        let parts: Vec<String> = self
            .stops
            .iter()
            .map(|s| format!("{} {:.1}%", Self::css_rgb(s.color), s.pos * 100.0))
            .collect();
        format!("linear-gradient(to right, {})", parts.join(", "))
    }

    /// Re-sort stops by position and keep `selected` pointing at the same stop.
    fn resort_keeping_selection(&mut self) {
        if self.stops.is_empty() {
            self.selected = 0;
            return;
        }
        let sel_ptr = self.stops[self.selected.min(self.stops.len() - 1)];
        self.stops
            .sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
        // Find where the selected stop landed (by identity of pos+color; ties
        // resolve to the first match, which is acceptable for selection).
        self.selected = self
            .stops
            .iter()
            .position(|s| *s == sel_ptr)
            .unwrap_or(0);
    }

    /// Index of the stop whose laid-out handle box contains `p`, if any.
    fn stop_at(&self, root: NodeId, p: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.stops.len() {
            if let Some(r) = layout.box_of_part(root, &Self::stop_part(i)) {
                if r.contains(p) {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Index of the palette swatch whose laid-out box contains `p`, if any.
    fn swatch_at(&self, root: NodeId, p: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.palette.len() {
            if let Some(r) = layout.box_of_part(root, &Self::swatch_part(i)) {
                if r.contains(p) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn add_stop_at(&mut self, pos: f32) -> WidgetOutcome {
        let pos = pos.clamp(0.0, 1.0);
        // Colour the new stop by sampling the existing gradient at `pos` (nearest
        // stop's colour) so it visually appears where clicked.
        let color = self
            .stops
            .iter()
            .min_by(|a, b| {
                (a.pos - pos)
                    .abs()
                    .partial_cmp(&(b.pos - pos).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|s| s.color)
            .unwrap_or((128, 128, 128));
        self.stops.push(Stop { pos, color });
        // Select the newly added stop.
        let added = Stop { pos, color };
        self.stops
            .sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
        self.selected = self.stops.iter().position(|s| *s == added).unwrap_or(0);
        WidgetOutcome::action_with(CHANGED_ACTION, self.payload())
    }

    fn move_selected(&mut self, pos: f32) -> WidgetOutcome {
        if self.selected >= self.stops.len() {
            return WidgetOutcome::Ignored;
        }
        let pos = pos.clamp(0.0, 1.0);
        if (self.stops[self.selected].pos - pos).abs() < 1e-4 {
            return WidgetOutcome::Ignored;
        }
        self.stops[self.selected].pos = pos;
        self.resort_keeping_selection();
        WidgetOutcome::action_with(CHANGED_ACTION, self.payload())
    }

    fn recolor_selected(&mut self, color: Rgb) -> WidgetOutcome {
        if self.selected >= self.stops.len() {
            return WidgetOutcome::Ignored;
        }
        if self.stops[self.selected].color == color {
            return WidgetOutcome::Ignored;
        }
        self.stops[self.selected].color = color;
        WidgetOutcome::action_with(CHANGED_ACTION, self.payload())
    }

    fn remove_selected(&mut self) -> WidgetOutcome {
        if self.stops.len() <= 2 {
            return WidgetOutcome::Ignored;
        }
        self.stops.remove(self.selected);
        if self.selected >= self.stops.len() {
            self.selected = self.stops.len() - 1;
        }
        WidgetOutcome::action_with(CHANGED_ACTION, self.payload())
    }

    fn bar_fraction(&self, root: NodeId, x: f32, y: f32, layout: &LayoutQuery) -> Option<f32> {
        let bar = layout.box_of_part(root, "bar")?;
        Some(LayoutQuery::fraction_along_x(bar, Point::new(x, y)))
    }
}

impl Default for GradientEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetBehavior for GradientEditor {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Other
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseDown {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
            DomEventKind::MouseMove { x: 0.0, y: 0.0 },
            DomEventKind::MouseUp {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
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
            DomEventKind::MouseDown {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let p = Point::new(*x, *y);
                // A press on a stop handle selects + begins a drag of it.
                if let Some(i) = self.stop_at(root, p, layout) {
                    self.selected = i;
                    self.dragging = true;
                    return WidgetOutcome::Changed;
                }
                // A press on a palette swatch recolours the selected stop.
                if let Some(i) = self.swatch_at(root, p, layout) {
                    let c = self.palette[i];
                    return self.recolor_selected(c);
                }
                // A press on the bar (away from a stop) adds a stop there.
                if let Some(frac) = self.bar_fraction(root, *x, *y, layout) {
                    // If we're within tolerance of an existing stop, select it
                    // instead of adding a duplicate.
                    if let Some((i, _)) = self
                        .stops
                        .iter()
                        .enumerate()
                        .find(|(_, s)| (s.pos - frac).abs() <= SELECT_TOLERANCE)
                    {
                        self.selected = i;
                        self.dragging = true;
                        return WidgetOutcome::Changed;
                    }
                    return self.add_stop_at(frac);
                }
                WidgetOutcome::Ignored
            }
            DomEventKind::MouseMove { x, y } => {
                if !self.dragging {
                    return WidgetOutcome::Ignored;
                }
                let Some(frac) = self.bar_fraction(root, *x, *y, layout) else {
                    return WidgetOutcome::Ignored;
                };
                self.move_selected(frac)
            }
            DomEventKind::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                if !self.dragging {
                    return WidgetOutcome::Ignored;
                }
                self.dragging = false;
                WidgetOutcome::Changed
            }
            // Click is consumed (down/up already did the work) so it doesn't
            // double-add; swatch clicks are handled on the down path.
            DomEventKind::Click { .. } => WidgetOutcome::Ignored,
            _ => WidgetOutcome::Ignored,
        }
    }

    fn on_keyboard(
        &mut self,
        _root: NodeId,
        key: KeyInput,
        _layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        let cur = self.selected_stop().map(|s| s.pos).unwrap_or(0.0);
        match key.key {
            keys::ARROW_LEFT => self.move_selected(cur - 0.02),
            keys::ARROW_RIGHT => self.move_selected(cur + 0.02),
            keys::DELETE | keys::BACKSPACE => self.remove_selected(),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut node = TemplateNode::el("lq-gradient-editor")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("data-stops", &self.payload())
            .pseudo_if(PseudoStateFlags::ACTIVE, self.dragging && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        // The bar paints the gradient (value IS the data) and hosts the stop
        // handles; its box geometry is CSS. Stops are placed by the proportional
        // flex-spacer pattern (percentage `left` on absolute children does NOT
        // resolve against the parent in this engine): the bar is a flex row of
        //   [spacer:pos0] [stop0] [spacer:pos1-pos0] [stop1] ... [spacer:rest]
        // so each stop's laid-out box sits at its position fraction of the REAL
        // bar width — the position the behavior reads back is genuinely
        // geometry-derived.
        let mut bar = TemplateNode::el("lq-gradient-bar")
            .attr("data-part", "bar")
            .style("background-image", &self.gradient_css());
        let grow = |frac: f32| -> String { format!("{}", (frac.max(0.0) * 1000.0).round()) };
        let mut prev = 0.0_f32;
        for (i, s) in self.stops.iter().enumerate() {
            let is_sel = i == self.selected;
            // The leading spacer advances the flex cursor to this stop's pos.
            bar = bar.child(
                TemplateNode::el("lq-gradient-spacer")
                    .attr("data-part", &format!("gap-{i}"))
                    .style("flex-grow", &grow(s.pos - prev)),
            );
            let handle = TemplateNode::el("lq-gradient-stop")
                .key(&format!("stop-{i}"))
                .attr("data-part", &Self::stop_part(i))
                .attr("data-index", &format!("{i}"))
                .attr("data-pos", &format!("{:.3}", s.pos))
                .attr("data-color", &Self::hex(s.color))
                .class_if("selected", is_sel)
                .pseudo_if(PseudoStateFlags::CHECKED, is_sel)
                .style("background-color", &Self::css_rgb(s.color));
            bar = bar.child(handle);
            prev = s.pos;
        }
        // Trailing spacer to the bar's end.
        bar = bar.child(
            TemplateNode::el("lq-gradient-spacer")
                .attr("data-part", "gap-rest")
                .style("flex-grow", &grow(1.0 - prev)),
        );
        node = node.child(bar);

        // The recolour palette for the selected stop.
        let mut grid = TemplateNode::el("lq-gradient-palette").attr("data-part", "palette");
        let sel_color = self.selected_stop().map(|s| s.color);
        for (i, &c) in self.palette.iter().enumerate() {
            let is_cur = sel_color == Some(c);
            let cell = TemplateNode::el("lq-gradient-swatch")
                .key(&format!("swatch-{i}"))
                .attr("data-part", &Self::swatch_part(i))
                .attr("data-index", &format!("{i}"))
                .attr("data-color", &Self::hex(c))
                .attr("role", "button")
                .style("background-color", &Self::css_rgb(c))
                .class_if("selected", is_cur)
                .pseudo_if(PseudoStateFlags::CHECKED, is_cur);
            grid = grid.child(cell);
        }
        node = node.child(grid);

        if self.disabled {
            node = node.attr("disabled", "true");
        }
        node
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
