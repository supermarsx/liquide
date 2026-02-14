//! CSS style resolver for converting CSS themes to render styles.

use std::sync::Arc;

use liquide_compositor::pixel::Color;
use liquide_theme_css::{
    engine::ThemeEngine,
    value::{LengthUnit, PropertyValue},
};

use crate::{
    Result, StyleError,
    glass::GlassStyle,
    shadow::ShadowStyle,
    style::{BorderLineStyle, BorderStyle, Margin, Padding, RenderStyle},
    transform::TransformStyle,
};

/// CSS-to-RenderStyle resolver.
///
/// Queries a CSS theme engine and builds complete `RenderStyle` objects
/// for UI elements. Caches resolved styles for performance.
pub struct StyleResolver {
    engine: Arc<ThemeEngine>,
}

impl StyleResolver {
    /// Create a new style resolver with the given theme engine.
    pub fn new(engine: ThemeEngine) -> Self {
        Self {
            engine: Arc::new(engine),
        }
    }

    /// Create from shared engine.
    pub fn from_arc(engine: Arc<ThemeEngine>) -> Self {
        Self { engine }
    }

    /// Resolve styles for an element.
    ///
    /// # Arguments
    ///
    /// * `element` - Element name (e.g., "window", "button", "titlebar")
    /// * `classes` - CSS class names applied to this element
    /// * `pseudo_classes` - Pseudo-class states (e.g., "hover", "focus")
    /// * `id` - Optional element ID
    pub fn resolve(
        &self,
        element: &str,
        classes: &[String],
        pseudo_classes: &[String],
        id: Option<String>,
    ) -> Result<RenderStyle> {
        // Query CSS properties
        let props = if let Some(ref id_str) = id {
            self.engine
                .query_with_id(element, Some(id_str.as_str()), classes, pseudo_classes)?
        } else {
            self.engine.query(element, classes, pseudo_classes)?
        };

        // Build RenderStyle from properties
        let mut style = RenderStyle::new();

        // Colors
        if let Some(color) = self
            .get_color(&props, "background")
            .or_else(|| self.get_color(&props, "background-color"))
        {
            style.background_color = Some(color);
        }

        if let Some(color) = self.get_color(&props, "color") {
            style.foreground_color = Some(color);
            style.text_color = Some(color);
        }

        if let Some(color) = self.get_color(&props, "border-color") {
            style.border_color = Some(color);
        }

        // Dimensions
        if let Some(width) = self.get_length(&props, "width") {
            style.width = Some(width);
        }

        if let Some(height) = self.get_length(&props, "height") {
            style.height = Some(height);
        }

        // Padding
        style.padding = self.resolve_padding(&props);

        // Margin
        style.margin = self.resolve_margin(&props);

        // Border
        style.border = self.resolve_border(&props);

        if let Some(radius) = self.get_length(&props, "border-radius") {
            style.border_radius = radius;
        }

        // Effects
        if let Some(opacity) = self.get_number(&props, "opacity") {
            style.opacity = opacity;
        }

        // Glass effect (custom property: glass-tint, glass-blur)
        if let Some(glass_tint) = self.get_color(&props, "glass-tint") {
            let blur_radius = self.get_length(&props, "glass-blur").unwrap_or(20.0) as u32;
            style.glass = Some(GlassStyle::new(blur_radius, glass_tint));
        }

        // Box shadow
        if let Some(shadow) = self.resolve_shadow(&props) {
            style.shadow = Some(shadow);
        }

        // Transform
        style.transform = self.resolve_transform(&props);

        // Text
        if let Some(size) = self.get_length(&props, "font-size") {
            style.font_size = Some(size);
        }

        if let Some(weight) = self.get_number(&props, "font-weight") {
            style.font_weight = Some(weight as u16);
        }

        if let Some(lh) = self.get_length(&props, "line-height") {
            style.line_height = Some(lh);
        }

        // Font family — extract the first family name from the CSS value.
        if let Some(family_val) = props.get("font-family") {
            if let Some(family_str) = family_val.as_string() {
                // CSS font-family may be a comma-separated list; take first.
                let first = family_str
                    .split(',')
                    .next()
                    .unwrap_or(family_str)
                    .trim()
                    .trim_matches(|c| c == '\'' || c == '"');
                style.font_family = Some(first.to_string());
            }
        }

        // Letter-spacing
        if let Some(ls) = self.get_length(&props, "letter-spacing") {
            style.letter_spacing = Some(ls);
        }

        // Layout
        if let Some(z) = self.get_number(&props, "z-index") {
            style.z_index = z as i32;
        }

        if let Some(vis) = props.get("visibility") {
            if let Some(keyword) = vis.as_string() {
                style.visibility = keyword != "hidden";
            }
        }

        // Backdrop filter (custom property — legacy single-blur shorthand)
        if let Some(blur) = self.get_length(&props, "backdrop-blur") {
            style.backdrop_filter = Some(crate::style::BackdropFilterOld::Blur {
                radius: blur as u32,
            });
        }

        Ok(style)
    }

    /// Resolve padding from CSS properties.
    fn resolve_padding(&self, props: &liquide_theme_css::property::PropertySet) -> Padding {
        let top = self.get_length(props, "padding-top").unwrap_or(0.0);
        let right = self.get_length(props, "padding-right").unwrap_or(0.0);
        let bottom = self.get_length(props, "padding-bottom").unwrap_or(0.0);
        let left = self.get_length(props, "padding-left").unwrap_or(0.0);

        // Check for shorthand "padding"
        if let Some(padding) = self.get_length(props, "padding") {
            return Padding::uniform(padding);
        }

        Padding {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Resolve margin from CSS properties.
    fn resolve_margin(&self, props: &liquide_theme_css::property::PropertySet) -> Margin {
        let top = self.get_length(props, "margin-top").unwrap_or(0.0);
        let right = self.get_length(props, "margin-right").unwrap_or(0.0);
        let bottom = self.get_length(props, "margin-bottom").unwrap_or(0.0);
        let left = self.get_length(props, "margin-left").unwrap_or(0.0);

        // Check for shorthand "margin"
        if let Some(margin) = self.get_length(props, "margin") {
            return Margin::uniform(margin);
        }

        Margin {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Resolve border from CSS properties.
    fn resolve_border(&self, props: &liquide_theme_css::property::PropertySet) -> BorderStyle {
        let width = self
            .get_length(props, "border-width")
            .or_else(|| self.get_length(props, "border"))
            .unwrap_or(0.0);

        let color = self
            .get_color(props, "border-color")
            .unwrap_or(Color::new(0, 0, 0, 255));

        let style = if let Some(style_val) = props.get("border-style") {
            match style_val.as_string() {
                Some("solid") => BorderLineStyle::Solid,
                Some("dashed") => BorderLineStyle::Dashed,
                Some("dotted") => BorderLineStyle::Dotted,
                Some("double") => BorderLineStyle::Double,
                _ => BorderLineStyle::None,
            }
        } else if width > 0.0 {
            BorderLineStyle::Solid
        } else {
            BorderLineStyle::None
        };

        BorderStyle {
            width,
            style,
            color,
        }
    }

    /// Resolve box-shadow from CSS properties.
    fn resolve_shadow(
        &self,
        props: &liquide_theme_css::property::PropertySet,
    ) -> Option<ShadowStyle> {
        // For now, use custom properties: shadow-offset-x, shadow-offset-y, shadow-blur, shadow-color
        let offset_x = self.get_length(props, "shadow-offset-x").unwrap_or(0.0);
        let offset_y = self.get_length(props, "shadow-offset-y").unwrap_or(4.0);
        let blur = self.get_length(props, "shadow-blur").unwrap_or(8.0);
        let color = self
            .get_color(props, "shadow-color")
            .unwrap_or(Color::new(0, 0, 0, 80));

        if blur > 0.0 || offset_x != 0.0 || offset_y != 0.0 {
            Some(ShadowStyle::new(offset_x, offset_y, blur, color))
        } else {
            None
        }
    }

    /// Resolve CSS transforms to TransformStyle.
    fn resolve_transform(
        &self,
        props: &liquide_theme_css::property::PropertySet,
    ) -> TransformStyle {
        let mut transform = TransformStyle::default();

        // Translate
        if let Some(tx) = self.get_length(props, "translate-x") {
            transform.translate.0 = tx;
        }
        if let Some(ty) = self.get_length(props, "translate-y") {
            transform.translate.1 = ty;
        }

        // Rotate
        if let Some(angle) = self.get_number(props, "rotate") {
            transform.rotate = angle;
        }

        // Scale
        if let Some(scale) = self.get_number(props, "scale") {
            transform.scale = (scale, scale);
        }
        if let Some(sx) = self.get_number(props, "scale-x") {
            transform.scale.0 = sx;
        }
        if let Some(sy) = self.get_number(props, "scale-y") {
            transform.scale.1 = sy;
        }

        transform
    }

    /// Extract color from property value.
    fn get_color(
        &self,
        props: &liquide_theme_css::property::PropertySet,
        name: &str,
    ) -> Option<Color> {
        props.get(name).and_then(|v| {
            v.as_color().map(|c| Color {
                r: c.r,
                g: c.g,
                b: c.b,
                a: c.a,
            })
        })
    }

    /// Extract length in pixels from property value.
    fn get_length(
        &self,
        props: &liquide_theme_css::property::PropertySet,
        name: &str,
    ) -> Option<f32> {
        props.get(name).and_then(|v| match v.as_length() {
            Some(LengthUnit::Px(px)) => Some(px),
            Some(LengthUnit::Pt(pt)) => Some(pt * 1.333), // 1pt = 1.333px
            Some(LengthUnit::Em(em)) => Some(em * 16.0),  // Assume 16px base
            Some(LengthUnit::Rem(rem)) => Some(rem * 16.0), // Assume 16px base
            Some(LengthUnit::Percent(pct)) => Some(pct),  // Return as-is, caller handles
            None => None,
        })
    }

    /// Extract number from property value.
    fn get_number(
        &self,
        props: &liquide_theme_css::property::PropertySet,
        name: &str,
    ) -> Option<f32> {
        props.get(name).and_then(|v| v.as_number())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_basic_style() {
        let css = r#"
            window {
                background: #2e3440;
                color: #eceff4;
                border: 1px solid #4c566a;
                opacity: 0.95;
            }
        "#;

        let engine = ThemeEngine::from_css(css).unwrap();
        let resolver = StyleResolver::new(engine);

        let style = resolver.resolve("window", &[], &[], None).unwrap();

        assert!(style.background_color.is_some());
        assert!(style.foreground_color.is_some());
        assert!(style.border.width > 0.0);
        assert_eq!(style.opacity, 0.95);
    }

    #[test]
    fn test_resolve_with_classes() {
        let css = r#"
            button {
                background: #5e81ac;
            }
            
            button.primary {
                background: #88c0d0;
            }
        "#;

        let engine = ThemeEngine::from_css(css).unwrap();
        let resolver = StyleResolver::new(engine);

        let style = resolver
            .resolve("button", &["primary".to_string()], &[], None)
            .unwrap();

        assert!(style.background_color.is_some());
    }

    #[test]
    fn test_resolve_glass_effect() {
        let css = r#"
            titlebar {
                glass-tint: rgba(255, 255, 255, 0.2);
                glass-blur: 25px;
            }
        "#;

        let engine = ThemeEngine::from_css(css).unwrap();
        let resolver = StyleResolver::new(engine);

        let style = resolver.resolve("titlebar", &[], &[], None).unwrap();

        assert!(style.glass.is_some());
        let glass = style.glass.unwrap();
        assert_eq!(glass.blur_radius, 25);
    }
}
