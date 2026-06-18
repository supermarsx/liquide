//! `<lq-color-wheel>` — an HSV colour wheel (CREATIVE/PRO).
//!
//! State: `(hue, sat, val)` — hue in `0..360`, sat/val in `0..=1`. The widget is
//! a hue RING wrapping a square saturation/value AREA. Behavior:
//! - **Click/drag the hue ring** (`data-part="ring"`): the hue is the ANGLE of
//!   the pointer about the ring's CENTER, taken from the LAID-OUT ring box
//!   (`box_of` -> center), never a constant. A press in the ring's annulus (near
//!   its edge, outside the inner area) drives hue.
//! - **Click/drag the sat/val area** (`data-part="area"`): saturation =
//!   `fraction_along_x` of the laid-out area, value = `1 - fraction_along_y`
//!   (top is bright, bottom is dark), both from the real area box.
//! - **Keyboard** (when focused): Left/Right rotate hue ±`hue_step`, Up/Down
//!   raise/lower value ±`sv_step`, Home resets sat/val high.
//! - The hue marker rotates on the ring; the sv cursor is positioned in the
//!   area; the area's base tint + the preview swatch carry the current colour as
//!   inline `background-color` (the value IS the data); all box geometry is CSS.
//! - Emits `Changed("#RRGGBB")`.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::{Point, Rect};

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the colour changes (payload: `#RRGGBB`).
pub const CHANGED_ACTION: &str = "changed";

/// Which sub-control a drag is currently bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragTarget {
    None,
    Ring,
    Area,
}

/// An HSV colour wheel.
#[derive(Debug, Clone)]
pub struct ColorWheel {
    hue: f32,
    sat: f32,
    val: f32,
    hue_step: f32,
    sv_step: f32,
    drag: DragTarget,
    disabled: bool,
}

impl ColorWheel {
    /// A wheel starting at `(hue, sat, val)` — hue degrees, sat/val in `0..=1`.
    pub fn new(hue: f32, sat: f32, val: f32) -> Self {
        Self {
            hue: hue.rem_euclid(360.0),
            sat: sat.clamp(0.0, 1.0),
            val: val.clamp(0.0, 1.0),
            hue_step: 5.0,
            sv_step: 0.05,
            drag: DragTarget::None,
            disabled: false,
        }
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The current hue in degrees (0..360).
    pub fn hue(&self) -> f32 {
        self.hue
    }
    /// The current saturation (0..=1).
    pub fn saturation(&self) -> f32 {
        self.sat
    }
    /// The current value/brightness (0..=1).
    pub fn value(&self) -> f32 {
        self.val
    }

    /// Whether a drag is in progress.
    pub fn is_dragging(&self) -> bool {
        self.drag != DragTarget::None
    }

    /// The current colour as `(r, g, b)`.
    pub fn rgb(&self) -> (u8, u8, u8) {
        Self::hsv_to_rgb(self.hue, self.sat, self.val)
    }

    /// The current colour as `#RRGGBB`.
    pub fn hex(&self) -> String {
        let (r, g, b) = self.rgb();
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    /// Convert HSV (h in degrees, s/v in 0..=1) to 8-bit RGB.
    pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
        let h = h.rem_euclid(360.0);
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        let (r1, g1, b1) = match (h / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let to8 = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        (to8(r1), to8(g1), to8(b1))
    }

    fn css_rgb((r, g, b): (u8, u8, u8)) -> String {
        format!("rgb({r}, {g}, {b})")
    }

    fn set_hue(&mut self, h: f32) -> WidgetOutcome {
        let nh = h.rem_euclid(360.0);
        if (nh - self.hue).abs() < 1e-4 {
            return WidgetOutcome::Ignored;
        }
        self.hue = nh;
        WidgetOutcome::action_with(CHANGED_ACTION, self.hex())
    }

    fn set_sv(&mut self, s: f32, v: f32) -> WidgetOutcome {
        let ns = s.clamp(0.0, 1.0);
        let nv = v.clamp(0.0, 1.0);
        if (ns - self.sat).abs() < 1e-4 && (nv - self.val).abs() < 1e-4 {
            return WidgetOutcome::Ignored;
        }
        self.sat = ns;
        self.val = nv;
        WidgetOutcome::action_with(CHANGED_ACTION, self.hex())
    }

    /// Hue from the pointer angle about the LAID-OUT ring center.
    fn hue_from_ring(&self, ring: Rect, x: f32, y: f32) -> f32 {
        let cx = ring.x + ring.width / 2.0;
        let cy = ring.y + ring.height / 2.0;
        // 0° at the top (12 o'clock), increasing clockwise (screen +y down).
        let mut deg = (x - cx).atan2(-(y - cy)).to_degrees();
        deg = deg.rem_euclid(360.0);
        deg
    }

    /// Whether a point lies in the ring's annulus (within the ring box but
    /// OUTSIDE the inner sat/val area). Used to disambiguate which control a
    /// press targets. Both rects are laid-out boxes.
    fn in_annulus(ring: Rect, area: Option<Rect>, p: Point) -> bool {
        if !ring.contains(p) {
            return false;
        }
        match area {
            Some(a) => !a.contains(p),
            None => true,
        }
    }
}

impl WidgetBehavior for ColorWheel {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Slider
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
        let ring = layout.box_of_part(root, "ring").or_else(|| layout.box_of(root));
        let area = layout.box_of_part(root, "area");
        match &event.kind {
            DomEventKind::MouseDown {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let p = Point::new(*x, *y);
                let Some(ring) = ring else {
                    return WidgetOutcome::Ignored;
                };
                // The sat/val area wins inside its own box; the ring annulus
                // drives hue. Both decided from the LAID-OUT boxes.
                if let Some(a) = area {
                    if a.contains(p) {
                        self.drag = DragTarget::Area;
                        let s = LayoutQuery::fraction_along_x(a, p);
                        let v = 1.0 - LayoutQuery::fraction_along_y(a, p);
                        return match self.set_sv(s, v) {
                            WidgetOutcome::Ignored => WidgetOutcome::Changed,
                            o => o,
                        };
                    }
                }
                if Self::in_annulus(ring, area, p) {
                    self.drag = DragTarget::Ring;
                    let h = self.hue_from_ring(ring, *x, *y);
                    return match self.set_hue(h) {
                        WidgetOutcome::Ignored => WidgetOutcome::Changed,
                        o => o,
                    };
                }
                WidgetOutcome::Ignored
            }
            DomEventKind::MouseMove { x, y } => match self.drag {
                DragTarget::Ring => {
                    let Some(ring) = ring else {
                        return WidgetOutcome::Ignored;
                    };
                    let h = self.hue_from_ring(ring, *x, *y);
                    self.set_hue(h)
                }
                DragTarget::Area => {
                    let Some(a) = area else {
                        return WidgetOutcome::Ignored;
                    };
                    let p = Point::new(*x, *y);
                    let s = LayoutQuery::fraction_along_x(a, p);
                    let v = 1.0 - LayoutQuery::fraction_along_y(a, p);
                    self.set_sv(s, v)
                }
                DragTarget::None => WidgetOutcome::Ignored,
            },
            DomEventKind::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                if self.drag == DragTarget::None {
                    return WidgetOutcome::Ignored;
                }
                self.drag = DragTarget::None;
                WidgetOutcome::Changed
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
            keys::ARROW_LEFT => self.set_hue(self.hue - self.hue_step),
            keys::ARROW_RIGHT => self.set_hue(self.hue + self.hue_step),
            keys::ARROW_UP => self.set_sv(self.sat, self.val + self.sv_step),
            keys::ARROW_DOWN => self.set_sv(self.sat, self.val - self.sv_step),
            keys::HOME => self.set_sv(1.0, 1.0),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let hue_deg = self.hue;
        // The pure-hue tint that the sat/val area shades from (full s+v).
        let pure_hue = Self::css_rgb(Self::hsv_to_rgb(self.hue, 1.0, 1.0));
        let cur = Self::css_rgb(self.rgb());
        let sx = self.sat * 100.0;
        let sy = (1.0 - self.val) * 100.0;

        let mut node = TemplateNode::el("lq-color-wheel")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "slider")
            .attr("data-hue", &format!("{:.1}", self.hue))
            .attr("data-sat", &format!("{:.3}", self.sat))
            .attr("data-val", &format!("{:.3}", self.val))
            .attr("data-color", &self.hex())
            .pseudo_if(PseudoStateFlags::ACTIVE, self.is_dragging() && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(
                TemplateNode::el("lq-wheel-ring")
                    .attr("data-part", "ring")
                    .child(
                        // The hue marker rotates around the ring to the hue angle.
                        TemplateNode::el("lq-wheel-hue-marker")
                            .attr("data-part", "hue-marker")
                            .style("transform", &format!("rotate({hue_deg}deg)")),
                    )
                    .child(
                        TemplateNode::el("lq-wheel-area")
                            .attr("data-part", "area")
                            // Base tint = the pure hue (value IS the data); the
                            // box geometry is CSS.
                            .style("background-color", &pure_hue)
                            .child(
                                TemplateNode::el("lq-wheel-sv-cursor")
                                    .attr("data-part", "sv-cursor")
                                    .style("left", &format!("{sx}%"))
                                    .style("top", &format!("{sy}%")),
                            ),
                    ),
            )
            .child(
                TemplateNode::el("lq-wheel-preview")
                    .attr("data-part", "preview")
                    .style("background-color", &cur)
                    .child(TemplateNode::text(&self.hex())),
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
