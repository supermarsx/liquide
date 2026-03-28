//! Display list clipper: efficiently finds display list items for a tile.
//!
//! Pre-filters display items by bounding box to avoid processing items
//! that don't intersect the current tile's region.

use crate::grid::PixelRect;
use liquide_paint::display_list::{DisplayItem, DisplayList};

/// A reference to a display item by index (avoids cloning).
#[derive(Debug, Clone, Copy)]
pub struct DisplayItemRef {
    /// Index into the display list's items vector.
    pub index: usize,
}

/// Clip a display list to a rectangular region, returning references to
/// items that intersect the region.
///
/// State operations (Push/Pop) are always included to maintain correct
/// rendering state. Draw operations are filtered by their bounding box.
pub fn clip_to_rect(display_list: &DisplayList, rect: &PixelRect) -> Vec<DisplayItemRef> {
    let items = &display_list.items;
    if items.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(items.len() / 2);

    // Track which Push operations we've included so we can pair them
    // with their Pop operations. Uses a depth counter per state type.
    let mut clip_depth: i32 = 0;
    let mut opacity_depth: i32 = 0;
    let mut transform_depth: i32 = 0;
    let mut blend_depth: i32 = 0;
    let mut filter_depth: i32 = 0;
    let mut backdrop_depth: i32 = 0;
    let mut mask_depth: i32 = 0;
    let mut stacking_depth: i32 = 0;
    let mut layer_depth: i32 = 0;

    for (index, item) in items.iter().enumerate() {
        match item {
            // State operations: always include to maintain correct rendering context.
            DisplayItem::PushClip { .. } | DisplayItem::PushClipPath { .. } => {
                clip_depth += 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PopClip => {
                clip_depth -= 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PushOpacity { .. } => {
                opacity_depth += 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PopOpacity => {
                opacity_depth -= 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PushTransform { .. } => {
                transform_depth += 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PopTransform => {
                transform_depth -= 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PushBlendMode { .. } => {
                blend_depth += 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PopBlendMode => {
                blend_depth -= 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PushFilter { .. } => {
                filter_depth += 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PopFilter => {
                filter_depth -= 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PushBackdropFilter { .. } => {
                backdrop_depth += 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PopBackdropFilter => {
                backdrop_depth -= 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PushMask { .. } => {
                mask_depth += 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PopMask => {
                mask_depth -= 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PushStackingContext { .. } => {
                stacking_depth += 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::PopStackingContext => {
                stacking_depth -= 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::SaveLayer { .. } => {
                layer_depth += 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::RestoreLayer => {
                layer_depth -= 1;
                result.push(DisplayItemRef { index });
            }
            DisplayItem::Noop => {}

            // Draw operations: only include if their bounding box intersects the tile.
            _ => {
                if let Some(bounds) = item_bounds_pixel(item) {
                    if pixel_rects_intersect(&bounds, rect) {
                        result.push(DisplayItemRef { index });
                    }
                }
            }
        }
    }

    // Suppress empty state pairs for efficiency: if a Push was included
    // but no draw ops between it and its Pop, we could remove both.
    // For simplicity and correctness we leave them in — the rasterizer
    // handles empty state pairs as no-ops.
    let _ = (clip_depth, opacity_depth, transform_depth, blend_depth,
             filter_depth, backdrop_depth, mask_depth, stacking_depth, layer_depth);

    result
}

/// Extract the bounding rect of a draw-type display item as a PixelRect.
fn item_bounds_pixel(item: &DisplayItem) -> Option<PixelRect> {
    match item {
        DisplayItem::SolidColor { rect, .. }
        | DisplayItem::LinearGradient { rect, .. }
        | DisplayItem::RadialGradient { rect, .. }
        | DisplayItem::ConicGradient { rect, .. }
        | DisplayItem::Border { rect, .. }
        | DisplayItem::BorderImage { rect, .. }
        | DisplayItem::Text { rect, .. }
        | DisplayItem::TextRun { rect, .. }
        | DisplayItem::Image { rect, .. }
        | DisplayItem::ImageRect { rect, .. }
        | DisplayItem::Icon { rect, .. }
        | DisplayItem::FillRect { rect, .. }
        | DisplayItem::StrokeRoundedRect { rect, .. }
        | DisplayItem::Surface { rect, .. }
        | DisplayItem::SetCursor { rect, .. }
        | DisplayItem::ScrollContainerHints { rect, .. }
        | DisplayItem::AnimationHints { rect, .. }
        | DisplayItem::TimelineHints { rect, .. }
        | DisplayItem::Annotate { rect, .. } => {
            Some(PixelRect::new(rect.x, rect.y, rect.width, rect.height))
        }

        DisplayItem::BoxShadow {
            rect, offset_x, offset_y, blur_radius, spread_radius, inset, ..
        } => {
            if *inset {
                Some(PixelRect::new(rect.x, rect.y, rect.width, rect.height))
            } else {
                let expand = *blur_radius + spread_radius.max(0.0);
                let shadow_x = rect.x + offset_x - expand;
                let shadow_y = rect.y + offset_y - expand;
                let shadow_r = rect.x + rect.width + offset_x + expand;
                let shadow_b = rect.y + rect.height + offset_y + expand;
                let min_x = rect.x.min(shadow_x);
                let min_y = rect.y.min(shadow_y);
                let max_x = (rect.x + rect.width).max(shadow_r);
                let max_y = (rect.y + rect.height).max(shadow_b);
                Some(PixelRect::new(min_x, min_y, max_x - min_x, max_y - min_y))
            }
        }

        DisplayItem::Outline { rect, width, offset, .. } => {
            let expand = *width + offset.max(0.0);
            Some(PixelRect::new(
                rect.x - expand,
                rect.y - expand,
                rect.width + expand * 2.0,
                rect.height + expand * 2.0,
            ))
        }

        DisplayItem::Line { x1, y1, x2, y2, width, .. } => {
            let half_w = width / 2.0;
            let min_x = x1.min(*x2) - half_w;
            let min_y = y1.min(*y2) - half_w;
            let max_x = x1.max(*x2) + half_w;
            let max_y = y1.max(*y2) + half_w;
            Some(PixelRect::new(min_x, min_y, max_x - min_x, max_y - min_y))
        }

        // State operations handled by the caller.
        _ => None,
    }
}

/// AABB intersection test for PixelRects.
#[inline]
fn pixel_rects_intersect(a: &PixelRect, b: &PixelRect) -> bool {
    a.x < b.right()
        && a.right() > b.x
        && a.y < b.bottom()
        && a.bottom() > b.y
}
