//! CSS clip-path parsing.

/// Parse a CSS `clip-path` string into a `ClipPath` shape.
pub(crate) fn parse_clip_path(value: &str, bounds: &liquide_layout::Rect) -> Option<crate::display_list::ClipPath> {
    use crate::display_list::ClipPath;
    let trimmed = value.trim();

    if trimmed.starts_with("circle(") {
        // circle(r at cx cy) or circle(r)
        let inner = trimmed.trim_start_matches("circle(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split_whitespace().collect();
        let r = parse_length_or_percent(parts.first().copied().unwrap_or("50%"), bounds.width * 0.5);
        let (cx, cy) = if parts.len() >= 4 && parts[1] == "at" {
            (
                parse_length_or_percent(parts[2], bounds.width) + bounds.x,
                parse_length_or_percent(parts[3], bounds.height) + bounds.y,
            )
        } else {
            (bounds.x + bounds.width * 0.5, bounds.y + bounds.height * 0.5)
        };
        Some(ClipPath::Circle { cx, cy, r })
    } else if trimmed.starts_with("ellipse(") {
        let inner = trimmed.trim_start_matches("ellipse(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split_whitespace().collect();
        let rx = parse_length_or_percent(parts.first().copied().unwrap_or("50%"), bounds.width * 0.5);
        let ry = parse_length_or_percent(parts.get(1).copied().unwrap_or("50%"), bounds.height * 0.5);
        let (cx, cy) = if parts.len() >= 5 && parts[2] == "at" {
            (
                parse_length_or_percent(parts[3], bounds.width) + bounds.x,
                parse_length_or_percent(parts[4], bounds.height) + bounds.y,
            )
        } else {
            (bounds.x + bounds.width * 0.5, bounds.y + bounds.height * 0.5)
        };
        Some(ClipPath::Ellipse { cx, cy, rx, ry })
    } else if trimmed.starts_with("inset(") {
        let inner = trimmed.trim_start_matches("inset(").trim_end_matches(')');
        // Split on whitespace, but also handle "round <radii>" suffix
        let parts: Vec<&str> = inner.split_whitespace().collect();
        // CSS shorthand: 1→all, 2→TB/LR, 3→T/LR/B, 4→T/R/B/L
        let top_s = parts.first().copied().unwrap_or("0");
        let right_s = parts.get(1).copied().unwrap_or(top_s);
        let bottom_s = parts.get(2).copied().unwrap_or(top_s);
        let left_s = parts.get(3).copied().unwrap_or(right_s);
        let top = parse_length_or_percent(top_s, bounds.height);
        let right = parse_length_or_percent(right_s, bounds.width);
        let bottom = parse_length_or_percent(bottom_s, bounds.height);
        let left = parse_length_or_percent(left_s, bounds.width);
        Some(ClipPath::Inset {
            top,
            right,
            bottom,
            left,
            radius: liquide_style_engine::dimension::Corners::all(0.0),
        })
    } else if trimmed.starts_with("polygon(") {
        let inner = trimmed.trim_start_matches("polygon(").trim_end_matches(')');
        let points: Vec<(f32, f32)> = inner
            .split(',')
            .filter_map(|pair| {
                let coords: Vec<&str> = pair.trim().split_whitespace().collect();
                if coords.len() == 2 {
                    Some((
                        parse_length_or_percent(coords[0], bounds.width) + bounds.x,
                        parse_length_or_percent(coords[1], bounds.height) + bounds.y,
                    ))
                } else {
                    None
                }
            })
            .collect();
        if points.len() >= 3 {
            Some(ClipPath::Polygon(points))
        } else {
            None
        }
    } else {
        None
    }
}

/// Parse a CSS length value (px) or percentage into a pixel value.
pub(crate) fn parse_length_or_percent(value: &str, reference: f32) -> f32 {
    let trimmed = value.trim();
    if let Some(pct) = trimmed.strip_suffix('%') {
        pct.trim().parse::<f32>().unwrap_or(0.0) / 100.0 * reference
    } else if let Some(px) = trimmed.strip_suffix("px") {
        px.trim().parse::<f32>().unwrap_or(0.0)
    } else {
        trimmed.parse::<f32>().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_list::ClipPath;
    use liquide_layout::Rect;

    #[test]
    fn parse_circle_default_center() {
        let bounds = Rect::new(0.0, 0.0, 200.0, 200.0);
        let clip = parse_clip_path("circle(50px)", &bounds);
        assert!(clip.is_some());
        if let Some(ClipPath::Circle { cx, cy, r }) = clip {
            assert!((r - 50.0).abs() < 0.01);
            assert!((cx - 100.0).abs() < 0.01); // default center
            assert!((cy - 100.0).abs() < 0.01);
        } else {
            panic!("expected Circle");
        }
    }

    #[test]
    fn parse_circle_with_center() {
        let bounds = Rect::new(10.0, 20.0, 200.0, 200.0);
        let clip = parse_clip_path("circle(30px at 50px 60px)", &bounds);
        assert!(clip.is_some());
        if let Some(ClipPath::Circle { cx, cy, r }) = clip {
            assert!((r - 30.0).abs() < 0.01);
            assert!((cx - 60.0).abs() < 0.01); // 50px + bounds.x
            assert!((cy - 80.0).abs() < 0.01); // 60px + bounds.y
        } else {
            panic!("expected Circle");
        }
    }

    #[test]
    fn parse_circle_percent_radius() {
        let bounds = Rect::new(0.0, 0.0, 200.0, 200.0);
        let clip = parse_clip_path("circle(50%)", &bounds);
        assert!(clip.is_some());
        if let Some(ClipPath::Circle { r, .. }) = clip {
            // 50% of half-width (reference = 100.0)
            assert!((r - 50.0).abs() < 0.01);
        } else {
            panic!("expected Circle");
        }
    }

    #[test]
    fn parse_ellipse_default_center() {
        let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
        let clip = parse_clip_path("ellipse(40px 30px)", &bounds);
        assert!(clip.is_some());
        if let Some(ClipPath::Ellipse { cx, cy, rx, ry }) = clip {
            assert!((rx - 40.0).abs() < 0.01);
            assert!((ry - 30.0).abs() < 0.01);
            assert!((cx - 100.0).abs() < 0.01); // center of bounds
            assert!((cy - 50.0).abs() < 0.01);
        } else {
            panic!("expected Ellipse");
        }
    }

    #[test]
    fn parse_ellipse_with_center() {
        let bounds = Rect::new(0.0, 0.0, 200.0, 200.0);
        let clip = parse_clip_path("ellipse(40px 30px at 10px 20px)", &bounds);
        assert!(clip.is_some());
        if let Some(ClipPath::Ellipse { cx, cy, rx, ry }) = clip {
            assert!((rx - 40.0).abs() < 0.01);
            assert!((ry - 30.0).abs() < 0.01);
            assert!((cx - 10.0).abs() < 0.01);
            assert!((cy - 20.0).abs() < 0.01);
        } else {
            panic!("expected Ellipse");
        }
    }

    #[test]
    fn parse_polygon_triangle() {
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let clip = parse_clip_path("polygon(50% 0%, 100% 100%, 0% 100%)", &bounds);
        assert!(clip.is_some());
        if let Some(ClipPath::Polygon(points)) = clip {
            assert_eq!(points.len(), 3);
            assert!((points[0].0 - 50.0).abs() < 0.01);
            assert!((points[0].1 - 0.0).abs() < 0.01);
            assert!((points[1].0 - 100.0).abs() < 0.01);
            assert!((points[1].1 - 100.0).abs() < 0.01);
        } else {
            panic!("expected Polygon");
        }
    }

    #[test]
    fn parse_polygon_too_few_points_returns_none() {
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let clip = parse_clip_path("polygon(50% 0%, 100% 100%)", &bounds);
        // Only 2 points — not enough for a polygon
        assert!(clip.is_none());
    }

    #[test]
    fn parse_inset_single_value() {
        let bounds = Rect::new(0.0, 0.0, 200.0, 200.0);
        let clip = parse_clip_path("inset(10px)", &bounds);
        assert!(clip.is_some());
        if let Some(ClipPath::Inset { top, right, bottom, left, .. }) = clip {
            assert!((top - 10.0).abs() < 0.01);
            assert!((right - 10.0).abs() < 0.01);
            assert!((bottom - 10.0).abs() < 0.01);
            assert!((left - 10.0).abs() < 0.01);
        } else {
            panic!("expected Inset");
        }
    }

    #[test]
    fn parse_inset_four_values() {
        let bounds = Rect::new(0.0, 0.0, 200.0, 200.0);
        let clip = parse_clip_path("inset(10px 20px 30px 40px)", &bounds);
        assert!(clip.is_some());
        if let Some(ClipPath::Inset { top, right, bottom, left, .. }) = clip {
            assert!((top - 10.0).abs() < 0.01);
            assert!((right - 20.0).abs() < 0.01);
            assert!((bottom - 30.0).abs() < 0.01);
            assert!((left - 40.0).abs() < 0.01);
        } else {
            panic!("expected Inset");
        }
    }

    #[test]
    fn parse_inset_percent() {
        let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
        let clip = parse_clip_path("inset(10%)", &bounds);
        assert!(clip.is_some());
        if let Some(ClipPath::Inset { top, right, .. }) = clip {
            // top: 10% of height=100 → 10.0
            assert!((top - 10.0).abs() < 0.01);
            // right: 10% of width=200 → 20.0
            assert!((right - 20.0).abs() < 0.01);
        } else {
            panic!("expected Inset");
        }
    }

    #[test]
    fn parse_unknown_clip_returns_none() {
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert!(parse_clip_path("unknown()", &bounds).is_none());
        assert!(parse_clip_path("", &bounds).is_none());
        assert!(parse_clip_path("path('M 0 0')", &bounds).is_none());
    }

    // ── parse_length_or_percent tests ──

    #[test]
    fn length_px() {
        assert!((parse_length_or_percent("10px", 200.0) - 10.0).abs() < 0.01);
    }

    #[test]
    fn length_percent() {
        assert!((parse_length_or_percent("50%", 200.0) - 100.0).abs() < 0.01);
    }

    #[test]
    fn length_plain_number() {
        assert!((parse_length_or_percent("25", 200.0) - 25.0).abs() < 0.01);
    }

    #[test]
    fn length_invalid_returns_zero() {
        assert!((parse_length_or_percent("abc", 200.0) - 0.0).abs() < 0.01);
    }
}
