//! `<lq-hotkey-input>` — a keyboard-shortcut capture field (Group GRID: G5).
//!
//! Focus the field and press a chord (e.g. Ctrl+Shift+K); it captures the
//! modifier set + the final non-modifier key and displays the canonical chord
//! string. Like the Win32 `HotKey` common control. Behavior:
//!
//! - **Click** the field (`data-part="field"`): focuses it (begins capture).
//! - **Key press while focused**: a NON-modifier key combined with the held
//!   modifiers becomes the captured chord; the display updates to e.g.
//!   `"Ctrl+Shift+K"` and a `Changed(chord)` action fires.
//! - **Backspace / Delete**: clears the captured chord (emits `Changed("")`).
//! - **Escape**: cancels capture (clears focus, no change).
//! - Pressing ONLY modifiers shows the in-progress modifier prefix (e.g.
//!   `"Ctrl+…"`) but does not commit until a real key arrives.
//!
//! The chord is rendered into the `data-part="field"` box; the displayed text is
//! the captured chord, so a pixel/text assertion can confirm the capture, and
//! the hit-test for the focus click reads the laid-out field box (never a const).

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the captured chord changes (payload: the canonical chord string,
/// or `""` when cleared).
pub const CHANGED_ACTION: &str = "changed";

/// A captured chord: a modifier bitmask + an optional final key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Chord {
    /// Modifier bitmask (see [`keys::modifiers`]).
    pub modifiers: u32,
    /// The final non-modifier key code, if a full chord was captured.
    pub key: Option<u32>,
}

impl Chord {
    /// Whether a complete chord (a non-modifier key) has been captured.
    pub fn is_complete(&self) -> bool {
        self.key.is_some()
    }

    /// The canonical display string, e.g. `"Ctrl+Shift+K"`. A modifier-only
    /// in-progress chord renders with a trailing `+…`. Empty when nothing held.
    pub fn display(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.modifiers & keys::modifiers::CTRL != 0 {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers & keys::modifiers::SHIFT != 0 {
            parts.push("Shift".to_string());
        }
        if self.modifiers & keys::modifiers::ALT != 0 {
            parts.push("Alt".to_string());
        }
        if self.modifiers & keys::modifiers::SUPER != 0 {
            parts.push("Super".to_string());
        }
        match self.key {
            Some(k) => {
                parts.push(key_name(k));
                parts.join("+")
            }
            None => {
                if parts.is_empty() {
                    String::new()
                } else {
                    // Modifiers held, no final key yet.
                    format!("{}+…", parts.join("+"))
                }
            }
        }
    }
}

/// A human-readable name for a key code (printables uppercased; named keys spelled
/// out). Used to build the canonical chord string.
fn key_name(k: u32) -> String {
    match k {
        keys::ENTER => "Enter".to_string(),
        keys::TAB => "Tab".to_string(),
        keys::BACKSPACE => "Backspace".to_string(),
        keys::DELETE => "Delete".to_string(),
        keys::ESCAPE => "Esc".to_string(),
        keys::ARROW_LEFT => "Left".to_string(),
        keys::ARROW_RIGHT => "Right".to_string(),
        keys::ARROW_UP => "Up".to_string(),
        keys::ARROW_DOWN => "Down".to_string(),
        keys::HOME => "Home".to_string(),
        keys::END => "End".to_string(),
        keys::PAGE_UP => "PageUp".to_string(),
        keys::PAGE_DOWN => "PageDown".to_string(),
        _ => match keys::printable_char(k) {
            Some(' ') => "Space".to_string(),
            Some(c) => c.to_ascii_uppercase().to_string(),
            None => "?".to_string(),
        },
    }
}

/// A hotkey / shortcut capture control.
#[derive(Debug, Clone, Default)]
pub struct HotkeyInput {
    chord: Chord,
    focused: bool,
    disabled: bool,
}

impl HotkeyInput {
    /// An empty hotkey input.
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialise with a pre-captured chord.
    pub fn with_chord(modifiers: u32, key: u32) -> Self {
        Self {
            chord: Chord {
                modifiers,
                key: Some(key),
            },
            focused: false,
            disabled: false,
        }
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Set focus (host plumbing / begins capture when true).
    pub fn set_focused(&mut self, f: bool) {
        self.focused = f;
    }

    /// Whether focused (capturing).
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// The captured chord.
    pub fn chord(&self) -> Chord {
        self.chord
    }

    /// The canonical chord display string.
    pub fn display(&self) -> String {
        let s = self.chord.display();
        if s.is_empty() {
            if self.focused {
                "Press a shortcut…".to_string()
            } else {
                "None".to_string()
            }
        } else {
            s
        }
    }

    /// Whether `k` is itself a modifier key (so it never becomes the FINAL key).
    /// The widget receives modifiers via `KeyInput::modifiers`; a raw modifier
    /// keystroke (if ever delivered as a key) is ignored as the chord terminator.
    fn is_modifier_key(_k: u32) -> bool {
        // Our key encoding does not assign codes to bare modifier presses (they
        // arrive only in the modifier bitmask), so no printable/named code is a
        // modifier. Kept as a hook for completeness.
        false
    }

    fn clear(&mut self) -> WidgetOutcome {
        if self.chord == Chord::default() {
            return WidgetOutcome::Changed;
        }
        self.chord = Chord::default();
        WidgetOutcome::action_with(CHANGED_ACTION, "")
    }

    fn capture(&mut self, key: KeyInput) -> WidgetOutcome {
        if Self::is_modifier_key(key.key) {
            // Update the in-progress modifier prefix only.
            self.chord = Chord {
                modifiers: key.modifiers,
                key: None,
            };
            return WidgetOutcome::Changed;
        }
        let new = Chord {
            modifiers: key.modifiers,
            key: Some(key.key),
        };
        let changed = new != self.chord;
        self.chord = new;
        if changed {
            WidgetOutcome::action_with(CHANGED_ACTION, self.chord.display())
        } else {
            WidgetOutcome::Changed
        }
    }
}

impl WidgetBehavior for HotkeyInput {
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
                let on_field = layout
                    .box_of_part(root, "field")
                    .map(|r| r.contains(p))
                    .unwrap_or(false);
                if on_field && !self.focused {
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
            keys::ESCAPE => {
                // Cancel capture (drop focus, no change to the stored chord).
                if self.focused {
                    self.focused = false;
                    return WidgetOutcome::Changed;
                }
                WidgetOutcome::Ignored
            }
            keys::BACKSPACE | keys::DELETE => self.clear(),
            _ => self.capture(key),
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let placeholder = self.chord.display().is_empty();
        let mut node = TemplateNode::el("lq-hotkey-input")
            .attr("role", "textbox")
            .attr("aria-label", "Shortcut")
            .attr("data-chord", &self.chord.display())
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .pseudo_if(PseudoStateFlags::FOCUS, self.focused && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(
                TemplateNode::el("lq-hotkey-field")
                    .attr("data-part", "field")
                    .class_if("placeholder", placeholder)
                    .class_if("capturing", self.focused && !self.disabled)
                    .pseudo_if(PseudoStateFlags::FOCUS, self.focused && !self.disabled)
                    .child(TemplateNode::text(&self.display())),
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
