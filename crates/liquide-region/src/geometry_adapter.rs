//! Geometry adapters between the local `region::Rect` and the canonical
//! `liquide_common::{FRect, IRect}` introduced in Phase 1.
//!
//! `region::Rect` uses `left/top/right/bottom` (edges, with right/bottom
//! exclusive). The canonical `IRect`/`FRect` uses `x/y/width/height`.
//! These `From` impls let damage/tile/layer code convert at the boundary
//! without a full migration.

use crate::rect::Rect as RegionRect;
use liquide_common::geometry::{FRect, IRect};

impl From<RegionRect> for IRect {
    fn from(r: RegionRect) -> Self {
        // Preserve empty-rect semantics: width/height zero when inputs are
        // normalised (RegionRect::new already clamps reversed inputs to
        // all-zero, so this is safe).
        let w = (r.right - r.left).max(0);
        let h = (r.bottom - r.top).max(0);
        IRect::new(r.left, r.top, w, h)
    }
}

impl From<IRect> for RegionRect {
    fn from(r: IRect) -> Self {
        // IRect may legitimately have zero width/height; RegionRect::new
        // will normalise those to the all-zero empty rect.
        RegionRect::new(r.x, r.y, r.x + r.width, r.y + r.height)
    }
}

impl From<RegionRect> for FRect {
    fn from(r: RegionRect) -> Self {
        FRect::new(
            r.left as f32,
            r.top as f32,
            (r.right - r.left).max(0) as f32,
            (r.bottom - r.top).max(0) as f32,
        )
    }
}

/// Convert an `FRect` to a `RegionRect` using floor(top-left) /
/// ceil(bottom-right) to ensure the integer rect covers the entire
/// floating-point damage footprint.
#[must_use]
pub fn frect_to_region_rect_enclosing(r: FRect) -> RegionRect {
    let left = r.x.floor() as i32;
    let top = r.y.floor() as i32;
    let right = (r.x + r.width).ceil() as i32;
    let bottom = (r.y + r.height).ceil() as i32;
    RegionRect::new(left, top, right, bottom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_rect_to_irect_roundtrip() {
        let a = RegionRect::new(10, 20, 40, 60);
        let i: IRect = a.into();
        assert_eq!(i, IRect::new(10, 20, 30, 40));
        let back: RegionRect = i.into();
        assert_eq!(back, a);
    }

    #[test]
    fn irect_to_region_rect_roundtrip() {
        let i = IRect::new(5, 7, 100, 50);
        let r: RegionRect = i.into();
        assert_eq!(r, RegionRect::new(5, 7, 105, 57));
        let back: IRect = r.into();
        assert_eq!(back, i);
    }

    #[test]
    fn region_rect_to_frect_exact_for_integer_coords() {
        let r = RegionRect::new(0, 0, 100, 200);
        let f: FRect = r.into();
        assert_eq!(f, FRect::new(0.0, 0.0, 100.0, 200.0));
    }

    #[test]
    fn frect_to_region_rect_ceil_encloses() {
        let f = FRect::new(0.25, 0.75, 10.5, 20.1);
        let r = frect_to_region_rect_enclosing(f);
        assert_eq!(r, RegionRect::new(0, 0, 11, 21));
    }

    #[test]
    fn empty_region_rect_maps_to_empty_irect() {
        let r = RegionRect::new(0, 0, 0, 0);
        let i: IRect = r.into();
        assert!(i.is_empty());
    }
}
