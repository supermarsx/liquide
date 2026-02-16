//! Dimension — a resolved or partially-resolved CSS length value.

use serde::{Deserialize, Serialize};

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
            CalcExpr::Clamp { min, preferred, max } => {
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
            Dimension::Ch(v) => Some(font_size * 0.5 * v), // approximate
            Dimension::Zero => Some(0.0),
            Dimension::Calc(expr) => Some(expr.resolve(parent_px, root_font_size, font_size, viewport_w, viewport_h)),
            Dimension::Auto | Dimension::None | Dimension::MinContent | Dimension::MaxContent | Dimension::FitContent(_) => None,
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
        assert_eq!(Dimension::Px(10.0).resolve_px(200.0, 16.0, 16.0, 1920.0, 1080.0), Some(10.0));
        assert_eq!(Dimension::Percent(50.0).resolve_px(200.0, 16.0, 16.0, 1920.0, 1080.0), Some(100.0));
        assert_eq!(Dimension::Em(2.0).resolve_px(200.0, 16.0, 14.0, 1920.0, 1080.0), Some(28.0));
        assert_eq!(Dimension::Rem(2.0).resolve_px(200.0, 16.0, 14.0, 1920.0, 1080.0), Some(32.0));
        assert_eq!(Dimension::Auto.resolve_px(200.0, 16.0, 16.0, 1920.0, 1080.0), None);
    }

    #[test]
    fn sides_all() {
        let s = Sides::all(Dimension::Px(5.0));
        assert_eq!(s.top, Dimension::Px(5.0));
        assert_eq!(s.left, Dimension::Px(5.0));
    }
}
