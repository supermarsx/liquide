//! SVG cursor builder for creating custom vector cursors

use crate::cursor_set::VectorCursor;

/// Builder for creating custom SVG cursors programmatically
pub struct SvgCursorBuilder {
    width: u32,
    height: u32,
    elements: Vec<String>,
    defs: Vec<String>,
}

impl Default for SvgCursorBuilder {
    fn default() -> Self {
        Self::new(32, 32)
    }
}

impl SvgCursorBuilder {
    /// Create a new SVG cursor builder
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            elements: Vec::new(),
            defs: Vec::new(),
        }
    }

    /// Add a circle
    pub fn circle(mut self, cx: f32, cy: f32, r: f32, fill: &str) -> Self {
        self.elements.push(format!(
            r#"<circle cx="{}" cy="{}" r="{}" fill="{}" />"#,
            cx, cy, r, fill
        ));
        self
    }

    /// Add a rectangle
    pub fn rect(mut self, x: f32, y: f32, width: f32, height: f32, fill: &str) -> Self {
        self.elements.push(format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" />"#,
            x, y, width, height, fill
        ));
        self
    }

    /// Add a line
    pub fn line(mut self, x1: f32, y1: f32, x2: f32, y2: f32, stroke: &str, width: f32) -> Self {
        self.elements.push(format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" />"#,
            x1, y1, x2, y2, stroke, width
        ));
        self
    }

    /// Add a path
    pub fn path(mut self, d: &str, fill: Option<&str>, stroke: Option<&str>) -> Self {
        let mut attrs = format!(r#"d="{}""#, d);

        if let Some(fill) = fill {
            attrs.push_str(&format!(r#" fill="{}""#, fill));
        }

        if let Some(stroke) = stroke {
            attrs.push_str(&format!(r#" stroke="{}""#, stroke));
        }

        self.elements.push(format!(r#"<path {} />"#, attrs));
        self
    }

    /// Add raw SVG element
    pub fn raw(mut self, svg: &str) -> Self {
        self.elements.push(svg.to_string());
        self
    }

    /// Add a definition (for filters, gradients, etc.)
    pub fn def(mut self, def: &str) -> Self {
        self.defs.push(def.to_string());
        self
    }

    /// Add a drop shadow filter
    pub fn drop_shadow(self, id: &str, dx: f32, dy: f32, blur: f32) -> Self {
        self.def(&format!(
            r#"<filter id="{}">
                <feDropShadow dx="{}" dy="{}" stdDeviation="{}" flood-opacity="0.5"/>
            </filter>"#,
            id, dx, dy, blur
        ))
    }

    /// Build the SVG and create a VectorCursor
    pub fn build(self, hotspot_x: f32, hotspot_y: f32) -> VectorCursor {
        let defs_section = if !self.defs.is_empty() {
            format!("<defs>{}</defs>", self.defs.join("\n"))
        } else {
            String::new()
        };

        let svg = format!(
            r#"<svg width="{}" height="{}" viewBox="0 0 {} {}" xmlns="http://www.w3.org/2000/svg">
{}
{}
</svg>"#,
            self.width,
            self.height,
            self.width,
            self.height,
            defs_section,
            self.elements.join("\n")
        );

        VectorCursor::new(svg, hotspot_x, hotspot_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_simple() {
        let cursor = SvgCursorBuilder::new(32, 32)
            .circle(16.0, 16.0, 8.0, "black")
            .build(0.5, 0.5);

        assert!(cursor.svg_data.contains("circle"));
        assert!(cursor.svg_data.contains("cx=\"16\""));
    }

    #[test]
    fn test_builder_complex() {
        let cursor = SvgCursorBuilder::new(32, 32)
            .drop_shadow("shadow", 1.0, 1.0, 1.0)
            .rect(8.0, 8.0, 16.0, 16.0, "white")
            .circle(16.0, 16.0, 4.0, "black")
            .build(0.5, 0.5);

        assert!(cursor.svg_data.contains("<defs>"));
        assert!(cursor.svg_data.contains("filter"));
        assert!(cursor.svg_data.contains("rect"));
        assert!(cursor.svg_data.contains("circle"));
    }
}
