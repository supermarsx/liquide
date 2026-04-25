//! Geometry adapters between the local `layers::Rect` (f32 `x/y/width/height`)
//! and the canonical `liquide_common::{FRect, IRect}` introduced in Phase 1.

use crate::layer::Rect as LayerRect;
use liquide_common::geometry::{FRect, IRect};

impl From<LayerRect> for FRect {
    #[inline]
    fn from(r: LayerRect) -> Self {
        FRect::new(r.x, r.y, r.width, r.height)
    }
}

impl From<FRect> for LayerRect {
    #[inline]
    fn from(r: FRect) -> Self {
        LayerRect::new(r.x, r.y, r.width, r.height)
    }
}

impl From<LayerRect> for IRect {
    /// Snap a layer rect to an enclosing integer rect via
    /// `floor(top-left) / ceil(bottom-right)`.
    fn from(r: LayerRect) -> Self {
        let x = r.x.floor() as i32;
        let y = r.y.floor() as i32;
        let right = (r.x + r.width).ceil() as i32;
        let bottom = (r.y + r.height).ceil() as i32;
        IRect::new(x, y, (right - x).max(0), (bottom - y).max(0))
    }
}

impl From<IRect> for LayerRect {
    #[inline]
    fn from(r: IRect) -> Self {
        LayerRect::new(r.x as f32, r.y as f32, r.width as f32, r.height as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_rect_frect_roundtrip() {
        let l = LayerRect::new(3.5, 4.25, 100.0, 200.0);
        let f: FRect = l.into();
        assert_eq!(f, FRect::new(3.5, 4.25, 100.0, 200.0));
        let back: LayerRect = f.into();
        assert_eq!(back, l);
    }

    #[test]
    fn layer_rect_irect_encloses() {
        let l = LayerRect::new(0.2, 0.8, 10.1, 19.3);
        let i: IRect = l.into();
        assert_eq!(i, IRect::new(0, 0, 11, 21));
    }

    #[test]
    fn irect_to_layer_rect_exact_for_integers() {
        let i = IRect::new(10, 20, 30, 40);
        let l: LayerRect = i.into();
        assert_eq!(l, LayerRect::new(10.0, 20.0, 30.0, 40.0));
    }
}
