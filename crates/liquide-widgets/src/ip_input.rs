//! `<lq-ip-input>` — an IPv4 address control (Group GRID: G4).
//!
//! Four octet fields separated by literal dots, like the Win32
//! `SysIPAddress32`. Behavior:
//!
//! - The widget owns FOUR octet buffers + an active octet index (0..=3).
//! - **Click** an octet field (`data-part="octet-<i>"`): focuses that octet —
//!   resolved from the LAID-OUT octet box, never a constant x-split, so the
//!   field widths can differ and the hit still lands correctly.
//! - **Digits**: append to the active octet; auto-advance to the next octet when
//!   the octet reaches 3 digits OR the value would exceed 25 in a way that can
//!   only be a 3-digit completion (Win32 advances on 3 digits).
//! - **`.`**: commit the current octet and advance.
//! - **Up/Down arrows**: increment/decrement the active octet (clamped 0..=255).
//! - **Left/Right arrows**: move between octets (at a field edge).
//! - **Backspace**: delete a digit; at an empty octet, move to the previous one.
//! - Every octet clamps to 0..=255 on commit.
//! - Emits `Changed("a.b.c.d")` whenever any octet's committed value changes.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the address changes (payload: `"a.b.c.d"`).
pub const CHANGED_ACTION: &str = "changed";

/// Number of octets in an IPv4 address.
const OCTETS: usize = 4;

/// An IPv4 address input.
#[derive(Debug, Clone)]
pub struct IpInput {
    /// Per-octet text buffers (each 0..=3 digits, clamped to 0..=255 on commit).
    octets: [String; OCTETS],
    /// The active octet (the one receiving digits).
    active: usize,
    focused: bool,
    disabled: bool,
}

impl IpInput {
    /// An empty IP input (all octets blank, first octet active).
    pub fn new() -> Self {
        Self {
            octets: [String::new(), String::new(), String::new(), String::new()],
            active: 0,
            focused: false,
            disabled: false,
        }
    }

    /// Initialise from four octet values (each clamped to 0..=255).
    pub fn with(a: u32, b: u32, c: u32, d: u32) -> Self {
        let mut s = Self::new();
        for (i, v) in [a, b, c, d].into_iter().enumerate() {
            s.octets[i] = v.min(255).to_string();
        }
        s
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Set focus (host plumbing).
    pub fn set_focused(&mut self, f: bool) {
        self.focused = f;
    }

    /// Whether focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// The active octet index.
    pub fn active_octet(&self) -> usize {
        self.active
    }

    /// The committed value of octet `i` (0 when blank), clamped 0..=255.
    pub fn octet(&self, i: usize) -> u32 {
        self.octets
            .get(i)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
            .min(255)
    }

    /// The raw text in octet `i`.
    pub fn octet_text(&self, i: usize) -> &str {
        self.octets.get(i).map(|s| s.as_str()).unwrap_or("")
    }

    /// The dotted-quad address string from the committed octets.
    pub fn address(&self) -> String {
        (0..OCTETS)
            .map(|i| self.octet(i).to_string())
            .collect::<Vec<_>>()
            .join(".")
    }

    fn octet_part(i: usize) -> String {
        format!("octet-{i}")
    }

    /// Clamp the active octet's buffer to 0..=255 (the canonical 0-255 range).
    fn clamp_active(&mut self) {
        let i = self.active;
        if let Ok(v) = self.octets[i].parse::<u32>() {
            if v > 255 {
                self.octets[i] = "255".to_string();
            }
        }
    }

    /// Emit a Changed action (always reflecting the full address).
    fn changed(&self) -> WidgetOutcome {
        WidgetOutcome::action_with(CHANGED_ACTION, self.address())
    }

    /// Append a digit to the active octet, clamping + auto-advancing per Win32.
    fn push_digit(&mut self, c: char) -> WidgetOutcome {
        let i = self.active;
        // Reject overlong octets (already 3 digits): advance first.
        if self.octets[i].len() >= 3 {
            if self.active < OCTETS - 1 {
                self.active += 1;
                return self.push_digit(c);
            }
            return WidgetOutcome::Ignored;
        }
        let old_value = self.address();
        self.octets[i].push(c);
        // Clamp if the partial value already exceeds 255.
        self.clamp_active();
        // Auto-advance when the octet is "full": 3 digits, or a 2-digit value
        // whose next digit could only overflow (>25 leading means 3rd digit can't
        // fit any 0-9 without exceeding 255 only when >25 — Win32 advances at 3
        // digits, so we keep the simpler 3-digit rule plus the >255 clamp).
        if self.octets[i].len() >= 3 && self.active < OCTETS - 1 {
            self.active += 1;
        }
        if self.address() != old_value {
            self.changed()
        } else {
            WidgetOutcome::Changed
        }
    }

    fn step_active(&mut self, delta: i32) -> WidgetOutcome {
        let i = self.active;
        let v = self.octet(i) as i32;
        let nv = (v + delta).clamp(0, 255) as u32;
        let old = self.address();
        self.octets[i] = nv.to_string();
        if self.address() != old {
            self.changed()
        } else {
            WidgetOutcome::Changed
        }
    }

    fn move_octet(&mut self, delta: i32) -> WidgetOutcome {
        let ni = (self.active as i32 + delta).clamp(0, OCTETS as i32 - 1) as usize;
        if ni == self.active {
            return WidgetOutcome::Ignored;
        }
        // Clamp the octet we are leaving.
        self.clamp_active();
        self.active = ni;
        WidgetOutcome::Changed
    }

    fn backspace(&mut self) -> WidgetOutcome {
        let i = self.active;
        if self.octets[i].pop().is_some() {
            return self.changed();
        }
        // Empty octet: hop to the previous one.
        if self.active > 0 {
            self.active -= 1;
            return WidgetOutcome::Changed;
        }
        WidgetOutcome::Ignored
    }

    fn octet_at(&self, root: NodeId, p: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..OCTETS {
            if let Some(r) = layout.box_of_part(root, &Self::octet_part(i)) {
                if r.contains(p) {
                    return Some(i);
                }
            }
        }
        None
    }
}

impl Default for IpInput {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetBehavior for IpInput {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Input
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        }]
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
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let p = Point::new(*x, *y);
                if let Some(i) = self.octet_at(root, p, layout) {
                    // Clamp the octet we are leaving, focus the clicked octet.
                    self.clamp_active();
                    self.active = i;
                    self.focused = true;
                    return WidgetOutcome::Changed;
                }
                // A click anywhere inside the widget focuses it.
                if layout.box_of(root).map(|r| r.contains(p)).unwrap_or(false) && !self.focused {
                    self.focused = true;
                    return WidgetOutcome::Changed;
                }
                WidgetOutcome::Ignored
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
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        match key.key {
            keys::ARROW_UP => self.step_active(1),
            keys::ARROW_DOWN => self.step_active(-1),
            keys::ARROW_LEFT => self.move_octet(-1),
            keys::ARROW_RIGHT => self.move_octet(1),
            keys::HOME => self.move_octet(-(OCTETS as i32)),
            keys::END => self.move_octet(OCTETS as i32),
            keys::BACKSPACE => self.backspace(),
            keys::TAB => self.move_octet(1),
            other => {
                if key.modifiers
                    & (keys::modifiers::CTRL | keys::modifiers::ALT | keys::modifiers::SUPER)
                    != 0
                {
                    return WidgetOutcome::Ignored;
                }
                match keys::printable_char(other) {
                    Some(c) if c.is_ascii_digit() => self.push_digit(c),
                    // A dot commits the active octet and advances.
                    Some('.') => {
                        self.clamp_active();
                        self.move_octet(1)
                    }
                    _ => WidgetOutcome::Ignored,
                }
            }
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut node = TemplateNode::el("lq-ip-input")
            .attr("role", "group")
            .attr("aria-label", "IP address")
            .attr("data-address", &self.address())
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .pseudo_if(PseudoStateFlags::FOCUS, self.focused && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        for i in 0..OCTETS {
            let is_active = self.active == i && self.focused && !self.disabled;
            node = node.child(
                TemplateNode::el("lq-ip-octet")
                    .attr("data-part", &Self::octet_part(i))
                    .attr("data-index", &format!("{i}"))
                    .attr("role", "spinbutton")
                    .attr("aria-valuenow", &self.octet(i).to_string())
                    .class_if("active", is_active)
                    .pseudo_if(PseudoStateFlags::FOCUS, is_active)
                    .child(TemplateNode::text(self.octet_text(i))),
            );
            if i < OCTETS - 1 {
                node = node.child(
                    TemplateNode::el("lq-ip-dot")
                        .attr("aria-hidden", "true")
                        .child(TemplateNode::text(".")),
                );
            }
        }
        if self.disabled {
            node = node.attr("disabled", "true");
        }
        node
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
