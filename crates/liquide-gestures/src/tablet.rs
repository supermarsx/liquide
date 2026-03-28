//! Stylus / tablet input support.
//!
//! Models libinput tablet-tool events: proximity, motion with pressure and
//! tilt, and tool buttons. Includes configurable pressure curves (linear,
//! soft, firm, custom) for mapping raw sensor pressure to application values.

/// Type of tablet tool (matches libinput `LIBINPUT_TABLET_TOOL_TYPE_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolType {
    Pen,
    Eraser,
    Brush,
    Pencil,
    Airbrush,
    Lens,
}

/// Capabilities that a tablet tool may advertise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolCapabilities {
    pub pressure: bool,
    pub tilt: bool,
    pub rotation: bool,
    pub distance: bool,
    pub slider: bool,
    pub wheel: bool,
}

impl Default for ToolCapabilities {
    fn default() -> Self {
        Self {
            pressure: true,
            tilt: true,
            rotation: false,
            distance: false,
            slider: false,
            wheel: false,
        }
    }
}

/// A tablet tool identity.
#[derive(Debug, Clone)]
pub struct TabletTool {
    /// Tool type.
    pub tool_type: ToolType,
    /// Hardware serial (if reported by the device).
    pub serial: u64,
    /// Capabilities of this tool.
    pub capabilities: ToolCapabilities,
}

impl TabletTool {
    pub fn new(tool_type: ToolType, serial: u64) -> Self {
        Self {
            tool_type,
            serial,
            capabilities: ToolCapabilities::default(),
        }
    }

    pub fn with_capabilities(mut self, caps: ToolCapabilities) -> Self {
        self.capabilities = caps;
        self
    }
}

/// Tablet event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabletEvent {
    /// Tool entered proximity of the tablet surface.
    ProximityIn {
        x: f64,
        y: f64,
    },
    /// Tool left proximity.
    ProximityOut,
    /// Tool motion with full axis data.
    Motion {
        x: f64,
        y: f64,
        /// Raw pressure in [0.0, 1.0].
        pressure: f64,
        /// Tilt along the X axis in degrees.
        tilt_x: f64,
        /// Tilt along the Y axis in degrees.
        tilt_y: f64,
    },
    /// Tool button press/release.
    Button {
        button: u32,
        pressed: bool,
    },
}

/// Pressure curve type for mapping raw sensor pressure to output.
#[derive(Debug, Clone, PartialEq)]
pub enum PressureCurve {
    /// 1:1 mapping.
    Linear,
    /// Gentle onset, heavier pressure required for full output.
    /// Implemented as `output = input^2`.
    Soft,
    /// Light touch produces strong output quickly.
    /// Implemented as `output = sqrt(input)`.
    Firm,
    /// Custom cubic Bezier control points `(x1, y1, x2, y2)` in [0,1].
    /// Evaluated via simplified De Casteljau.
    Custom {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
}

impl Default for PressureCurve {
    fn default() -> Self {
        PressureCurve::Linear
    }
}

/// Apply a pressure curve to a raw pressure value in [0.0, 1.0].
/// Output is clamped to [0.0, 1.0].
pub fn apply_pressure_curve(raw: f64, curve: &PressureCurve) -> f64 {
    let clamped = raw.clamp(0.0, 1.0);
    let result = match curve {
        PressureCurve::Linear => clamped,
        PressureCurve::Soft => clamped * clamped,
        PressureCurve::Firm => clamped.sqrt(),
        PressureCurve::Custom { x1, y1, x2, y2 } => {
            cubic_bezier_y_at_x(clamped, *x1, *y1, *x2, *y2)
        }
    };
    result.clamp(0.0, 1.0)
}

/// Approximate y-value of cubic Bezier curve `(0,0), (x1,y1), (x2,y2), (1,1)`
/// at a given x using iterative bisection on the parameter `t`.
fn cubic_bezier_y_at_x(x: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    // Find t such that bezier_x(t) ≈ x, then return bezier_y(t).
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;

    for _ in 0..20 {
        let mid = (lo + hi) * 0.5;
        let bx = bezier_component(mid, x1, x2);
        if bx < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    let t = (lo + hi) * 0.5;
    bezier_component(t, y1, y2)
}

/// Evaluate one component of a cubic Bezier: P0=0, P1=p1, P2=p2, P3=1 at parameter t.
fn bezier_component(t: f64, p1: f64, p2: f64) -> f64 {
    let mt = 1.0 - t;
    3.0 * mt * mt * t * p1 + 3.0 * mt * t * t * p2 + t * t * t
}

/// Tablet state tracker: keeps track of proximity, position, and button state.
pub struct TabletState {
    pub in_proximity: bool,
    pub x: f64,
    pub y: f64,
    pub pressure: f64,
    pub tilt_x: f64,
    pub tilt_y: f64,
    pub buttons: u32,
    pub curve: PressureCurve,
}

impl TabletState {
    pub fn new(curve: PressureCurve) -> Self {
        Self {
            in_proximity: false,
            x: 0.0,
            y: 0.0,
            pressure: 0.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
            buttons: 0,
            curve,
        }
    }

    /// Process a tablet event and update internal state.
    /// Returns the mapped pressure (with curve applied) for Motion events.
    pub fn process(&mut self, event: TabletEvent) -> Option<f64> {
        match event {
            TabletEvent::ProximityIn { x, y } => {
                self.in_proximity = true;
                self.x = x;
                self.y = y;
                self.pressure = 0.0;
                None
            }
            TabletEvent::ProximityOut => {
                self.in_proximity = false;
                self.pressure = 0.0;
                self.buttons = 0;
                None
            }
            TabletEvent::Motion { x, y, pressure, tilt_x, tilt_y } => {
                self.x = x;
                self.y = y;
                self.tilt_x = tilt_x;
                self.tilt_y = tilt_y;
                let mapped = apply_pressure_curve(pressure, &self.curve);
                self.pressure = mapped;
                Some(mapped)
            }
            TabletEvent::Button { button, pressed } => {
                if pressed {
                    self.buttons |= 1 << button;
                } else {
                    self.buttons &= !(1 << button);
                }
                None
            }
        }
    }

    /// Whether a specific button is pressed.
    pub fn is_button_pressed(&self, button: u32) -> bool {
        (self.buttons & (1 << button)) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_curve() {
        assert!((apply_pressure_curve(0.5, &PressureCurve::Linear) - 0.5).abs() < f64::EPSILON);
        assert!((apply_pressure_curve(0.0, &PressureCurve::Linear)).abs() < f64::EPSILON);
        assert!((apply_pressure_curve(1.0, &PressureCurve::Linear) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn soft_curve() {
        let out = apply_pressure_curve(0.5, &PressureCurve::Soft);
        assert!((out - 0.25).abs() < 0.001);
        assert!(out < 0.5, "Soft should reduce mid-range pressure");
    }

    #[test]
    fn firm_curve() {
        let out = apply_pressure_curve(0.25, &PressureCurve::Firm);
        assert!((out - 0.5).abs() < 0.001);
        assert!(out > 0.25, "Firm should amplify low pressure");
    }

    #[test]
    fn pressure_curve_clamped() {
        assert!((apply_pressure_curve(-0.5, &PressureCurve::Linear)).abs() < f64::EPSILON);
        assert!((apply_pressure_curve(1.5, &PressureCurve::Linear) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn custom_curve_linear_equivalent() {
        // Control points (0.33, 0.33) and (0.66, 0.66) approximate a linear curve
        let curve = PressureCurve::Custom { x1: 0.33, y1: 0.33, x2: 0.66, y2: 0.66 };
        let out = apply_pressure_curve(0.5, &curve);
        assert!((out - 0.5).abs() < 0.05, "Should be near-linear, got {}", out);
    }

    #[test]
    fn custom_curve_endpoints() {
        let curve = PressureCurve::Custom { x1: 0.25, y1: 0.1, x2: 0.75, y2: 0.9 };
        let out0 = apply_pressure_curve(0.0, &curve);
        let out1 = apply_pressure_curve(1.0, &curve);
        assert!(out0 < 0.05, "Start should be near 0, got {}", out0);
        assert!(out1 > 0.95, "End should be near 1, got {}", out1);
    }

    #[test]
    fn tool_type_creation() {
        let tool = TabletTool::new(ToolType::Pen, 12345);
        assert_eq!(tool.tool_type, ToolType::Pen);
        assert_eq!(tool.serial, 12345);
        assert!(tool.capabilities.pressure);
    }

    #[test]
    fn tool_capabilities() {
        let caps = ToolCapabilities { pressure: true, tilt: false, rotation: true, distance: false, slider: false, wheel: true };
        let tool = TabletTool::new(ToolType::Brush, 0).with_capabilities(caps);
        assert!(!tool.capabilities.tilt);
        assert!(tool.capabilities.rotation);
        assert!(tool.capabilities.wheel);
    }

    #[test]
    fn tablet_state_proximity() {
        let mut state = TabletState::new(PressureCurve::Linear);
        assert!(!state.in_proximity);
        state.process(TabletEvent::ProximityIn { x: 100.0, y: 200.0 });
        assert!(state.in_proximity);
        assert!((state.x - 100.0).abs() < f64::EPSILON);
        state.process(TabletEvent::ProximityOut);
        assert!(!state.in_proximity);
    }

    #[test]
    fn tablet_state_motion() {
        let mut state = TabletState::new(PressureCurve::Soft);
        state.process(TabletEvent::ProximityIn { x: 0.0, y: 0.0 });
        let mapped = state.process(TabletEvent::Motion {
            x: 50.0, y: 60.0, pressure: 0.5, tilt_x: 10.0, tilt_y: -5.0,
        });
        assert!(mapped.is_some());
        let p = mapped.unwrap();
        // Soft: 0.5^2 = 0.25
        assert!((p - 0.25).abs() < 0.001);
        assert!((state.tilt_x - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tablet_state_buttons() {
        let mut state = TabletState::new(PressureCurve::Linear);
        state.process(TabletEvent::Button { button: 0, pressed: true });
        assert!(state.is_button_pressed(0));
        assert!(!state.is_button_pressed(1));
        state.process(TabletEvent::Button { button: 0, pressed: false });
        assert!(!state.is_button_pressed(0));
    }

    #[test]
    fn tablet_state_multiple_buttons() {
        let mut state = TabletState::new(PressureCurve::Linear);
        state.process(TabletEvent::Button { button: 0, pressed: true });
        state.process(TabletEvent::Button { button: 2, pressed: true });
        assert!(state.is_button_pressed(0));
        assert!(state.is_button_pressed(2));
        assert!(!state.is_button_pressed(1));
    }

    #[test]
    fn proximity_out_clears_buttons() {
        let mut state = TabletState::new(PressureCurve::Linear);
        state.process(TabletEvent::Button { button: 1, pressed: true });
        state.process(TabletEvent::ProximityOut);
        assert!(!state.is_button_pressed(1));
    }
}
