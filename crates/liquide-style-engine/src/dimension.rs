//! Dimension — a resolved or partially-resolved CSS length value.

use serde::{Deserialize, Serialize};

/// Viewport size tiers for resolving dynamic/small/large viewport units.
///
/// The standard `width`/`height` correspond to `vw`/`vh`.  The dynamic, small,
/// and large tiers are used by `dvw`/`dvh`, `svw`/`svh`, and `lvw`/`lvh`
/// respectively.  When no distinct tiers are configured, all fields default to
/// the standard `width`/`height`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportSizes {
    /// Standard viewport width (for `vw`).
    pub width: f32,
    /// Standard viewport height (for `vh`).
    pub height: f32,
    /// Dynamic viewport width (for `dvw`).
    pub dynamic_width: f32,
    /// Dynamic viewport height (for `dvh`).
    pub dynamic_height: f32,
    /// Small viewport width (for `svw`).
    pub small_width: f32,
    /// Small viewport height (for `svh`).
    pub small_height: f32,
    /// Large viewport width (for `lvw`).
    pub large_width: f32,
    /// Large viewport height (for `lvh`).
    pub large_height: f32,
}

impl ViewportSizes {
    /// Create viewport sizes where all tiers equal the standard size.
    pub fn uniform(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            dynamic_width: width,
            dynamic_height: height,
            small_width: width,
            small_height: height,
            large_width: width,
            large_height: height,
        }
    }
}

impl Default for ViewportSizes {
    fn default() -> Self {
        Self::uniform(1920.0, 1080.0)
    }
}

/// A CSS `calc()` expression stored for deferred resolution.
///
/// This mirrors `CssMathExpr` from `liquide-theme-css` but lives in the style
/// engine so it can be embedded inside `Dimension` without a cross-crate
/// dependency on the parser types at layout time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CalcExpr {
    /// A concrete pixel value.
    Px(f32),
    /// A percentage of the containing block.
    Percent(f32),
    /// Relative to the element's font-size.
    Em(f32),
    /// Relative to the root font-size.
    Rem(f32),
    /// Viewport width percentage.
    Vw(f32),
    /// Viewport height percentage.
    Vh(f32),
    /// Smaller of vw/vh.
    Vmin(f32),
    /// Larger of vw/vh.
    Vmax(f32),
    /// Dynamic viewport width percentage.
    Dvw(f32),
    /// Dynamic viewport height percentage.
    Dvh(f32),
    /// Small viewport width percentage.
    Svw(f32),
    /// Small viewport height percentage.
    Svh(f32),
    /// Large viewport width percentage.
    Lvw(f32),
    /// Large viewport height percentage.
    Lvh(f32),
    /// A plain number (for multiplication / division).
    Number(f32),
    /// `a + b`
    Add(Box<CalcExpr>, Box<CalcExpr>),
    /// `a - b`
    Sub(Box<CalcExpr>, Box<CalcExpr>),
    /// `a * b`
    Mul(Box<CalcExpr>, Box<CalcExpr>),
    /// `a / b`
    Div(Box<CalcExpr>, Box<CalcExpr>),
    /// `min(a, b, …)`
    Min(Vec<CalcExpr>),
    /// `max(a, b, …)`
    Max(Vec<CalcExpr>),
    /// `clamp(min, preferred, max)`
    Clamp {
        min: Box<CalcExpr>,
        preferred: Box<CalcExpr>,
        max: Box<CalcExpr>,
    },
}

impl CalcExpr {
    /// Resolve to pixels given contextual sizes.
    pub fn resolve(
        &self,
        parent_px: f32,
        root_font_size: f32,
        font_size: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> f32 {
        match self {
            CalcExpr::Px(v) => *v,
            CalcExpr::Percent(v) => parent_px * v / 100.0,
            CalcExpr::Em(v) => font_size * v,
            CalcExpr::Rem(v) => root_font_size * v,
            CalcExpr::Vw(v) => viewport_w * v / 100.0,
            CalcExpr::Vh(v) => viewport_h * v / 100.0,
            CalcExpr::Vmin(v) => viewport_w.min(viewport_h) * v / 100.0,
            CalcExpr::Vmax(v) => viewport_w.max(viewport_h) * v / 100.0,
            CalcExpr::Dvw(v) | CalcExpr::Svw(v) | CalcExpr::Lvw(v) => viewport_w * v / 100.0,
            CalcExpr::Dvh(v) | CalcExpr::Svh(v) | CalcExpr::Lvh(v) => viewport_h * v / 100.0,
            CalcExpr::Number(v) => *v,
            CalcExpr::Add(a, b) => {
                a.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h)
                    + b.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h)
            }
            CalcExpr::Sub(a, b) => {
                a.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h)
                    - b.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h)
            }
            CalcExpr::Mul(a, b) => {
                a.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h)
                    * b.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h)
            }
            CalcExpr::Div(a, b) => {
                let divisor =
                    b.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h);
                if divisor.abs() < f32::EPSILON {
                    0.0
                } else {
                    a.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h)
                        / divisor
                }
            }
            CalcExpr::Min(args) => args
                .iter()
                .map(|e| e.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h))
                .fold(f32::INFINITY, f32::min),
            CalcExpr::Max(args) => args
                .iter()
                .map(|e| e.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h))
                .fold(f32::NEG_INFINITY, f32::max),
            CalcExpr::Clamp {
                min,
                preferred,
                max,
            } => {
                let min_v =
                    min.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h);
                let pref =
                    preferred.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h);
                let max_v =
                    max.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h);
                pref.clamp(min_v, max_v)
            }
        }
    }

    /// Resolve to pixels using full viewport tier information.
    pub fn resolve_viewport(
        &self,
        parent_px: f32,
        root_font_size: f32,
        font_size: f32,
        vp: &ViewportSizes,
    ) -> f32 {
        match self {
            CalcExpr::Px(v) => *v,
            CalcExpr::Percent(v) => parent_px * v / 100.0,
            CalcExpr::Em(v) => font_size * v,
            CalcExpr::Rem(v) => root_font_size * v,
            CalcExpr::Vw(v) => vp.width * v / 100.0,
            CalcExpr::Vh(v) => vp.height * v / 100.0,
            CalcExpr::Vmin(v) => vp.width.min(vp.height) * v / 100.0,
            CalcExpr::Vmax(v) => vp.width.max(vp.height) * v / 100.0,
            CalcExpr::Dvw(v) => vp.dynamic_width * v / 100.0,
            CalcExpr::Dvh(v) => vp.dynamic_height * v / 100.0,
            CalcExpr::Svw(v) => vp.small_width * v / 100.0,
            CalcExpr::Svh(v) => vp.small_height * v / 100.0,
            CalcExpr::Lvw(v) => vp.large_width * v / 100.0,
            CalcExpr::Lvh(v) => vp.large_height * v / 100.0,
            CalcExpr::Number(v) => *v,
            CalcExpr::Add(a, b) => {
                a.resolve_viewport(parent_px, root_font_size, font_size, vp)
                    + b.resolve_viewport(parent_px, root_font_size, font_size, vp)
            }
            CalcExpr::Sub(a, b) => {
                a.resolve_viewport(parent_px, root_font_size, font_size, vp)
                    - b.resolve_viewport(parent_px, root_font_size, font_size, vp)
            }
            CalcExpr::Mul(a, b) => {
                a.resolve_viewport(parent_px, root_font_size, font_size, vp)
                    * b.resolve_viewport(parent_px, root_font_size, font_size, vp)
            }
            CalcExpr::Div(a, b) => {
                let divisor = b.resolve_viewport(parent_px, root_font_size, font_size, vp);
                if divisor.abs() < f32::EPSILON {
                    0.0
                } else {
                    a.resolve_viewport(parent_px, root_font_size, font_size, vp) / divisor
                }
            }
            CalcExpr::Min(args) => args
                .iter()
                .map(|e| e.resolve_viewport(parent_px, root_font_size, font_size, vp))
                .fold(f32::INFINITY, f32::min),
            CalcExpr::Max(args) => args
                .iter()
                .map(|e| e.resolve_viewport(parent_px, root_font_size, font_size, vp))
                .fold(f32::NEG_INFINITY, f32::max),
            CalcExpr::Clamp {
                min,
                preferred,
                max,
            } => {
                let min_v = min.resolve_viewport(parent_px, root_font_size, font_size, vp);
                let pref = preferred.resolve_viewport(parent_px, root_font_size, font_size, vp);
                let max_v = max.resolve_viewport(parent_px, root_font_size, font_size, vp);
                pref.clamp(min_v, max_v)
            }
        }
    }
}

/// A CSS dimension value.  Most layout properties are expressed as `Dimension`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Dimension {
    /// Resolved pixel value.
    Px(f32),
    /// Percentage of containing block.
    Percent(f32),
    /// Relative to font-size of parent.
    Em(f32),
    /// Relative to root font-size.
    Rem(f32),
    /// Viewport width percentage.
    Vw(f32),
    /// Viewport height percentage.
    Vh(f32),
    /// Smaller of vw/vh.
    Vmin(f32),
    /// Larger of vw/vh.
    Vmax(f32),
    /// Dynamic viewport width percentage.
    Dvw(f32),
    /// Dynamic viewport height percentage.
    Dvh(f32),
    /// Small viewport width percentage.
    Svw(f32),
    /// Small viewport height percentage.
    Svh(f32),
    /// Large viewport width percentage.
    Lvw(f32),
    /// Large viewport height percentage.
    Lvh(f32),
    /// Width of the '0' glyph.
    Ch(f32),
    /// Auto (browser decides).
    Auto,
    /// `min-content` intrinsic size.
    MinContent,
    /// `max-content` intrinsic size.
    MaxContent,
    /// `fit-content(limit)`.
    FitContent(Box<Dimension>),
    /// `none` — e.g. `max-width: none`.
    None,
    /// Zero.
    Zero,
    /// `calc()` expression for deferred resolution.
    Calc(Box<CalcExpr>),
}

impl Default for Dimension {
    fn default() -> Self {
        Dimension::Auto
    }
}

impl Dimension {
    /// Is this a definite (non-auto, non-intrinsic) length?
    pub fn is_definite(&self) -> bool {
        matches!(
            self,
            Dimension::Px(_)
                | Dimension::Percent(_)
                | Dimension::Em(_)
                | Dimension::Rem(_)
                | Dimension::Vw(_)
                | Dimension::Vh(_)
                | Dimension::Vmin(_)
                | Dimension::Vmax(_)
                | Dimension::Dvw(_)
                | Dimension::Dvh(_)
                | Dimension::Svw(_)
                | Dimension::Svh(_)
                | Dimension::Lvw(_)
                | Dimension::Lvh(_)
                | Dimension::Ch(_)
                | Dimension::Zero
                | Dimension::Calc(_)
        )
    }

    /// Resolve to pixels given contextual sizes.
    pub fn resolve_px(
        &self,
        parent_px: f32,
        root_font_size: f32,
        font_size: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Option<f32> {
        match self {
            Dimension::Px(v) => Some(*v),
            Dimension::Percent(v) => Some(parent_px * v / 100.0),
            Dimension::Em(v) => Some(font_size * v),
            Dimension::Rem(v) => Some(root_font_size * v),
            Dimension::Vw(v) => Some(viewport_w * v / 100.0),
            Dimension::Vh(v) => Some(viewport_h * v / 100.0),
            Dimension::Vmin(v) => Some(viewport_w.min(viewport_h) * v / 100.0),
            Dimension::Vmax(v) => Some(viewport_w.max(viewport_h) * v / 100.0),
            Dimension::Dvw(v) | Dimension::Svw(v) | Dimension::Lvw(v) => {
                Some(viewport_w * v / 100.0)
            }
            Dimension::Dvh(v) | Dimension::Svh(v) | Dimension::Lvh(v) => {
                Some(viewport_h * v / 100.0)
            }
            Dimension::Ch(v) => Some(font_size * 0.5 * v), // approximate
            Dimension::Zero => Some(0.0),
            Dimension::Calc(expr) => {
                Some(expr.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h))
            }
            Dimension::Auto
            | Dimension::None
            | Dimension::MinContent
            | Dimension::MaxContent
            | Dimension::FitContent(_) => None,
        }
    }

    /// Resolve to pixels using full viewport tier information.
    ///
    /// Unlike [`resolve_px`](Self::resolve_px), this method correctly
    /// distinguishes dynamic/small/large viewport units by using the
    /// separate viewport size tiers provided in a [`ViewportSizes`].
    pub fn resolve_px_viewport(
        &self,
        parent_px: f32,
        root_font_size: f32,
        font_size: f32,
        vp: &ViewportSizes,
    ) -> Option<f32> {
        match self {
            Dimension::Px(v) => Some(*v),
            Dimension::Percent(v) => Some(parent_px * v / 100.0),
            Dimension::Em(v) => Some(font_size * v),
            Dimension::Rem(v) => Some(root_font_size * v),
            Dimension::Vw(v) => Some(vp.width * v / 100.0),
            Dimension::Vh(v) => Some(vp.height * v / 100.0),
            Dimension::Vmin(v) => Some(vp.width.min(vp.height) * v / 100.0),
            Dimension::Vmax(v) => Some(vp.width.max(vp.height) * v / 100.0),
            Dimension::Dvw(v) => Some(vp.dynamic_width * v / 100.0),
            Dimension::Dvh(v) => Some(vp.dynamic_height * v / 100.0),
            Dimension::Svw(v) => Some(vp.small_width * v / 100.0),
            Dimension::Svh(v) => Some(vp.small_height * v / 100.0),
            Dimension::Lvw(v) => Some(vp.large_width * v / 100.0),
            Dimension::Lvh(v) => Some(vp.large_height * v / 100.0),
            Dimension::Ch(v) => Some(font_size * 0.5 * v),
            Dimension::Zero => Some(0.0),
            Dimension::Calc(expr) => Some(expr.resolve_viewport(
                parent_px,
                root_font_size,
                font_size,
                vp,
            )),
            Dimension::Auto
            | Dimension::None
            | Dimension::MinContent
            | Dimension::MaxContent
            | Dimension::FitContent(_) => None,
        }
    }
}

/// Four-sided value (top, right, bottom, left).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sides<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T: Default> Default for Sides<T> {
    fn default() -> Self {
        Self {
            top: T::default(),
            right: T::default(),
            bottom: T::default(),
            left: T::default(),
        }
    }
}

impl<T: Clone> Sides<T> {
    pub fn all(value: T) -> Self {
        Self {
            top: value.clone(),
            right: value.clone(),
            bottom: value.clone(),
            left: value,
        }
    }
}

/// Four corners (top-left, top-right, bottom-right, bottom-left).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Corners<T> {
    pub top_left: T,
    pub top_right: T,
    pub bottom_right: T,
    pub bottom_left: T,
}

impl<T: Default> Default for Corners<T> {
    fn default() -> Self {
        Self {
            top_left: T::default(),
            top_right: T::default(),
            bottom_right: T::default(),
            bottom_left: T::default(),
        }
    }
}

impl<T: Clone> Corners<T> {
    pub fn all(value: T) -> Self {
        Self {
            top_left: value.clone(),
            top_right: value.clone(),
            bottom_right: value.clone(),
            bottom_left: value,
        }
    }
}

/// An elliptical border-radius value with separate horizontal (x) and vertical (y) radii.
///
/// When both axes are equal this degenerates to a circular corner.
/// CSS syntax: `border-radius: 10px / 20px` → `EllipticalRadius { x: 10.0, y: 20.0 }`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EllipticalRadius {
    pub x: f32,
    pub y: f32,
}

impl Default for EllipticalRadius {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl From<f32> for EllipticalRadius {
    fn from(v: f32) -> Self {
        Self { x: v, y: v }
    }
}

impl std::fmt::Display for EllipticalRadius {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if (self.x - self.y).abs() < f32::EPSILON {
            write!(f, "{}", self.x)
        } else {
            write!(f, "{} / {}", self.x, self.y)
        }
    }
}

/// A 2D size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Size<T> {
    pub width: T,
    pub height: T,
}

impl<T: Default> Default for Size<T> {
    fn default() -> Self {
        Self {
            width: T::default(),
            height: T::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_px() {
        assert_eq!(
            Dimension::Px(10.0).resolve_px(200.0, 16.0, 16.0, 1920.0, 1080.0),
            Some(10.0)
        );
        assert_eq!(
            Dimension::Percent(50.0).resolve_px(200.0, 16.0, 16.0, 1920.0, 1080.0),
            Some(100.0)
        );
        assert_eq!(
            Dimension::Em(2.0).resolve_px(200.0, 16.0, 14.0, 1920.0, 1080.0),
            Some(28.0)
        );
        assert_eq!(
            Dimension::Rem(2.0).resolve_px(200.0, 16.0, 14.0, 1920.0, 1080.0),
            Some(32.0)
        );
        assert_eq!(
            Dimension::Auto.resolve_px(200.0, 16.0, 16.0, 1920.0, 1080.0),
            None
        );
    }

    #[test]
    fn sides_all() {
        let s = Sides::all(Dimension::Px(5.0));
        assert_eq!(s.top, Dimension::Px(5.0));
        assert_eq!(s.left, Dimension::Px(5.0));
    }

    #[test]
    fn elliptical_radius_from_f32() {
        let r = EllipticalRadius::from(10.0);
        assert_eq!(r.x, 10.0);
        assert_eq!(r.y, 10.0);
    }

    #[test]
    fn elliptical_radius_default() {
        let r = EllipticalRadius::default();
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 0.0);
    }

    #[test]
    fn elliptical_radius_display_circular() {
        let r = EllipticalRadius { x: 5.0, y: 5.0 };
        assert_eq!(format!("{}", r), "5");
    }

    #[test]
    fn elliptical_radius_display_elliptical() {
        let r = EllipticalRadius { x: 10.0, y: 20.0 };
        assert_eq!(format!("{}", r), "10 / 20");
    }

    #[test]
    fn corners_of_elliptical_radius() {
        let c = Corners::all(EllipticalRadius::from(8.0));
        assert_eq!(c.top_left.x, 8.0);
        assert_eq!(c.top_left.y, 8.0);
        assert_eq!(c.bottom_right.x, 8.0);
    }

    #[test]
    fn dynamic_viewport_units_fallback() {
        // When using the basic resolve_px (no viewport tiers), dynamic units
        // fall back to the standard viewport_w / viewport_h.
        assert_eq!(
            Dimension::Dvw(50.0).resolve_px(0.0, 16.0, 16.0, 1000.0, 800.0),
            Some(500.0)
        );
        assert_eq!(
            Dimension::Dvh(25.0).resolve_px(0.0, 16.0, 16.0, 1000.0, 800.0),
            Some(200.0)
        );
        assert_eq!(
            Dimension::Svw(100.0).resolve_px(0.0, 16.0, 16.0, 1000.0, 800.0),
            Some(1000.0)
        );
        assert_eq!(
            Dimension::Lvh(10.0).resolve_px(0.0, 16.0, 16.0, 1000.0, 800.0),
            Some(80.0)
        );
    }

    #[test]
    fn dynamic_viewport_units_tiered() {
        let vp = ViewportSizes {
            width: 1920.0,
            height: 1080.0,
            dynamic_width: 1920.0,
            dynamic_height: 900.0,
            small_width: 1920.0,
            small_height: 800.0,
            large_width: 1920.0,
            large_height: 1100.0,
        };

        // dvh should use dynamic_height
        assert_eq!(
            Dimension::Dvh(100.0).resolve_px_viewport(0.0, 16.0, 16.0, &vp),
            Some(900.0)
        );
        // svh should use small_height
        assert_eq!(
            Dimension::Svh(100.0).resolve_px_viewport(0.0, 16.0, 16.0, &vp),
            Some(800.0)
        );
        // lvh should use large_height
        assert_eq!(
            Dimension::Lvh(100.0).resolve_px_viewport(0.0, 16.0, 16.0, &vp),
            Some(1100.0)
        );
        // standard vh should use standard height
        assert_eq!(
            Dimension::Vh(100.0).resolve_px_viewport(0.0, 16.0, 16.0, &vp),
            Some(1080.0)
        );
    }

    #[test]
    fn calc_dynamic_viewport_resolve() {
        let vp = ViewportSizes {
            width: 1000.0,
            height: 800.0,
            dynamic_width: 1000.0,
            dynamic_height: 700.0,
            small_width: 1000.0,
            small_height: 600.0,
            large_width: 1000.0,
            large_height: 900.0,
        };

        let expr = CalcExpr::Add(
            Box::new(CalcExpr::Dvh(50.0)),
            Box::new(CalcExpr::Px(10.0)),
        );
        // 50% of dynamic_height(700) + 10 = 360
        assert_eq!(expr.resolve_viewport(0.0, 16.0, 16.0, &vp), 360.0);

        let expr2 = CalcExpr::Svw(100.0);
        assert_eq!(expr2.resolve_viewport(0.0, 16.0, 16.0, &vp), 1000.0);

        let expr3 = CalcExpr::Lvh(50.0);
        assert_eq!(expr3.resolve_viewport(0.0, 16.0, 16.0, &vp), 450.0);
    }

    #[test]
    fn viewport_sizes_uniform() {
        let vp = ViewportSizes::uniform(1024.0, 768.0);
        assert_eq!(vp.width, 1024.0);
        assert_eq!(vp.dynamic_width, 1024.0);
        assert_eq!(vp.small_height, 768.0);
        assert_eq!(vp.large_height, 768.0);
    }

    #[test]
    fn dynamic_viewport_is_definite() {
        assert!(Dimension::Dvw(50.0).is_definite());
        assert!(Dimension::Dvh(50.0).is_definite());
        assert!(Dimension::Svw(50.0).is_definite());
        assert!(Dimension::Svh(50.0).is_definite());
        assert!(Dimension::Lvw(50.0).is_definite());
        assert!(Dimension::Lvh(50.0).is_definite());
    }
}
