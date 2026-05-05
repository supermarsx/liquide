//! CSS gradient conversion — linear, radial, conic (including repeating variants).
//!
//! Converts lightningcss gradient representations into our `Gradient` type,
//! handling direction parsing, color stop extraction, and position normalisation.

use crate::value::{
    Color, ColorStop, Gradient, GradientPosition, GradientPositionComponent, GradientStopPosition,
    HorizontalGradientSide, LengthUnit, RadialGradientExtent, RadialGradientShape,
    VerticalGradientSide,
};

use super::ThemeParser;

#[allow(dead_code)]
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
                let shape = self.convert_radial_shape(&rg.shape);
                let position = self.convert_position(&rg.position);
                let stops = self.convert_gradient_items(&rg.items);
                if is_repeating {
                    Some(Gradient::RepeatingRadial {
                        shape,
                        position,
                        stops,
                    })
                } else {
                    Some(Gradient::Radial {
                        shape,
                        position,
                        stops,
                    })
                }
            }
            LGrad::Conic(cg) | LGrad::RepeatingConic(cg) => {
                let is_repeating = matches!(grad, LGrad::RepeatingConic(_));
                let angle = self.angle_to_degrees(&cg.angle);
                let position = self.convert_position(&cg.position);
                let stops = self.convert_conic_gradient_items(&cg.items);
                if is_repeating {
                    Some(Gradient::RepeatingConic {
                        from_angle: angle,
                        position,
                        stops,
                    })
                } else {
                    Some(Gradient::Conic {
                        from_angle: angle,
                        position,
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
        let degrees = if let Some(v) = s.strip_suffix("deg") {
            v.trim().parse::<f32>().unwrap_or(0.0)
        } else if let Some(v) = s.strip_suffix("turn") {
            v.trim().parse::<f32>().unwrap_or(0.0) * 360.0
        } else if let Some(v) = s.strip_suffix("grad") {
            v.trim().parse::<f32>().unwrap_or(0.0) * 0.9
        } else if let Some(v) = s.strip_suffix("rad") {
            v.trim().parse::<f32>().unwrap_or(0.0) * (180.0 / std::f32::consts::PI)
        } else {
            s.parse::<f32>().unwrap_or(0.0)
        };

        degrees.rem_euclid(360.0)
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
                    let position = cs
                        .position
                        .as_ref()
                        .and_then(|value| self.convert_length_percentage_stop_position(value));
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
                    let position = cs
                        .position
                        .as_ref()
                        .and_then(|value| self.convert_angle_percentage_stop_position(value));
                    stops.push(ColorStop { color, position });
                }
                lightningcss::values::gradient::GradientItem::Hint(_) => {}
            }
        }
        stops
    }

    fn convert_length_css(&self, css: &str) -> Option<LengthUnit> {
        self.parse_length_value(css)
            .and_then(|value| value.as_length())
    }

    fn convert_length(&self, value: &lightningcss::values::length::Length) -> Option<LengthUnit> {
        self.convert_length_css(&self.to_css_string(value))
    }

    fn convert_length_percentage(
        &self,
        value: &lightningcss::values::length::LengthPercentage,
    ) -> Option<LengthUnit> {
        self.convert_length_css(&self.to_css_string(value))
    }

    fn convert_length_percentage_stop_position(
        &self,
        value: &lightningcss::values::length::LengthPercentage,
    ) -> Option<GradientStopPosition> {
        self.convert_length_percentage(value)
            .map(GradientStopPosition::Length)
    }

    fn convert_angle_percentage_stop_position(
        &self,
        value: &lightningcss::values::angle::AnglePercentage,
    ) -> Option<GradientStopPosition> {
        let css = self.to_css_string(value);
        if let Some(percent) = css.strip_suffix('%') {
            let degrees = percent.trim().parse::<f32>().ok()? * 3.6;
            Some(GradientStopPosition::Angle(degrees.rem_euclid(360.0)))
        } else {
            Some(GradientStopPosition::Angle(Self::parse_angle_string(&css)))
        }
    }

    fn convert_radial_shape(
        &self,
        shape: &lightningcss::values::gradient::EndingShape,
    ) -> RadialGradientShape {
        use lightningcss::values::gradient::{Circle, Ellipse, EndingShape};

        match shape {
            EndingShape::Circle(Circle::Radius(radius)) => RadialGradientShape::Circle {
                radius: self.convert_length(radius),
                extent: None,
            },
            EndingShape::Circle(Circle::Extent(extent)) => RadialGradientShape::Circle {
                radius: None,
                extent: Some(self.convert_shape_extent(extent)),
            },
            EndingShape::Ellipse(Ellipse::Size { x, y }) => RadialGradientShape::Ellipse {
                radius_x: self.convert_length_percentage(x),
                radius_y: self.convert_length_percentage(y),
                extent: None,
            },
            EndingShape::Ellipse(Ellipse::Extent(extent)) => RadialGradientShape::Ellipse {
                radius_x: None,
                radius_y: None,
                extent: Some(self.convert_shape_extent(extent)),
            },
        }
    }

    fn convert_shape_extent(
        &self,
        extent: &lightningcss::values::gradient::ShapeExtent,
    ) -> RadialGradientExtent {
        use lightningcss::values::gradient::ShapeExtent;

        match extent {
            ShapeExtent::ClosestSide => RadialGradientExtent::ClosestSide,
            ShapeExtent::FarthestSide => RadialGradientExtent::FarthestSide,
            ShapeExtent::ClosestCorner => RadialGradientExtent::ClosestCorner,
            ShapeExtent::FarthestCorner => RadialGradientExtent::FarthestCorner,
        }
    }

    fn convert_position(
        &self,
        position: &lightningcss::values::position::Position,
    ) -> GradientPosition {
        GradientPosition {
            x: self.convert_horizontal_position(&position.x),
            y: self.convert_vertical_position(&position.y),
        }
    }

    fn convert_horizontal_position(
        &self,
        position: &lightningcss::values::position::HorizontalPosition,
    ) -> GradientPositionComponent<HorizontalGradientSide> {
        use lightningcss::values::position::{HorizontalPositionKeyword, PositionComponent};

        match position {
            PositionComponent::Center => GradientPositionComponent::Center,
            PositionComponent::Length(value) => self
                .convert_length_percentage(value)
                .map(GradientPositionComponent::Value)
                .unwrap_or(GradientPositionComponent::Center),
            PositionComponent::Side { side, offset } => GradientPositionComponent::Side {
                side: match side {
                    HorizontalPositionKeyword::Left => HorizontalGradientSide::Left,
                    HorizontalPositionKeyword::Right => HorizontalGradientSide::Right,
                },
                offset: offset
                    .as_ref()
                    .and_then(|value| self.convert_length_percentage(value)),
            },
        }
    }

    fn convert_vertical_position(
        &self,
        position: &lightningcss::values::position::VerticalPosition,
    ) -> GradientPositionComponent<VerticalGradientSide> {
        use lightningcss::values::position::{PositionComponent, VerticalPositionKeyword};

        match position {
            PositionComponent::Center => GradientPositionComponent::Center,
            PositionComponent::Length(value) => self
                .convert_length_percentage(value)
                .map(GradientPositionComponent::Value)
                .unwrap_or(GradientPositionComponent::Center),
            PositionComponent::Side { side, offset } => GradientPositionComponent::Side {
                side: match side {
                    VerticalPositionKeyword::Top => VerticalGradientSide::Top,
                    VerticalPositionKeyword::Bottom => VerticalGradientSide::Bottom,
                },
                offset: offset
                    .as_ref()
                    .and_then(|value| self.convert_length_percentage(value)),
            },
        }
    }
}
