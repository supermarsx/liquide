//! `<lq-color-picker>` — a swatch grid + RGB channel readout (Group D: D8 part 2).
//!
//! State: a current `(r, g, b)` colour + a palette of selectable swatches + the
//! index of the focused swatch. The widget is a single subtree: a
//! `data-part="button"` trigger showing the current colour, and — when open — a
//! `data-part="popup"` swatch grid (`swatch-<i>` cells) plus a `channels`
//! readout. Behavior:
//!
//! - **Click the button**: toggles the popup.
//! - **Click a swatch's LAID-OUT box** (`data-part="swatch-<i>"`): selects that
//!   colour + closes; emits `Changed`(#RRGGBB). Hit per-swatch from layout, never
//!   a constant grid offset.
//! - **Keyboard** (open): arrows move the focused swatch across the grid (Left/
//!   Right ±1, Up/Down ±cols), Enter selects, Esc closes.
//!
//! The swatch FILL colour is an inline `background-color` (the value IS the data),
//! but every box GEOMETRY comes from CSS/layout. This is a deliberate, non-stub
//! channel readout (text), not a full HSV canvas — that richer surface is a
//! follow-up; this ships a real, interactive picker.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when a colour is selected (payload: `#RRGGBB`).
pub const CHANGED_ACTION: &str = "changed";

/// An (r, g, b) colour, each channel 0..=255.
pub type Rgb = (u8, u8, u8);

/// A default palette of 12 colours arranged in a 6-column grid.
pub const DEFAULT_PALETTE: [Rgb; 12] = [
    (239, 68, 68),   // red
    (249, 115, 22),  // orange
    (234, 179, 8),   // yellow
    (34, 197, 94),   // green
    (20, 184, 166),  // teal
    (59, 130, 246),  // blue
    (139, 92, 246),  // violet
    (236, 72, 153),  // pink
    (250, 250, 250), // white
    (161, 161, 170), // gray
    (63, 63, 70),    // dark gray
    (9, 9, 11),      // black
];

/// A swatch-grid colour picker.
#[derive(Debug, Clone)]
pub struct ColorPicker {
    /// The palette swatches.
    palette: Vec<Rgb>,
    /// Columns in the swatch grid (for keyboard up/down).
    columns: usize,
    /// The selected swatch index, if any.
    selected: Option<usize>,
    /// The keyboard-focused swatch index.
    focus: usize,
    /// The hovered swatch index.
    hovered: Option<usize>,
    open: bool,
}

impl ColorPicker {
    /// A picker over the default 12-colour palette (6 columns).
    pub fn new() -> Self {
        Self {
            palette: DEFAULT_PALETTE.to_vec(),
            columns: 6,
            selected: None,
            focus: 0,
            hovered: None,
            open: false,
        }
    }

    /// A picker over a custom palette with `columns` columns.
    pub fn with_palette(palette: impl IntoIterator<Item = Rgb>, columns: usize) -> Self {
        Self {
            palette: palette.into_iter().collect(),
            columns: columns.max(1),
            selected: None,
            focus: 0,
            hovered: None,
            open: false,
        }
    }

    /// Pre-select a swatch by index.
    pub fn select(mut self, idx: usize) -> Self {
        if idx < self.palette.len() {
            self.selected = Some(idx);
            self.focus = idx;
        }
        self
    }

    /// Whether the popup is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The selected swatch index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    /// The selected colour.
    pub fn selected_color(&self) -> Option<Rgb> {
        self.selected.and_then(|i| self.palette.get(i).copied())
    }

    /// The keyboard-focused swatch index.
    pub fn focus(&self) -> usize {
        self.focus
    }

    /// Format an rgb as `#RRGGBB`.
    pub fn hex(c: Rgb) -> String {
        format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2)
    }

    fn swatch_part(i: usize) -> String {
        format!("swatch-{i}")
    }

    fn css_color(c: Rgb) -> String {
        format!("rgb({}, {}, {})", c.0, c.1, c.2)
    }

    fn open_popup(&mut self) -> WidgetOutcome {
        if self.open {
            return WidgetOutcome::Ignored;
        }
        self.open = true;
        if let Some(i) = self.selected {
            self.focus = i;
        }
        WidgetOutcome::Changed
    }

    fn close_popup(&mut self) -> WidgetOutcome {
        if !self.open {
            return WidgetOutcome::Ignored;
        }
        self.open = false;
        self.hovered = None;
        WidgetOutcome::Changed
    }

    fn choose(&mut self, idx: usize) -> WidgetOutcome {
        if idx >= self.palette.len() {
            return WidgetOutcome::Ignored;
        }
        let changed = self.selected != Some(idx);
        self.selected = Some(idx);
        self.focus = idx;
        self.open = false;
        self.hovered = None;
        if changed {
            WidgetOutcome::action_with(CHANGED_ACTION, Self::hex(self.palette[idx]))
        } else {
            WidgetOutcome::Changed
        }
    }

    fn move_focus(&mut self, delta: isize) -> WidgetOutcome {
        let n = self.palette.len() as isize;
        if n == 0 {
            return WidgetOutcome::Ignored;
        }
        let nf = (self.focus as isize + delta).clamp(0, n - 1) as usize;
        if nf == self.focus {
            return WidgetOutcome::Ignored;
        }
        self.focus = nf;
        WidgetOutcome::Changed
    }

    fn swatch_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.palette.len() {
            if let Some(r) = layout.box_of_part(root, &Self::swatch_part(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }
}

impl Default for ColorPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetBehavior for ColorPicker {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Other
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
            DomEventKind::MouseMove { x, y } => {
                if !self.open {
                    return WidgetOutcome::Ignored;
                }
                let hit = self.swatch_at(root, Point::new(*x, *y), layout);
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
            } => {
                let p = Point::new(*x, *y);
                if !self.open {
                    if layout
                        .box_of_part(root, "button")
                        .map(|r| r.contains(p))
                        .unwrap_or(false)
                    {
                        return self.open_popup();
                    }
                    return WidgetOutcome::Ignored;
                }
                if let Some(i) = self.swatch_at(root, p, layout) {
                    return self.choose(i);
                }
                self.close_popup()
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
        let cols = self.columns as isize;
        match key.key {
            keys::ARROW_DOWN if !self.open => self.open_popup(),
            keys::ENTER if !self.open => self.open_popup(),
            keys::ARROW_LEFT => self.move_focus(-1),
            keys::ARROW_RIGHT => self.move_focus(1),
            keys::ARROW_UP => self.move_focus(-cols),
            keys::ARROW_DOWN => self.move_focus(cols),
            keys::ENTER => self.choose(self.focus),
            keys::ESCAPE => self.close_popup(),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn render(&self) -> TemplateNode {
        let mut root = TemplateNode::el("lq-color-picker")
            .attr(FOCUSABLE_ATTR, "true")
            .attr("aria-expanded", if self.open { "true" } else { "false" })
            .class_if("open", self.open)
            .pseudo_if(PseudoStateFlags::ACTIVE, self.open);

        let sel = self.selected_color();
        let mut button = TemplateNode::el("lq-color-button").attr("data-part", "button");
        // The swatch preview carries the colour as an inline fill (the value IS
        // the data); its box geometry is CSS.
        let mut preview = TemplateNode::el("lq-color-preview").attr("data-part", "preview");
        if let Some(c) = sel {
            preview = preview.style("background-color", &Self::css_color(c));
        }
        button = button.child(preview).child(
            TemplateNode::el("lq-color-value").child(TemplateNode::text(
                &sel.map(Self::hex).unwrap_or_else(|| "Pick…".to_string()),
            )),
        );
        root = root.child(button);

        if self.open {
            let mut popup = TemplateNode::el("lq-popup").attr("data-part", "popup");

            let mut grid = TemplateNode::el("lq-color-grid");
            for (i, &c) in self.palette.iter().enumerate() {
                let is_sel = self.selected == Some(i);
                let is_focus = self.focus == i;
                let cell = TemplateNode::el("lq-swatch")
                    .key(&format!("swatch-{i}"))
                    .attr("data-part", &Self::swatch_part(i))
                    .attr("data-index", &format!("{i}"))
                    .attr("data-color", &Self::hex(c))
                    .attr("role", "button")
                    .style("background-color", &Self::css_color(c))
                    .class_if("selected", is_sel)
                    .pseudo_if(PseudoStateFlags::CHECKED, is_sel)
                    .pseudo_if(PseudoStateFlags::FOCUS, is_focus)
                    .pseudo_if(PseudoStateFlags::HOVER, self.hovered == Some(i));
                grid = grid.child(cell);
            }
            popup = popup.child(grid);

            // The channel readout (R/G/B of the focused or selected swatch).
            let shown = self
                .palette
                .get(self.focus)
                .copied()
                .or(sel)
                .unwrap_or((0, 0, 0));
            let channels = TemplateNode::el("lq-color-channels")
                .attr("data-part", "channels")
                .child(
                    TemplateNode::el("lq-channel")
                        .child(TemplateNode::text(&format!("R {}", shown.0))),
                )
                .child(
                    TemplateNode::el("lq-channel")
                        .child(TemplateNode::text(&format!("G {}", shown.1))),
                )
                .child(
                    TemplateNode::el("lq-channel")
                        .child(TemplateNode::text(&format!("B {}", shown.2))),
                );
            popup = popup.child(channels);
            root = root.child(popup);
        }
        root
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
