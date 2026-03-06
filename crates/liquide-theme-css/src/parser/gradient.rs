//! CSS gradient conversion — linear, radial, conic (including repeating variants).
//!
//! Converts lightningcss gradient representations into our `Gradient` type,
//! handling direction parsing, color stop extraction, and position normalisation.

use crate::value::{Color, ColorStop, Gradient};

use super::ThemeParser;

impl ThemeParser {
    /// Convert a lightningcss gradient to our `Gradient` type.
    pub(crate) fn convert_gradient(
        &self,
        grad: &lightningcss::values::gradient::Gradient,
    ) -> Option<Gradient> {
        use lightningcss::values::gradient::Gradient as LGrad;

        match grad {
            LGrad::Linear(lg) | LGrad::RepeatingLinear(lg) => {
                let is_repeating = matches!(grad, LGrad::RepeatingLinear(_));
                let angle = self.gradient_direction_to_degrees(&lg.direction);
                let stops = self.convert_gradient_items(&lg.items);
                if is_repeating {
                    Some(Gradient::RepeatingLinear { angle, stops })
                } else {
                    Some(Gradient::Linear { angle, stops })
                }
            }
            LGrad::Radial(rg) | LGrad::RepeatingRadial(rg) => {
                let is_repeating = matches!(grad, LGrad::RepeatingRadial(_));
                let stops = self.convert_gradient_items(&rg.items);
                if is_repeating {
                    Some(Gradient::RepeatingRadial { stops })
                } else {
                    Some(Gradient::Radial { stops })
                }
            }
            LGrad::Conic(cg) | LGrad::RepeatingConic(cg) => {
                let is_repeating = matches!(grad, LGrad::RepeatingConic(_));
                let angle = self.angle_to_degrees(&cg.angle);
                let pos_str = self.to_css_string(&cg.position);
                let (at_x, at_y) = Self::parse_position_percentages(&pos_str);
                let stops = self.convert_conic_gradient_items(&cg.items);
                if is_repeating {
                    Some(Gradient::RepeatingConic {
                        from_angle: angle,
                        at_x,
                        at_y,
                        stops,
                    })
                } else {
                    Some(Gradient::Conic {
                        from_angle: angle,
                        at_x,
                        at_y,
                        stops,
                    })
                }
            }
            _ => None, // WebKitGradient — skip
        }
    }

    /// Convert gradient line direction to angle in degrees.
    fn gradient_direction_to_degrees(
        &self,
        direction: &lightningcss::values::gradient::LineDirection,
    ) -> f32 {
        use lightningcss::values::gradient::LineDirection;
        match direction {
            LineDirection::Angle(angle) => self.angle_to_degrees(angle),
            LineDirection::Vertical(v) => {
                let v_str = format!("{:?}", v);
                match v_str.as_str() {
                    "Top" => 0.0,
                    "Bottom" => 180.0,
                    _ => 180.0,
                }
            }
            LineDirection::Horizontal(h) => {
                let h_str = format!("{:?}", h);
                match h_str.as_str() {
                    "Left" => 270.0,
                    "Right" => 90.0,
                    _ => 90.0,
                }
            }
            LineDirection::Corner {
                horizontal,
                vertical,
            } => {
                let h_str = format!("{:?}", horizontal);
                let v_str = format!("{:?}", vertical);
                match (h_str.as_str(), v_str.as_str()) {
                    ("Right", "Top") => 45.0,
                    ("Right", "Bottom") => 135.0,
                    ("Left", "Bottom") => 225.0,
                    ("Left", "Top") => 315.0,
                    _ => 180.0,
                }
            }
        }
    }

    /// Convert an Angle to degrees.
    pub(crate) fn angle_to_degrees(&self, angle: &lightningcss::values::angle::Angle) -> f32 {
        Self::parse_angle_string(&self.to_css_string(angle))
    }

    /// Parse an angle string like "180deg", "0.5turn", "3.14rad", "200grad" to degrees.
    pub(crate) fn parse_angle_string(s: &str) -> f32 {
        let s = s.trim();
        if let Some(v) = s.strip_suffix("deg") {
            v.trim().parse::<f32>().unwrap_or(0.0)
        } else if let Some(v) = s.strip_suffix("turn") {
            v.trim().parse::<f32>().unwrap_or(0.0) * 360.0
        } else if let Some(v) = s.strip_suffix("rad") {
            v.trim().parse::<f32>().unwrap_or(0.0) * (180.0 / std::f32::consts::PI)
        } else if let Some(v) = s.strip_suffix("grad") {
            v.trim().parse::<f32>().unwrap_or(0.0) * 0.9
        } else {
            s.parse::<f32>().unwrap_or(180.0)
        }
    }

    /// Convert gradient items (color stops) with `LengthPercentage` positions.
    fn convert_gradient_items(
        &self,
        items: &[lightningcss::values::gradient::GradientItem<
            lightningcss::values::length::LengthPercentage,
        >],
    ) -> Vec<ColorStop> {
        let mut stops = Vec::new();
        for item in items {
            match item {
                lightningcss::values::gradient::GradientItem::ColorStop(cs) => {
                    let color_str = self.to_css_string(&cs.color);
                    let color = Color::parse_css(&color_str).unwrap_or(Color::rgb(0, 0, 0));
                    let position = cs.position.as_ref().map(|p| {
                        let p_str = self.to_css_string(p);
                        if let Some(pct) = p_str.strip_suffix('%') {
                            pct.trim().parse::<f32>().unwrap_or(0.0) / 100.0
                        } else if let Some(px) = p_str.strip_suffix("px") {
                            // Absolute px — store raw, caller interprets
                            px.trim().parse::<f32>().unwrap_or(0.0)
                        } else {
                            p_str.trim().parse::<f32>().unwrap_or(0.0)
                        }
                    });
                    stops.push(ColorStop { color, position });
                }
                lightningcss::values::gradient::GradientItem::Hint(_) => {
                    // Color hints (transition midpoints) — skip for now
                }
            }
        }
        stops
    }

    /// Convert conic gradient items (color stops) with `AnglePercentage` positions.
    fn convert_conic_gradient_items(
        &self,
        items: &[lightningcss::values::gradient::GradientItem<
            lightningcss::values::angle::AnglePercentage,
        >],
    ) -> Vec<ColorStop> {
        let mut stops = Vec::new();
        for item in items {
            match item {
                lightningcss::values::gradient::GradientItem::ColorStop(cs) => {
                    let color_str = self.to_css_string(&cs.color);
                    let color = Color::parse_css(&color_str).unwrap_or(Color::rgb(0, 0, 0));
                    let position = cs.position.as_ref().map(|p| {
                        let p_str = self.to_css_string(p);
                        if let Some(pct) = p_str.strip_suffix('%') {
                            pct.trim().parse::<f32>().unwrap_or(0.0) / 100.0
                        } else if let Some(deg) = p_str.strip_suffix("deg") {
                            deg.trim().parse::<f32>().unwrap_or(0.0) / 360.0
                        } else {
                            p_str.trim().parse::<f32>().unwrap_or(0.0)
                        }
                    });
                    stops.push(ColorStop { color, position });
                }
                lightningcss::values::gradient::GradientItem::Hint(_) => {}
            }
        }
        stops
    }

    /// Parse position string like "50% 50%" into (x, y) as 0.0–1.0.
    fn parse_position_percentages(pos_str: &str) -> (f32, f32) {
        let parts: Vec<&str> = pos_str.split_whitespace().collect();
        let parse_one = |s: &str| -> f32 {
            match s {
                "center" => 0.5,
                "left" | "top" => 0.0,
                "right" | "bottom" => 1.0,
                other => {
                    if let Some(pct) = other.strip_suffix('%') {
                        pct.parse::<f32>().unwrap_or(50.0) / 100.0
                    } else {
                        other.parse::<f32>().unwrap_or(0.5)
                    }
                }
            }
        };
        let x = parts.first().map(|s| parse_one(s)).unwrap_or(0.5);
        let y = parts.get(1).map(|s| parse_one(s)).unwrap_or(0.5);
        (x, y)
    }
}
