//! CSS style resolver for converting CSS themes to render styles.

use std::sync::Arc;

use liquide_compositor::pixel::Color;
use liquide_theme_css::{engine::ThemeEngine, value::LengthUnit};

use crate::{
    Result,
    glass::GlassStyle,
    shadow::ShadowStyle,
    style::{BorderLineStyle, BorderStyle, Margin, Padding, RenderStyle},
    transform::TransformStyle,
};

/// Context used to resolve relative/responsive CSS length units. (TODO 14)
///
/// Without this context the resolver cannot turn `%`, `vw`/`vh`, dynamic
/// viewport units (`dvh`, …) or container units (`cq*`) into pixels, so it
/// would previously return their raw numeric magnitudes (e.g. `70vh` → `70.0`).
#[derive(Debug, Clone, Copy)]
pub struct ResolveContext {
    /// Viewport width in CSS pixels.
    pub viewport_width: f32,
    /// Viewport height in CSS pixels.
    pub viewport_height: f32,
    /// Query container width in CSS pixels (defaults to the viewport width).
    pub container_width: f32,
    /// Query container height in CSS pixels (defaults to the viewport height).
    pub container_height: f32,
    /// Base font size in CSS pixels (the `em`/`rem` reference).
    pub font_size: f32,
    /// Root font size in CSS pixels (the `rem`/`rlh` reference).
    pub root_font_size: f32,
}

impl Default for ResolveContext {
    fn default() -> Self {
        Self {
            viewport_width: 1920.0,
            viewport_height: 1080.0,
            container_width: 1920.0,
            container_height: 1080.0,
            font_size: 16.0,
            root_font_size: 16.0,
        }
    }
}

impl ResolveContext {
    /// Build a context from a viewport size, defaulting container to viewport
    /// and font sizes to 16px.
    pub fn from_viewport(width: f32, height: f32) -> Self {
        Self {
            viewport_width: width,
            viewport_height: height,
            container_width: width,
            container_height: height,
            ..Self::default()
        }
    }

    fn vmin(&self) -> f32 {
        self.viewport_width.min(self.viewport_height)
    }
    fn vmax(&self) -> f32 {
        self.viewport_width.max(self.viewport_height)
    }
    fn cqmin(&self) -> f32 {
        self.container_width.min(self.container_height)
    }
    fn cqmax(&self) -> f32 {
        self.container_width.max(self.container_height)
    }
}

/// CSS-to-RenderStyle resolver.
///
/// Queries a CSS theme engine and builds complete `RenderStyle` objects
/// for UI elements. Caches resolved styles for performance.
pub struct StyleResolver {
    engine: Arc<ThemeEngine>,
    context: ResolveContext,
}

impl StyleResolver {
    /// Create a new style resolver with the given theme engine.
    pub fn new(engine: ThemeEngine) -> Self {
        Self {
            engine: Arc::new(engine),
            context: ResolveContext::default(),
        }
    }

    /// Create from shared engine.
    pub fn from_arc(engine: Arc<ThemeEngine>) -> Self {
        Self {
            engine,
            context: ResolveContext::default(),
        }
    }

    /// Set the viewport/container/font context used to resolve responsive units.
    pub fn set_context(&mut self, context: ResolveContext) {
        self.context = context;
    }

    /// Builder-style variant of [`set_context`](Self::set_context).
    #[must_use]
    pub fn with_context(mut self, context: ResolveContext) -> Self {
        self.context = context;
        self
    }

    /// The active responsive-unit resolution context.
    pub fn context(&self) -> ResolveContext {
        self.context
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

        // Dimensions — percentages resolve against the matching viewport axis.
        if let Some(width) = self.get_length_pct(&props, "width", self.context.viewport_width) {
            style.width = Some(width);
        }

        if let Some(height) = self.get_length_pct(&props, "height", self.context.viewport_height) {
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

    /// Extract length in pixels from property value, resolving responsive units
    /// against the resolver's [`ResolveContext`]. Percentages resolve against
    /// the viewport width by default. (TODO 14)
    fn get_length(
        &self,
        props: &liquide_theme_css::property::PropertySet,
        name: &str,
    ) -> Option<f32> {
        self.get_length_pct(props, name, self.context.viewport_width)
    }

    /// Extract length in pixels, resolving `%` against `pct_base` and all other
    /// responsive units against the resolver context.
    fn get_length_pct(
        &self,
        props: &liquide_theme_css::property::PropertySet,
        name: &str,
        pct_base: f32,
    ) -> Option<f32> {
        let ctx = &self.context;
        props
            .get(name)
            .and_then(|v| v.as_length())
            .map(|unit| Self::resolve_length_unit(unit, ctx, pct_base))
    }

    /// Resolve a single [`LengthUnit`] to CSS pixels using the given context.
    fn resolve_length_unit(unit: LengthUnit, ctx: &ResolveContext, pct_base: f32) -> f32 {
        match unit {
            LengthUnit::Px(px) => px,
            LengthUnit::Pt(pt) => pt * 1.333, // 1pt = 1.333px
            LengthUnit::Em(em) => em * ctx.font_size,
            LengthUnit::Rem(rem) => rem * ctx.root_font_size,
            LengthUnit::Percent(pct) => pct / 100.0 * pct_base,
            LengthUnit::Vw(vw) => vw / 100.0 * ctx.viewport_width,
            LengthUnit::Vh(vh) => vh / 100.0 * ctx.viewport_height,
            LengthUnit::Vmin(vmin) => vmin / 100.0 * ctx.vmin(),
            LengthUnit::Vmax(vmax) => vmax / 100.0 * ctx.vmax(),
            // 1ch ≈ 0.5em, 1ex ≈ 0.5em as font-metric approximations.
            LengthUnit::Ch(ch) => ch * ctx.font_size * 0.5,
            LengthUnit::Ex(ex) => ex * ctx.font_size * 0.5,
            // Dynamic / small / large viewport units → resolve against the
            // current viewport (we do not model UA chrome separately).
            LengthUnit::Dvw(v) | LengthUnit::Svw(v) | LengthUnit::Lvw(v) => {
                v / 100.0 * ctx.viewport_width
            }
            LengthUnit::Dvh(v) | LengthUnit::Svh(v) | LengthUnit::Lvh(v) => {
                v / 100.0 * ctx.viewport_height
            }
            // Container query units → resolve against the container size.
            LengthUnit::Cqw(v) | LengthUnit::Cqi(v) => v / 100.0 * ctx.container_width,
            LengthUnit::Cqh(v) | LengthUnit::Cqb(v) => v / 100.0 * ctx.container_height,
            LengthUnit::Cqmin(v) => v / 100.0 * ctx.cqmin(),
            LengthUnit::Cqmax(v) => v / 100.0 * ctx.cqmax(),
            // Line-height units → approximate line box as 1.2 × font size.
            LengthUnit::Lh(v) => v * ctx.font_size * 1.2,
            LengthUnit::Rlh(v) => v * ctx.root_font_size * 1.2,
        }
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

    #[test]
    fn test_resolve_viewport_units() {
        // TODO 14: vw/vh must resolve to pixels against the context viewport.
        let css = "launcher { width: 50vw; height: 70vh; }";
        let engine = ThemeEngine::from_css(css).unwrap();
        let resolver =
            StyleResolver::new(engine).with_context(ResolveContext::from_viewport(1000.0, 800.0));
        let style = resolver.resolve("launcher", &[], &[], None).unwrap();
        assert_eq!(style.width, Some(500.0), "50vw of 1000px");
        assert_eq!(style.height, Some(560.0), "70vh of 800px");
    }

    #[test]
    fn test_resolve_percent_against_axis() {
        // TODO 14: width% resolves against viewport width, height% against height.
        let css = "panel { width: 25%; height: 50%; }";
        let engine = ThemeEngine::from_css(css).unwrap();
        let resolver =
            StyleResolver::new(engine).with_context(ResolveContext::from_viewport(1200.0, 600.0));
        let style = resolver.resolve("panel", &[], &[], None).unwrap();
        assert_eq!(style.width, Some(300.0), "25% of 1200px width");
        assert_eq!(style.height, Some(300.0), "50% of 600px height");
    }

    #[test]
    fn test_resolve_dynamic_viewport_units() {
        // TODO 14: dvh resolves like vh against the viewport height.
        let css = "notif { height: 100dvh; }";
        let engine = ThemeEngine::from_css(css).unwrap();
        let resolver =
            StyleResolver::new(engine).with_context(ResolveContext::from_viewport(900.0, 720.0));
        let style = resolver.resolve("notif", &[], &[], None).unwrap();
        assert_eq!(style.height, Some(720.0), "100dvh of 720px");
    }

    #[test]
    fn test_resolve_em_against_font_context() {
        // TODO 14: em resolves against the context font size, not a hardcoded 16.
        let css = "label { font-size: 12px; letter-spacing: 2em; }";
        let engine = ThemeEngine::from_css(css).unwrap();
        let mut ctx = ResolveContext::default();
        ctx.font_size = 20.0;
        let resolver = StyleResolver::new(engine).with_context(ctx);
        let style = resolver.resolve("label", &[], &[], None).unwrap();
        assert_eq!(style.letter_spacing, Some(40.0), "2em of 20px font");
    }

    #[test]
    fn test_resolve_container_units() {
        // TODO 14: cqw resolves against the container width.
        let css = "card { width: 50cqw; }";
        let engine = ThemeEngine::from_css(css).unwrap();
        let mut ctx = ResolveContext::from_viewport(1920.0, 1080.0);
        ctx.container_width = 400.0;
        ctx.container_height = 300.0;
        let resolver = StyleResolver::new(engine).with_context(ctx);
        let style = resolver.resolve("card", &[], &[], None).unwrap();
        assert_eq!(style.width, Some(200.0), "50cqw of 400px container");
    }

    #[test]
    fn test_resolve_container_block_and_minmax() {
        // Guard cqh/cqb/cqmin/cqmax against raw passthrough or wrong-axis bugs.
        // Container 400×300 → min axis 300, max axis 400.
        let css = "card { width: 50cqmin; height: 50cqmax; border-radius: 10cqh; }";
        let engine = ThemeEngine::from_css(css).unwrap();
        let mut ctx = ResolveContext::from_viewport(1920.0, 1080.0);
        ctx.container_width = 400.0;
        ctx.container_height = 300.0;
        let resolver = StyleResolver::new(engine).with_context(ctx);
        let style = resolver.resolve("card", &[], &[], None).unwrap();
        // 50cqmin = 50% of min(400,300)=300 → 150 (raw=50, cqmax-confusion=200).
        assert_eq!(style.width, Some(150.0), "50cqmin of min(400,300)=300");
        // 50cqmax = 50% of max(400,300)=400 → 200 (raw=50, cqmin-confusion=150).
        assert_eq!(style.height, Some(200.0), "50cqmax of max(400,300)=400");
        // 10cqh = 10% of container height 300 → 30 (raw=10, against-width=40).
        assert_eq!(style.border_radius, 30.0, "10cqh of 300px container height");
    }

    #[test]
    fn test_resolve_vmin_vmax() {
        // Guard vmin/vmax against raw passthrough or axis confusion.
        // Viewport 1000×800 → vmin axis 800, vmax axis 1000.
        let css = "panel { width: 10vmin; height: 10vmax; }";
        let engine = ThemeEngine::from_css(css).unwrap();
        let resolver =
            StyleResolver::new(engine).with_context(ResolveContext::from_viewport(1000.0, 800.0));
        let style = resolver.resolve("panel", &[], &[], None).unwrap();
        // 10vmin = 10% of min(1000,800)=800 → 80 (raw=10, vmax-confusion=100).
        assert_eq!(style.width, Some(80.0), "10vmin of min(1000,800)=800");
        // 10vmax = 10% of max(1000,800)=1000 → 100 (raw=10, vmin-confusion=80).
        assert_eq!(style.height, Some(100.0), "10vmax of max(1000,800)=1000");
    }

    #[test]
    fn test_resolve_small_large_viewport_units() {
        // Guard svh/lvh against raw passthrough; resolve against viewport height.
        let css = "a { height: 100svh; } b { height: 50lvh; }";
        let engine = ThemeEngine::from_css(css).unwrap();
        let resolver =
            StyleResolver::new(engine).with_context(ResolveContext::from_viewport(900.0, 600.0));
        let svh = resolver.resolve("a", &[], &[], None).unwrap();
        let lvh = resolver.resolve("b", &[], &[], None).unwrap();
        assert_eq!(svh.height, Some(600.0), "100svh of 600px (raw would be 100)");
        assert_eq!(lvh.height, Some(300.0), "50lvh of 600px (raw would be 50)");
    }

    #[test]
    fn test_resolve_line_height_unit() {
        // Guard lh against raw passthrough; lh ≈ 1.2 × font-size.
        let css = "row { height: 2lh; }";
        let engine = ThemeEngine::from_css(css).unwrap();
        let mut ctx = ResolveContext::default();
        ctx.font_size = 20.0;
        let resolver = StyleResolver::new(engine).with_context(ctx);
        let style = resolver.resolve("row", &[], &[], None).unwrap();
        // 2lh = 2 × 20 × 1.2 → 48 (raw would be 2, em-only would be 40).
        assert_eq!(style.height, Some(48.0), "2lh of 20px font (1.2 line box)");
    }
}
